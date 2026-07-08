//! Generic tool executor driven by a skill's tools.json descriptor.
//! Builds argv from the execution spec's arg mapping and runs via the allowlist.
//! Supports resolve-by-command or resolve-by-script for param resolution,
//! sandbox-validated path enforcement (readPath/writePath), runtime path-like
//! value checking (absolute paths, home-relative paths, `file://` URLs,
//! directory traversal) for unannotated parameters, default CWD confinement
//! to the sandbox root, side-read augmentation (append a nearby file's
//! contents to the tool result), parameter-based execution routing via
//! `paramCondition` (multiple specs per tool name), and schema-enforced
//! validation (the tool schema is the contract — undeclared parameters and
//! type mismatches are rejected before execution).

mod argv;
mod deny;
mod output;
mod sandbox;
mod validate;

// Re-export for use in dry_run and execute.
use validate::check_type;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::agent::ToolExecutor;
use crate::exec::{Allowlist, WriteSandbox};
use crate::skills::{ArgKind, ToolDescriptor, ToolSpec};
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
/// One execution variant for a tool name. When multiple specs share a tool
/// name, `param_condition` selects the right one at call time.
#[derive(Debug, Clone)]
struct ExecEntry {
    allowlist: Allowlist,
    spec: crate::skills::ExecutionSpec,
    skill_dir: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone)]
pub struct GenericToolExecutor {
    /// tool_name -> (execution entries, optional tool schema for validation)
    /// Multiple entries per tool name support `paramCondition`-based routing.
    map: HashMap<String, (Vec<ExecEntry>, Option<ToolSpec>)>,
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
        let mut map: HashMap<String, (Vec<ExecEntry>, Option<ToolSpec>)> = HashMap::new();
        for (skill_name, desc) in descriptors {
            let allowlist = desc.to_allowlist();
            let skill_dir = dir_map.get(skill_name).cloned().cloned();
            for spec in &desc.execution {
                let entry = ExecEntry {
                    allowlist: allowlist.clone(),
                    spec: spec.clone(),
                    skill_dir: skill_dir.clone(),
                };
                map.entry(spec.tool.clone())
                    .or_insert_with(|| (Vec::new(), None))
                    .0
                    .push(entry);
            }
            // Index tool specs by name for schema validation.
            for tool_spec in &desc.tools {
                if let Some((_, schema)) = map.get_mut(&tool_spec.name) {
                    *schema = Some(tool_spec.clone());
                } else {
                    map.entry(tool_spec.name.clone())
                        .or_insert_with(|| (Vec::new(), None))
                        .1 = Some(tool_spec.clone());
                }
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

    /// Resolve the execution entry for a tool name given the call arguments.
    /// When multiple entries share a tool name, selects based on `paramCondition`.
    /// Returns `Err` if the tool is unknown or no condition matches. When no
    /// condition matches and there are partial matches (some `present` parameters
    /// satisfied but not all), the error includes a hint about missing paired
    /// parameters.
    fn resolve_entry(
        &self,
        name: &str,
        args: &serde_json::Value,
    ) -> Result<&ExecEntry, String> {
        let (entries, _schema) = self
            .map
            .get(name)
            .ok_or_else(|| format!("unknown tool: {}", name))?;
        if entries.len() == 1 {
            return Ok(&entries[0]);
        }
        // Multiple entries: select based on paramCondition.
        // First, try entries with a paramCondition that matches.
        let mut matched: Vec<&ExecEntry> = Vec::new();
        for entry in entries {
            if let Some(ref cond) = entry.spec.param_condition {
                if cond.matches(args) {
                    matched.push(entry);
                }
            }
        }
        if matched.len() == 1 {
            return Ok(matched[0]);
        }
        if matched.len() > 1 {
            // Collect the parameter names that caused each condition to match,
            // so the caller knows which parameters are conflicting.
            let conditions: Vec<String> = matched
                .iter()
                .filter_map(|e| {
                    e.spec.param_condition.as_ref().map(|cond| {
                        let matched_params: Vec<&str> = cond
                            .present
                            .iter()
                            .filter(|name| {
                                args.as_object()
                                    .and_then(|obj| obj.get(name.as_str()))
                                    .map_or(false, |v| !v.is_null())
                            })
                            .map(|n| n.as_str())
                            .collect();
                        if matched_params.is_empty() {
                            // Condition matched via absent-only — describe what was absent.
                            let absent_params: Vec<&str> = cond
                                .absent
                                .iter()
                                .filter(|name| {
                                    args.as_object()
                                        .and_then(|obj| obj.get(name.as_str()))
                                        .map_or(true, |v| v.is_null())
                                })
                                .map(|n| n.as_str())
                                .collect();
                            format!("absent: [{}]", absent_params.join(", "))
                        } else {
                            format!("present: [{}]", matched_params.join(", "))
                        }
                    })
                })
                .collect();
            return Err(format!(
                "tool {}: multiple execution specs match the given parameters (matching conditions: {})",
                name,
                conditions.join("; ")
            ));
        }
        // No paramCondition matched: fall back to the default entry (no paramCondition).
        let defaults: Vec<&ExecEntry> = entries
            .iter()
            .filter(|e| e.spec.param_condition.is_none())
            .collect();
        if defaults.len() == 1 {
            return Ok(defaults[0]);
        }

        // No match and no default. Check for partial paramCondition matches
        // to provide a helpful hint about missing paired parameters.
        let mut partial_hints: Vec<String> = Vec::new();
        for entry in entries {
            if let Some(ref cond) = entry.spec.param_condition {
                if let Some((satisfied, missing)) = cond.partial_present_match(args) {
                    let satisfied_list = satisfied
                        .iter()
                        .map(|n| format!("'{}'", n))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let missing_list = missing
                        .iter()
                        .map(|n| format!("'{}'", n))
                        .collect::<Vec<_>>()
                        .join(", ");
                    partial_hints.push(format!(
                        "parameter(s) {} must be provided together with {}",
                        missing_list, satisfied_list
                    ));
                }
            }
        }
        // Deduplicate identical hints (e.g., when multiple entries produce
        // the same missing-parameter message).
        partial_hints.sort();
        partial_hints.dedup();

        if partial_hints.is_empty() {
            Err(format!(
                "tool {}: no execution spec matches the given parameters",
                name
            ))
        } else {
            Err(format!(
                "tool {}: no execution spec matches the given parameters; {}",
                name,
                partial_hints.join("; ")
            ))
        }
    }

    /// Validate tool call arguments against the tool's parameter schema.
    /// The schema is the contract: undeclared parameters and type mismatches
    /// are rejected before execution.
    fn validate_schema(
        &self,
        name: &str,
        args: &serde_json::Value,
    ) -> Result<(), String> {
        let (_entries, schema) = self
            .map
            .get(name)
            .ok_or_else(|| format!("unknown tool: {}", name))?;
        let Some(ref tool_spec) = schema else {
            // No schema available — skip validation. This supports skills
            // that define execution specs without corresponding tool schemas.
            return Ok(());
        };
        let params_schema = &tool_spec.parameters;
        let obj = match args.as_object() {
            Some(o) => o,
            None => {
                return Err(format!(
                    "tool {}: arguments must be a JSON object",
                    name
                ));
            }
        };

        // Extract declared properties and required fields from the schema.
        let properties = params_schema
            .get("properties")
            .and_then(|p| p.as_object());
        let required: Vec<&str> = params_schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        // Check for undeclared parameters.
        if let Some(props) = properties {
            for key in obj.keys() {
                if !props.contains_key(key) {
                    return Err(format!(
                        "tool {}: undeclared parameter '{}' (not in schema)",
                        name, key
                    ));
                }
            }
        }

        // Check for type mismatches on declared parameters.
        if let Some(props) = properties {
            for (key, value) in obj {
                if value.is_null() {
                    continue; // null is treated as absent for type checking.
                }
                if let Some(param_schema) = props.get(key) {
                    if let Some(type_err) = check_type(param_schema, value) {
                        return Err(format!(
                            "tool {}: parameter '{}' type mismatch: {}",
                            name, key, type_err
                        ));
                    }
                }
            }
        }

        // Check for missing required parameters.
        for req in &required {
            match obj.get(*req) {
                Some(v) if !v.is_null() => {}
                _ => {
                    return Err(format!(
                        "tool {}: missing required parameter '{}'",
                        name, req
                    ));
                }
            }
        }

        Ok(())
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
        // Schema validation (Decision 2: the schema is the contract).
        self.validate_schema(name, args)?;

        let entry = self.resolve_entry(name, args)?;
        let (allowlist, spec, skill_dir) = (&entry.allowlist, &entry.spec, &entry.skill_dir);

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
        // Schema validation (Decision 2: the schema is the contract).
        self.validate_schema(name, args)?;

        let entry = self.resolve_entry(name, args)?;
        let (allowlist, spec, skill_dir) = (&entry.allowlist, &entry.spec, &entry.skill_dir);

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

    // --- resolve_entry partial match hint tests ---

    fn make_entry(tool: &str, subcommand: &str, param_condition: Option<crate::skills::ParamConditionSpec>) -> ExecEntry {
        ExecEntry {
            allowlist: Allowlist::default(),
            spec: ExecutionSpec {
                tool: tool.to_string(),
                binary: "test".to_string(),
                subcommand: subcommand.to_string(),
                param_condition: param_condition,
                ..Default::default()
            },
            skill_dir: None,
        }
    }

    #[test]
    fn resolve_entry_includes_partial_match_hint_when_start_line_without_original_content() {
        // Generic paramCondition test: two entries where present params
        // must be provided together. files_write no longer uses
        // paramCondition, so we use a hypothetical test_tool.
        let entries = vec![
            make_entry(
                "test_tool",
                "mode_a",
                Some(crate::skills::ParamConditionSpec {
                    present: vec![],
                    absent: vec!["param_a".to_string(), "param_b".to_string()],
                }),
            ),
            make_entry(
                "test_tool",
                "mode_b",
                Some(crate::skills::ParamConditionSpec {
                    present: vec!["param_a".to_string(), "param_b".to_string()],
                    absent: vec![],
                }),
            ),
        ];
        let executor = GenericToolExecutor {
            map: vec![(
                "test_tool".to_string(),
                (entries, None),
            )]
            .into_iter()
            .collect(),
            sandbox: None,
            side_read_seen: Arc::new(Mutex::new(HashMap::new())),
        };
        // param_a provided but param_b missing
        let args = serde_json::json!({ "path": "foo.txt", "param_a": 5 });
        let result = executor.resolve_entry("test_tool", &args);
        let err = result.unwrap_err();
        assert!(err.contains("parameter(s) 'param_b' must be provided together with 'param_a'"), "unexpected error: {}", err);
    }

    #[test]
    fn resolve_entry_includes_partial_match_hint_when_original_content_without_start_line() {
        // Generic paramCondition test: param_b provided but param_a missing.
        let entries = vec![
            make_entry(
                "test_tool",
                "mode_a",
                Some(crate::skills::ParamConditionSpec {
                    present: vec![],
                    absent: vec!["param_a".to_string(), "param_b".to_string()],
                }),
            ),
            make_entry(
                "test_tool",
                "mode_b",
                Some(crate::skills::ParamConditionSpec {
                    present: vec!["param_a".to_string(), "param_b".to_string()],
                    absent: vec![],
                }),
            ),
        ];
        let executor = GenericToolExecutor {
            map: vec![(
                "test_tool".to_string(),
                (entries, None),
            )]
            .into_iter()
            .collect(),
            sandbox: None,
            side_read_seen: Arc::new(Mutex::new(HashMap::new())),
        };
        // param_b provided but param_a missing
        let args = serde_json::json!({ "path": "foo.txt", "param_b": "old" });
        let result = executor.resolve_entry("test_tool", &args);
        let err = result.unwrap_err();
        assert!(err.contains("parameter(s) 'param_a' must be provided together with 'param_b'"), "unexpected error: {}", err);
    }

    #[test]
    fn resolve_entry_no_partial_hint_when_no_present_params_satisfied() {
        // Use entries where neither matches and no present params are partially
        // satisfied — so no partial hint should be included.
        let entries = vec![
            make_entry(
                "test_tool",
                "mode_a",
                Some(crate::skills::ParamConditionSpec {
                    present: vec!["mode_a_param".to_string()],
                    absent: vec![],
                }),
            ),
            make_entry(
                "test_tool",
                "mode_b",
                Some(crate::skills::ParamConditionSpec {
                    present: vec!["mode_b_param".to_string()],
                    absent: vec![],
                }),
            ),
        ];
        let executor = GenericToolExecutor {
            map: vec![(
                "test_tool".to_string(),
                (entries, None),
            )]
            .into_iter()
            .collect(),
            sandbox: None,
            side_read_seen: Arc::new(Mutex::new(HashMap::new())),
        };
        // No mode params at all — no partial match
        let args = serde_json::json!({ "path": "foo.txt" });
        let result = executor.resolve_entry("test_tool", &args);
        let err = result.unwrap_err();
        assert!(!err.contains("also required"), "unexpected hint in error: {}", err);
        assert!(err.contains("no execution spec matches the given parameters"), "unexpected error: {}", err);
    }
    #[test]
    fn resolve_entry_multiple_match_includes_present_param_names() {
        // Simulate git_rebase: two entries with present-based conditions
        let entries = vec![
            make_entry(
                "git_rebase",
                "rebase continue",
                Some(crate::skills::ParamConditionSpec {
                    present: vec!["continue".to_string()],
                    absent: vec![],
                }),
            ),
            make_entry(
                "git_rebase",
                "rebase abort",
                Some(crate::skills::ParamConditionSpec {
                    present: vec!["abort".to_string()],
                    absent: vec![],
                }),
            ),
        ];
        let executor = GenericToolExecutor {
            map: vec![(
                "git_rebase".to_string(),
                (entries, None),
            )]
            .into_iter()
            .collect(),
            sandbox: None,
            side_read_seen: Arc::new(Mutex::new(HashMap::new())),
        };
        // Both continue and abort provided — both conditions match
        let args = serde_json::json!({ "continue": true, "abort": true, "repo": "chai" });
        let result = executor.resolve_entry("git_rebase", &args);
        let err = result.unwrap_err();
        assert!(err.contains("multiple execution specs match the given parameters"), "unexpected error: {}", err);
        assert!(err.contains("present: [continue]"), "should list 'continue' as matching param: {}", err);
        assert!(err.contains("present: [abort]"), "should list 'abort' as matching param: {}", err);
    }

    #[test]
    fn resolve_entry_multiple_match_includes_absent_param_names() {
        // Simulate a tool with two absent-only conditions that both match
        let entries = vec![
            make_entry(
                "test_tool",
                "mode_a",
                Some(crate::skills::ParamConditionSpec {
                    present: vec![],
                    absent: vec!["flag_a".to_string()],
                }),
            ),
            make_entry(
                "test_tool",
                "mode_b",
                Some(crate::skills::ParamConditionSpec {
                    present: vec![],
                    absent: vec!["flag_b".to_string()],
                }),
            ),
        ];
        let executor = GenericToolExecutor {
            map: vec![(
                "test_tool".to_string(),
                (entries, None),
            )]
            .into_iter()
            .collect(),
            sandbox: None,
            side_read_seen: Arc::new(Mutex::new(HashMap::new())),
        };
        // Neither flag_a nor flag_b provided — both absent-conditions match
        let args = serde_json::json!({ "path": "foo.txt" });
        let result = executor.resolve_entry("test_tool", &args);
        let err = result.unwrap_err();
        assert!(err.contains("multiple execution specs match the given parameters"), "unexpected error: {}", err);
        assert!(err.contains("absent: [flag_a]"), "should list 'flag_a' as absent matching param: {}", err);
        assert!(err.contains("absent: [flag_b]"), "should list 'flag_b' as absent matching param: {}", err);
    }

    #[test]
    fn resolve_entry_multiple_match_with_present_and_absent_conditions() {
        // One condition uses present, another uses absent — both match
        let entries = vec![
            make_entry(
                "test_tool",
                "mode_a",
                Some(crate::skills::ParamConditionSpec {
                    present: vec!["flag_a".to_string()],
                    absent: vec![],
                }),
            ),
            make_entry(
                "test_tool",
                "mode_b",
                Some(crate::skills::ParamConditionSpec {
                    present: vec![],
                    absent: vec!["flag_b".to_string()],
                }),
            ),
        ];
        let executor = GenericToolExecutor {
            map: vec![(
                "test_tool".to_string(),
                (entries, None),
            )]
            .into_iter()
            .collect(),
            sandbox: None,
            side_read_seen: Arc::new(Mutex::new(HashMap::new())),
        };
        // flag_a provided, flag_b absent — both conditions match
        let args = serde_json::json!({ "flag_a": true, "path": "foo.txt" });
        let result = executor.resolve_entry("test_tool", &args);
        let err = result.unwrap_err();
        assert!(err.contains("multiple execution specs match the given parameters"), "unexpected error: {}", err);
        assert!(err.contains("present: [flag_a]"), "should list 'flag_a' as present matching param: {}", err);
        assert!(err.contains("absent: [flag_b]"), "should list 'flag_b' as absent matching param: {}", err);
    }
}
