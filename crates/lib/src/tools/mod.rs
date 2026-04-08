//! Tool layer: generic executor driven by skills' tools.json (allowlist + execution mapping).

mod generic;

pub use crate::providers::ToolDefinition;
pub use generic::GenericToolExecutor;
