//! Tool layer: Ollama tool definitions and execution (e.g. Obsidian skills → Obsidian CLIs).

mod obsidian;
mod notesmd_cli;

pub use obsidian::{
    execute_obsidian_tool, obsidian_tool_definitions, ObsidianToolExecutor,
};
pub use notesmd_cli::{
    execute_notesmd_cli_tool, notesmd_cli_tool_definitions, NotesmdCliToolExecutor,
};
pub use crate::llm::ToolDefinition;
