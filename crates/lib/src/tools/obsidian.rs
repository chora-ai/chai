//! Obsidian skill tools: map agent intent to the official Obsidian CLI subcommands via the safe exec layer.
//! Uses the `obsidian` CLI: https://help.obsidian.md/cli

use crate::agent::ToolExecutor;
use crate::exec::Allowlist;
use crate::llm::{ToolDefinition, ToolFunctionDefinition};
use serde_json::json;

/// Executor that runs the Obsidian CLI via the allowlist (safe execution).
pub struct ObsidianToolExecutor {
    pub allowlist: Allowlist,
}

impl ToolExecutor for ObsidianToolExecutor {
    fn execute(&self, name: &str, args: &serde_json::Value) -> Result<String, String> {
        execute_obsidian_tool(&self.allowlist, name, args)
    }
}

/// Return Ollama tool definitions for the Obsidian CLI (search, search:context, create only).
pub fn obsidian_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            typ: "function".to_string(),
            function: ToolFunctionDefinition {
                name: "obsidian_search".to_string(),
                description: Some("Search note names in the default vault (obsidian search).".to_string()),
                parameters: json!({
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": { "type": "string", "description": "Search query for note names" }
                    }
                }),
            },
        },
        ToolDefinition {
            typ: "function".to_string(),
            function: ToolFunctionDefinition {
                name: "obsidian_search_content".to_string(),
                description: Some("Search inside note content in the default vault (obsidian search:context).".to_string()),
                parameters: json!({
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": { "type": "string", "description": "Search query for content inside notes" }
                    }
                }),
            },
        },
        ToolDefinition {
            typ: "function".to_string(),
            function: ToolFunctionDefinition {
                name: "obsidian_create".to_string(),
                description: Some("Create a new note in the default vault (obsidian create).".to_string()),
                parameters: json!({
                    "type": "object",
                    "required": ["path"],
                    "properties": {
                        "path": { "type": "string", "description": "Path for the new note, e.g. Folder/New note" },
                        "content": { "type": "string", "description": "Optional initial content" }
                    }
                }),
            },
        },
    ]
}

/// Execute an Obsidian tool by name and JSON arguments. Uses the default Obsidian CLI allowlist.
/// Returns the command output or an error string.
pub fn execute_obsidian_tool(
    allowlist: &Allowlist,
    name: &str,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let args = arguments.as_object().ok_or("arguments must be an object")?;
    match name {
        "obsidian_search" => {
            let query = args.get("query").and_then(|v| v.as_str()).ok_or("missing query")?;
            allowlist.run("obsidian", "search", &[query.to_string()])
        }
        "obsidian_search_content" => {
            let query = args.get("query").and_then(|v| v.as_str()).ok_or("missing query")?;
            allowlist.run("obsidian", "search:context", &[query.to_string()])
        }
        "obsidian_create" => {
            let path = args.get("path").and_then(|v| v.as_str()).ok_or("missing path")?;
            let mut a: Vec<String> = vec![path.to_string()];
            if let Some(c) = args.get("content").and_then(|v| v.as_str()) {
                a.push("--content".to_string());
                a.push(c.to_string());
            }
            allowlist.run("obsidian", "create", &a)
        }
        _ => Err(format!("unknown obsidian tool: {}", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::obsidian_allowlist;

    #[test]
    fn obsidian_tool_definitions_non_empty() {
        let defs = obsidian_tool_definitions();
        assert!(!defs.is_empty());
        assert!(defs.iter().any(|d| d.function.name == "obsidian_search"));
    }

    #[test]
    fn execute_obsidian_tool_unknown_fails() {
        let allowlist = obsidian_allowlist();
        let err = execute_obsidian_tool(&allowlist, "unknown", &json!({})).unwrap_err();
        assert!(err.contains("unknown"));
    }
}
