//! Tool layer: Ollama tool definitions and execution (e.g. Obsidian skill → obsidian-cli).

mod obsidian;

pub use obsidian::{
    execute_obsidian_tool, obsidian_tool_definitions, ObsidianToolExecutor,
};
pub use crate::llm::ToolDefinition;
