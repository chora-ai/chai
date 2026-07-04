//! Tool descriptor for declarative skill tools.
//!
//! A skill's tool surface is defined by up to three JSON files:
//! - `tools.json` — tool definitions for the LLM (name, description, parameter schemas)
//! - `allowlist.json` — binary→subcommand security grants
//! - `execution.json` — per-tool execution mapping
//!
//! The legacy single-file format (`tools.json` with a root object containing `tools`,
//! `allowlist`, and `execution` keys) is still supported during migration.
//!
//! The loader parses these files and constructs a `ToolDescriptor`, which the gateway
//! uses to build the LLM tool list and drive a generic executor.

use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;

/// In-memory representation of a skill's tool surface, assembled from
/// `tools.json`, `allowlist.json`, and `execution.json` (new three-file format)
/// or the legacy single-file `tools.json` (root object with `tools`/`allowlist`/`execution` keys).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDescriptor {
    /// Tool definitions for the LLM (name, description, parameters schema).
    #[serde(default)]
    pub tools: Vec<ToolSpec>,

    /// Allowlist: binary name -> allowed subcommands. Only these (binary, subcommand) pairs may be run.
    #[serde(default)]
    pub allowlist: HashMap<String, Vec<String>>,

    /// Per-tool execution: how to run each tool (binary, subcommand, arg mapping).
    #[serde(default)]
    pub execution: Vec<ExecutionSpec>,
}

/// One tool as exposed to the LLM (Ollama function-calling shape).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSpec {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// JSON schema for parameters (same shape Ollama expects: type, properties, required, etc.).
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// How to execute one tool: which binary/subcommand and how to map JSON params to argv.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionSpec {
    /// Tool name (must match a name in `tools`).
    pub tool: String,
    /// Binary to run (e.g. "chai").
    pub binary: String,
    /// Subcommand (e.g. "search"). Must be in allowlist for this binary.
    pub subcommand: String,
    /// Order of arguments: how each JSON param becomes a CLI arg.
    #[serde(default)]
    pub args: Vec<ArgMapping>,
    /// Optional: wrap the binary invocation through a command prefix (e.g.
    /// `["nix", "develop", "--command"]`). When present, the executor
    /// constructs `Command::new(wrapper[0]).args(wrapper[1..]).arg(binary).args(subcommand).args(args)`
    /// instead of `Command::new(binary).args(subcommand).args(args)`.
    /// The allowlist validates the declared `binary` and `subcommand`, not the
    /// wrapper — the wrapper is a transport mechanism, not a privilege escalation.
    #[serde(default, rename = "binaryWrapper")]
    pub binary_wrapper: Option<Vec<String>>,
    /// Optional: condition that must be satisfied for this execution spec to
    /// be selected by the loader. When present, the loader filters execution
    /// specs to only those whose condition matches the loading context.
    #[serde(default)]
    pub condition: Option<ConditionSpec>,
    /// Optional: parameter-based condition for selecting between multiple
    /// execution specs with the same tool name. When multiple specs share a
    /// tool name, the executor selects the spec whose `paramCondition` matches
    /// the tool call arguments. At most one spec should match for any given
    /// tool call. A spec without `paramCondition` is the default (matches
    /// when no other spec's condition is satisfied).
    #[serde(default, rename = "paramCondition")]
    pub param_condition: Option<ParamConditionSpec>,
    /// Optional: post-process the command's stdout through a script before
    /// returning the result to the model. The script receives stdout on stdin
    /// and its own stdout becomes the tool result. On failure, the original
    /// stdout is returned unmodified.
    #[serde(default)]
    pub post_process: Option<PostProcessSpec>,
    /// Optional: after the command (and any post-processing) completes, look
    /// for a file relative to a path parameter and append its contents to the
    /// tool result. Silently skipped when the file is absent.
    #[serde(default)]
    pub side_read: Option<SideReadSpec>,
    /// Optional: exit codes beyond 0 that should be treated as success.
    /// For example, `grep` exits with 1 when no matches are found: this is
    /// not an error. Setting `[0, 1]` causes exit code 1 to return `Ok(stdout)`
    /// instead of `Err(...)`. Exit codes not in this list (e.g. 2 for grep)
    /// still surface as tool errors.
    #[serde(default)]
    pub success_exit_codes: Option<Vec<i32>>,
    /// Optional: maximum number of output lines to return to the model.
    /// When set, output exceeding this limit is truncated and a notice is
    /// appended indicating how many lines were omitted. This prevents
    /// unbounded tool output (e.g. from `grep` or `git diff`) from exceeding
    /// the model's context window. The truncation notice includes the total
    /// line count and a hint to narrow the query. Applies after `postProcess`
    /// but before `sideRead` (side-read content is not counted against the
    /// limit and is always appended in full).
    #[serde(default)]
    pub max_output_lines: Option<usize>,
    /// Optional: per-tool truncation hint template. When set, this replaces the
    /// generic "Narrow your query path, pattern, or range to reduce results."
    /// notice with a tool-specific message. Template variables:
    /// `{kept}` = non-hint lines shown, `{total}` = total lines (including hints),
    /// `{omitted}` = non-hint lines omitted, `{next_start}` = first omitted line.
    /// When the output lines are prefixed with line numbers (e.g. `501\tcontent`),
    /// `{next_start}` is derived from the last kept line number + 1. Otherwise,
    /// `{next_start}` = `kept + 1` (output-line numbering). For line-range
    /// companion tools, use `{next_start}` to tell the agent the exact
    /// `start_line` to use for pagination.
    #[serde(default, rename = "truncationHint")]
    pub truncation_hint: Option<String>,
    /// Optional: inline hint conditions evaluated after postProcess and before
    /// truncation. Each matching condition appends a `hint:` line to the output
    /// with the standard blank-line separator. When absent, no hints are injected.
    #[serde(default, rename = "hintConditions")]
    pub hint_conditions: Option<Vec<HintCondition>>,
}

impl Default for ExecutionSpec {
    fn default() -> Self {
        Self {
            tool: String::new(),
            binary: String::new(),
            subcommand: String::new(),
            args: Vec::new(),
            binary_wrapper: None,
            condition: None,
            param_condition: None,
            post_process: None,
            side_read: None,
            success_exit_codes: None,
            max_output_lines: None,
            truncation_hint: None,
            hint_conditions: None,
        }
    }
}

/// Condition that must be satisfied for an execution spec to be selected.
///
/// The loader evaluates conditions during skill loading and filters execution
/// specs to only those whose conditions match. This keeps the executor unaware
/// of bin group logic — the loader handles selection, and the executor just
/// runs what it is given.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionSpec {
    /// Index of the bin group (in `metadata.requires.bins` OR-groups) that
    /// must have matched for this execution spec to be selected. For example,
    /// `0` means the first group matched (e.g. `["cargo"]`), `1` means the
    /// second group matched (e.g. `["nix"]`).
    pub bin_group: usize,
}

/// Parameter-based condition for selecting between multiple execution specs
/// with the same tool name. All specified constraints must be satisfied (AND logic).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamConditionSpec {
    /// Parameter names that must be present (non-null) in the tool call JSON.
    #[serde(default)]
    pub present: Vec<String>,
    /// Parameter names that must be absent (or null) from the tool call JSON.
    #[serde(default)]
    pub absent: Vec<String>,
}

impl ParamConditionSpec {
    /// Evaluate whether this condition matches the given tool call arguments.
    /// Returns true when all `present` parameters exist and are non-null, and
    /// all `absent` parameters are missing or null.
    pub fn matches(&self, args: &serde_json::Value) -> bool {
        let obj = match args.as_object() {
            Some(o) => o,
            None => return false,
        };
        for name in &self.present {
            match obj.get(name) {
                Some(v) if !v.is_null() => {}
                _ => return false,
            }
        }
        for name in &self.absent {
            match obj.get(name) {
                Some(v) if !v.is_null() => return false,
                _ => {}
            }
        }
        true
    }

    /// Check whether this condition partially matches the given arguments —
    /// i.e., at least one `present` parameter is satisfied but at least one
    /// is not. Returns a tuple of (satisfied, missing) parameter name lists
    /// when there is a partial match, or `None` when there is no partial match
    /// (either no `present` params were satisfied, or all were satisfied).
    pub fn partial_present_match(&self, args: &serde_json::Value) -> Option<(Vec<&str>, Vec<&str>)> {
        if self.present.is_empty() {
            return None;
        }
        let obj = match args.as_object() {
            Some(o) => o,
            None => return None,
        };
        let mut satisfied: Vec<&str> = Vec::new();
        let mut missing: Vec<&str> = Vec::new();
        for name in &self.present {
            match obj.get(name.as_str()) {
                Some(v) if !v.is_null() => satisfied.push(name),
                _ => missing.push(name),
            }
        }
        if !satisfied.is_empty() && !missing.is_empty() {
            Some((satisfied, missing))
        } else {
            None
        }
    }
}

/// Spec for appending a file's contents to the tool result when it exists.
///
/// After the main command (and optional `postProcess`) runs, the executor
/// looks for `<resolved-path-param>/<filename>`. If found, its contents are
/// appended to the tool result with a labeled separator. When `oncePerSession`
/// is true, the append is skipped for any (session, path) pair that was
/// already surfaced in the current session.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SideReadSpec {
    /// Name of the arg param whose resolved value is the directory to look in
    /// (e.g. `"path"`). Must be a param present in this tool's `args` list.
    pub path_param: String,
    /// Filename to look for within that directory (e.g. `"AGENTS.md"`).
    /// Must not contain path separators or `..`.
    pub filename: String,
    /// Label shown as a section header before the appended content.
    /// Defaults to the filename when absent.
    #[serde(default)]
    pub label: Option<String>,
    /// When `true`, append this file's content at most once per session per
    /// unique resolved path. Subsequent tool calls that would produce the same
    /// side-read within the same session are silently skipped.
    #[serde(default)]
    pub once_per_session: Option<bool>,
}

/// Spec for post-processing a tool's stdout: either a script in the skill's
/// scripts/ dir or an allowlisted command. Receives the raw stdout on stdin;
/// its own stdout becomes the tool result.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostProcessSpec {
    /// Name of script under the skill's scripts/ directory (e.g. "parse-rss").
    /// No allowlist entry needed.
    #[serde(default)]
    pub script: Option<String>,
    /// Binary name for allowlisted command post-processing.
    #[serde(default)]
    pub binary: Option<String>,
    /// Subcommand for allowlisted command. Required when binary is set.
    #[serde(default)]
    pub subcommand: Option<String>,
    /// Additional arguments passed to the script or command.
    #[serde(default)]
    pub args: Vec<String>,
    /// When true, empty output from the post-process script is treated as the
    /// final result (not a fallback to the original input). This is needed for
    /// filter-style post-processors where empty output means "nothing matched
    /// the filter" — e.g., `check-broken-links.sh` outputs nothing when all
    /// links resolve, and the tool should return empty (no broken links) rather
    /// than the raw grep output (all link targets). When false (default), empty
    /// output falls back to the original input, which is correct for
    /// sanitization-style post-processors that pass through unmodified content.
    #[serde(default)]
    pub empty_is_result: Option<bool>,
}

/// Spec for resolving a string param: either a script in the skill's scripts/ dir or an allowlisted command; stdout (trimmed) becomes the value.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveCommandSpec {
    /// Name of script under the skill's scripts/ directory (e.g. "resolve-daily-path"). No allowlist entry needed.
    #[serde(default)]
    pub script: Option<String>,
    /// Binary name for allowlisted command resolution. Required when script is not set.
    #[serde(default)]
    pub binary: Option<String>,
    /// Subcommand for allowlisted command. Required when script is not set.
    #[serde(default)]
    pub subcommand: Option<String>,
    /// Args passed to the script or command; use "$param" as placeholder for the current param value.
    #[serde(default)]
    pub args: Vec<String>,
}

/// How one JSON parameter is passed to the CLI.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgMapping {
    /// JSON parameter name (e.g. "query").
    /// Required for all kinds except `literal` (which pushes a fixed value
    /// and reads nothing from the tool call JSON).
    #[serde(default)]
    pub param: Option<String>,
    /// How to pass it: "positional", "flag", "flagifboolean", "stdin", "workingdir", "tempfile", or "literal".
    #[serde(default)]
    pub kind: ArgKind,
    /// For kind "literal", the fixed value to push onto argv.
    #[serde(default)]
    pub value: Option<String>,
    /// For kind "flag" or "tempfile", the flag name. Single-character names produce short flags
    /// (e.g. "n" -> `-n`); multi-character names produce long flags (e.g. "path" -> `--path`).
    /// If absent, uses param (which will always produce a long flag). Leading dashes are stripped
    /// before prefixing, so pre-dashed values like `"-p"` also work correctly.
    #[serde(default)]
    pub flag: Option<String>,
    /// For kind "flagIfBoolean", the flag to emit when the param value is true (e.g. "--overwrite").
    #[serde(default)]
    pub flag_if_true: Option<String>,
    /// For kind "flagIfBoolean", the flag to emit when the param value is false (e.g. "--append").
    #[serde(default)]
    pub flag_if_false: Option<String>,
    /// The value to use when a parameter is absent from the tool call JSON.
    /// For `flagIfBoolean`, this provides the boolean default (previously,
    /// absent boolean parameters were always treated as false). For `flag`,
    /// this provides a string or numeric default (e.g., `"warn"` for a level
    /// parameter, `"10"` for a count). The schema `"default"` field is only a
    /// hint to the LLM; `absentDefault` is enforced by the executor.
    #[serde(default, rename = "absentDefault")]
    pub absent_default: Option<serde_json::Value>,
    /// Optional: run this allowlisted command with param value substituted for "$param" in args; use trimmed stdout as the value.
    #[serde(default)]
    pub resolve_command: Option<ResolveCommandSpec>,
    /// When true, this parameter is a filesystem write target. The executor
    /// validates the resolved value against the write sandbox before execution.
    #[serde(default)]
    pub write_path: Option<bool>,
    /// When true, this parameter is a filesystem read target. The executor
    /// validates the resolved value against the write sandbox before execution,
    /// preventing reads from paths outside the authorized sandbox boundary.
    #[serde(default)]
    pub read_path: Option<bool>,
    /// When true, a missing or null JSON parameter is omitted from argv (unless
    /// `resolveCommand` is set, in which case the resolver runs with an empty
    /// string). Default: required (same as false).
    #[serde(default)]
    pub optional: Option<bool>,
    /// For `positional` only: when true, split the value on whitespace and
    /// push each element as a separate argv entry. Use for tools that accept
    /// multiple positional arguments (e.g. `git add file1.rs file2.rs`).
    #[serde(default)]
    pub split: Option<bool>,
    /// For `positional` only: when true, insert `--` before this value if any
    /// prior optional positional in this spec was skipped (disambiguates paths
    /// from refs for commands like `git diff`).
    #[serde(default, rename = "disambiguateAfterSkippedPositionals")]
    pub disambiguate_after_skipped_positionals: Option<bool>,
    /// Optional: a regex pattern that the resolved parameter value must NOT match.
    /// When set, the executor checks the resolved value against this pattern
    /// before executing the command. If the value matches, the operation is
    /// rejected with an error. This is a tool-level enforcement mechanism for
    /// constraints that the schema cannot express (e.g., branch protection).
    #[serde(default, rename = "denyPattern")]
    pub deny_pattern: Option<String>,
    /// Optional: a resolve command that provides the effective value to check
    /// against `denyPattern`. When `denyAlwaysResolve` is false (default),
    /// the raw parameter value is checked directly when present, and this
    /// command is only invoked when the parameter is absent or empty.
    /// When `denyAlwaysResolve` is true, this command always provides the
    /// value to check — the raw parameter value may be unrelated to what
    /// the deny pattern matches (e.g., the param is a working directory
    /// path, but the deny pattern checks the current branch name).
    #[serde(default, rename = "denyResolveCommand")]
    pub deny_resolve_command: Option<ResolveCommandSpec>,
    /// When true, `denyResolveCommand` always provides the value to check
    /// against `denyPattern`, even when the raw parameter value is present.
    /// This is needed when the parameter value is not the thing being
    /// denied (e.g., a path parameter whose value is a directory, but the
    /// deny pattern checks the git branch within that directory).
    #[serde(default, rename = "denyAlwaysResolve")]
    pub deny_always_resolve: Option<bool>,
    /// When true, this parameter is a filesystem path that intentionally needs
    /// unrestricted access — it may receive values that resolve outside the
    /// sandbox. The executor skips all sandbox validation and the runtime
    /// path-like value check. This is a red flag: every use must be justified.
    /// No current bundled skill parameter needs this.
    #[serde(default)]
    pub unsafe_path: Option<bool>,
    /// Optional: overrides the execution spec's `subcommand` when this
    /// `flagIfBoolean` parameter evaluates to true. The override subcommand
    /// must be in the allowlist. Use for tools where a boolean flag changes
    /// the git subcommand (e.g., `force: true` switches from `branch -d` to
    /// `branch -D`). The default subcommand is used when the boolean is
    /// false or absent.
    #[serde(default, rename = "subcommandOverride")]
    pub subcommand_override: Option<String>,
}

impl Default for ArgMapping {
    fn default() -> Self {
        Self {
            param: None,
            kind: ArgKind::default(),
            value: None,
            flag: None,
            flag_if_true: None,
            flag_if_false: None,
            absent_default: None,
            resolve_command: None,
            write_path: None,
            read_path: None,
            optional: None,
            split: None,
            disambiguate_after_skipped_positionals: None,
            deny_pattern: None,
            deny_resolve_command: None,
            deny_always_resolve: None,
            unsafe_path: None,
            subcommand_override: None,
        }
    }
}

impl ArgMapping {
    /// Return the parameter name as a string slice.
    /// For `literal` kind args (which have no param), returns a placeholder.
    /// All other kinds require a param name; this panics if it is missing.
    pub fn param_name(&self) -> &str {
        self.param.as_deref().unwrap_or_else(|| {
            if self.kind == ArgKind::Literal {
                "(literal)"
            } else {
                panic!("arg mapping for kind {:?} is missing required 'param' field", self.kind)
            }
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArgKind {
    /// Pass the value as a single positional argument.
    #[default]
    Positional,
    /// Pass as a flag. Single-character `flag` values produce `-x`; multi-character
    /// values produce `--xx`. Uses `flag` if set, else `param` (always long form).
    Flag,
    /// Param is boolean: emit `flag_if_true` when true, `flag_if_false` when false (e.g. replace -> --overwrite | --append).
    FlagIfBoolean,
    /// Pipe the value to the process's stdin instead of adding it to argv.
    Stdin,
    /// Set the process working directory to the resolved value. The value is
    /// validated against the sandbox (like `readPath`) and used as `current_dir`
    /// for the child process, but is NOT added to argv. When `resolveCommand`
    /// is set, the resolver runs with an empty string when the param is omitted,
    /// defaulting to the sandbox root.
    WorkingDir,
    /// Write the value to a temporary file and pass the file path as a flag.
    /// Uses `flag` for the CLI flag name (like `Flag` kind). The executor
    /// manages temp file creation and cleanup. Use for content-rich parameters
    /// that cannot use stdin (because stdin is already in use) or that must
    /// match file content byte-for-byte (e.g. verification tokens like
    /// original_content). No size limits, no encoding issues.
    TempFile,
    /// A fixed value pushed directly onto argv. No parameter is read from the
    /// tool call JSON. Used for command flags like `--continue` and `--abort`
    /// that are always present when the tool is called.
    Literal,
}

/// A declarative hint condition that the executor evaluates after postProcess
/// and before truncation. When all specified conditions are met, the hint
/// message is appended to the output with the standard `hint:` prefix and
/// blank-line separator.
///
/// At least one of `match_`, `exit_code`, `not_empty`, or `when_arg` must be
/// present. If multiple are present, all must be true (AND logic). Multiple
/// `hintConditions` entries are all evaluated; all matching entries produce
/// hints.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HintCondition {
    /// Substring to search for in the post-processed output. Case-sensitive.
    /// When present, the hint fires if the string appears anywhere in the output.
    #[serde(default, rename = "match")]
    pub match_: Option<String>,

    /// Exit code condition: `"nonzero"` matches any non-zero code, or an integer
    /// matches that specific code.
    #[serde(default, rename = "exitCode")]
    pub exit_code: Option<HintExitCode>,

    /// When `true`, the hint fires only if the post-processed output is non-empty.
    /// When `false` or absent, output emptiness is not checked.
    #[serde(default)]
    pub not_empty: Option<bool>,

    /// Parameter-value conditions. Keys are parameter names; values are expected
    /// values (string, boolean, or number). All specified parameters must match
    /// their expected values for the condition to fire. Evaluated against the
    /// effective args (after `absentDefault` augmentation).
    #[serde(default)]
    pub when_arg: Option<HashMap<String, serde_json::Value>>,

    /// The hint message. Supports `{param_name}` template variables that are
    /// replaced with the corresponding parameter value from the effective args.
    /// The executor prepends `hint: ` and a blank-line separator.
    pub hint: String,
}

/// Exit code condition for hint evaluation.
///
/// Deserialized from either the string `"nonzero"` (matching any non-zero
/// exit code) or an integer (matching that specific exit code).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum HintExitCode {
    /// Match any non-zero exit code. Only valid value: `"nonzero"`.
    Nonzero(String),
    /// Match a specific exit code.
    Specific(i32),
}

impl HintExitCode {
    /// Returns true if the given exit code satisfies this condition.
    pub fn matches(&self, code: i32) -> bool {
        match self {
            HintExitCode::Nonzero(s) if s == "nonzero" => code != 0,
            HintExitCode::Nonzero(invalid) => {
                // Reject invalid string values at match time (should be
                // caught by validation, but fail safe).
                log::warn!("hintConditions: invalid exitCode string (expected \"nonzero\"): {:?}", invalid);
                false
            }
            HintExitCode::Specific(expected) => code == *expected,
        }
    }
}

impl HintCondition {
    /// Evaluate all conditions against the given exit code, output, and tool
    /// args. Returns true when all specified conditions are met (AND logic).
    /// A condition with no condition fields is treated as not matching
    /// (at least one condition must be specified).
    pub fn matches(&self, exit_code: i32, output: &str, tool_args: &serde_json::Value) -> bool {
        let has_condition = self.match_.is_some()
            || self.exit_code.is_some()
            || self.not_empty.is_some()
            || self.when_arg.is_some();

        if !has_condition {
            return false;
        }

        // match: substring must appear in the output
        if let Some(ref pattern) = self.match_ {
            if !output.contains(pattern.as_str()) {
                return false;
            }
        }

        // exitCode: must match the command's exit code
        if let Some(ref ec) = self.exit_code {
            if !ec.matches(exit_code) {
                return false;
            }
        }

        // notEmpty: when true, output must be non-empty
        if self.not_empty == Some(true) && output.is_empty() {
            return false;
        }

        // whenArg: all specified parameter values must match
        if let Some(ref expected) = self.when_arg {
            let obj = match tool_args.as_object() {
                Some(o) => o,
                None => return false,
            };
            for (key, expected_val) in expected {
                let actual = match obj.get(key) {
                    Some(v) if !v.is_null() => v,
                    _ => return false, // absent or null parameter fails
                };
                if !values_match(expected_val, actual) {
                    return false;
                }
            }
        }

        true
    }

    /// Expand `{param_name}` template variables in the hint message using
    /// values from the tool call args. Unknown or absent parameters are
    /// replaced with an empty string.
    pub fn expand_hint(&self, tool_args: &serde_json::Value) -> String {
        let obj = tool_args.as_object();
        expand_template(&self.hint, obj)
    }
}

/// Expand `{param_name}` template variables in a hint string.
///
/// Variables use the format `{name}`. If the parameter is absent or null in
/// the args object, the placeholder is replaced with an empty string.
/// Literal braces can be produced by double-brace escaping: `{{` → `{`,
/// `}}` → `}`.
fn expand_template(template: &str, args: Option<&serde_json::Map<String, serde_json::Value>>) -> String {
    let mut result = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            // Check for escaped brace `{{`
            if chars.peek() == Some(&'{') {
                chars.next();
                result.push('{');
                continue;
            }
            // Read until closing `}`
            let mut name = String::new();
            let mut found_close = false;
            while let Some(c) = chars.next() {
                if c == '}' {
                    // Check for escaped brace `}}`
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        name.push('}');
                        continue;
                    }
                    found_close = true;
                    break;
                }
                name.push(c);
            }
            if found_close && !name.is_empty() {
                let value = args
                    .and_then(|o| o.get(&name))
                    .and_then(|v| if v.is_null() { None } else { Some(v) });
                match value {
                    Some(serde_json::Value::String(s)) => result.push_str(s),
                    Some(v) => result.push_str(&v.to_string()),
                    None => { /* absent param → empty string */ }
                }
            } else if found_close {
                // Empty name `{}` — leave as-is
                result.push_str("{}");
            } else {
                // Unclosed brace — push what we consumed
                result.push('{');
                result.push_str(&name);
            }
        } else if ch == '}' {
            // Check for escaped brace `}}`
            if chars.peek() == Some(&'}') {
                chars.next();
                result.push('}');
                continue;
            }
            result.push('}');
        } else {
            result.push(ch);
        }
    }

    result
}

/// Compare an expected JSON value from `whenArg` against the actual parameter
/// value from the tool call. Supports string, boolean, and number comparisons.
fn values_match(expected: &serde_json::Value, actual: &serde_json::Value) -> bool {
    match (expected, actual) {
        // Both strings: exact match
        (serde_json::Value::String(a), serde_json::Value::String(b)) => a == b,
        // Both booleans: exact match
        (serde_json::Value::Bool(a), serde_json::Value::Bool(b)) => a == b,
        // Both numbers: compare as i64 if possible, then f64
        (serde_json::Value::Number(a), serde_json::Value::Number(b)) => {
            if let (Some(ai), Some(bi)) = (a.as_i64(), b.as_i64()) {
                ai == bi
            } else if let (Some(af), Some(bf)) = (a.as_f64(), b.as_f64()) {
                (af - bf).abs() < f64::EPSILON
            } else {
                false
            }
        }
        // Cross-type: string expected, actual is non-string — convert actual to string
        (serde_json::Value::String(exp), actual_other) => {
            match actual_other {
                serde_json::Value::Bool(b) => exp == &b.to_string(),
                serde_json::Value::Number(n) => exp == &n.to_string(),
                _ => false,
            }
        }
        // Cross-type: number expected, actual is string — try parsing
        (serde_json::Value::Number(exp), serde_json::Value::String(act)) => {
            if let (Some(ei), Ok(ai)) = (exp.as_i64(), act.parse::<i64>()) {
                ei == ai
            } else if let (Some(ef), Ok(af)) = (exp.as_f64(), act.parse::<f64>()) {
                (ef - af).abs() < f64::EPSILON
            } else {
                false
            }
        }
        // Cross-type: boolean expected, actual is string — parse "true"/"false"
        (serde_json::Value::Bool(exp), serde_json::Value::String(act)) => {
            act.eq_ignore_ascii_case(&exp.to_string())
        }
        _ => false,
    }
}

impl fmt::Display for HintExitCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HintExitCode::Nonzero(s) => write!(f, "{}", s),
            HintExitCode::Specific(n) => write!(f, "{}", n),
        }
    }
}

impl ToolDescriptor {
    /// Convert descriptor tools to Ollama tool definitions for the chat API.
    pub fn to_tool_definitions(&self) -> Vec<crate::providers::ToolDefinition> {
        self.tools
            .iter()
            .map(|t| crate::providers::ToolDefinition {
                typ: "function".to_string(),
                function: crate::providers::ToolFunctionDefinition {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.parameters.clone(),
                },
            })
            .collect()
    }

    /// Build an exec::Allowlist from the descriptor's allowlist map.
    pub fn to_allowlist(&self) -> crate::exec::Allowlist {
        let mut a = crate::exec::Allowlist::new();
        for (binary, subcommands) in &self.allowlist {
            a.allow_subcommands(binary.clone(), subcommands.clone());
        }
        a
    }

    /// Construct a `ToolDescriptor` from the three separate sources used by the
    /// new three-file format (`tools.json`, `allowlist.json`, `execution.json`).
    pub fn from_parts(
        tools: Vec<ToolSpec>,
        allowlist: HashMap<String, Vec<String>>,
        execution: Vec<ExecutionSpec>,
    ) -> Self {
        Self {
            tools,
            allowlist,
            execution,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- HintExitCode tests ---

    #[test]
    fn hint_exit_code_nonzero_matches_non_zero() {
        let ec: HintExitCode = serde_json::from_value(serde_json::json!("nonzero")).unwrap();
        assert!(ec.matches(1));
        assert!(ec.matches(128));
        assert!(!ec.matches(0));
    }

    #[test]
    fn hint_exit_code_specific_matches_exact_code() {
        let ec: HintExitCode = serde_json::from_value(serde_json::json!(1)).unwrap();
        assert!(ec.matches(1));
        assert!(!ec.matches(0));
        assert!(!ec.matches(2));
    }

    #[test]
    fn hint_exit_code_specific_zero() {
        let ec: HintExitCode = serde_json::from_value(serde_json::json!(0)).unwrap();
        assert!(ec.matches(0));
        assert!(!ec.matches(1));
    }

    #[test]
    fn hint_exit_code_invalid_string_returns_false() {
        // "error" is not a valid exitCode string — should be caught at
        // match time and return false.
        let ec = HintExitCode::Nonzero("error".to_string());
        assert!(!ec.matches(1));
        assert!(!ec.matches(0));
    }

    // --- HintCondition::matches tests ---

    fn make_condition(
        match_: Option<&str>,
        exit_code: Option<HintExitCode>,
        not_empty: Option<bool>,
        when_arg: Option<HashMap<String, serde_json::Value>>,
        hint: &str,
    ) -> HintCondition {
        HintCondition {
            match_: match_.map(|s| s.to_string()),
            exit_code,
            not_empty,
            when_arg,
            hint: hint.to_string(),
        }
    }

    #[test]
    fn hint_condition_match_substring_found() {
        let c = make_condition(Some("not found"), None, None, None, "hint msg");
        let args = serde_json::json!({});
        assert!(c.matches(0, "error: file not found", &args));
    }

    #[test]
    fn hint_condition_match_substring_not_found() {
        let c = make_condition(Some("not found"), None, None, None, "hint msg");
        let args = serde_json::json!({});
        assert!(!c.matches(0, "success", &args));
    }

    #[test]
    fn hint_condition_exit_code_nonzero() {
        let c = make_condition(None, Some(HintExitCode::Nonzero("nonzero".to_string())), None, None, "hint msg");
        let args = serde_json::json!({});
        assert!(c.matches(1, "output", &args));
        assert!(!c.matches(0, "output", &args));
    }

    #[test]
    fn hint_condition_exit_code_specific() {
        let c = make_condition(None, Some(HintExitCode::Specific(1)), None, None, "hint msg");
        let args = serde_json::json!({});
        assert!(c.matches(1, "output", &args));
        assert!(!c.matches(0, "output", &args));
        assert!(!c.matches(2, "output", &args));
    }

    #[test]
    fn hint_condition_not_empty_true() {
        let c = make_condition(None, None, Some(true), None, "hint msg");
        let args = serde_json::json!({});
        assert!(c.matches(0, "some output", &args));
        assert!(!c.matches(0, "", &args));
    }

    #[test]
    fn hint_condition_not_empty_false_does_not_check() {
        let c = make_condition(None, None, Some(false), None, "hint msg");
        let args = serde_json::json!({});
        // not_empty: false means emptiness is not checked — condition is
        // trivially true (no other conditions present). But a condition
        // with only not_empty: false has no real condition — wait, the
        // has_condition check will pass because not_empty is Some(false).
        // However, not_empty == Some(true) is the only check that can fail.
        // not_empty == Some(false) does not reject empty output.
        assert!(c.matches(0, "some output", &args));
        assert!(c.matches(0, "", &args));
    }

    #[test]
    fn hint_condition_when_arg_string_match() {
        let mut when = HashMap::new();
        when.insert("ref".to_string(), serde_json::json!("main"));
        let c = make_condition(None, None, None, Some(when), "hint msg");
        let args = serde_json::json!({ "ref": "main" });
        assert!(c.matches(0, "output", &args));
    }

    #[test]
    fn hint_condition_when_arg_string_mismatch() {
        let mut when = HashMap::new();
        when.insert("ref".to_string(), serde_json::json!("main"));
        let c = make_condition(None, None, None, Some(when), "hint msg");
        let args = serde_json::json!({ "ref": "dev" });
        assert!(!c.matches(0, "output", &args));
    }

    #[test]
    fn hint_condition_when_arg_absent_param() {
        let mut when = HashMap::new();
        when.insert("ref".to_string(), serde_json::json!("main"));
        let c = make_condition(None, None, None, Some(when), "hint msg");
        let args = serde_json::json!({});
        assert!(!c.matches(0, "output", &args));
    }

    #[test]
    fn hint_condition_when_arg_null_param() {
        let mut when = HashMap::new();
        when.insert("ref".to_string(), serde_json::json!("main"));
        let c = make_condition(None, None, None, Some(when), "hint msg");
        let args = serde_json::json!({ "ref": null });
        assert!(!c.matches(0, "output", &args));
    }

    #[test]
    fn hint_condition_when_arg_boolean_match() {
        let mut when = HashMap::new();
        when.insert("no_commit".to_string(), serde_json::json!(true));
        let c = make_condition(None, None, None, Some(when), "hint msg");
        let args = serde_json::json!({ "no_commit": true });
        assert!(c.matches(0, "output", &args));
    }

    #[test]
    fn hint_condition_when_arg_number_match() {
        let mut when = HashMap::new();
        when.insert("count".to_string(), serde_json::json!(3));
        let c = make_condition(None, None, None, Some(when), "hint msg");
        let args = serde_json::json!({ "count": 3 });
        assert!(c.matches(0, "output", &args));
    }

    #[test]
    fn hint_condition_when_arg_multiple_all_must_match() {
        let mut when = HashMap::new();
        when.insert("ref".to_string(), serde_json::json!("main"));
        when.insert("squash".to_string(), serde_json::json!(true));
        let c = make_condition(None, None, None, Some(when), "hint msg");
        let args_both = serde_json::json!({ "ref": "main", "squash": true });
        let args_one = serde_json::json!({ "ref": "main", "squash": false });
        assert!(c.matches(0, "output", &args_both));
        assert!(!c.matches(0, "output", &args_one));
    }

    #[test]
    fn hint_condition_and_logic_match_and_exit_code() {
        let c = make_condition(
            Some("permission denied"),
            Some(HintExitCode::Nonzero("nonzero".to_string())),
            None,
            None,
            "hint msg",
        );
        let args = serde_json::json!({});
        // Both conditions met
        assert!(c.matches(1, "error: permission denied", &args));
        // Exit code is 0 (not nonzero)
        assert!(!c.matches(0, "error: permission denied", &args));
        // String not found
        assert!(!c.matches(1, "success", &args));
    }

    #[test]
    fn hint_condition_and_logic_match_and_not_empty() {
        let c = make_condition(Some("results"), None, Some(true), None, "hint msg");
        let args = serde_json::json!({});
        assert!(c.matches(0, "3 results found", &args));
        assert!(!c.matches(0, "", &args)); // empty output, notEmpty fails
    }

    #[test]
    fn hint_condition_no_condition_fields_returns_false() {
        let c = HintCondition {
            match_: None,
            exit_code: None,
            not_empty: None,
            when_arg: None,
            hint: "always hint".to_string(),
        };
        let args = serde_json::json!({});
        assert!(!c.matches(0, "output", &args));
    }

    // --- HintCondition::expand_hint tests ---

    #[test]
    fn expand_hint_no_template() {
        let c = make_condition(None, None, None, None, "simple hint message");
        let args = serde_json::json!({});
        assert_eq!(c.expand_hint(&args), "simple hint message");
    }

    #[test]
    fn expand_hint_with_template_variable() {
        let c = make_condition(None, None, None, None, "reset to {ref} — use git_status");
        let args = serde_json::json!({ "ref": "HEAD~1" });
        assert_eq!(c.expand_hint(&args), "reset to HEAD~1 — use git_status");
    }

    #[test]
    fn expand_hint_multiple_template_variables() {
        let c = make_condition(None, None, None, None, "{action} {target}");
        let args = serde_json::json!({ "action": "read", "target": "file.txt" });
        assert_eq!(c.expand_hint(&args), "read file.txt");
    }

    #[test]
    fn expand_hint_absent_param_replaced_with_empty() {
        let c = make_condition(None, None, None, None, "reset to {ref}");
        let args = serde_json::json!({});
        assert_eq!(c.expand_hint(&args), "reset to ");
    }

    #[test]
    fn expand_hint_null_param_replaced_with_empty() {
        let c = make_condition(None, None, None, None, "reset to {ref}");
        let args = serde_json::json!({ "ref": null });
        assert_eq!(c.expand_hint(&args), "reset to ");
    }

    #[test]
    fn expand_hint_non_string_param() {
        let c = make_condition(None, None, None, None, "count is {n}");
        let args = serde_json::json!({ "n": 42 });
        assert_eq!(c.expand_hint(&args), "count is 42");
    }

    #[test]
    fn expand_hint_boolean_param() {
        let c = make_condition(None, None, None, None, "flag is {flag}");
        let args = serde_json::json!({ "flag": true });
        assert_eq!(c.expand_hint(&args), "flag is true");
    }

    // --- expand_template tests ---

    #[test]
    fn expand_template_escaped_braces() {
        let args = serde_json::json!({ "name": "test" });
        let obj = args.as_object();
        assert_eq!(expand_template("{{name}} = {name}", obj), "{name} = test");
    }

    #[test]
    fn expand_template_no_args() {
        assert_eq!(expand_template("no vars", None), "no vars");
    }

    #[test]
    fn expand_template_unclosed_brace() {
        let args = serde_json::json!({});
        let obj = args.as_object();
        assert_eq!(expand_template("{unclosed", obj), "{unclosed");
    }

    #[test]
    fn expand_template_empty_name() {
        let args = serde_json::json!({});
        let obj = args.as_object();
        assert_eq!(expand_template("value: {}", obj), "value: {}");
    }

    // --- values_match tests ---

    #[test]
    fn values_match_string_to_string() {
        assert!(values_match(&serde_json::json!("main"), &serde_json::json!("main")));
        assert!(!values_match(&serde_json::json!("main"), &serde_json::json!("dev")));
    }

    #[test]
    fn values_match_bool_to_bool() {
        assert!(values_match(&serde_json::json!(true), &serde_json::json!(true)));
        assert!(!values_match(&serde_json::json!(true), &serde_json::json!(false)));
    }

    #[test]
    fn values_match_number_to_number() {
        assert!(values_match(&serde_json::json!(3), &serde_json::json!(3)));
        assert!(!values_match(&serde_json::json!(3), &serde_json::json!(4)));
    }

    #[test]
    fn values_match_cross_type_string_to_bool() {
        assert!(values_match(&serde_json::json!("true"), &serde_json::json!(true)));
        assert!(!values_match(&serde_json::json!("false"), &serde_json::json!(true)));
    }

    #[test]
    fn values_match_cross_type_number_to_string() {
        assert!(values_match(&serde_json::json!(3), &serde_json::json!("3")));
        assert!(!values_match(&serde_json::json!(3), &serde_json::json!("4")));
    }

    #[test]
    fn values_match_cross_type_bool_to_string() {
        assert!(values_match(&serde_json::json!(true), &serde_json::json!("true")));
        assert!(!values_match(&serde_json::json!(true), &serde_json::json!("false")));
    }

    // --- ParamConditionSpec::partial_present_match tests ---

    #[test]
    fn partial_present_match_returns_satisfied_and_missing_when_some_present() {
        let cond = ParamConditionSpec {
            present: vec!["start_line".to_string(), "original_content".to_string()],
            absent: vec![],
        };
        // start_line is present, original_content is missing
        let args = serde_json::json!({ "start_line": 5, "path": "foo.txt" });
        let (satisfied, missing) = cond.partial_present_match(&args).unwrap();
        assert_eq!(satisfied, vec!["start_line"]);
        assert_eq!(missing, vec!["original_content"]);
    }

    #[test]
    fn partial_present_match_returns_satisfied_and_missing_when_other_present() {
        let cond = ParamConditionSpec {
            present: vec!["start_line".to_string(), "original_content".to_string()],
            absent: vec![],
        };
        // original_content is present, start_line is missing
        let args = serde_json::json!({ "original_content": "old", "path": "foo.txt" });
        let (satisfied, missing) = cond.partial_present_match(&args).unwrap();
        assert_eq!(satisfied, vec!["original_content"]);
        assert_eq!(missing, vec!["start_line"]);
    }

    #[test]
    fn partial_present_match_returns_none_when_all_present() {
        let cond = ParamConditionSpec {
            present: vec!["start_line".to_string(), "original_content".to_string()],
            absent: vec![],
        };
        let args = serde_json::json!({ "start_line": 5, "original_content": "old" });
        assert!(cond.partial_present_match(&args).is_none());
    }

    #[test]
    fn partial_present_match_returns_none_when_none_present() {
        let cond = ParamConditionSpec {
            present: vec!["start_line".to_string(), "original_content".to_string()],
            absent: vec![],
        };
        let args = serde_json::json!({ "path": "foo.txt", "content": "hello" });
        assert!(cond.partial_present_match(&args).is_none());
    }

    #[test]
    fn partial_present_match_returns_none_when_empty_present() {
        let cond = ParamConditionSpec {
            present: vec![],
            absent: vec!["overwrite".to_string()],
        };
        let args = serde_json::json!({ "path": "foo.txt" });
        assert!(cond.partial_present_match(&args).is_none());
    }

    #[test]
    fn partial_present_match_returns_none_for_non_object_args() {
        let cond = ParamConditionSpec {
            present: vec!["start_line".to_string()],
            absent: vec![],
        };
        let args = serde_json::json!("not an object");
        assert!(cond.partial_present_match(&args).is_none());
    }

    #[test]
    fn partial_present_match_treats_null_as_absent() {
        let cond = ParamConditionSpec {
            present: vec!["start_line".to_string(), "original_content".to_string()],
            absent: vec![],
        };
        // start_line is present, original_content is null
        let args = serde_json::json!({ "start_line": 5, "original_content": null });
        let (satisfied, missing) = cond.partial_present_match(&args).unwrap();
        assert_eq!(satisfied, vec!["start_line"]);
        assert_eq!(missing, vec!["original_content"]);
    }
    // --- ToolDescriptor::from_parts tests ---

    #[test]
    fn tool_descriptor_from_parts_constructs_descriptor() {
        let tools = vec![ToolSpec {
            name: "test_tool".to_string(),
            description: Some("A test tool".to_string()),
            parameters: serde_json::json!({"type": "object"}),
        }];
        let mut allowlist = HashMap::new();
        allowlist.insert("echo".to_string(), vec!["".to_string()]);
        let execution = vec![ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "echo".to_string(),
            subcommand: "".to_string(),
            ..Default::default()
        }];
        let desc = ToolDescriptor::from_parts(tools, allowlist, execution);
        assert_eq!(desc.tools.len(), 1);
        assert_eq!(desc.tools[0].name, "test_tool");
        assert!(desc.allowlist.contains_key("echo"));
        assert_eq!(desc.execution.len(), 1);
        assert_eq!(desc.execution[0].tool, "test_tool");
    }

    // --- Deserialization tests ---

    #[test]
    fn deserialize_hint_condition_with_match() {
        let json = serde_json::json!({
            "match": "not a git repository",
            "hint": "not a git repository — specify a valid repo path"
        });
        let c: HintCondition = serde_json::from_value(json).unwrap();
        assert_eq!(c.match_.as_deref(), Some("not a git repository"));
        assert!(c.exit_code.is_none());
        assert!(c.not_empty.is_none());
        assert!(c.when_arg.is_none());
        assert_eq!(c.hint, "not a git repository — specify a valid repo path");
    }

    #[test]
    fn deserialize_hint_condition_with_exit_code_nonzero() {
        let json = serde_json::json!({
            "exitCode": "nonzero",
            "hint": "file not found"
        });
        let c: HintCondition = serde_json::from_value(json).unwrap();
        assert!(c.match_.is_none());
        assert!(matches!(c.exit_code, Some(HintExitCode::Nonzero(ref s)) if s == "nonzero"));
        assert_eq!(c.hint, "file not found");
    }

    #[test]
    fn deserialize_hint_condition_with_exit_code_integer() {
        let json = serde_json::json!({
            "exitCode": 1,
            "hint": "no matches"
        });
        let c: HintCondition = serde_json::from_value(json).unwrap();
        assert!(matches!(c.exit_code, Some(HintExitCode::Specific(1))));
    }

    #[test]
    fn deserialize_hint_condition_with_not_empty() {
        let json = serde_json::json!({
            "notEmpty": true,
            "hint": "results found"
        });
        let c: HintCondition = serde_json::from_value(json).unwrap();
        assert_eq!(c.not_empty, Some(true));
    }

    #[test]
    fn deserialize_hint_condition_with_when_arg() {
        let json = serde_json::json!({
            "whenArg": { "no_commit": true },
            "hint": "staged changes"
        });
        let c: HintCondition = serde_json::from_value(json).unwrap();
        let when = c.when_arg.unwrap();
        assert_eq!(when.get("no_commit"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn deserialize_hint_condition_with_compound_conditions() {
        let json = serde_json::json!({
            "match": "permission denied",
            "exitCode": "nonzero",
            "hint": "permission error"
        });
        let c: HintCondition = serde_json::from_value(json).unwrap();
        assert_eq!(c.match_.as_deref(), Some("permission denied"));
        assert!(c.exit_code.is_some());
    }

    #[test]
    fn deserialize_execution_spec_with_hint_conditions() {
        let json = serde_json::json!({
            "tool": "files_read",
            "binary": "cat",
            "subcommand": "",
            "hintConditions": [
                {
                    "exitCode": "nonzero",
                    "hint": "file not found"
                }
            ]
        });
        let spec: ExecutionSpec = serde_json::from_value(json).unwrap();
        assert_eq!(spec.tool, "files_read");
        let conditions = spec.hint_conditions.unwrap();
        assert_eq!(conditions.len(), 1);
        assert_eq!(conditions[0].hint, "file not found");
    }
}
