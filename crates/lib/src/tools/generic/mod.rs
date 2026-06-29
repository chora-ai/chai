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

use serde::Serialize;

use crate::agent::ToolExecutor;
use crate::exec::{Allowlist, WriteSandbox};
use crate::skills::{ArgKind, ToolDescriptor};
use crate::tools::post_process::run_post_process;

/// Result of a dry-run preview: shows what a tool call would execute without
/// running the command. The preview walks the execution pipeline up to (but
/// not including) the actual command execution, reporting sandbox validation,
/// deny pattern checks, argv construction, and post-processing pipeline status.
///
/// Sandbox validation failure short-circuits the pipeline (no argv, subcommand,
/// or resolved_params). Deny pattern failure does **not** short-circuit — the
/// argv and subcommand are still computed and reported, with the deny failure
/// recorded in `deny_patterns`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DryRunResult {
    /// Tool name from the execution spec.
    pub tool: String,
    /// Binary that would be executed (e.g. "git", "chai").
    pub binary: String,
    /// Effective subcommand (after subcommandOverride resolution).
    pub subcommand: String,
    /// Binary wrapper (e.g. ["nix", "develop", "--command"]), if configured.
    pub binary_wrapper: Option<Vec<String>>,
    /// Fully constructed argument list (after resolve, canonical path substitution, etc.).
    pub argv: Vec<String>,
    /// Resolved working directory for the process, if any.
    pub working_dir: Option<String>,
    /// Content that would be piped to stdin, if any.
    pub stdin_content: Option<String>,
    /// Temp files that would be written (path, content), if any.
    pub temp_files: Vec<TempFilePreview>,
    /// Sandbox validation result.
    pub sandbox_validation: StepResult,
    /// Deny pattern check result.
    pub deny_patterns: StepResult,
    /// Resolved parameter values (param_name -> resolved_value).
    pub resolved_params: HashMap<String, String>,
    /// Post-execution pipeline preview (postProcess, hintConditions, truncation).
    pub post_pipeline: PostPipelinePreview,
}

/// A temp file that would be written during execution.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TempFilePreview {
    /// Path where the temp file would be written.
    pub path: String,
    /// Content that would be written to the temp file.
    pub content: String,
}

/// Result of a validation step (sandbox or deny).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct StepResult {
    /// "pass", "fail", or "skipped".
    pub status: String,
    /// Error message when status is "fail".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Canonical paths resolved by sandbox validation (param_name -> canonical_path).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_paths: Option<HashMap<String, String>>,
}

/// Post-execution pipeline preview: describes what would happen after command
/// execution (postProcess, hintConditions, truncation, sideRead).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PostPipelinePreview {
    /// Whether postProcess would run.
    pub has_post_process: bool,
    /// Whether hintConditions would be evaluated.
    pub has_hint_conditions: bool,
    /// Whether output truncation would apply.
    pub has_truncation: bool,
    /// Whether sideRead would be applied.
    pub has_side_read: bool,
    /// Whether simulated output was provided for postProcess preview.
    pub simulated_output_provided: bool,
    /// Post-processed output when simulated_output is provided and postProcess is configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simulated_post_processed_output: Option<String>,
    /// Hints that would fire for the simulated output (when provided).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simulated_hints: Option<Vec<String>>,
    /// Truncated simulated output (when truncation would apply).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simulated_truncated_output: Option<String>,
}

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

    /// Preview what a tool call would execute without running the command.
    ///
    /// Walks the execution pipeline up to (but not including) the actual command
    /// execution. Returns a structured preview showing sandbox validation, deny
    /// pattern checks, argv construction, stdin content, temp files, and the
    /// post-execution pipeline status.
    ///
    /// Sandbox validation failures short-circuit the pipeline (nothing downstream
    /// can be computed without valid paths). Deny pattern failures do **not**
    /// short-circuit — the argv, subcommand, and resolved_params are still
    /// computed so the author can see what *would* execute even if the deny
    /// check would block the real execution.
    ///
    /// When `simulated_output` is provided, the post-execution pipeline
    /// (postProcess, hintConditions, truncation) is run on the simulated output
    /// so the author can verify the full transformation chain.
    pub fn dry_run(
        &self,
        name: &str,
        args: &serde_json::Value,
        simulated_output: Option<&str>,
    ) -> Result<DryRunResult, String> {
        let (allowlist, spec, skill_dir) = self
            .map
            .get(name)
            .ok_or_else(|| format!("unknown tool: {}", name))?;

        // Step 1: Sandbox validation
        let (working_dir, canonical_paths) =
            match sandbox::validate_write_paths(spec, args, allowlist, skill_dir.as_deref(), &self.sandbox) {
                Ok(result) => result,
                Err(e) => {
                    return Ok(DryRunResult {
                        tool: spec.tool.clone(),
                        binary: spec.binary.clone(),
                        subcommand: String::new(),
                        binary_wrapper: spec.binary_wrapper.clone(),
                        argv: Vec::new(),
                        working_dir: None,
                        stdin_content: None,
                        temp_files: Vec::new(),
                        sandbox_validation: StepResult {
                            status: "fail".to_string(),
                            error: Some(e),
                            canonical_paths: None,
                        },
                        deny_patterns: StepResult {
                            status: "skipped".to_string(),
                            error: None,
                            canonical_paths: None,
                        },
                        resolved_params: HashMap::new(),
                        post_pipeline: PostPipelinePreview {
                            has_post_process: spec.post_process.is_some(),
                            has_hint_conditions: spec.hint_conditions.is_some(),
                            has_truncation: spec.max_output_lines.is_some(),
                            has_side_read: spec.side_read.is_some(),
                            simulated_output_provided: simulated_output.is_some(),
                            simulated_post_processed_output: None,
                            simulated_hints: None,
                            simulated_truncated_output: None,
                        },
                    });
                }
            };

        let sandbox_validation = StepResult {
            status: "pass".to_string(),
            error: None,
            canonical_paths: if canonical_paths.is_empty() {
                None
            } else {
                Some(canonical_paths.clone())
            },
        };

        // Default CWD to sandbox root when no working directory was determined
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

        // Step 2: Deny pattern enforcement
        let deny_patterns = match deny::enforce_deny_patterns(
            spec,
            effective_args,
            allowlist,
            skill_dir.as_deref(),
            working_dir.as_deref(),
        ) {
            Ok(()) => StepResult {
                status: "pass".to_string(),
                error: None,
                canonical_paths: None,
            },
            Err(e) => StepResult {
                status: "fail".to_string(),
                error: Some(e),
                canonical_paths: None,
            },
        };

        // Deny check result is recorded but does not short-circuit the pipeline.
        // The dry-run's purpose is to preview what *would* execute — even if the
        // deny check would block the real execution, argv/subcommand/resolved_params
        // are still valuable for the author.

        // Step 3: Build argv
        let mut argv = argv::build_argv(spec, effective_args, allowlist, skill_dir.as_deref())?;

        // Step 4: Extract stdin content
        let stdin_content =
            argv::extract_stdin_content(spec, effective_args, allowlist, skill_dir.as_deref())?;

        // Step 5: Compute temp files (without writing)
        let (temp_argv, _temp_paths, temp_file_details) =
            argv::compute_temp_files(spec, effective_args, allowlist, skill_dir.as_deref())?;
        argv.extend(temp_argv);

        let temp_files: Vec<TempFilePreview> = temp_file_details
            .into_iter()
            .map(|(path, content)| TempFilePreview { path, content })
            .collect();

        // Step 6: Resolve subcommand
        let effective_subcommand = resolve_subcommand(spec, effective_args);

        // Build resolved_params: collect the effective parameter values
        // after resolution and canonical path substitution.
        let mut resolved_params = HashMap::new();
        if let Some(obj) = effective_args.as_object() {
            for (key, val) in obj {
                if let Some(s) = val.as_str() {
                    resolved_params.insert(key.clone(), s.to_string());
                } else if !val.is_null() {
                    resolved_params.insert(key.clone(), val.to_string());
                }
            }
        }

        // Step 7: Post-execution pipeline preview
        let has_post_process = spec.post_process.is_some();
        let has_hint_conditions = spec.hint_conditions.is_some();
        let has_truncation = spec.max_output_lines.is_some();
        let has_side_read = spec.side_read.is_some();

        let (simulated_post_processed_output, simulated_hints, simulated_truncated_output) =
            if let Some(sim_out) = simulated_output {
                let mut current = sim_out.to_string();

                // Run postProcess on simulated output
                if let Some(ref pp) = spec.post_process {
                    let pp_args = augment_with_absent_defaults(spec, effective_args);
                    current = run_post_process(
                        pp,
                        0, // assume success exit code
                        &current,
                        allowlist,
                        skill_dir.as_deref(),
                        &pp_args,
                    );
                }
                let post_processed = if has_post_process {
                    Some(current.clone())
                } else {
                    None
                };

                // Evaluate hintConditions on simulated output
                let hints = if let Some(ref conditions) = spec.hint_conditions {
                    let hc_args = augment_with_absent_defaults(spec, effective_args);
                    let mut fired = Vec::new();
                    for condition in conditions {
                        if condition.matches(0, &current, &hc_args) {
                            fired.push(condition.expand_hint(&hc_args));
                        }
                    }
                    if fired.is_empty() {
                        None
                    } else {
                        // Apply hint formatting (blank line + "hint: " prefix)
                        for hint in &fired {
                            if !current.is_empty() && !current.ends_with('\n') {
                                current.push('\n');
                            }
                            current.push('\n');
                            current.push_str("hint: ");
                            current.push_str(hint);
                            current.push('\n');
                        }
                        Some(fired)
                    }
                } else {
                    None
                };

                // Apply truncation
                let truncated = if let Some(max_lines) = spec.max_output_lines {
                    let truncated = output::truncate_output(
                        &current,
                        max_lines,
                        spec.truncation_hint.as_deref(),
                    );
                    Some(truncated)
                } else {
                    None
                };

                (post_processed, hints, truncated)
            } else {
                (None, None, None)
            };

        Ok(DryRunResult {
            tool: spec.tool.clone(),
            binary: spec.binary.clone(),
            subcommand: effective_subcommand.to_string(),
            binary_wrapper: spec.binary_wrapper.clone(),
            argv,
            working_dir: working_dir.map(|d| d.to_string_lossy().into_owned()),
            stdin_content,
            temp_files,
            sandbox_validation,
            deny_patterns,
            resolved_params,
            post_pipeline: PostPipelinePreview {
                has_post_process,
                has_hint_conditions,
                has_truncation,
                has_side_read,
                simulated_output_provided: simulated_output.is_some(),
                simulated_post_processed_output,
                simulated_hints,
                simulated_truncated_output,
            },
        })
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

        let hc_args = augment_with_absent_defaults(spec, effective_args);
        let result = if let Some(ref conditions) = spec.hint_conditions {
            output::apply_hint_conditions(conditions, exit_code, &result, &hc_args)
        } else {
            result
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
