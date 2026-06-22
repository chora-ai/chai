//! Tool descriptor (tools.json) for declarative skill tools.
//!
//! When a skill directory contains `tools.json`, the loader parses it and attaches
//! tool definitions, allowlist, and per-tool execution mapping to the skill. The gateway can
//! use this to build the LLM tool list and drive a generic executor.

use serde::Deserialize;
use std::collections::HashMap;

/// Root structure of a skill's tools.json file.
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
            post_process: None,
            side_read: None,
            success_exit_codes: None,
            max_output_lines: None,
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
}
