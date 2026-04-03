//! Orchestration: orchestrator and worker flows (see `.agents/epic/ORCHESTRATION.md`).
//!
//! **Worker / delegation primitive** — [`crate::agent::run_turn_with_messages`] with explicit messages.
//!
//! **Provider dispatch (phase 2)** — [`ProviderChoice`], [`ProviderClients::as_dyn`], and [`resolve_model`]
//! centralize “which client” + default model resolution for the gateway and future orchestrator code.
//!
//! **Orchestrator loop (phase 3, done)** — [`DELEGATE_TASK_TOOL_NAME`], [`merge_delegate_task`], [`execute_delegate_task`]:
//! when workers are configured, the orchestrator may delegate via `delegate_task`; the worker uses a per-worker system context and skill tools when
//! `workerId` is set (nested `delegate_task` disabled). Gateway inbound and WebSocket `agent` both pass [`DelegateContext`].

mod catalog;
mod choice;
mod delegate;
mod dispatch;
mod model;
mod policy;
mod workers_context;

pub use choice::{provider_choice_from_canonical, provider_id, resolve_provider_choice, ProviderChoice};
pub use delegate::{
    delegate_task_tool_definition, execute_delegate_task, merge_delegate_task, worker_tool_list,
    parse_delegate_tool_calls, parse_delegate_tool_results, system_context_with_today,
    DelegateContext, DelegateObservability, WorkerDelegateRuntime, DELEGATE_TASK_TOOL_NAME,
    EVENT_DELEGATE_COMPLETE, EVENT_DELEGATE_ERROR, EVENT_DELEGATE_REJECTED, EVENT_DELEGATE_START,
};
pub use dispatch::ProviderClients;
pub use model::{
    resolve_model, DEFAULT_MODEL_FALLBACK, DEFAULT_MODEL_FALLBACK_LMS, DEFAULT_MODEL_FALLBACK_NIM,
    DEFAULT_MODEL_FALLBACK_VLLM,
};
pub use policy::{
    apply_delegation_instruction_routes, assert_delegation_pair_allowed,
    assert_delegate_provider_not_blocked, assert_session_delegation_limits,
};

pub use catalog::{build_orchestration_catalog, OrchestrationCatalogEntry};
pub use workers_context::{build_workers_context, effective_worker_defaults};

pub use crate::agent::{
    run_turn_dyn, run_turn_with_messages, run_turn_with_messages_dyn, AgentTurnResult,
};
