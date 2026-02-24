//! Obsidian skill tools: map agent intent to obsidian-cli subcommands via the safe exec layer.

use crate::agent::ToolExecutor;
use crate::exec::Allowlist;
use crate::llm::{ToolDefinition, ToolFunctionDefinition};
use serde_json::json;

/// Executor that runs obsidian-cli via the allowlist (safe execution).
pub struct ObsidianToolExecutor {
    pub allowlist: Allowlist,
}

impl ToolExecutor for ObsidianToolExecutor {
    fn execute(&self, name: &str, args: &serde_json::Value) -> Result<String, String> {
        execute_obsidian_tool(&self.allowlist, name, args)
    }
}

/// Return Ollama tool definitions for obsidian-cli (search, search-content, create, move, delete).
pub fn obsidian_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            typ: "function".to_string(),
            function: ToolFunctionDefinition {
                name: "obsidian_search".to_string(),
                description: Some("Search note names in the default vault (obsidian-cli search).".to_string()),
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
                description: Some("Search inside note content in the default vault (obsidian-cli search-content).".to_string()),
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
                description: Some("Create a new note in the default vault (obsidian-cli create).".to_string()),
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
        ToolDefinition {
            typ: "function".to_string(),
            function: ToolFunctionDefinition {
                name: "obsidian_move".to_string(),
                description: Some("Move or rename a note; updates wikilinks (obsidian-cli move).".to_string()),
                parameters: json!({
                    "type": "object",
                    "required": ["old_path", "new_path"],
                    "properties": {
                        "old_path": { "type": "string", "description": "Current path of the note" },
                        "new_path": { "type": "string", "description": "New path for the note" }
                    }
                }),
            },
        },
        ToolDefinition {
            typ: "function".to_string(),
            function: ToolFunctionDefinition {
                name: "obsidian_delete".to_string(),
                description: Some("Delete a note in the default vault (obsidian-cli delete).".to_string()),
                parameters: json!({
                    "type": "object",
                    "required": ["path"],
                    "properties": {
                        "path": { "type": "string", "description": "Path of the note to delete" }
                    }
                }),
            },
        },
    ]
}

/// Execute an Obsidian tool by name and JSON arguments. Uses the default obsidian-cli allowlist.
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
            allowlist.run("obsidian-cli", "search", &[query.to_string()])
        }
        "obsidian_search_content" => {
            let query = args.get("query").and_then(|v| v.as_str()).ok_or("missing query")?;
            allowlist.run("obsidian-cli", "search-content", &[query.to_string()])
        }
        "obsidian_create" => {
            let path = args.get("path").and_then(|v| v.as_str()).ok_or("missing path")?;
            let mut a: Vec<String> = vec![path.to_string()];
            if let Some(c) = args.get("content").and_then(|v| v.as_str()) {
                a.push("--content".to_string());
                a.push(c.to_string());
            }
            a.push("--open".to_string());
            allowlist.run("obsidian-cli", "create", &a)
        }
        "obsidian_move" => {
            let old_path = args.get("old_path").and_then(|v| v.as_str()).ok_or("missing old_path")?;
            let new_path = args.get("new_path").and_then(|v| v.as_str()).ok_or("missing new_path")?;
            allowlist.run("obsidian-cli", "move", &[old_path.to_string(), new_path.to_string()])
        }
        "obsidian_delete" => {
            let path = args.get("path").and_then(|v| v.as_str()).ok_or("missing path")?;
            allowlist.run("obsidian-cli", "delete", &[path.to_string()])
        }
        _ => Err(format!("unknown obsidian tool: {}", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::obsidian_cli_allowlist;

    #[test]
    fn obsidian_tool_definitions_non_empty() {
        let defs = obsidian_tool_definitions();
        assert!(!defs.is_empty());
        assert!(defs.iter().any(|d| d.function.name == "obsidian_search"));
    }

    #[test]
    fn execute_obsidian_tool_unknown_fails() {
        let allowlist = obsidian_cli_allowlist();
        let err = execute_obsidian_tool(&allowlist, "unknown", &json!({})).unwrap_err();
        assert!(err.contains("unknown"));
    }
}
