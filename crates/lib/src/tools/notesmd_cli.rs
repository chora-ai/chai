//! notesmd-cli skill tools: map agent intent to the notesmd-cli binary via the safe exec layer.
//! Uses the `notesmd-cli` command: https://github.com/yakitrak/notesmd-cli

use std::path::Path;

use crate::agent::ToolExecutor;
use crate::exec::Allowlist;
use crate::llm::{ToolDefinition, ToolFunctionDefinition};
use serde_json::json;

/// Executor that runs the notesmd-cli binary via the allowlist (safe execution).
pub struct NotesmdCliToolExecutor {
    pub allowlist: Allowlist,
}

impl ToolExecutor for NotesmdCliToolExecutor {
    fn execute(&self, name: &str, args: &serde_json::Value) -> Result<String, String> {
        execute_notesmd_cli_tool(&self.allowlist, name, args)
    }
}

/// Return Ollama tool definitions for the notesmd-cli binary (search, search-content, create, daily, read_note, update_daily with optional replace).
pub fn notesmd_cli_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            typ: "function".to_string(),
            function: ToolFunctionDefinition {
                name: "notesmd_cli_search".to_string(),
                description: Some(
                    "Search notes by name. Call only when the user asks to find or list notes. Required: query. Required: query."
                        .to_string(),
                ),
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
                name: "notesmd_cli_search_content".to_string(),
                description: Some(
                    "Search note content. Call only when the user asks to search the contents of a note or find notes containing a term. Required: query."
                        .to_string(),
                ),
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
                name: "notesmd_cli_create".to_string(),
                description: Some(
                    "Create a new note. Call only when the user asks to create, add, or write a note. Required: path and content."
                    .to_string(),
                ),
                    parameters: json!({
                    "type": "object",
                    "required": ["path", "content"],
                    "properties": {
                        "path": { "type": "string", "description": "Path for the new note or its name" },
                        "content": { "type": "string", "description": "Initial content for the new note" }
                    }
                }),
            },
        },
        ToolDefinition {
            typ: "function".to_string(),
            function: ToolFunctionDefinition {
                name: "notesmd_cli_daily".to_string(),
                description: Some(
                    "Create or open today's daily note in the vault. Call only when the user asks for their daily note or today's note."
                        .to_string(),
                ),
                parameters: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        },
        ToolDefinition {
            typ: "function".to_string(),
            function: ToolFunctionDefinition {
                name: "notesmd_cli_read_note".to_string(),
                description: Some(
                    "Read a note by path (daily notes use date YYYY-MM-DD). Call only when following a protocol or when asked to read or share the contents of a note."
                        .to_string(),
                ),
                parameters: json!({
                    "type": "object",
                    "required": ["path"],
                    "properties": {
                        "path": { "type": "string", "description": "Note path or daily note date, e.g. 2026-02-25" }
                    }
                }),
            },
        },
        ToolDefinition {
            typ: "function".to_string(),
            function: ToolFunctionDefinition {
                name: "notesmd_cli_update_daily".to_string(),
                description: Some(
                    "Create or update a daily note. Call only when the user asks to create or update a daily note. Required: date and content. Optional: replace."
                        .to_string(),
                ),
                parameters: json!({
                    "type": "object",
                    "required": ["date", "content"],
                    "properties": {
                        "date": { "type": "string", "description": "Date for the daily note, e.g. 2026-02-25" },
                        "content": { "type": "string", "description": "The entire note returned from notesmd_cli_read_note with the changes applied. Never pass only the content that is being edited or updated." },
                        "replace": { "type": "boolean", "description": "Set to true when editing or updating a daily note: checking a box, adding a new task item, or changing any existing text. Do not use when appending information to an existing note." }
                    }
                }),
            },
        },
    ]
}

/// Normalize content so literal `\n` / `\t` from LLM JSON become real newlines/tabs.
fn normalize_note_content(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\t", "\t")
}

/// Parse replace flag from tool args; models sometimes send the string "true" or use wrong key case.
fn parse_replace_flag(v: Option<&serde_json::Value>) -> bool {
    match v {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => s.eq_ignore_ascii_case("true"),
        Some(serde_json::Value::Number(n)) => n.as_i64() == Some(1),
        _ => false,
    }
}

/// Get a parameter by key, checking lowercase "replace" then any key that matches case-insensitively.
fn get_replace_arg(args: &serde_json::Map<String, serde_json::Value>) -> Option<&serde_json::Value> {
    args.get("replace").or_else(|| {
        args.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("replace"))
            .map(|(_, v)| v)
    })
}

/// True if `path` looks like a bare daily note date (YYYY-MM-DD).
fn is_bare_date_path(path: &str) -> bool {
    path.len() == 10
        && path.as_bytes().get(4) == Some(&b'-')
        && path.as_bytes().get(7) == Some(&b'-')
        && path.chars().all(|c| c.is_ascii_digit() || c == '-')
}

/// Resolve the daily note path (folder/date or date) using the vault's .obsidian/daily-notes.json
/// so that replace writes to the same file as `notesmd-cli daily`. Falls back to `date` on any error.
fn resolve_daily_note_path(allowlist: &Allowlist, date: &str) -> String {
    let vault_path = match allowlist.run("notesmd-cli", "print-default", &["--path-only".to_string()]) {
        Ok(s) => s.trim().to_string(),
        Err(_) => return date.to_string(),
    };
    if vault_path.is_empty() {
        return date.to_string();
    }
    let config_path = Path::new(&vault_path).join(".obsidian").join("daily-notes.json");
    let contents = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return date.to_string(),
    };
    let config: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return date.to_string(),
    };
    let folder = config.get("folder").and_then(|v| v.as_str()).unwrap_or("").trim();
    if folder.is_empty() {
        date.to_string()
    } else {
        format!("{}/{}", folder, date)
    }
}

/// Execute a notesmd-cli tool by name and JSON arguments.
pub fn execute_notesmd_cli_tool(
    allowlist: &Allowlist,
    name: &str,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let args = arguments.as_object().ok_or("arguments must be an object")?;
    match name {
        "notesmd_cli_search" => {
            let query = args.get("query").and_then(|v| v.as_str()).ok_or("missing query")?;
            allowlist.run("notesmd-cli", "search", &[query.to_string()])
        }
        "notesmd_cli_search_content" => {
            let query = args.get("query").and_then(|v| v.as_str()).ok_or("missing query")?;
            allowlist.run("notesmd-cli", "search-content", &[query.to_string()])
        }
        "notesmd_cli_create" => {
            let path = args.get("path").and_then(|v| v.as_str()).ok_or("missing path")?;
            let mut a: Vec<String> = vec![path.to_string()];
            if let Some(c) = args.get("content").and_then(|v| v.as_str()) {
                a.push("--content".to_string());
                a.push(normalize_note_content(c));
            }
            allowlist.run("notesmd-cli", "create", &a)
        }
        "notesmd_cli_daily" => allowlist.run("notesmd-cli", "daily", &[]),
        "notesmd_cli_read_note" => {
            let path = args.get("path").and_then(|v| v.as_str()).ok_or("missing path")?;
            let path = if is_bare_date_path(path) {
                resolve_daily_note_path(allowlist, path)
            } else {
                path.to_string()
            };
            allowlist.run("notesmd-cli", "print", &[path])
        }
        "notesmd_cli_update_daily" => {
            let date = args.get("date").and_then(|v| v.as_str()).ok_or("missing date")?;
            let content = args.get("content").and_then(|v| v.as_str()).ok_or("missing content")?;
            let replace = parse_replace_flag(get_replace_arg(args));
            let content = normalize_note_content(content);
            // Use the same path as the daily note (from daily-notes.json) so we read/write the file
            // the user sees; create with just the date uses app.json and can point elsewhere.
            let note_path = if is_bare_date_path(date) {
                resolve_daily_note_path(allowlist, date)
            } else {
                date.to_string()
            };
            let mut a = vec![
                note_path,
                "--content".to_string(),
                content,
            ];
            if replace {
                a.push("--overwrite".to_string());
            } else {
                a.push("--append".to_string());
            }
            allowlist.run("notesmd-cli", "create", &a)
        }
        _ => Err(format!("unknown notesmd-cli tool: {}", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::notesmd_cli_allowlist;

    #[test]
    fn notesmd_cli_tool_definitions_non_empty() {
        let defs = notesmd_cli_tool_definitions();
        assert!(!defs.is_empty());
        assert!(defs.iter().any(|d| d.function.name == "notesmd_cli_search"));
    }

    #[test]
    fn execute_notesmd_cli_tool_unknown_fails() {
        let allowlist = notesmd_cli_allowlist();
        let err = execute_notesmd_cli_tool(&allowlist, "unknown", &json!({})).unwrap_err();
        assert!(err.contains("unknown"));
    }

    #[test]
    fn normalize_note_content_unescapes_literal_backslash_n() {
        // LLMs sometimes send literal \n in JSON; we normalize to real newlines.
        let s = "# 2026-02-25\\n\\n## Action Items\\n- [x] done";
        assert_eq!(
            super::normalize_note_content(s),
            "# 2026-02-25\n\n## Action Items\n- [x] done"
        );
        let already_good = "line1\nline2";
        assert_eq!(super::normalize_note_content(already_good), "line1\nline2");
    }

    #[test]
    fn parse_replace_flag_accepts_bool_string_and_number() {
        assert!(super::parse_replace_flag(Some(&json!(true))));
        assert!(!super::parse_replace_flag(Some(&json!(false))));
        assert!(super::parse_replace_flag(Some(&json!("true"))));
        assert!(super::parse_replace_flag(Some(&json!("True"))));
        assert!(super::parse_replace_flag(Some(&json!(1))));
        assert!(!super::parse_replace_flag(Some(&json!("false"))));
        assert!(!super::parse_replace_flag(Some(&json!("yes"))));
        assert!(!super::parse_replace_flag(Some(&json!(0))));
        assert!(!super::parse_replace_flag(None));
    }

    #[test]
    fn get_replace_arg_finds_case_insensitive_key() {
        let args = json!({"date": "2026-02-25", "content": "x", "Replace": true}).as_object().unwrap().clone();
        assert!(super::parse_replace_flag(super::get_replace_arg(&args)));
        let args2 = json!({"date": "2026-02-25", "content": "x", "replace": false}).as_object().unwrap().clone();
        assert!(!super::parse_replace_flag(super::get_replace_arg(&args2)));
    }
}
