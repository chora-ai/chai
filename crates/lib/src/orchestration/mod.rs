//! Orchestration: orchestrator and worker flows (see `base/adr/ORCHESTRATION.md`).
//!
//! **Worker / delegation primitive** — [`crate::agent::run_turn_with_messages`] with explicit messages.
//!
//! **Provider dispatch** — [`ProviderChoice`], [`ProviderClients`], and [`resolve_model`]
//! centralize "which client" + default model resolution for the gateway and future orchestrator code.
//!
//! **Orchestrator loop** — [`DELEGATE_TASK_TOOL_NAME`], [`merge_delegate_task`], [`execute_delegate_task`]:
//! when workers are configured, the orchestrator may delegate via `delegate_task`; the worker uses a per-worker system context and skill tools when
//! `workerId` is set (nested `delegate_task` disabled). Gateway inbound and WebSocket `agent` both pass [`DelegateContext`].

mod choice;
pub mod delegate;
mod dispatch;
mod model;
mod policy;
mod workers_context;

pub use choice::{resolve_provider_choice, ProviderChoice};
pub use delegate::{
    delegate_task_tool_definition, execute_delegate_task, merge_delegate_task,
    worker_tool_list, DelegateContext, DelegateObservability, DelegateTaskResult,
    WorkerDelegateRuntime, EVENT_DELEGATE_ERROR, EVENT_DELEGATE_REJECTED,
    EVENT_DELEGATE_START, EVENT_TOOL_CALL, DELEGATE_TASK_TOOL_NAME,
    EVENT_ASSISTANT_PROGRESS, EVENT_DELEGATE_COMPLETE, EVENT_TOOL_RESULT,
    EVENT_TOOL_LOOP_LIMIT,
};
pub use dispatch::ProviderClients;
pub use model::{resolve_model, DEFAULT_MODEL_FALLBACK};
pub use policy::{apply_delegation_bracket_match, assert_session_delegation_limits};

pub use workers_context::{build_workers_context, effective_worker_defaults};

pub use crate::agent::{
    run_turn_dyn, run_turn_with_messages, run_turn_with_messages_dyn, AgentTurnResult,
};
