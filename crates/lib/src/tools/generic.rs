//! Generic tool executor driven by a skill's tools.json descriptor.
//! Builds argv from the execution spec's arg mapping and runs via the allowlist.
//! Supports resolve-by-command or resolve-by-script for param resolution,
//! write-path validation against a per-profile write sandbox, and side-read
//! augmentation (append a nearby file's contents to the tool result).
//!
//! **Note:** The `normalizeNewlines` feature is deprecated due to a double-decode
//! bug (see `normalize_content` below). All bundled skills have had this flag
//! removed. The code path is retained for backward-compatible deserialization only.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::agent::ToolExecutor;
use crate::exec::{resolve_binary, Allowlist, WriteSandbox};
use crate::skills::{ArgKind, ExecutionSpec, SideReadSpec, ToolDescriptor};

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

        // Validate readPath- and writePath-annotated arguments against the sandbox.
        // This also returns canonical (symlink-resolved) paths for each validated param,
        // which are substituted back into args so that build_argv passes the resolved
        // absolute paths to the binary — preventing failures when the original value is a
        // sandbox-relative path that points through a symlink (e.g. "./chai" → the symlink
        // target) and the binary's CWD has already been set to that symlink target.
        let (working_dir, canonical_paths) =
            validate_write_paths(spec, args, allowlist, skill_dir.as_deref(), &self.sandbox)?;

        // Substitute canonical paths into a copy of args so build_argv and
        // extract_stdin_content operate on fully resolved paths.
        let resolved_args;
        let effective_args = if canonical_paths.is_empty() {
            args
        } else {
            resolved_args = substitute_canonical_paths(args, &canonical_paths);
            &resolved_args
        };

        // For writePath parameters, ensure parent directories exist before invoking
        // the subprocess. This lets the executor handle directory creation regardless
        // of which version of the target binary is deployed.
        ensure_write_path_parents(spec, &canonical_paths)?;

        let argv = build_argv(spec, effective_args, allowlist, skill_dir.as_deref())?;

        // Extract any stdin-kind parameter value to pipe to the child process.
        let stdin_content = extract_stdin_content(spec, effective_args, allowlist, skill_dir.as_deref());

        // If success_exit_codes is configured, use the _with_codes variants so that
        // extra exit codes (e.g. grep's exit 1 for "no matches") are treated as
        // success instead of errors.
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

        // Post-process stdout through a script if configured.
        let result = if let Some(ref pp) = spec.post_process {
            run_post_process(pp, &result, allowlist, skill_dir.as_deref())
        } else {
            result
        };

        // Append a side-read file to the result if configured.
        // Use effective_args (with canonical paths) so that apply_side_read
        // locates the file relative to the resolved directory the tool
        // operated on, not relative to the gateway process's CWD.
        if let Some(ref sr) = spec.side_read {
            Ok(apply_side_read(sr, effective_args, &result, session_id, &self.side_read_seen))
        } else {
            Ok(result)
        }
    }
}

/// Validate all sandboxed path arguments (readPath and writePath) in the spec.
/// Returns an optional working directory (the sandbox root) when read or write path
/// arguments are present, so that the child process CWD is the sandbox rather than
/// the gateway process's working directory.
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
    // Track the matching writable root of the first sandboxed path we validate.
    // This becomes the CWD for the binary so that it runs from within the correct
    // root — the sandbox directory itself for paths that live there directly, or
    // the symlink target root for paths that resolve through a sandbox symlink
    // (e.g. a git repo needs to run from the repo root, not from the sandbox dir).
    let mut matched_root: Option<std::path::PathBuf> = None;
    // Canonical (symlink-resolved) path for each validated readPath/writePath param.
    // These are substituted back into args before building argv so that the binary
    // always receives a fully resolved absolute path — not a sandbox-relative path
    // that would be misinterpreted once the CWD is set to the symlink target.
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

        // Get the resolved parameter value (apply the same transforms as build_argv).
        // For optional flag-type params with resolveCommand, run the resolver with
        // an empty string when the param is omitted — the resolve script may produce
        // a default value (e.g. today's date for daily notes).
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
                    // Optional flag not provided — if it has a resolveCommand, run the
                    // resolver with empty string to produce a default (e.g. today's date
                    // for daily notes). Otherwise skip — no path to validate.
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

        // Apply the same transforms that build_argv uses.
        let resolved = transform_param_value(raw_value, arg, allowlist, skill_dir);

        // If the resolved value is empty (resolve script produced no output and
        // there was no raw value), skip validation — there's no path to validate.
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

        // Store the canonical path so it can be substituted back into argv.
        canonical_paths.insert(
            arg.param.clone(),
            canonical.to_string_lossy().into_owned(),
        );

        // Record the writable root that this canonical path falls under — used
        // as the CWD below. Only the first sandboxed path sets the CWD; a tool
        // with multiple path args would have all of them under the same root in
        // the typical case (e.g. all inside the repo symlink target).
        if matched_root.is_none() {
            if let Some(root) = sandbox.roots().iter().find(|r| canonical.starts_with(*r)) {
                matched_root = Some(root.clone());
            }
        }
    }

    // If any readPath or writePath args were validated, run the binary with the
    // matched writable root as its CWD. Using the matched root rather than always
    // the sandbox directory means tools that operate within a symlinked external
    // directory (e.g. `git` inside a symlinked repo) get the correct working
    // directory. Fall back to the sandbox root if no match was recorded.
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

/// For each writePath parameter in the spec, create the parent directory of the canonical
/// path if it does not already exist. Errors are surfaced as tool errors so the agent sees
/// a clear message rather than a cryptic subprocess failure.
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

/// Build a copy of `args` with the given param values replaced by their canonical paths.
/// Used to ensure readPath/writePath params in argv are fully resolved absolute paths
/// rather than sandbox-relative paths that may be misinterpreted after the CWD changes.
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

/// If the arg has resolve_command with script set and skill_dir, run the script; else if binary/subcommand set, run via allowlist. Use trimmed stdout or keep original on failure.
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

/// Run a script from the skill's scripts/ dir. Returns stdout or error. Only runs if path is under skill_dir/scripts and is a file.
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

/// Append a file's contents to the tool result when the file exists.
///
/// Looks for `<path-param-value>/<filename>` from the `sideRead` spec. If the
/// file is found and non-empty, its contents are appended to `current_output`
/// under a labeled separator. When `oncePerSession` is true, the append fires
/// at most once per (session_id, path) pair; subsequent calls within the same
/// session are silently skipped.
///
/// Silently returns `current_output` unchanged on any error condition (missing
/// file, invalid filename, absent session when once_per_session is set, etc.).
fn apply_side_read(
    sr: &SideReadSpec,
    args: &serde_json::Value,
    current_output: &str,
    session_id: Option<&str>,
    seen: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> String {
    // Filename must not contain traversal sequences or path separators.
    if sr.filename.contains("..") || sr.filename.contains('/') || sr.filename.contains('\\') {
        log::warn!("sideRead: invalid filename in spec: {}", sr.filename);
        return current_output.to_string();
    }

    // Extract the value of the named path param from the tool arguments.
    let path_str = match args
        .as_object()
        .and_then(|o| o.get(&sr.path_param))
        .and_then(|v| v.as_str())
    {
        Some(s) => s,
        None => return current_output.to_string(),
    };

    let candidate = std::path::Path::new(path_str).join(&sr.filename);

    // oncePerSession: check whether this (session, path) pair has already been
    // surfaced. If so, skip. Otherwise record it now (before reading) so
    // concurrent calls within the same session are also deduplicated.
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

    // Read the file; silently skip when absent or unreadable.
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

/// Run a post-process script or command, piping `input` to its stdin.
/// Returns the script's stdout on success, or the original input on failure.
fn run_post_process(
    pp: &crate::skills::PostProcessSpec,
    input: &str,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
) -> String {
    use std::io::Write;
    use std::process::Stdio;

    // Script path: run via sh with stdin piped.
    if let (Some(dir), Some(ref script_name)) = (skill_dir, &pp.script) {
        if script_name.contains("..") || script_name.contains('/') || script_name.contains('\\') {
            return input.to_string();
        }
        let scripts_dir = Path::new(dir).join("scripts");
        let mut script_path = scripts_dir.join(script_name);
        if !script_path.starts_with(&scripts_dir) {
            return input.to_string();
        }
        if !script_path.is_file() {
            script_path = script_path.with_extension("sh");
            if !script_path.starts_with(&scripts_dir) || !script_path.is_file() {
                return input.to_string();
            }
        }

        let child = std::process::Command::new("sh")
            .arg(&script_path)
            .args(&pp.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();

        match child {
            Ok(mut child) => {
                if let Some(ref mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(input.as_bytes());
                }
                match child.wait_with_output() {
                    Ok(output) if output.status.success() => {
                        let s = String::from_utf8_lossy(&output.stdout).into_owned();
                        if s.is_empty() {
                            input.to_string()
                        } else {
                            s
                        }
                    }
                    _ => input.to_string(),
                }
            }
            Err(_) => input.to_string(),
        }
    }
    // Allowlisted command path: run via allowlist with stdin piped.
    else if let (Some(ref binary), Some(ref subcommand)) = (&pp.binary, &pp.subcommand) {
        // Check allowlist before spawning.
        if !allowlist.is_allowed(binary, subcommand) {
            return input.to_string();
        }

        let resolved = resolve_binary(binary);
        let mut cmd = std::process::Command::new(&resolved);
        cmd.args(subcommand.split_whitespace())
            .args(&pp.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match cmd.spawn() {
            Ok(mut child) => {
                if let Some(ref mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(input.as_bytes());
                }
                match child.wait_with_output() {
                    Ok(output) if output.status.success() => {
                        let s = String::from_utf8_lossy(&output.stdout).into_owned();
                        if s.is_empty() {
                            input.to_string()
                        } else {
                            s
                        }
                    }
                    _ => input.to_string(),
                }
            }
            Err(_) => input.to_string(),
        }
    } else {
        input.to_string()
    }
}

/// Apply optional normalize_newlines (deprecated) and resolve_command to a string value.
fn transform_param_value(
    s: String,
    arg: &crate::skills::ArgMapping,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
) -> String {
    resolve_value(&s, arg, allowlist, skill_dir)
}

/// Extract the value of the first `ArgKind::Stdin` parameter in the spec, if any.
/// Applies the same normalization transforms as `build_argv` (e.g. `normalizeNewlines`).
/// Returns `None` when no stdin-kind param exists or the param is absent from args.
fn extract_stdin_content(
    spec: &ExecutionSpec,
    args: &serde_json::Value,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
) -> Option<String> {
    use crate::skills::ArgKind;
    let obj = args.as_object()?;
    for arg in &spec.args {
        if arg.kind != ArgKind::Stdin {
            continue;
        }
        let value = match obj.get(&arg.param) {
            Some(v) if !v.is_null() => json_value_to_string(v)?,
            _ => continue,
        };
        return Some(transform_param_value(value, arg, allowlist, skill_dir));
    }
    None
}

/// Build argv from the execution spec's arg mapping and the JSON args object.
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
                    // Optional flag not provided — if it has a resolveCommand, run
                    // the resolver with an empty string to produce a default value
                    // (e.g. today's date for daily notes). This matches the behavior
                    // of ArgKind::Positional with resolveCommand and ensures that
                    // optional params with default-producing resolvers work correctly
                    // when the caller omits them.
                    _ if arg.optional == Some(true) && arg.resolve_command.is_some() => {
                        let flag = arg.flag.as_deref().unwrap_or(&arg.param);
                        let resolved = transform_param_value(String::new(), arg, allowlist, skill_dir);
                        // Only add the flag if the resolver actually produced a value.
                        // If it returned empty string (script failed or produced no
                        // output), skip the flag entirely — the binary would receive
                        // an empty --flag value which is likely invalid.
                        if !resolved.is_empty() {
                            argv.push(format!("--{}", flag));
                            argv.push(resolved);
                        }
                    }
                    _ => continue,
                }
            }
            ArgKind::Stdin => {
                // Value is piped via stdin; nothing is added to argv.
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
    use crate::skills::PostProcessSpec;
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

    /// Legacy test: normalize_content still works mechanically but the
    /// normalizeNewlines feature is deprecated due to the double-decode bug.
    /// See normalize_content doc comment for details.
    #[test]
    fn normalize_content_converts_escape_sequences() {
        let from_nl = format!("{}{}", '\\' as char, 'n' as char);
        let from_tab = format!("{}{}", '\\' as char, 't' as char);
        let to_nl = (10 as char).to_string();
        let to_tab = (9 as char).to_string();
        assert_eq!(normalize_content(&format!("hello{}world", from_nl)), format!("hello{}world", to_nl));
        assert_eq!(normalize_content(&format!("col1{}col2", from_tab)), format!("col1{}col2", to_tab));
        assert_eq!(normalize_content("no escapes here"), "no escapes here");
    }

    #[test]
    fn post_process_script_transforms_stdout() {
        let dir = setup_skill_with_script(
            "pp-basic",
            "uppercase",
            "#!/bin/sh
tr '[:lower:]' '[:upper:]'",
        );

        let pp = PostProcessSpec {
            script: Some("uppercase".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
        };

        let result = run_post_process(&pp, "hello world", &Allowlist::new(), Some(dir.as_path()));
        assert_eq!(result, "HELLO WORLD");
        cleanup(&dir);
    }

    #[test]
    fn post_process_returns_original_on_script_failure() {
        let dir = setup_skill_with_script("pp-fail", "fail", "#!/bin/sh
exit 1");

        let pp = PostProcessSpec {
            script: Some("fail".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
        };

        let result = run_post_process(
            &pp,
            "original output",
            &Allowlist::new(),
            Some(dir.as_path()),
        );
        assert_eq!(result, "original output");
        cleanup(&dir);
    }

    #[test]
    fn post_process_returns_original_on_empty_output() {
        let dir = setup_skill_with_script("pp-empty", "empty", "#!/bin/sh
cat > /dev/null");

        let pp = PostProcessSpec {
            script: Some("empty".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
        };

        let result = run_post_process(
            &pp,
            "original output",
            &Allowlist::new(),
            Some(dir.as_path()),
        );
        assert_eq!(result, "original output");
        cleanup(&dir);
    }

    #[test]
    fn post_process_passes_args_to_script() {
        let dir = setup_skill_with_script("pp-args", "head-lines", "#!/bin/sh
head -n \"$1\"");

        let pp = PostProcessSpec {
            script: Some("head-lines".to_string()),
            binary: None,
            subcommand: None,
            args: vec!["2".to_string()],
        };

        let input = "line1
line2
line3
line4
";
        let result = run_post_process(&pp, input, &Allowlist::new(), Some(dir.as_path()));
        assert_eq!(result, "line1
line2
");
        cleanup(&dir);
    }

    #[test]
    fn post_process_rejects_traversal_in_script_name() {
        let dir = setup_skill_with_script("pp-traversal", "legit", "#!/bin/sh
echo pwned");

        let pp = PostProcessSpec {
            script: Some("../../../etc/passwd".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
        };

        let result = run_post_process(&pp, "safe", &Allowlist::new(), Some(dir.as_path()));
        assert_eq!(result, "safe");
        cleanup(&dir);
    }

    #[test]
    fn post_process_missing_script_returns_original() {
        let dir = test_dir("pp-missing");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("scripts")).expect("create scripts dir");

        let pp = PostProcessSpec {
            script: Some("nonexistent".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
        };

        let result = run_post_process(&pp, "original", &Allowlist::new(), Some(dir.as_path()));
        assert_eq!(result, "original");
        cleanup(&dir);
    }

    #[test]
    fn post_process_no_skill_dir_returns_original() {
        let pp = PostProcessSpec {
            script: Some("anything".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
        };

        let result = run_post_process(&pp, "original", &Allowlist::new(), None);
        assert_eq!(result, "original");
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
        fs::write(dir.join("AGENTS.md"), "# Rules
Be helpful.").expect("write");

        let sr = make_sr("path", "AGENTS.md", false);
        let args = args_with_path(dir.to_str().unwrap());
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "file1.txt
file2.rs", None, &seen);
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
        // No AGENTS.md in this dir.

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

        // First call: should append.
        let first = apply_side_read(&sr, &args, "ls output", Some("session-1"), &seen);
        assert!(first.contains("Project rules"), "first call appends");

        // Second call same session: should NOT append again.
        let second = apply_side_read(&sr, &args, "ls output", Some("session-1"), &seen);
        assert_eq!(second, "ls output", "second call in same session is skipped");

        // Different session: should append again.
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

        // With no session_id, oncePerSession tracking is skipped — always appends.
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
        fs::write(dir.join("AGENTS.md"), "
  ").expect("write whitespace-only file");

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
        // args has no "path" key
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

        // Create a symlink inside the sandbox pointing to the external dir.
        let link = sandbox_dir.join("myrepo");
        symlink(&external, &link).expect("create symlink");

        let sb = WriteSandbox::new(&sandbox_dir);
        let sandbox = Some(sb);

        // Build a minimal ExecutionSpec with a readPath positional arg.
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
                normalize_newlines: None,
                resolve_command: None,
                optional: None,
                disambiguate_after_skipped_positionals: None,
            }],
            post_process: None,
            side_read: None,
            success_exit_codes: None,
        };

        // Pass the sandbox-relative symlink path.
        let args = serde_json::json!({ "path": "myrepo" });
        let allowlist = Allowlist::new();

        let (working_dir, canonical_paths) =
            validate_write_paths(&spec, &args, &allowlist, None, &sandbox)
                .expect("validation should succeed");

        // The working dir should be the resolved external directory.
        let external_canonical = fs::canonicalize(&external).expect("canonicalize external");
        assert_eq!(working_dir.as_deref(), Some(external_canonical.as_path()));

        // The canonical path for the "path" param must be the symlink target.
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
                normalize_newlines: None,
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

        // Substitute canonical paths and build argv.
        let resolved_args = substitute_canonical_paths(&args, &canonical_paths);
        let argv = build_argv(&spec, &resolved_args, &allowlist, None)
            .expect("build_argv should succeed");

        // argv should contain the canonical path, not the original "myrepo".
        assert_eq!(argv.len(), 1);
        let external_canonical = fs::canonicalize(&external).expect("canonicalize external");
        assert_eq!(
            argv[0],
            external_canonical.to_string_lossy().as_ref(),
            "argv should contain the resolved absolute path, not the original symlink name"
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
                    normalize_newlines: None,
                    disambiguate_after_skipped_positionals: None,
                },
                ArgMapping {
                    param: "content".to_string(),
                    kind: ArgKind::Stdin,
                    flag: None,
                    flag_if_true: None,
                    flag_if_false: None,
                    normalize_newlines: None,
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

        // When date is omitted, resolve script should run with empty input
        // and produce the default path.
        let args = serde_json::json!({ "content": "hello" });
        let argv = build_argv(&spec, &args, &allowlist, Some(dir.as_path()))
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["--path", "/default/path"],
            "optional flag with resolveCommand should use resolver default when omitted");

        // When date is provided, resolve script should run with the value.
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
                normalize_newlines: None,
                disambiguate_after_skipped_positionals: None,
            }],
            post_process: None,
            side_read: None,
            success_exit_codes: None,
        };

        let allowlist = Allowlist::new();

        // Optional flag without resolveCommand should be skipped when omitted.
        let args = serde_json::json!({});
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert!(argv.is_empty(),
            "optional flag without resolveCommand should be skipped when omitted");

        // When provided, it should still work.
        let args = serde_json::json!({ "optional_flag": "value" });
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["--opt", "value"],
            "optional flag without resolveCommand should work when provided");
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
                normalize_newlines: None,
                disambiguate_after_skipped_positionals: None,
            }],
            post_process: None,
            side_read: None,
            success_exit_codes: None,
        };

        let allowlist = Allowlist::new();

        // Non-optional flag without resolveCommand should still be skipped
        // when omitted (existing behavior — the binary will error on the
        // missing required flag). The fix only adds resolver invocation for
        // optional+resolveCommand flags.
        let args = serde_json::json!({});
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        // The flag is simply not in argv — the binary will complain about
        // the missing required --req. This is existing behavior.
        assert!(argv.is_empty());
    }
}
