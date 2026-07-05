//! Argv construction for the generic tool executor.
//!
//! Builds command-line argument vectors from execution spec arg mappings,
//! resolves parameter values via scripts or allowlisted commands, extracts
//! stdin content, writes temp-file parameters, and formats flags.

use std::path::Path;

use crate::exec::Allowlist;
use crate::skills::{ArgKind, ExecutionSpec};

/// Substitute `$param_name` placeholders in resolve command args with values
/// from the tool call JSON. The special `$param` placeholder is replaced with
/// the current parameter's value; all other `$param_name` placeholders are
/// replaced with the corresponding parameter from the tool call args (or an
/// empty string if absent or null).
fn substitute_resolve_args(
    cmd_args: &[String],
    param_value: &str,
    tool_args: &serde_json::Value,
) -> Vec<String> {
    let obj = tool_args.as_object();
    cmd_args
        .iter()
        .map(|a| {
            if a == "$param" {
                param_value.to_string()
            } else if a.starts_with('$') {
                let key = &a[1..];
                if let Some(o) = obj {
                    match o.get(key) {
                        Some(v) if !v.is_null() => v.as_str().unwrap_or("").to_string(),
                        _ => String::new(),
                    }
                } else {
                    String::new()
                }
            } else {
                a.clone()
            }
        })
        .collect()
}

/// Resolve a parameter value by running its `resolveCommand` (script or
/// allowlisted command). If no resolve command is configured, returns the
/// value unchanged. `tool_args` provides parameter values for `$param_name`
/// substitution in resolve command args (e.g. `$scope` is replaced with the
/// `scope` parameter value from the tool call JSON).
///
/// Returns `Err` if the resolve command fails (non-zero exit), allowing
/// callers to reject the tool call before the command runs. This is
/// critical for resolve scripts that perform validation (e.g. verifying
/// a git repository root is inside the sandbox).
pub(crate) fn resolve_value(
    value: &str,
    arg: &crate::skills::ArgMapping,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
    tool_args: &serde_json::Value,
) -> Result<String, String> {
    let Some(ref cmd) = arg.resolve_command else {
        return Ok(value.to_string());
    };
    let argv = substitute_resolve_args(&cmd.args, value, tool_args);

    if let (Some(dir), Some(ref script_name)) = (skill_dir, &cmd.script) {
        let out = run_script(dir, script_name, &argv)?;
        let s = out.trim();
        return Ok(if s.is_empty() {
            value.to_string()
        } else {
            s.to_string()
        });
    }

    if let (Some(ref binary), Some(ref subcommand)) = (&cmd.binary, &cmd.subcommand) {
        match allowlist.run(binary, subcommand, &argv, None) {
            Ok(out) => {
                let s = out.trim();
                return Ok(if s.is_empty() {
                    value.to_string()
                } else {
                    s.to_string()
                });
            }
            Err(e) => return Err(e),
        }
    }
    Ok(value.to_string())
}

/// Run a script from the skill's `scripts/` directory via `sh`.
/// Validates the script name for path traversal and checks that the
/// resolved path stays within the scripts directory.
pub(crate) fn run_script(skill_dir: &Path, script_name: &str, args: &[String]) -> Result<String, String> {
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

/// Transform a raw parameter value by running its resolve command (if any).
/// Returns `Err` if the resolve command fails.
pub(crate) fn transform_param_value(
    s: String,
    arg: &crate::skills::ArgMapping,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
    tool_args: &serde_json::Value,
) -> Result<String, String> {
    resolve_value(&s, arg, allowlist, skill_dir, tool_args)
}

/// Extract the value of the `Stdin`-kind argument from the tool call JSON,
/// resolving it via its resolve command if configured.
pub(crate) fn extract_stdin_content(
    spec: &ExecutionSpec,
    args: &serde_json::Value,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
) -> Result<Option<String>, String> {
    let obj = match args.as_object() {
        Some(o) => o,
        None => return Ok(None),
    };
    for arg in &spec.args {
        if arg.kind != ArgKind::Stdin {
            continue;
        }
        let is_optional = arg.optional == Some(true);
        let value = match obj.get(arg.param_name()) {
            Some(v) if !v.is_null() => {
                match json_value_to_string(v) {
                    Some(s) => s,
                    None => {
                        if is_optional {
                            log::warn!(
                                "tool {}: stdin parameter '{}' has non-string type, skipping",
                                spec.tool,
                                arg.param_name()
                            );
                            return Ok(None);
                        }
                        return Err(format!(
                            "stdin parameter '{}' must be a string, number, or boolean",
                            arg.param_name()
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
                    arg.param_name()
                );
                return Err(format!(
                    "missing required parameter: {}",
                    arg.param_name()
                ));
            }
        };
        return Ok(Some(transform_param_value(value, arg, allowlist, skill_dir, args)?));
    }
    Ok(None)
}

/// Compute temp-file parameters without writing, returning the argv entries
/// (--flag <path>), temp file paths, and the content that would be written.
/// Used by the dry-run preview to show temp file details without side effects.
pub(crate) fn compute_temp_files(
    spec: &ExecutionSpec,
    args: &serde_json::Value,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
) -> Result<(Vec<String>, Vec<std::path::PathBuf>, Vec<(String, String)>), String> {
    let obj = match args.as_object() {
        Some(o) => o,
        None => return Ok((Vec::new(), Vec::new(), Vec::new())),
    };
    let mut argv_entries = Vec::new();
    let mut temp_paths = Vec::new();
    let mut temp_file_details = Vec::new();
    for arg in &spec.args {
        if arg.kind != ArgKind::TempFile {
            continue;
        }
        let value = match obj.get(arg.param_name()) {
            Some(v) if !v.is_null() => {
                match json_value_to_string(v) {
                    Some(s) => s,
                    None => {
                        if arg.optional == Some(true) {
                            continue;
                        }
                        return Err(format!(
                            "tempfile parameter '{}' must be a string, number, or boolean",
                            arg.param_name()
                        ));
                    }
                }
            }
            _ => {
                if arg.optional == Some(true) {
                    continue;
                }
                return Err(format!(
                    "missing required parameter: {}",
                    arg.param_name()
                ));
            }
        };
        let resolved = transform_param_value(value, arg, allowlist, skill_dir, args)?;

        let temp_dir = std::env::temp_dir();
        let file_name = format!("chai_{}_{}", spec.tool, arg.param_name());
        let temp_path = temp_dir.join(&file_name);
        let temp_path_str = temp_path.to_string_lossy().into_owned();
        temp_paths.push(temp_path.clone());
        temp_file_details.push((temp_path_str.clone(), resolved));

        let flag = arg.flag.as_deref().unwrap_or(arg.param_name());
        argv_entries.push(format_flag(flag));
        argv_entries.push(temp_path_str);
    }
    Ok((argv_entries, temp_paths, temp_file_details))
}

/// Write temp-file parameters to temporary files, returning the list of temp file paths
/// (for cleanup after execution) and the argv entries (--flag <path>) to append to the
/// command's argument list.
pub(crate) fn write_temp_files(
    spec: &ExecutionSpec,
    args: &serde_json::Value,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
) -> Result<(Vec<String>, Vec<std::path::PathBuf>), String> {
    let obj = match args.as_object() {
        Some(o) => o,
        None => return Ok((Vec::new(), Vec::new())),
    };
    let mut argv_entries = Vec::new();
    let mut temp_paths = Vec::new();
    for arg in &spec.args {
        if arg.kind != ArgKind::TempFile {
            continue;
        }
        let value = match obj.get(arg.param_name()) {
            Some(v) if !v.is_null() => {
                match json_value_to_string(v) {
                    Some(s) => s,
                    None => {
                        if arg.optional == Some(true) {
                            continue;
                        }
                        return Err(format!(
                            "tempfile parameter '{}' must be a string, number, or boolean",
                            arg.param_name()
                        ));
                    }
                }
            }
            _ => {
                if arg.optional == Some(true) {
                    continue;
                }
                return Err(format!(
                    "missing required parameter: {}",
                    arg.param_name()
                ));
            }
        };
        let resolved = transform_param_value(value, arg, allowlist, skill_dir, args)?;

        // Write the value to a temp file.
        let temp_dir = std::env::temp_dir();
        let file_name = format!("chai_{}_{}", spec.tool, arg.param_name());
        let temp_path = temp_dir.join(&file_name);
        std::fs::write(&temp_path, resolved.as_bytes())
            .map_err(|e| format!("failed to write temp file for '{}': {}", arg.param_name(), e))?;
        let temp_path_str = temp_path.to_string_lossy().into_owned();
        temp_paths.push(temp_path);

        // Append --flag <path> to argv.
        let flag = arg.flag.as_deref().unwrap_or(arg.param_name());
        argv_entries.push(format_flag(flag));
        argv_entries.push(temp_path_str);
    }
    Ok((argv_entries, temp_paths))
}

/// Format a flag name for argv: single-character names get a single-dash prefix
/// (`-n`), multi-character names get a double-dash prefix (`--number`).
/// This follows the universal CLI convention for short vs long flags.
/// Leading dashes are stripped first so that both bare names (`"p"`) and
/// pre-dashed values (`"-p"`) produce the correct result.
pub(crate) fn format_flag(flag: &str) -> String {
    let bare = flag.trim_start_matches('-');
    if bare.len() == 1 {
        format!("-{}", bare)
    } else {
        format!("--{}", bare)
    }
}

/// Build the argv list from the execution spec and tool call JSON.
/// Handles positional args, flags (short and long), flagIfBoolean, stdin,
/// workingDir, and tempfile arg kinds. Optional args are skipped when
/// absent; resolveCommand runs when configured.
pub(crate) fn build_argv(
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
                let s = match obj.get(arg.param_name()) {
                    Some(v) if !v.is_null() => json_value_to_string(v).ok_or_else(|| {
                        format!(
                            "parameter {} must be a string, number, or boolean",
                            arg.param_name()
                        )
                    })?,
                    _ if arg.absent_default.is_some() => {
                        json_value_to_string(arg.absent_default.as_ref().unwrap()).ok_or_else(|| {
                            format!(
                                "absentDefault for parameter {} must be a string, number, or boolean",
                                arg.param_name()
                            )
                        })?
                    }
                    _ if arg.optional != Some(true) => {
                        return Err(format!("missing parameter: {}", arg.param_name()));
                    }
                    _ if arg.resolve_command.is_some() => {
                        String::new()
                    }
                    _ => {
                        skipped_optional_positional = true;
                        continue;
                    }
                };
                if arg.disambiguate_after_skipped_positionals == Some(true)
                    && skipped_optional_positional
                {
                    argv.push("--".to_string());
                }
                skipped_optional_positional = false;
                let resolved = transform_param_value(s, arg, allowlist, skill_dir, args)?;
                if arg.split == Some(true) {
                    for part in resolved.split_whitespace() {
                        argv.push(part.to_string());
                    }
                } else {
                    argv.push(resolved);
                }
            }
            ArgKind::Flag => {
                match obj.get(arg.param_name()) {
                    Some(v) if !v.is_null() => {
                        let s = json_value_to_string(v).ok_or_else(|| {
                            format!(
                                "parameter {} must be a string, number, or boolean",
                                arg.param_name()
                            )
                        })?;
                        let flag = arg.flag.as_deref().unwrap_or(arg.param_name());
                        argv.push(format_flag(flag));
                        argv.push(transform_param_value(s, arg, allowlist, skill_dir, args)?);
                    }
                    _ if arg.absent_default.is_some() => {
                        let default = arg.absent_default.as_ref().unwrap();
                        let s = json_value_to_string(default).ok_or_else(|| {
                            format!(
                                "absentDefault for parameter {} must be a string, number, or boolean",
                                arg.param_name()
                            )
                        })?;
                        let flag = arg.flag.as_deref().unwrap_or(arg.param_name());
                        argv.push(format_flag(flag));
                        argv.push(transform_param_value(s, arg, allowlist, skill_dir, args)?);
                    }
                    _ if arg.optional == Some(true) && arg.resolve_command.is_some() => {
                        let flag = arg.flag.as_deref().unwrap_or(arg.param_name());
                        let resolved = transform_param_value(String::new(), arg, allowlist, skill_dir, args)?;
                        if !resolved.is_empty() {
                            argv.push(format_flag(flag));
                            argv.push(resolved);
                        }
                    }
                    _ => continue,
                }
            }
            ArgKind::Stdin => {
                continue;
            }
            ArgKind::WorkingDir => {
                // workingDir args set the process CWD via sandbox validation,
                // not via argv — skip them here.
                continue;
            }
            ArgKind::TempFile => {
                // tempfile args are handled by write_temp_files which appends
                // the flag + temp-file-path directly to argv — skip here.
                continue;
            }
            ArgKind::FlagIfBoolean => {
                // When subcommandOverride is set, this arg controls the
                // subcommand selection (handled by resolve_subcommand in
                // the executor) and does not produce an argv entry.
                if arg.subcommand_override.is_some() {
                    continue;
                }
                let value = obj.get(arg.param_name());
                // When the parameter is absent, use absentDefault if set;
                // otherwise treat as false (the original behavior).
                let effective_value = if value.is_none() || value == Some(&serde_json::Value::Null) {
                    arg.absent_default.clone()
                } else {
                    value.cloned()
                };
                let effective_ref = effective_value.as_ref();
                let flag = match parse_bool(effective_ref) {
                    Some(true) => arg.flag_if_true.as_deref(),
                    _ => arg.flag_if_false.as_deref(),
                };
                if let Some(f) = flag {
                    argv.push(f.to_string());
                }
            }
            ArgKind::Literal => {
                if let Some(ref v) = arg.value {
                    argv.push(v.clone());
                }
            }
        }
    }
    Ok(argv)
}

/// Convert a JSON value to a string representation.
/// Returns `None` for types that cannot be meaningfully converted (null, arrays, objects).
pub(crate) fn json_value_to_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Parse a JSON value as a boolean. Accepts native bools, the string "true"
/// (case-insensitive), and non-zero numbers as true.
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
    use crate::skills::{ArgMapping, ExecutionSpec, ResolveCommandSpec};
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
        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: Some("content".to_string()),
                kind: ArgKind::Stdin,
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "content": "hello world" });
        let result = extract_stdin_content(&spec, &args, &Allowlist::new(), None)
            .expect("should not error");
        assert_eq!(result, Some("hello world".to_string()));
    }

    #[test]
    fn extract_stdin_content_errors_on_required_missing() {
        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: Some("content".to_string()),
                kind: ArgKind::Stdin,
                ..Default::default()
            }],
            ..Default::default()
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
        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: Some("content".to_string()),
                kind: ArgKind::Stdin,
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "content": null });
        let result = extract_stdin_content(&spec, &args, &Allowlist::new(), None);
        assert!(result.is_err(), "null required stdin param should error");
    }

    #[test]
    fn extract_stdin_content_optional_missing_returns_none() {
        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: Some("content".to_string()),
                kind: ArgKind::Stdin,
                optional: Some(true),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "path": "/some/file" });
        let result = extract_stdin_content(&spec, &args, &Allowlist::new(), None)
            .expect("optional missing should not error");
        assert_eq!(result, None, "optional missing stdin should return None");
    }

    // --- optional flag with resolveCommand tests ---

    #[test]
    fn build_argv_optional_flag_with_resolve_command_runs_resolver_when_omitted() {
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
                    param: Some("date".to_string()),
                    kind: ArgKind::Flag,
                    flag: Some("path".to_string()),
                    optional: Some(true),
                    resolve_command: Some(ResolveCommandSpec {
                        script: Some("resolve-path".to_string()),
                        binary: None,
                        subcommand: None,
                        args: vec!["$param".to_string()],
                    }),
                    ..Default::default()
                },
                ArgMapping {
                    param: Some("content".to_string()),
                    kind: ArgKind::Stdin,
                    ..Default::default()
                },
            ],
            ..Default::default()
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
        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: Some("optional_flag".to_string()),
                kind: ArgKind::Flag,
                flag: Some("opt".to_string()),
                optional: Some(true),
                ..Default::default()
            }],
            ..Default::default()
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
        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: Some("required_flag".to_string()),
                kind: ArgKind::Flag,
                flag: Some("req".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();

        let args = serde_json::json!({});
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");
        assert!(argv.is_empty());
    }

    // --- format_flag tests ---

    #[test]
    fn format_flag_bare_single_char() {
        assert_eq!(format_flag("n"), "-n");
    }

    #[test]
    fn format_flag_bare_multi_char() {
        assert_eq!(format_flag("path"), "--path");
    }

    #[test]
    fn format_flag_pre_dashed_single_char() {
        assert_eq!(format_flag("-p"), "-p");
    }

    #[test]
    fn format_flag_pre_dashed_multi_char() {
        assert_eq!(format_flag("--path"), "--path");
    }

    #[test]
    fn format_flag_triple_dashed_multi_char() {
        assert_eq!(format_flag("---verbose"), "--verbose");
    }

    // --- short flag vs long flag tests ---
    #[test]
    fn build_argv_single_char_flag_uses_single_dash() {
        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "git".to_string(),
            subcommand: "log".to_string(),
            args: vec![ArgMapping {
                param: Some("count".to_string()),
                kind: ArgKind::Flag,
                flag: Some("n".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({ "count": "5" });
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["-n", "5"],
            "single-char flag 'n' should produce '-n', not '--n'");
    }

    #[test]
    fn build_argv_multi_char_flag_uses_double_dash() {
        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "read".to_string(),
            args: vec![ArgMapping {
                param: Some("path".to_string()),
                kind: ArgKind::Flag,
                flag: Some("path".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({ "path": "./chai" });
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["--path", "./chai"],
            "multi-char flag 'path' should produce '--path'");
    }

    #[test]
    fn build_argv_flag_defaults_to_param_name_long_form() {
        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "cmd".to_string(),
            args: vec![ArgMapping {
                param: Some("output".to_string()),
                kind: ArgKind::Flag,
                flag: None, // no explicit flag — uses param name
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({ "output": "result.txt" });
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["--output", "result.txt"],
            "flag with no explicit name should default to --paramname");
    }

    #[test]
    fn build_argv_git_log_with_count_and_oneline() {
        // Simulates the git_log execution spec from tools.json
        let spec = ExecutionSpec {
            tool: "git_log".to_string(),
            binary: "git".to_string(),
            subcommand: "log".to_string(),
            args: vec![
                ArgMapping {
                    param: Some("count".to_string()),
                    kind: ArgKind::Flag,
                    flag: Some("n".to_string()),
                    ..Default::default()
                },
                ArgMapping {
                    param: Some("oneline".to_string()),
                    kind: ArgKind::FlagIfBoolean,
                    flag_if_true: Some("--oneline".to_string()),
                    ..Default::default()
                },
                ArgMapping {
                    param: Some("path".to_string()),
                    kind: ArgKind::WorkingDir,
                    optional: Some(true),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({ "count": "5", "oneline": true, "path": "/some/repo" });
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["-n", "5", "--oneline"],
            "git_log with count=5 and oneline=true should produce '-n 5 --oneline'; workingdir arg should be excluded from argv");
    }

    #[test]
    fn build_argv_git_commit_with_message() {
        // Simulates the git_commit execution spec from tools.json
        let spec = ExecutionSpec {
            tool: "git_commit".to_string(),
            binary: "git".to_string(),
            subcommand: "commit".to_string(),
            args: vec![ArgMapping {
                param: Some("message".to_string()),
                kind: ArgKind::Flag,
                flag: Some("m".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({ "message": "Add search endpoint" });
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["-m", "Add search endpoint"],
            "git_commit with message should produce '-m \"msg\"', not '--m \"msg\"'");
    }

    // --- Literal kind tests ---

    #[test]
    fn build_argv_literal_pushes_fixed_value() {
        let spec = ExecutionSpec {
            tool: "git_rebase".to_string(),
            binary: "git".to_string(),
            subcommand: "rebase".to_string(),
            args: vec![ArgMapping {
                param: None,
                kind: ArgKind::Literal,
                value: Some("--continue".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({});
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["--continue"]);
    }

    #[test]
    fn build_argv_literal_with_no_value_is_skipped() {
        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "git".to_string(),
            subcommand: "rebase".to_string(),
            args: vec![ArgMapping {
                param: None,
                kind: ArgKind::Literal,
                value: None,
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({});
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert!(argv.is_empty(), "literal with no value should produce no argv entry");
    }

    // --- Split positional tests ---

    #[test]
    fn build_argv_split_positional_splits_on_whitespace() {
        let spec = ExecutionSpec {
            tool: "git_add".to_string(),
            binary: "git".to_string(),
            subcommand: "add".to_string(),
            args: vec![ArgMapping {
                param: Some("paths".to_string()),
                kind: ArgKind::Positional,
                split: Some(true),
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({ "paths": "file1.rs file2.rs file3.rs" });
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["file1.rs", "file2.rs", "file3.rs"]);
    }

    #[test]
    fn build_argv_split_positional_single_value() {
        let spec = ExecutionSpec {
            tool: "git_add".to_string(),
            binary: "git".to_string(),
            subcommand: "add".to_string(),
            args: vec![ArgMapping {
                param: Some("paths".to_string()),
                kind: ArgKind::Positional,
                split: Some(true),
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({ "paths": "." });
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["."]);
    }

    #[test]
    fn build_argv_positional_without_split_keeps_value_intact() {
        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: Some("query".to_string()),
                kind: ArgKind::Positional,
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({ "query": "hello world" });
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["hello world"],
            "positional without split should keep value as single argv entry");
    }

    #[test]
    fn build_argv_positional_absent_default_when_param_absent() {
        let spec = ExecutionSpec {
            tool: "git_reset".to_string(),
            binary: "git".to_string(),
            subcommand: "reset".to_string(),
            args: vec![ArgMapping {
                param: Some("ref".to_string()),
                kind: ArgKind::Positional,
                optional: Some(true),
                absent_default: Some(serde_json::json!("HEAD~1")),
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({});
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["HEAD~1"],
            "absent optional positional should use absentDefault value");
    }

    #[test]
    fn build_argv_positional_absent_default_overridden_by_explicit_value() {
        let spec = ExecutionSpec {
            tool: "git_reset".to_string(),
            binary: "git".to_string(),
            subcommand: "reset".to_string(),
            args: vec![ArgMapping {
                param: Some("ref".to_string()),
                kind: ArgKind::Positional,
                optional: Some(true),
                absent_default: Some(serde_json::json!("HEAD~1")),
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({ "ref": "HEAD~3" });
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["HEAD~3"],
            "explicit value should override absentDefault");
    }

    #[test]
    fn build_argv_positional_no_absent_default_still_skips() {
        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: Some("ref".to_string()),
                kind: ArgKind::Positional,
                optional: Some(true),
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({});
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert!(argv.is_empty(),
            "optional positional without absentDefault should still be skipped when absent");
    }

    #[test]
    fn build_argv_skips_flagifboolean_with_subcommand_override() {
        let spec = ExecutionSpec {
            tool: "git_branch_delete".to_string(),
            binary: "git".to_string(),
            subcommand: "branch -d".to_string(),
            args: vec![
                ArgMapping {
                    param: Some("force".to_string()),
                    kind: ArgKind::FlagIfBoolean,
                    optional: Some(true),
                    absent_default: Some(serde_json::json!(false)),
                    subcommand_override: Some("branch -D".to_string()),
                    ..Default::default()
                },
                ArgMapping {
                    param: Some("branch_name".to_string()),
                    kind: ArgKind::Positional,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({ "force": true, "branch_name": "feat/test" });
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        // force should NOT produce an argv entry (subcommandOverride controls
        // the subcommand, not argv); only the branch_name positional should appear.
        assert_eq!(argv, vec!["feat/test"]);
    }

    #[test]
    fn build_argv_flagifboolean_without_subcommand_override_still_produces_flag() {
        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: Some("verbose".to_string()),
                kind: ArgKind::FlagIfBoolean,
                flag_if_true: Some("--verbose".to_string()),
                optional: Some(true),
                // no subcommand_override — should still produce argv entry
                ..Default::default()
            }],
            ..Default::default()
        };

        let allowlist = Allowlist::new();
        let args = serde_json::json!({ "verbose": true });
        let argv = build_argv(&spec, &args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv, vec!["--verbose"]);
    }
}
