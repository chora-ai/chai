//! Tool layer: generic executor driven by skills' tools.json (allowlist + execution mapping).

mod generic;
mod post_process;

pub use crate::providers::ToolDefinition;
pub use generic::{DryRunResult, GenericToolExecutor, PostPipelinePreview, StepResult, TempFilePreview};
