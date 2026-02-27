//! Tool descriptor (tools.json) for declarative skill tools.
//!
//! When a skill directory contains `tools.json`, the loader parses it and attaches
//! tool definitions, allowlist, and per-tool execution mapping. The gateway can
//! use this to build the LLM tool list and drive a generic executor (future).

use serde::Deserialize;
use std::collections::HashMap;

/// Root structure of a skill's tools.json file.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionSpec {
    /// Tool name (must match a name in `tools`).
    pub tool: String,
    /// Binary to run (e.g. "notesmd-cli").
    pub binary: String,
    /// Subcommand (e.g. "search"). Must be in allowlist for this binary.
    pub subcommand: String,
    /// Order of arguments: how each JSON param becomes a CLI arg.
    #[serde(default)]
    pub args: Vec<ArgMapping>,
}

/// Spec for resolving a string param: either a script in the skill's scripts/ dir (when allowScripts is true) or an allowlisted command; stdout (trimmed) becomes the value.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveCommandSpec {
    /// Name of script under the skill's scripts/ directory (e.g. "resolve-daily-path"). Used when skills.allowScripts is true; no allowlist entry needed.
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArgMapping {
    /// JSON parameter name (e.g. "query").
    pub param: String,
    /// How to pass it: "positional", "flag", or "flagIfBoolean".
    #[serde(default)]
    pub kind: ArgKind,
    /// For kind "flag", the flag name (e.g. "content" -> --content). If absent, uses param.
    #[serde(default)]
    pub flag: Option<String>,
    /// For kind "flagIfBoolean", the flag to emit when the param value is true (e.g. "--overwrite").
    #[serde(default)]
    pub flag_if_true: Option<String>,
    /// For kind "flagIfBoolean", the flag to emit when the param value is false (e.g. "--append").
    #[serde(default)]
    pub flag_if_false: Option<String>,
    /// When true, string values have literal `\n` and `\t` converted to newlines and tabs.
    #[serde(default)]
    pub normalize_newlines: Option<bool>,
    /// Optional: run this allowlisted command with param value substituted for "$param" in args; use trimmed stdout as the value.
    #[serde(default)]
    pub resolve_command: Option<ResolveCommandSpec>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArgKind {
    /// Pass the value as a single positional argument.
    #[default]
    Positional,
    /// Pass as --flag value. Uses `flag` if set, else `param`.
    Flag,
    /// Param is boolean: emit `flag_if_true` when true, `flag_if_false` when false (e.g. replace -> --overwrite | --append).
    FlagIfBoolean,
}

impl ToolDescriptor {
    /// Convert descriptor tools to Ollama tool definitions for the chat API.
    pub fn to_tool_definitions(&self) -> Vec<crate::llm::ToolDefinition> {
        self.tools
            .iter()
            .map(|t| crate::llm::ToolDefinition {
                typ: "function".to_string(),
                function: crate::llm::ToolFunctionDefinition {
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
