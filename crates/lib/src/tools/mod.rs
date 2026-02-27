//! Tool layer: generic executor driven by skills' tools.json (allowlist + execution mapping).

mod generic;

pub use generic::GenericToolExecutor;
pub use crate::llm::ToolDefinition;
