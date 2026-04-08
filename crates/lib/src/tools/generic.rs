//! Generic tool executor driven by a skill's tools.json descriptor.
//! Builds argv from the execution spec's arg mapping and runs via the allowlist.
//! Supports optional content normalization (literal \n/\t -> newline/tab),
//! resolve-by-command or resolve-by-script for param resolution, and
//! write-path validation against a per-profile write sandbox.

use std::collections::HashMap;
use std::path::Path;

use crate::agent::ToolExecutor;
use crate::exec::{Allowlist, WriteSandbox};
use crate::skills::{ArgKind, ExecutionSpec, ToolDescriptor};

/// Executes tools using a descriptor's allowlist and execution mapping.
/// Holds per-tool (allowlist, spec, skill_dir) for param resolution and argv building.
/// When a `WriteSandbox` is present, validates `writePath`-annotated arguments
/// against writable roots before executing.
#[derive(Debug, Clone)]
pub struct GenericToolExecutor {
    /// tool_name -> (allowlist, execution spec, skill dir for script resolution)
    map: HashMap<String, (Allowlist, ExecutionSpec, Option<std::path::PathBuf>)>,
    /// Optional per-profile write sandbox for path boundary enforcement.
    sandbox: Option<WriteSandbox>,
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
        Self { map, sandbox }
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
    fn execute(&self, name: &str, args: &serde_json::Value) -> Result<String, String> {
        let (allowlist, spec, skill_dir) = self
            .map
            .get(name)
            .ok_or_else(|| format!("unknown tool: {}", name))?;
        let argv = build_argv(spec, args, allowlist, skill_dir.as_deref())?;

        // Validate writePath-annotated arguments against the sandbox.
        let working_dir =
            validate_write_paths(spec, args, allowlist, skill_dir.as_deref(), &self.sandbox)?;

        let result = allowlist.run(
            &spec.binary,
            &spec.subcommand,
            &argv,
            working_dir.as_deref(),
        )?;

        // Post-process stdout through a script if configured.
        if let Some(ref pp) = spec.post_process {
            Ok(run_post_process(
                pp,
                &result,
                allowlist,
                skill_dir.as_deref(),
            ))
        } else {
            Ok(result)
        }
    }
}

/// Validate all writePath-annotated arguments in the spec against the sandbox.
/// Returns an optional working directory (the sandbox root) when write paths are present.
fn validate_write_paths(
    spec: &ExecutionSpec,
    args: &serde_json::Value,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
    sandbox: &Option<WriteSandbox>,
) -> Result<Option<std::path::PathBuf>, String> {
    let obj = match args.as_object() {
        Some(o) => o,
        None => return Ok(None),
    };

    let mut has_write_path = false;

    for arg in &spec.args {
        if arg.write_path != Some(true) {
            continue;
        }

        let sandbox = sandbox.as_ref().ok_or_else(|| {
            format!(
                "tool {} has writePath parameter '{}' but no write sandbox is configured",
                spec.tool, arg.param
            )
        })?;

        // Get the resolved parameter value (apply the same transforms as build_argv).
        let raw_value = match arg.kind {
            ArgKind::Positional => match obj.get(&arg.param) {
                Some(v) if !v.is_null() => json_value_to_string(v)
                    .ok_or_else(|| format!("writePath parameter {} must be a string", arg.param))?,
                _ => {
                    if arg.optional == Some(true) {
                        continue;
                    }
                    return Err(format!("missing writePath parameter: {}", arg.param));
                }
            },
            ArgKind::Flag => {
                match obj.get(&arg.param) {
                    Some(v) if !v.is_null() => json_value_to_string(v).ok_or_else(|| {
                        format!("writePath parameter {} must be a string", arg.param)
                    })?,
                    // Optional flag not provided — no write path to validate.
                    _ => continue,
                }
            }
            ArgKind::FlagIfBoolean => {
                // FlagIfBoolean emits a flag string, not a path — writePath doesn't apply.
                continue;
            }
        };

        has_write_path = true;
        // Apply the same transforms that build_argv uses.
        let resolved = transform_param_value(raw_value, arg, allowlist, skill_dir);
        sandbox.validate(&resolved)?;
    }

    // If any writePath args were validated, use the sandbox root as CWD.
    if has_write_path {
        if let Some(ref sb) = sandbox {
            if let Some(root) = sb.roots().first() {
                return Ok(Some(root.clone()));
            }
        }
    }

    Ok(None)
}

/// Normalize string so literal `\n` and `\t` from JSON become real newlines/tabs.
fn normalize_content(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\t", "\t")
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

        let mut cmd = std::process::Command::new(binary);
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

/// Apply optional normalize_newlines and resolve_command to a string value.
fn transform_param_value(
    s: String,
    arg: &crate::skills::ArgMapping,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
) -> String {
    let s = if arg.normalize_newlines == Some(true) {
        normalize_content(&s)
    } else {
        s
    };
    resolve_value(&s, arg, allowlist, skill_dir)
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
                let value = match obj.get(&arg.param) {
                    Some(v) if !v.is_null() => v,
                    _ => continue,
                };
                let s = json_value_to_string(value).ok_or_else(|| {
                    format!(
                        "parameter {} must be a string, number, or boolean",
                        arg.param
                    )
                })?;
                let flag = arg.flag.as_deref().unwrap_or(&arg.param);
                argv.push(format!("--{}", flag));
                argv.push(transform_param_value(s, arg, allowlist, skill_dir));
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

    #[test]
    fn post_process_script_transforms_stdout() {
        let dir = setup_skill_with_script(
            "pp-basic",
            "uppercase",
            "#!/bin/sh\ntr '[:lower:]' '[:upper:]'",
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
        let dir = setup_skill_with_script("pp-fail", "fail", "#!/bin/sh\nexit 1");

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
        let dir = setup_skill_with_script("pp-empty", "empty", "#!/bin/sh\ncat > /dev/null");

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
        let dir = setup_skill_with_script("pp-args", "head-lines", "#!/bin/sh\nhead -n \"$1\"");

        let pp = PostProcessSpec {
            script: Some("head-lines".to_string()),
            binary: None,
            subcommand: None,
            args: vec!["2".to_string()],
        };

        let input = "line1\nline2\nline3\nline4\n";
        let result = run_post_process(&pp, input, &Allowlist::new(), Some(dir.as_path()));
        assert_eq!(result, "line1\nline2\n");
        cleanup(&dir);
    }

    #[test]
    fn post_process_rejects_traversal_in_script_name() {
        let dir = setup_skill_with_script("pp-traversal", "legit", "#!/bin/sh\necho pwned");

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
}
