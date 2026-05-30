//! Generic tool executor driven by a skill's tools.json descriptor.
//! Builds argv from the execution spec's arg mapping and runs via the allowlist.
//! Supports resolve-by-command or resolve-by-script for param resolution,
//! write-path validation against a per-profile write sandbox, and side-read
//! augmentation (append a nearby file's contents to the tool result).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::agent::ToolExecutor;
use crate::exec::{Allowlist, WriteSandbox};
use crate::skills::{ArgKind, ExecutionSpec, SideReadSpec, ToolDescriptor};
use crate::tools::post_process::run_post_process;

/// Executes tools using a descriptor's allowlist and execution mapping.
/// Holds per-tool (allowlist, spec, skill_dir) for param resolution and argv building.
/// When a `WriteSandbox` is present, validates `readPath`- and `writePath`-annotated
/// arguments against writable roots before executing. When a tool has a `sideRead`
/// spec, the named file is appended to the tool result when found; `oncePerSession`
/// prevents re-appending the same file within the same session.
#[derive(Debug, Clone)]
pub struct GenericToolExecutor {
    /// tool_name -> (allowlist, execution spec, skill_dir for script resolution)
    map: HashMap<String, (Allowlist, ExecutionSpec, Option<std::path::PathBuf>)>,
    /// Optional per-profile write sandbox for path boundary enforcement.
    sandbox: Option<WriteSandbox>,
    /// session_id -> set of "path/filename" keys already surfaced by sideRead
    /// this session. Shared via Arc so clones of the executor share state.
    /// Grows monotonically; no eviction (sessions are few relative to memory).
    side_read_seen: Arc<Mutex<HashMap<String, HashSet<String>>>>,
}

impl GenericToolExecutor {
    /// Build an executor from skill descriptors and optional skill dirs.
    /// `resolveCommand.script` in tools.json runs the named script from the skill's `scripts/` directory.
    /// When `sandbox` is provided, `writePath`-annotated arguments are validated before execution.
    pub fn from_descriptors(
        descriptors: &[(String, ToolDescriptor)],
        skill_dirs: &[(String, std::path::PathBuf)],
        sandbox: Option<WriteSandbox>,
    ) -> Self {
        let dir_map: HashMap<&String, &std::path::PathBuf> =
            skill_dirs.iter().map(|(n, p)| (n, p)).collect();
        let mut map = HashMap::new();
        for (skill_name, desc) in descriptors {
            let allowlist = desc.to_allowlist();
            let skill_dir = dir_map.get(skill_name).cloned().cloned();
            for spec in &desc.execution {
                map.insert(
                    spec.tool.clone(),
                    (allowlist.clone(), spec.clone(), skill_dir.clone()),
                );
            }
        }
        Self {
            map,
            sandbox,
            side_read_seen: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Return true if this executor handles the given tool name.
    pub fn has_tool(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    /// Tool names this executor can run.
    pub fn tool_names(&self) -> impl Iterator<Item = &String> {
        self.map.keys()
    }
}

impl ToolExecutor for GenericToolExecutor {
    fn execute(&self, name: &str, args: &serde_json::Value, session_id: Option<&str>) -> Result<String, String> {
        let (allowlist, spec, skill_dir) = self
            .map
            .get(name)
            .ok_or_else(|| format!("unknown tool: {}", name))?;

        let (working_dir, canonical_paths) =
            validate_write_paths(spec, args, allowlist, skill_dir.as_deref(), &self.sandbox)?;

        let resolved_args;
        let effective_args = if canonical_paths.is_empty() {
            args
        } else {
            resolved_args = substitute_canonical_paths(args, &canonical_paths);
            &resolved_args
        };

        ensure_write_path_parents(spec, &canonical_paths)?;

        let argv = build_argv(spec, effective_args, allowlist, skill_dir.as_deref())?;

        let stdin_content = extract_stdin_content(spec, effective_args, allowlist, skill_dir.as_deref())?;

        let success_codes = spec.success_exit_codes.as_deref().unwrap_or(&[]);
        let result = if let Some(ref content) = stdin_content {
            allowlist.run_with_stdin_with_codes(
                &spec.binary,
                &spec.subcommand,
                &argv,
                working_dir.as_deref(),
                content.as_bytes(),
                success_codes,
            )?
        } else {
            allowlist.run_with_codes(
                &spec.binary,
                &spec.subcommand,
                &argv,
                working_dir.as_deref(),
                success_codes,
            )?
        };

        let result = if let Some(ref pp) = spec.post_process {
            run_post_process(pp, &result, allowlist, skill_dir.as_deref())
        } else {
            result
        };

        if let Some(ref sr) = spec.side_read {
            Ok(apply_side_read(sr, effective_args, &result, session_id, &self.side_read_seen))
        } else {
            Ok(result)
        }
    }
}

fn validate_write_paths(
    spec: &ExecutionSpec,
    args: &serde_json::Value,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
    sandbox: &Option<WriteSandbox>,
) -> Result<(Option<std::path::PathBuf>, HashMap<String, String>), String> {
    let obj = match args.as_object() {
        Some(o) => o,
        None => return Ok((None, HashMap::new())),
    };

    let mut has_sandboxed_path = false;
    let mut matched_root: Option<std::path::PathBuf> = None;
    let mut canonical_paths: HashMap<String, String> = HashMap::new();

    for arg in &spec.args {
        let is_write = arg.write_path == Some(true);
        let is_read = arg.read_path == Some(true);
        if !is_write && !is_read {
            continue;
        }

        let kind_label = if is_write { "writePath" } else { "readPath" };

        let sandbox = sandbox.as_ref().ok_or_else(|| {
            format!(
                "tool {} has {} parameter '{}' but no write sandbox is configured",
                spec.tool, kind_label, arg.param
            )
        })?;

        let raw_value = match arg.kind {
            ArgKind::Positional => match obj.get(&arg.param) {
                Some(v) if !v.is_null() => json_value_to_string(v)
                    .ok_or_else(|| format!("{} parameter {} must be a string", kind_label, arg.param))?,
                _ => {
                    if arg.optional == Some(true) {
                        if arg.resolve_command.is_some() {
                            let _ = String::new();
                        } else {
                            continue;
                        }
                    }
                    return Err(format!("missing {} parameter: {}", kind_label, arg.param));
                }
            },
            ArgKind::Flag => {
                match obj.get(&arg.param) {
                    Some(v) if !v.is_null() => json_value_to_string(v).ok_or_else(|| {
                        format!("{} parameter {} must be a string", kind_label, arg.param)
                    })?,
                    _ => {
                        if arg.optional == Some(true) && arg.resolve_command.is_some() {
                            String::new()
                        } else {
                            continue;
                        }
                    }
                }
            }
            ArgKind::FlagIfBoolean => continue,
            ArgKind::Stdin => continue,
        };

        has_sandboxed_path = true;

        let resolved = transform_param_value(raw_value, arg, allowlist, skill_dir);

        if resolved.is_empty() {
            continue;
        }

        let canonical = sandbox.validate(&resolved).map_err(|e| {
            if is_read {
                e.replace("write path outside sandbox", "read path outside sandbox")
            } else {
                e
            }
        })?;

        canonical_paths.insert(
            arg.param.clone(),
            canonical.to_string_lossy().into_owned(),
        );

        if matched_root.is_none() {
            if let Some(root) = sandbox.roots().iter().find(|r| canonical.starts_with(*r)) {
                matched_root = Some(root.clone());
            }
        }
    }

    if has_sandboxed_path {
        if let Some(root) = matched_root {
            return Ok((Some(root), canonical_paths));
        }
        if let Some(ref sb) = sandbox {
            if let Some(root) = sb.roots().first() {
                return Ok((Some(root.clone()), canonical_paths));
            }
        }
    }

    Ok((None, canonical_paths))
}

fn ensure_write_path_parents(
    spec: &ExecutionSpec,
    canonical_paths: &HashMap<String, String>,
) -> Result<(), String> {
    for arg in &spec.args {
        if arg.write_path != Some(true) {
            continue;
        }
        let canonical = match canonical_paths.get(&arg.param) {
            Some(p) => std::path::Path::new(p),
            None => continue,
        };
        if let Some(parent) = canonical.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "failed to create parent directory {}: {}",
                        parent.display(),
                        e
                    )
                })?;
            }
        }
    }
    Ok(())
}

fn substitute_canonical_paths(
    args: &serde_json::Value,
    canonical_paths: &HashMap<String, String>,
) -> serde_json::Value {
    let mut patched = args.clone();
    if let Some(obj) = patched.as_object_mut() {
        for (param, canonical) in canonical_paths {
            obj.insert(param.clone(), serde_json::Value::String(canonical.clone()));
        }
    }
    patched
}

fn resolve_value(
    value: &str,
    arg: &crate::skills::ArgMapping,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
) -> String {
    let Some(ref cmd) = arg.resolve_command else {
        return value.to_string();
    };
    let argv: Vec<String> = cmd
        .args
        .iter()
        .map(|a| a.replace("$param", value))
        .collect();

    if let (Some(dir), Some(ref script_name)) = (skill_dir, &cmd.script) {
        if let Ok(out) = run_script(dir, script_name, &argv) {
            let s = out.trim();
            return if s.is_empty() {
                value.to_string()
            } else {
                s.to_string()
            };
        }
        return value.to_string();
    }

    if let (Some(ref binary), Some(ref subcommand)) = (&cmd.binary, &cmd.subcommand) {
        match allowlist.run(binary, subcommand, &argv, None) {
            Ok(out) => {
                let s = out.trim();
                return if s.is_empty() {
                    value.to_string()
                } else {
                    s.to_string()
                };
            }
            Err(_) => {}
        }
    }
    value.to_string()
}

fn run_script(skill_dir: &Path, script_name: &str, args: &[String]) -> Result<String, String> {
    if script_name.contains("..") || script_name.contains('/') || script_name.contains('\\') {
        return Err("invalid script name".to_string());
    }
    let scripts_dir = skill_dir.join("scripts");
    let mut script_path = scripts_dir.join(script_name);
    if !script_path.starts_with(&scripts_dir) {
        return Err("script path outside scripts dir".to_string());
    }
    if !script_path.is_file() {
        script_path = script_path.with_extension("sh");
        if !script_path.starts_with(&scripts_dir) || !script_path.is_file() {
            return Err("script not found".to_string());
        }
    }
    let output = std::process::Command::new("sh")
        .arg(&script_path)
        .args(args)
        .output()
        .map_err(|e| format!("exec failed: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(format!("exit {}: {}", output.status, stderr))
    }
}

fn apply_side_read(
    sr: &SideReadSpec,
    args: &serde_json::Value,
    current_output: &str,
    session_id: Option<&str>,
    seen: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> String {
    if sr.filename.contains("..") || sr.filename.contains('/') || sr.filename.contains('\\') {
        log::warn!("sideRead: invalid filename in spec: {}", sr.filename);
        return current_output.to_string();
    }

    let path_str = match args
        .as_object()
        .and_then(|o| o.get(&sr.path_param))
        .and_then(|v| v.as_str())
    {
        Some(s) => s,
        None => return current_output.to_string(),
    };

    let candidate = std::path::Path::new(path_str).join(&sr.filename);

    if sr.once_per_session == Some(true) {
        if let Some(sid) = session_id {
            let seen_key = format!("{}/{}", path_str, sr.filename);
            let already_seen = {
                let mut map = seen.lock().unwrap_or_else(|e| e.into_inner());
                let session_seen = map.entry(sid.to_string()).or_default();
                if session_seen.contains(&seen_key) {
                    true
                } else {
                    session_seen.insert(seen_key);
                    false
                }
            };
            if already_seen {
                return current_output.to_string();
            }
        }
    }

    let content = match std::fs::read_to_string(&candidate) {
        Ok(s) => s,
        Err(_) => return current_output.to_string(),
    };

    if content.trim().is_empty() {
        return current_output.to_string();
    }

    let label = sr.label.as_deref().unwrap_or(&sr.filename);
    format!(
        "{}

--- {} ---
{}",
        current_output.trim_end(),
        label,
        content.trim_end()
    )
}

fn transform_param_value(
    s: String,
    arg: &crate::skills::ArgMapping,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
) -> String {
    resolve_value(&s, arg, allowlist, skill_dir)
}

fn extract_stdin_content(
    spec: &ExecutionSpec,
    args: &serde_json::Value,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
) -> Result<Option<String>, String> {
    use crate::skills::ArgKind;
    let obj = match args.as_object() {
        Some(o) => o,
        None => return Ok(None),
    };
    for arg in &spec.args {
        if arg.kind != ArgKind::Stdin {
            continue;
        }
        let is_optional = arg.optional == Some(true);
        let value = match obj.get(&arg.param) {
            Some(v) if !v.is_null() => {
                match json_value_to_string(v) {
                    Some(s) => s,
                    None => {
                        if is_optional {
                            log::warn!(
                                "tool {}: stdin parameter '{}' has non-string type, skipping",
                                spec.tool,
                                arg.param
                            );
                            return Ok(None);
                        }
                        return Err(format!(
                            "stdin parameter '{}' must be a string, number, or boolean",
                            arg.param
                        ));
                    }
                }
            }
            _ => {
                if is_optional {
                    return Ok(None);
                }
                log::warn!(
                    "tool {}: required stdin parameter '{}' is missing or null",
                    spec.tool,
                    arg.param
                );
                return Err(format!(
                    "missing required parameter: {}",
                    arg.param
                ));
            }
        };
        return Ok(Some(transform_param_value(value, arg, allowlist, skill_dir)));
    }
    Ok(None)
}

fn build_argv(
    spec: &ExecutionSpec,
    args: &serde_json::Value,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
) -> Result<Vec<String>, String> {
    let obj = args
        .as_object()
        .ok_or_else(|| "arguments must be an object".to_string())?;
    let mut argv = Vec::new();
    let mut skipped_optional_positional = false;
    for arg in &spec.args {
        match arg.kind {
            ArgKind::Positional => {
                let s = match obj.get(&arg.param) {
                    Some(v) if !v.is_null() => json_value_to_string(v).ok_or_else(|| {
                        format!(
                            "parameter {} must be a string, number, or boolean",
                            arg.param
                        )
                    })?,
                    _ => {
                        if arg.optional != Some(true) {
                            return Err(format!("missing parameter: {}", arg.param));
                        }
                        if arg.resolve_command.is_some() {
                            String::new()
                        } else {
                            skipped_optional_positional = true;
                            continue;
                        }
                    }
                };
                if arg.disambiguate_after_skipped_positionals == Some(true)
                    && skipped_optional_positional
                {
                    argv.push("--".to_string());
                }
                skipped_optional_positional = false;
                argv.push(transform_param_value(s, arg, allowlist, skill_dir));
            }
            ArgKind::Flag => {
                match obj.get(&arg.param) {
                    Some(v) if !v.is_null() => {
                        let s = json_value_to_string(v).ok_or_else(|| {
                            format!(
                                "parameter {} must be a string, number, or boolean",
                                arg.param
                            )
                        })?;
                        let flag = arg.flag.as_deref().unwrap_or(&arg.param);
                        argv.push(format!("--{}", flag));
                        argv.push(transform_param_value(s, arg, allowlist, skill_dir));
                    }
                    _ if arg.optional == Some(true) && arg.resolve_command.is_some() => {
                        let flag = arg.flag.as_deref().unwrap_or(&arg.param);
                        let resolved = transform_param_value(String::new(), arg, allowlist, skill_dir);
                        if !resolved.is_empty() {
                            argv.push(format!("--{}", flag));
                            argv.push(resolved);
                        }
                    }
                    _ => continue,
                }
            }
            ArgKind::Stdin => {
                continue;
            }
            ArgKind::FlagIfBoolean => {
                let value = obj.get(&arg.param);
                let flag = match parse_bool(value) {
                    Some(true) => arg.flag_if_true.as_deref(),
                    _ => arg.flag_if_false.as_deref(),
                };
                if let Some(f) = flag {
                    argv.push(f.to_string());
                }
            }
        }
    }
    Ok(argv)
}

fn json_value_to_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn parse_bool(v: Option<&serde_json::Value>) -> Option<bool> {
    match v {
        Some(serde_json::Value::Bool(b)) => Some(*b),
        Some(serde_json::Value::String(s)) => Some(s.eq_ignore_ascii_case("true")),
        Some(serde_json::Value::Number(n)) => n.as_i64().map(|i| i != 0),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("chai-generic-test-{}-{}", name, std::process::id()))
    }

    fn setup_skill_with_script(name: &str, script_name: &str, script_content: &str) -> PathBuf {
        let dir = test_dir(name);
        let _ = fs::remove_dir_all(&dir);
        let scripts_dir = dir.join("scripts");
        fs::create_dir_all(&scripts_dir).expect("create scripts dir");
        let script_path = scripts_dir.join(format!("{}.sh", script_name));
        fs::write(&script_path, script_content).expect("write script");
        dir
    }

    fn cleanup(dir: &PathBuf) {
        let _ = fs::remove_dir_all(dir);
    }

    // --- extract_stdin_content tests ---

    #[test]
    fn extract_stdin_content_returns_content_when_present() {
        use crate::skills::{ArgMapping, ExecutionSpec};

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "content".to_string(),
                kind: ArgKind::Stdin,
                flag: None,
                flag_if_true: None,
                flag_if_false: None,
                resolve_command: None,
                optional: None,
                write_path: None,
                read_path: None,
                disambiguate_after_skipped_positionals: None,
            }],
            post_process: None,
            side_read: None,
            success_exit_codes: None,
        };

        let args = serde_json::json!({ "content": "hello world" });
        let result = extract_stdin_content(&spec, &args, &Allowlist::new(), None)
            .expect("should not error");
        assert_eq!(result, Some("hello world".to_string()));
    }

    #[test]
    fn extract_stdin_content_errors_on_required_missing() {
        use crate::skills::{ArgMapping, ExecutionSpec};

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "content".to_string(),
                kind: ArgKind::Stdin,
                flag: None,
                flag_if_true: None,
                flag_if_false: None,
                resolve_command: None,
                optional: None,
                write_path: None,
                read_path: None,
                disambiguate_after_skipped_positionals: None,
            }],
            post_process: None,
            side_read: None,
            success_exit_codes: None,
        };

        let args = serde_json::json!({ "path": "/some/file" });
        let result = extract_stdin_content(&spec, &args, &Allowlist::new(), None);
        assert!(result.is_err(), "missing required stdin param should error");
        assert!(
            result.unwrap_err().contains("content"),
            "error should mention the param name"
        );
    }

    #[test]
    fn extract_stdin_content_errors_on_required_null() {
        use crate::skills::{ArgMapping, ExecutionSpec};

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "content".to_string(),
                kind: ArgKind::Stdin,
                flag: None,
                flag_if_true: None,
                flag_if_false: None,
                resolve_command: None,
                optional: None,
                write_path: None,
                read_path: None,
                disambiguate_after_skipped_positionals: None,
            }],
            post_process: None,
            side_read: None,
            success_exit_codes: None,
        };

        let args = serde_json::json!({ "content": null });
        let result = extract_stdin_content(&spec, &args, &Allowlist::new(), None);
        assert!(result.is_err(), "null required stdin param should error");
    }

    #[test]
    fn extract_stdin_content_optional_missing_returns_none() {
        use crate::skills::{ArgMapping, ExecutionSpec};

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "content".to_string(),
                kind: ArgKind::Stdin,
                flag: None,
                flag_if_true: None,
                flag_if_false: None,
                resolve_command: None,
                optional: Some(true),
                write_path: None,
                read_path: None,
                disambiguate_after_skipped_positionals: None,
            }],
            post_process: None,
            side_read: None,
            success_exit_codes: None,
        };

        let args = serde_json::json!({ "path": "/some/file" });
        let result = extract_stdin_content(&spec, &args, &Allowlist::new(), None)
            .expect("optional missing should not error");
        assert_eq!(result, None, "optional missing stdin should return None");
    }

    // --- apply_side_read tests ---

    fn make_seen() -> Arc<Mutex<HashMap<String, HashSet<String>>>> {
        Arc::new(Mutex::new(HashMap::new()))
    }

    fn make_sr(path_param: &str, filename: &str, once_per_session: bool) -> SideReadSpec {
        SideReadSpec {
            path_param: path_param.to_string(),
            filename: filename.to_string(),
            label: None,
            once_per_session: if once_per_session { Some(true) } else { None },
        }
    }

    fn args_with_path(path: &str) -> serde_json::Value {
        serde_json::json!({ "path": path })
    }

    #[test]
    fn side_read_appends_file_content() {
        let dir = test_dir("sr-basic");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("AGENTS.md"), "# Rules\nBe helpful.").expect("write");

        let sr = make_sr("path", "AGENTS.md", false);
        let args = args_with_path(dir.to_str().unwrap());
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "file1.txt\nfile2.rs", None, &seen);
        assert!(result.contains("file1.txt"), "original output preserved");
        assert!(result.contains("--- AGENTS.md ---"), "separator present");
        assert!(result.contains("Be helpful."), "file content appended");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn side_read_absent_file_returns_original() {
        let dir = test_dir("sr-absent");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");

        let sr = make_sr("path", "AGENTS.md", false);
        let args = args_with_path(dir.to_str().unwrap());
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "listing output", None, &seen);
        assert_eq!(result, "listing output");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn side_read_rejects_traversal_in_filename() {
        let sr = SideReadSpec {
            path_param: "path".to_string(),
            filename: "../../../etc/passwd".to_string(),
            label: None,
            once_per_session: None,
        };
        let args = args_with_path("/tmp");
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "safe output", None, &seen);
        assert_eq!(result, "safe output");
    }

    #[test]
    fn side_read_rejects_slash_in_filename() {
        let sr = SideReadSpec {
            path_param: "path".to_string(),
            filename: "sub/AGENTS.md".to_string(),
            label: None,
            once_per_session: None,
        };
        let args = args_with_path("/tmp");
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "safe output", None, &seen);
        assert_eq!(result, "safe output");
    }

    #[test]
    fn side_read_once_per_session_deduplicates() {
        let dir = test_dir("sr-once");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("AGENTS.md"), "# Project rules").expect("write");

        let sr = make_sr("path", "AGENTS.md", true);
        let args = args_with_path(dir.to_str().unwrap());
        let seen = make_seen();

        let first = apply_side_read(&sr, &args, "ls output", Some("session-1"), &seen);
        assert!(first.contains("Project rules"), "first call appends");

        let second = apply_side_read(&sr, &args, "ls output", Some("session-1"), &seen);
        assert_eq!(second, "ls output", "second call in same session is skipped");

        let other_session = apply_side_read(&sr, &args, "ls output", Some("session-2"), &seen);
        assert!(other_session.contains("Project rules"), "different session appends");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn side_read_no_session_always_appends_when_once_per_session() {
        let dir = test_dir("sr-no-session");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("AGENTS.md"), "# Always").expect("write");

        let sr = make_sr("path", "AGENTS.md", true);
        let args = args_with_path(dir.to_str().unwrap());
        let seen = make_seen();

        let first = apply_side_read(&sr, &args, "output", None, &seen);
        assert!(first.contains("Always"), "appends without session");

        let second = apply_side_read(&sr, &args, "output", None, &seen);
        assert!(second.contains("Always"), "appends again without session");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn side_read_uses_custom_label() {
        let dir = test_dir("sr-label");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("AGENTS.md"), "content").expect("write");

        let sr = SideReadSpec {
            path_param: "path".to_string(),
            filename: "AGENTS.md".to_string(),
            label: Some("Project Instructions".to_string()),
            once_per_session: None,
        };
        let args = args_with_path(dir.to_str().unwrap());
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "listing", None, &seen);
        assert!(result.contains("--- Project Instructions ---"), "custom label used");
        assert!(!result.contains("--- AGENTS.md ---"), "default label not used");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn side_read_empty_file_returns_original() {
        let dir = test_dir("sr-empty");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("AGENTS.md"), "\n  ").expect("write whitespace-only file");

        let sr = make_sr("path", "AGENTS.md", false);
        let args = args_with_path(dir.to_str().unwrap());
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "listing", None, &seen);
        assert_eq!(result, "listing", "whitespace-only file treated as empty");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn side_read_missing_path_param_returns_original() {
        let sr = make_sr("path", "AGENTS.md", false);
        let args = serde_json::json!({ "other": "/tmp" });
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "output", None, &seen);
        assert_eq!(result, "output");
    }

    // --- substitute_canonical_paths tests ---

    #[test]
    fn substitute_canonical_paths_replaces_named_params() {
        let args = serde_json::json!({ "path": "./chai", "other": "unchanged" });
        let mut overrides = HashMap::new();
        overrides.insert("path".to_string(), "/resolved/chai".to_string());

        let patched = substitute_canonical_paths(&args, &overrides);
        assert_eq!(patched["path"], "/resolved/chai");
        assert_eq!(patched["other"], "unchanged");
    }

    #[test]
    fn substitute_canonical_paths_leaves_non_object_untouched() {
        let args = serde_json::json!("not an object");
        let mut overrides = HashMap::new();
        overrides.insert("path".to_string(), "/resolved".to_string());

        let patched = substitute_canonical_paths(&args, &overrides);
        assert_eq!(patched, serde_json::json!("not an object"));
    }

    #[test]
    fn substitute_canonical_paths_no_overrides_returns_clone() {
        let args = serde_json::json!({ "path": "./chai" });
        let patched = substitute_canonical_paths(&args, &HashMap::new());
        assert_eq!(patched["path"], "./chai");
    }

    // --- validate_write_paths canonical path return tests ---

    #[cfg(unix)]
    #[test]
    fn validate_write_paths_returns_canonical_for_symlinked_read_path() {
        use crate::exec::WriteSandbox;
        use crate::skills::{ArgMapping, ExecutionSpec};
        use std::os::unix::fs::symlink;

        let base = test_dir("vwp-symlink");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        let external = base.join("external");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");
        fs::create_dir_all(&external).expect("create external");

        let link = sandbox_dir.join("myrepo");
        symlink(&external, &link).expect("create symlink");

        let sb = WriteSandbox::new(&sandbox_dir);
        let sandbox = Some(sb);

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "ls".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "path".to_string(),
                kind: ArgKind::Positional,
                read_path: Some(true),
                write_path: None,
                flag: None,
                flag_if_true: None,
                flag_if_false: None,
                resolve_command: None,
                optional: None,
                disambiguate_after_skipped_positionals: None,
            }],
            post_process: None,
            side_read: None,
            success_exit_codes: None,
        };

        let args = serde_json::json!({ "path": "myrepo" });
        let allowlist = Allowlist::new();

        let (working_dir, canonical_paths) =
            validate_write_paths(&spec, &args, &allowlist, None, &sandbox)
                .expect("validation should succeed");

        let external_canonical = fs::canonicalize(&external).expect("canonicalize external");
        assert_eq!(working_dir.as_deref(), Some(external_canonical.as_path()));

        let resolved = canonical_paths.get("path").expect("path should be in canonical map");
        assert_eq!(
            std::path::Path::new(resolved),
            external_canonical.as_path(),
            "canonical path should resolve through the symlink"
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn validate_write_paths_canonical_path_is_substituted_into_argv() {
        use crate::exec::WriteSandbox;
        use crate::skills::{ArgMapping, ExecutionSpec};
        use std::os::unix::fs::symlink;

        let base = test_dir("vwp-argv-sub");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        let external = base.join("external");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");
        fs::create_dir_all(&external).expect("create external");

        let link = sandbox_dir.join("myrepo");
        symlink(&external, &link).expect("create symlink");

        let sb = WriteSandbox::new(&sandbox_dir);

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "ls".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "path".to_string(),
                kind: ArgKind::Positional,
                read_path: Some(true),
                write_path: None,
                flag: None,
                flag_if_true: None,
                flag_if_false: None,
                resolve_command: None,
                optional: None,
                disambiguate_after_skipped_positionals: None,
            }],
            post_process: None,
            side_read: None,
            success_exit_codes: None,
        };

        let args = serde_json::json!({ "path": "myrepo" });
        let allowlist = Allowlist::new();

        let (_, canonical_paths) =
            validate_write_paths(&spec, &args, &allowlist, None, &Some(sb))
                .expect("validation should succeed");

        let resolved_args = substitute_canonical_paths(&args, &canonical_paths);
        let argv = build_argv(&spec, &resolved_args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv.len(), 1);
        let external_canonical = fs::canonicalize(&external).expect("canonicalize external");
        assert_eq!(
            argv[0],
            external_canonical.to_string_lossy().as_ref(),
            "argv should contain the resolved absolute path"
        );

        let _ = fs::remove_dir_all(&base);
    }

    // --- optional flag with resolveCommand tests ---

    #[test]
    fn build_argv_optional_flag_with_resolve_command_runs_resolver_when_omitted() {
        use crate::skills::{ArgMapping, ExecutionSpec, ResolveCommandSpec};

        let dir = setup_skill_with_script(
            "opt-flag-resolve",
            "resolve-path",
            "#!/bin/sh\nif [ -z \"$1\" ]; then echo \"/default/path\"; else echo \"/resolved/$1\"; fi",
        );

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![
                ArgMapping {
                    param: "date".to_string(),
                    kind: ArgKind::Flag,
                    flag: Some("path".to_string()),
                    optional: Some(true),
                    resolve_command: Some(ResolveCommandSpec {
                        script: Some("resolve-path".to_string()),
                        binary: None,
                        subcommand: None,
                        args: vec!["$param".to_string()],
                    }),
                    write_path: None,
                    read_path: None,
                    flag_if_true: None,
                    flag_if_false: None,
                    disambiguate_after_skipped_positionals: None,
                },
                ArgMapping {
                    param: "content".to_string(),
                    kind: ArgKind::Stdin,
                    flag: None,
                    flag_if_true: None,
                    flag_if_false: None,
                    resolve_command: None,
                    optional: None,
                    write_path: None,
                    read_path: None,
                    disambiguate_after_skipped_positionals: None,
                },
            ],
            post_process: None,
            side_read: None,
            success_exit_codes: None,
        };

        let allowlist = Allowlist::new();

        let args = serde_json::json!({ "content": "hello" });
        let argv = build_argv(&spec, &args, &allowlist, Some(dir.as_path()))
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["--path", "/default/path"],
            "optional flag with resolveCommand should use resolver default when omitted");

        let args = serde_json::json!({ "date": "2026-05-28", "content": "hello" });
        let argv = build_argv(&spec, &args, &allowlist, Some(dir.as_path()))
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["--path", "/resolved/2026-05-28"],
            "optional flag with resolveCommand should use resolved value when provided");

        cleanup(&dir);
    }

    #[test]
    fn build_argv_optional_flag_without_resolve_command_is_skipped_when_omitted() {
        use crate::skills::{ArgMapping, ExecutionSpec};

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "optional_flag".to_string(),
                kind: ArgKind::Flag,
                flag: Some("opt".to_string()),
                optional: Some(true),
                resolve_command: None,
                write_path: None,
                read_path: None,
                flag_if_true: None,
                flag_if_false: None,
                disambiguate_after_skipped_positionals: None,
            }],
            post_process: None,
            side_read: None,
            success_exit_codes: None,
        };

        let allowlist = Allowlist::new();

        let args = serde_json::json!({});
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");
        assert!(argv.is_empty());

        let args = serde_json::json!({ "optional_flag": "value" });
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");
        assert_eq!(argv, vec!["--opt", "value"]);
    }

    #[test]
    fn build_argv_non_optional_flag_without_value_is_error() {
        use crate::skills::{ArgMapping, ExecutionSpec};

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "required_flag".to_string(),
                kind: ArgKind::Flag,
                flag: Some("req".to_string()),
                optional: None,
                resolve_command: None,
                write_path: None,
                read_path: None,
                flag_if_true: None,
                flag_if_false: None,
                disambiguate_after_skipped_positionals: None,
            }],
            post_process: None,
            side_read: None,
            success_exit_codes: None,
        };

        let allowlist = Allowlist::new();

        let args = serde_json::json!({});
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");
        assert!(argv.is_empty());
    }
}
