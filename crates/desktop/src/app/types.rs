use std::collections::{BTreeMap, HashMap};
use std::vec::Vec;

/// Chat message used across the app (chat screen, session timelines, logs).
#[derive(Clone)]
pub struct ChatMessage {
    pub(crate) role: String,
    pub(crate) content: String,
    pub(crate) tool_calls: Option<Vec<serde_json::Value>>,
    pub(crate) tool_results: Option<Vec<String>>,
    /// When set, this row is a gateway orchestration line (`orchestration.delegate.*`), not a model role.
    pub(crate) delegation_event: Option<String>,
    /// Tool name for `tool_call` and `tool_result` role messages.
    pub(crate) tool_name: Option<String>,
    /// Tool arguments (JSON) for `tool_call` role messages.
    pub(crate) tool_args: Option<serde_json::Value>,
    /// Tool result for `tool_result` role messages.
    pub(crate) tool_result: Option<String>,
    /// Index of this tool call within the agent turn (for correlating tool_call → tool_result).
    pub(crate) tool_index: Option<usize>,
    /// Source of this message — the agent id (e.g. `"orchestrator"` or a worker id like
    /// `"engineer"`). Used to display the author label and to style worker tool calls
    /// with a blue border matching delegation events.
    pub(crate) source: Option<String>,
    /// Tool calls that were generated but not executed because the loop limit was reached.
    /// Set on `tool_loop_limit` role messages.
    pub(crate) pending_tool_calls: Option<Vec<serde_json::Value>>,
}

impl ChatMessage {
    pub(crate) fn system(text: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: text.into(),
            tool_calls: None,
            tool_results: None,
            delegation_event: None,
            tool_name: None,
            tool_args: None,
            tool_result: None,
            tool_index: None,
            source: None,
            pending_tool_calls: None,
        }
    }

    pub(crate) fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: text.into(),
            tool_calls: None,
            tool_results: None,
            delegation_event: None,
            tool_name: None,
            tool_args: None,
            tool_result: None,
            tool_index: None,
            source: None,
            pending_tool_calls: None,
        }
    }

    pub(crate) fn assistant(
        text: impl Into<String>,
        tool_calls: Option<Vec<serde_json::Value>>,
        tool_results: Option<Vec<String>>,
    ) -> Self {
        Self {
            role: "assistant".to_string(),
            content: text.into(),
            tool_calls,
            tool_results,
            delegation_event: None,
            tool_name: None,
            tool_args: None,
            tool_result: None,
            tool_index: None,
            source: None,
            pending_tool_calls: None,
        }
    }

    pub(crate) fn error(text: impl Into<String>) -> Self {
        Self {
            role: "error".to_string(),
            content: text.into(),
            tool_calls: None,
            tool_results: None,
            delegation_event: None,
            tool_name: None,
            tool_args: None,
            tool_result: None,
            tool_index: None,
            source: None,
            pending_tool_calls: None,
        }
    }

    /// Tool loop iteration limit reached; the listed tool calls were not executed.
    pub(crate) fn tool_loop_limit(
        content: impl Into<String>,
        pending_tool_calls: Vec<serde_json::Value>,
    ) -> Self {
        Self {
            role: "tool_loop_limit".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_results: None,
            delegation_event: None,
            tool_name: None,
            tool_args: None,
            tool_result: None,
            tool_index: None,
            source: None,
            pending_tool_calls: if pending_tool_calls.is_empty() {
                None
            } else {
                Some(pending_tool_calls)
            },
        }
    }

    /// Agent turn was stopped by the user. The session transcript is preserved
    /// and the user can send a new message to continue.
    pub(crate) fn turn_stopped() -> Self {
        Self {
            role: "turn_stopped".to_string(),
            content: "turn paused — send a message to continue".to_string(),
            tool_calls: None,
            tool_results: None,
            delegation_event: None,
            tool_name: None,
            tool_args: None,
            tool_result: None,
            tool_index: None,
            source: None,
            pending_tool_calls: None,
        }
    }
}

/// Reply from a single agent turn, as seen by the app.
#[derive(Clone)]
pub struct AgentReply {
    pub(crate) session_id: String,
    pub(crate) reply: String,
    pub(crate) tool_calls: Vec<serde_json::Value>,
    pub(crate) tool_results: Vec<String>,
    /// Whether the agent loop hit its iteration limit while the model was still generating tool calls.
    pub(crate) loop_limit_reached: bool,
    /// Tool calls that were generated but not executed because the loop limit was reached.
    pub(crate) pending_tool_calls: Vec<serde_json::Value>,
    /// Whether the turn was stopped by the user via the stop button.
    pub(crate) stopped: bool,
}

/// Event emitted by the gateway for session timelines.
#[derive(Clone)]
pub struct SessionEvent {
    pub(crate) session_id: String,
    pub(crate) role: String,
    pub(crate) content: String,
    pub(crate) channel_id: Option<String>,
    pub(crate) conversation_id: Option<String>,
    pub(crate) tool_calls: Option<Vec<serde_json::Value>>,
    pub(crate) tool_results: Option<Vec<String>>,
    /// Gateway `event` field when this is an orchestration row (e.g. `orchestration.delegate.start`).
    pub(crate) delegation_event: Option<String>,
    /// Tool name for tool events.
    pub(crate) tool_name: Option<String>,
    /// Tool arguments for `session.tool_call` events.
    pub(crate) tool_args: Option<serde_json::Value>,
    /// Tool result for `session.tool_result` events.
    pub(crate) tool_result: Option<String>,
    /// Index of this tool call within the agent turn.
    pub(crate) tool_index: Option<usize>,
    /// Source of this message — the agent id (e.g. `"orchestrator"` or a worker id like
    /// `"engineer"`). Used to display the author label and to style worker tool calls
    /// with a blue border matching delegation events.
    pub(crate) source: Option<String>,
    /// Tool calls that were generated but not executed because the loop limit was reached.
    /// Set on `session.tool_loop_limit` events.
    pub(crate) pending_tool_calls: Option<Vec<serde_json::Value>>,
}

/// One worker row derived from gateway **`status`** `payload.agents` (**`role`** **`worker`**): effective defaults for delegation.
#[derive(Clone, Default)]
pub struct StatusWorkerRow {
    pub(crate) id: String,
    pub(crate) default_provider: String,
    pub(crate) default_model: String,
    /// **`payload.agents[].enabledSkills`** for this worker.
    pub(crate) enabled_skills: Vec<String>,
    /// **`payload.agents[].contextMode`** for this worker.
    pub(crate) context_mode: Option<String>,
}

/// Per-provider info parsed from the gateway status `providers` block.
#[derive(Clone, Default)]
pub struct ProviderStatusInfo {
    /// Endpoint type string (e.g. `"ollama"`, `"openai-compat"`).
    pub(crate) endpoint_type: String,
    /// Discovery method string (e.g. `"default"`, `"lmstudio"`, `"static"`).
    pub(crate) model_discovery: String,
    /// Discovered model names (empty if provider unreachable or not in discovery scope).
    pub(crate) models: Vec<String>,
}

/// Live gateway details from WebSocket `status` method.
#[derive(Clone, Default)]
pub struct GatewayStatusDetails {
    pub(crate) protocol: u32,
    pub(crate) port: u16,
    pub(crate) bind: String,
    pub(crate) auth: String,
    /// **`payload.gateway.status`** (e.g. **`running`**).
    pub(crate) status: String,
    /// **`payload.sandbox.mode`** — the sandbox enforcement mode (`"strict"`, `"current"`, or `"unsafe"`).
    pub(crate) sandbox_mode: String,
    /// **`payload.sandbox.roots`** — number of writable roots in the sandbox.
    pub(crate) sandbox_roots: u64,
    /// Resolved orchestrator agent id from config (same id used for the main agent turn).
    pub(crate) orchestrator_id: Option<String>,
    /// Resolved default provider id (from config).
    pub(crate) default_provider: Option<String>,
    /// Resolved default model id (from config or provider fallback).
    pub(crate) default_model: Option<String>,
    /// Provider ids with discovery enabled (`payload.agents.enabledProviders`). `None` if the gateway omitted the field.
    pub(crate) enabled_providers: Option<Vec<String>>,
    /// Per-provider info (endpoint type, discovery, models) keyed by provider id, parsed from `payload.providers`.
    pub(crate) provider_info: HashMap<String, ProviderStatusInfo>,
    /// **`payload.skills.packagesDiscovered`**.
    pub(crate) skills_packages_discovered: Option<u64>,
    /// **`payload.skills.lockMode`** — `"strict"` or `"warn"`.
    pub(crate) skills_lock_mode: Option<String>,
    /// **`payload.skills.lockGeneration`** — lockfile generation number, or `None` when no lockfile.
    pub(crate) skills_lock_generation: Option<u64>,
    /// **`payload.skills.lockedSkills`** — number of skills pinned in the lockfile.
    pub(crate) skills_locked_count: Option<u64>,
    /// Per-agent skill **`contextMode`** from **`payload.agents[].contextMode`**.
    pub(crate) agent_context_modes: BTreeMap<String, String>,
    /// Worker rows from **`payload.agents`** (**`role`** **`worker`**).
    pub(crate) workers: Vec<StatusWorkerRow>,
    /// Per-agent skill runtime data from **`payload.agents[]`**, keyed by agent id.
    pub(crate) agent_skills: BTreeMap<String, AgentSkillsRuntime>,
    /// Orchestrator **`payload.agents[].maxToolLoopsPerTurn`**.
    pub(crate) max_tool_loops_per_turn: Option<u32>,
    /// Orchestrator **`payload.agents[].maxDelegationsPerTurn`**.
    pub(crate) max_delegations_per_turn: Option<usize>,
    /// Orchestrator **`payload.agents[].maxDelegationsPerSession`**.
    pub(crate) max_delegations_per_session: Option<usize>,
    /// Orchestrator **`payload.agents[].maxDelegationsPerWorker`** — worker id → limit.
    pub(crate) max_delegations_per_worker: Option<BTreeMap<String, usize>>,
    /// Orchestrator **`payload.agents[].enabledSkills`**.
    pub(crate) orchestrator_enabled_skills: Vec<String>,
    /// Pretty-printed JSON for the full WebSocket **`res`** to the `status` request (type, id, ok, payload).
    pub(crate) status_response_json: Option<String>,
    /// Full **`payload.channels`** (active, configured, transport, errors, Matrix verification summary, …).
    pub(crate) channels_block: Option<serde_json::Value>,
}

/// Per-agent skill runtime data parsed from **`payload.agents[]`**.
#[derive(Clone, Default)]
pub struct AgentSkillsRuntime {
    /// Skill package names loaded for this agent.
    pub(crate) enabled_skills: Vec<String>,
    /// Skill context mode: "full" or "readOnDemand".
    pub(crate) context_mode: Option<String>,
}

/// On-demand per-agent detail fetched via the `agentDetail` WS method.
/// Contains the heavy fields (systemContext, tools, skillsContext) that were
/// removed from the polling `status` response to reduce payload size.
#[derive(Clone, Default)]
pub struct AgentDetail {
    /// Agent id this detail belongs to.
    pub(crate) agent_id: String,
    /// Agent role ("orchestrator" or "worker").
    pub(crate) role: String,
    /// Full static system context string for this agent.
    pub(crate) system_context: Option<String>,
    /// Pretty-printed tool JSON string for this agent.
    pub(crate) tools: Option<String>,
    /// Per-skill body (name → frontmatter-stripped body) from **`skillsContext`**.
    pub(crate) skills_context: BTreeMap<String, String>,
}
