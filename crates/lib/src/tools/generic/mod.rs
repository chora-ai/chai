//! Generic tool executor driven by a skill's tools.json descriptor.
//! Builds argv from the execution spec's arg mapping and runs via the allowlist.
//! Supports resolve-by-command or resolve-by-script for param resolution,
//! sandbox-validated path enforcement (readPath/writePath), runtime path-like
//! value checking (absolute paths, home-relative paths, `file://` URLs,
//! directory traversal) for unannotated parameters, default CWD confinement
//! to the sandbox root, and side-read augmentation (append a nearby file's
//! contents to the tool result).

mod argv;
mod deny;
mod output;
mod sandbox;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::agent::ToolExecutor;
use crate::exec::{Allowlist, WriteSandbox};
use crate::skills::{ArgKind, ToolDescriptor};
use crate::tools::post_process::run_post_process;

/// Augment tool call args with `absentDefault` values from the execution spec.
/// When a parameter is absent from the tool call JSON and its `ArgMapping` has an
/// `absent_default`, the default is injected so that post-process scripts can
/// reference `$param_name` and get the effective value rather than an empty string.
fn augment_with_absent_defaults(
    spec: &crate::skills::ExecutionSpec,
    args: &serde_json::Value,
) -> serde_json::Value {
    let obj = match args.as_object() {
        Some(o) => o,
        None => return args.clone(),
    };
    let mut needs_augment = false;
    for arg in &spec.args {
        // Only consider args that have an absent_default and are currently
        // absent from the tool call JSON. WorkingDir and Literal args don't
        // have user-facing params; Stdin and TempFile are not referenced in
        // postProcess args.
        if arg.absent_default.is_some()
            && arg.kind != ArgKind::WorkingDir
            && arg.kind != ArgKind::Literal
            && arg.kind != ArgKind::Stdin
            && arg.kind != ArgKind::TempFile
        {
            let param_name = arg.param_name();
            if !obj.contains_key(param_name) || obj.get(param_name) == Some(&serde_json::Value::Null) {
                needs_augment = true;
                break;
            }
        }
    }
    if !needs_augment {
        return args.clone();
    }
    let mut augmented = obj.clone();
    for arg in &spec.args {
        if arg.absent_default.is_some()
            && arg.kind != ArgKind::WorkingDir
            && arg.kind != ArgKind::Literal
            && arg.kind != ArgKind::Stdin
            && arg.kind != ArgKind::TempFile
        {
            let param_name = arg.param_name();
            if !augmented.contains_key(param_name) || augmented.get(param_name) == Some(&serde_json::Value::Null) {
                augmented.insert(param_name.to_string(), arg.absent_default.as_ref().unwrap().clone());
            }
        }
    }
    serde_json::Value::Object(augmented)
}

/// Resolve the effective subcommand for an execution spec.
///
/// Checks whether any `flagIfBoolean` arg has a `subcommandOverride` and
/// its boolean parameter evaluates to true in the tool call args. If so,
/// returns the override subcommand; otherwise returns the spec's default.
/// The override subcommand must be in the allowlist (the allowlist check
/// happens later when the command is executed).
fn resolve_subcommand<'a>(
    spec: &'a crate::skills::ExecutionSpec,
    args: &serde_json::Value,
) -> &'a str {
    let obj = match args.as_object() {
        Some(o) => o,
        None => return &spec.subcommand,
    };

    for arg in &spec.args {
        if arg.kind != ArgKind::FlagIfBoolean {
            continue;
        }
        let Some(ref override_sub) = arg.subcommand_override else {
            continue;
        };
        let value = obj.get(arg.param_name());
        let effective_value = if value.is_none() || value == Some(&serde_json::Value::Null) {
            arg.absent_default.as_ref()
        } else {
            value
        };
        if parse_bool_ref(effective_value) == Some(true) {
            return override_sub;
        }
    }

    &spec.subcommand
}

/// Parse a JSON value reference as a boolean (mirrors argv::parse_bool but
/// takes a reference to avoid cloning).
fn parse_bool_ref(v: Option<&serde_json::Value>) -> Option<bool> {
    match v {
        Some(serde_json::Value::Bool(b)) => Some(*b),
        Some(serde_json::Value::String(s)) => Some(s.eq_ignore_ascii_case("true")),
        Some(serde_json::Value::Number(n)) => n.as_i64().map(|i| i != 0),
        _ => None,
    }
}

/// Executes tools using a descriptor's allowlist and execution mapping.
/// Holds per-tool (allowlist, spec, skill_dir) for param resolution and argv building.
/// When a `WriteSandbox` is present, validates `readPath`- and `writePath`-annotated
/// arguments against writable roots before executing. When a tool has a `sideRead`
/// spec, the named file is appended to the tool result when found; `oncePerSession`
/// prevents re-appending the same file within the same session.
#[derive(Debug, Clone)]
pub struct GenericToolExecutor {
    /// tool_name -> (allowlist, execution spec, skill_dir for script resolution)
    map: HashMap<String, (Allowlist, crate::skills::ExecutionSpec, Option<std::path::PathBuf>)>,
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
            sandbox::validate_write_paths(spec, args, allowlist, skill_dir.as_deref(), &self.sandbox)?;

        // Default CWD to sandbox root when no working directory was determined
        // from path-annotated parameters. This ensures that relative paths in
        // unannotated parameters resolve within the sandbox boundary rather than
        // relative to the gateway's launch directory.
        let working_dir = match (working_dir, &self.sandbox) {
            (Some(dir), _) => Some(dir),
            (None, Some(sb)) => sb.roots().first().cloned(),
            (None, None) => None,
        };

        let resolved_args;
        let effective_args = if canonical_paths.is_empty() {
            args
        } else {
            resolved_args = sandbox::substitute_canonical_paths(args, &canonical_paths);
            &resolved_args
        };

        sandbox::ensure_write_path_parents(spec, &canonical_paths)?;

        deny::enforce_deny_patterns(spec, effective_args, allowlist, skill_dir.as_deref(), working_dir.as_deref())?;

        let mut argv = argv::build_argv(spec, effective_args, allowlist, skill_dir.as_deref())?;

        let stdin_content = argv::extract_stdin_content(spec, effective_args, allowlist, skill_dir.as_deref())?;

        let (temp_argv, temp_paths) = argv::write_temp_files(spec, effective_args, allowlist, skill_dir.as_deref())?;
        argv.extend(temp_argv);

        let effective_subcommand = resolve_subcommand(spec, effective_args);
        let success_codes = spec.success_exit_codes.as_deref().unwrap_or(&[]);
        let binary_wrapper = spec.binary_wrapper.as_deref();
        let result = if let Some(ref content) = stdin_content {
            allowlist.run_with_stdin_with_codes_and_exit(
                &spec.binary,
                effective_subcommand,
                &argv,
                working_dir.as_deref(),
                content.as_bytes(),
                success_codes,
                binary_wrapper,
            )
        } else {
            allowlist.run_with_codes_and_exit(
                &spec.binary,
                effective_subcommand,
                &argv,
                working_dir.as_deref(),
                success_codes,
                binary_wrapper,
            )
        };

        // Clean up temp files regardless of execution success or failure.
        for path in &temp_paths {
            let _ = std::fs::remove_file(path);
        }

        let (exit_code, output) = result?;

        let result = if let Some(ref pp) = spec.post_process {
            let pp_args = augment_with_absent_defaults(spec, effective_args);
            run_post_process(pp, exit_code, &output, allowlist, skill_dir.as_deref(), &pp_args)
        } else {
            output
        };

        let result = if let Some(max_lines) = spec.max_output_lines {
            output::truncate_output(&result, max_lines, spec.truncation_hint.as_deref())
        } else {
            result
        };

        if let Some(ref sr) = spec.side_read {
            Ok(output::apply_side_read(sr, effective_args, &result, session_id, &self.side_read_seen))
        } else {
            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{ArgKind, ArgMapping, ExecutionSpec};

    #[test]
    fn augment_injects_absent_default_for_missing_positional() {
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

        let args = serde_json::json!({});
        let augmented = augment_with_absent_defaults(&spec, &args);
        assert_eq!(augmented["ref"], "HEAD~1");
    }

    #[test]
    fn augment_does_not_override_explicit_value() {
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

        let args = serde_json::json!({ "ref": "HEAD~3" });
        let augmented = augment_with_absent_defaults(&spec, &args);
        assert_eq!(augmented["ref"], "HEAD~3");
    }

    #[test]
    fn augment_preserves_existing_params() {
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

        let args = serde_json::json!({ "path": "./chai" });
        let augmented = augment_with_absent_defaults(&spec, &args);
        assert_eq!(augmented["path"], "./chai");
        assert_eq!(augmented["ref"], "HEAD~1");
    }

    #[test]
    fn augment_skips_working_dir_and_literal_kinds() {
        let spec = ExecutionSpec {
            tool: "test".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![
                ArgMapping {
                    param: Some("path".to_string()),
                    kind: ArgKind::WorkingDir,
                    optional: Some(true),
                    absent_default: Some(serde_json::json!("/should/not/inject")),
                    ..Default::default()
                },
                ArgMapping {
                    kind: ArgKind::Literal,
                    value: Some("--continue".to_string()),
                    absent_default: Some(serde_json::json!("should-not-inject")),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let args = serde_json::json!({});
        let augmented = augment_with_absent_defaults(&spec, &args);
        // WorkingDir and Literal args should not inject into tool_args
        assert!(augmented.as_object().unwrap().is_empty());
    }

    #[test]
    fn augment_handles_null_value_as_absent() {
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

        let args = serde_json::json!({ "ref": null });
        let augmented = augment_with_absent_defaults(&spec, &args);
        assert_eq!(augmented["ref"], "HEAD~1");
    }

    #[test]
    fn augment_returns_clone_when_no_defaults_needed() {
        let spec = ExecutionSpec {
            tool: "test".to_string(),
            binary: "test".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: Some("ref".to_string()),
                kind: ArgKind::Positional,
                optional: Some(true),
                // no absent_default
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({});
        let augmented = augment_with_absent_defaults(&spec, &args);
        assert!(augmented.as_object().unwrap().is_empty());
    }

    // --- resolve_subcommand tests ---

    #[test]
    fn resolve_subcommand_returns_default_when_no_override() {
        let spec = ExecutionSpec {
            tool: "git_branch_delete".to_string(),
            binary: "git".to_string(),
            subcommand: "branch -d".to_string(),
            args: vec![ArgMapping {
                param: Some("branch_name".to_string()),
                kind: ArgKind::Positional,
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "branch_name": "feat/test" });
        assert_eq!(resolve_subcommand(&spec, &args), "branch -d");
    }

    #[test]
    fn resolve_subcommand_returns_override_when_force_true() {
        let spec = ExecutionSpec {
            tool: "git_branch_delete".to_string(),
            binary: "git".to_string(),
            subcommand: "branch -d".to_string(),
            args: vec![ArgMapping {
                param: Some("force".to_string()),
                kind: ArgKind::FlagIfBoolean,
                optional: Some(true),
                absent_default: Some(serde_json::json!(false)),
                subcommand_override: Some("branch -D".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "force": true });
        assert_eq!(resolve_subcommand(&spec, &args), "branch -D");
    }

    #[test]
    fn resolve_subcommand_returns_default_when_force_false() {
        let spec = ExecutionSpec {
            tool: "git_branch_delete".to_string(),
            binary: "git".to_string(),
            subcommand: "branch -d".to_string(),
            args: vec![ArgMapping {
                param: Some("force".to_string()),
                kind: ArgKind::FlagIfBoolean,
                optional: Some(true),
                absent_default: Some(serde_json::json!(false)),
                subcommand_override: Some("branch -D".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "force": false });
        assert_eq!(resolve_subcommand(&spec, &args), "branch -d");
    }

    #[test]
    fn resolve_subcommand_returns_default_when_force_absent() {
        let spec = ExecutionSpec {
            tool: "git_branch_delete".to_string(),
            binary: "git".to_string(),
            subcommand: "branch -d".to_string(),
            args: vec![ArgMapping {
                param: Some("force".to_string()),
                kind: ArgKind::FlagIfBoolean,
                optional: Some(true),
                absent_default: Some(serde_json::json!(false)),
                subcommand_override: Some("branch -D".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({});
        assert_eq!(resolve_subcommand(&spec, &args), "branch -d");
    }

    #[test]
    fn resolve_subcommand_returns_override_when_absent_default_true() {
        let spec = ExecutionSpec {
            tool: "test".to_string(),
            binary: "git".to_string(),
            subcommand: "branch -d".to_string(),
            args: vec![ArgMapping {
                param: Some("force".to_string()),
                kind: ArgKind::FlagIfBoolean,
                optional: Some(true),
                absent_default: Some(serde_json::json!(true)),
                subcommand_override: Some("branch -D".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({});
        assert_eq!(resolve_subcommand(&spec, &args), "branch -D");
    }

    #[test]
    fn resolve_subcommand_ignores_non_flagifboolean_args() {
        let spec = ExecutionSpec {
            tool: "git_branch_delete".to_string(),
            binary: "git".to_string(),
            subcommand: "branch -d".to_string(),
            args: vec![
                ArgMapping {
                    param: Some("force".to_string()),
                    kind: ArgKind::Positional, // not FlagIfBoolean
                    subcommand_override: Some("branch -D".to_string()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let args = serde_json::json!({ "force": "true" });
        assert_eq!(resolve_subcommand(&spec, &args), "branch -d");
    }

    #[test]
    fn resolve_subcommand_handles_string_true() {
        let spec = ExecutionSpec {
            tool: "git_branch_delete".to_string(),
            binary: "git".to_string(),
            subcommand: "branch -d".to_string(),
            args: vec![ArgMapping {
                param: Some("force".to_string()),
                kind: ArgKind::FlagIfBoolean,
                optional: Some(true),
                absent_default: Some(serde_json::json!(false)),
                subcommand_override: Some("branch -D".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "force": "true" });
        assert_eq!(resolve_subcommand(&spec, &args), "branch -D");
    }
}
