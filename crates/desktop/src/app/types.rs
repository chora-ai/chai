use std::collections::BTreeMap;
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
}

impl ChatMessage {
    pub(crate) fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: text.into(),
            tool_calls: None,
            tool_results: None,
            delegation_event: None,
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
        }
    }

    pub(crate) fn error(text: impl Into<String>) -> Self {
        Self {
            role: "error".to_string(),
            content: text.into(),
            tool_calls: None,
            tool_results: None,
            delegation_event: None,
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
}

/// One worker row derived from gateway **`status`** `payload.agents.entries` (**`role`** **`worker`**): effective defaults for delegation.
#[derive(Clone, Default)]
pub struct StatusWorkerRow {
    pub(crate) id: String,
    pub(crate) default_provider: String,
    pub(crate) default_model: String,
}

/// One row of the merged orchestration catalog from gateway **`status`** (`payload.agents.orchestrationCatalog`).
#[derive(Clone, Default)]
pub struct OrchestrationCatalogRow {
    pub(crate) provider: String,
    pub(crate) model: String,
    pub(crate) discovered: bool,
    pub(crate) local: Option<bool>,
    pub(crate) tool_capable: Option<bool>,
}

/// Live gateway details from WebSocket `status` method.
#[derive(Clone, Default)]
pub struct GatewayStatusDetails {
    pub(crate) protocol: u32,
    pub(crate) port: u16,
    pub(crate) bind: String,
    pub(crate) auth: String,
    /// Resolved orchestrator agent id from config (same id used for the main agent turn).
    pub(crate) orchestrator_id: Option<String>,
    /// Orchestrator context directory from **`payload.agents.entries`** (**`role`** **`orchestrator`**, **`contextDirectory`**).
    pub(crate) orchestrator_context_dir: Option<String>,
    /// Resolved default provider: "ollama", "lms", "vllm", "nim", "openai", or "hf".
    pub(crate) default_provider: Option<String>,
    /// Resolved default model id (from config or provider fallback).
    pub(crate) default_model: Option<String>,
    /// Canonical provider ids with discovery enabled (`payload.agents.enabledProviders`). `None` if the gateway omitted the field.
    pub(crate) enabled_providers: Option<Vec<String>>,
    /// Ollama model names from gateway discovery (empty if Ollama unreachable).
    pub(crate) ollama_models: Vec<String>,
    /// LM Studio model names from gateway discovery (empty if LM Studio unreachable).
    pub(crate) lms_models: Vec<String>,
    /// vLLM model ids from gateway discovery (GET /v1/models).
    pub(crate) vllm_models: Vec<String>,
    /// NIM model ids (static catalog; hosted API).
    pub(crate) nim_models: Vec<String>,
    /// OpenAI API model ids from GET /v1/models (empty if unreachable or key missing).
    pub(crate) openai_models: Vec<String>,
    /// Hugging Face endpoint model ids from GET /v1/models when supported.
    pub(crate) hf_models: Vec<String>,
    /// Full orchestrator static system context (same string as that agent’s **`payload.agents.entries[]`** row with **`role`** **`orchestrator`**).
    pub(crate) system_context: Option<String>,
    /// Per-agent static system context (with date line), keyed by agent id — built from **`payload.agents.entries[].systemContext`**.
    pub(crate) agent_system_contexts: BTreeMap<String, String>,
    /// Current date (YYYY-MM-DD) from gateway **`payload.clock.date`**.
    pub(crate) date: Option<String>,
    /// **`payload.skillPackages.discoveryRoot`**.
    pub(crate) skill_packages_discovery_root: Option<String>,
    /// **`payload.skillPackages.packagesDiscovered`**.
    pub(crate) skill_packages_discovered: Option<u64>,
    /// **`payload.providers`** (discovery + models per backend).
    pub(crate) providers_block: Option<serde_json::Value>,
    /// Per-agent skill **`contextMode`** from **`payload.agents.entries[].skills`**.
    pub(crate) agent_context_modes: BTreeMap<String, String>,
    /// Skills portion of system context (full or compact per context mode).
    pub(crate) skills_context: Option<String>,
    /// Full skill content for display (always full; use for UI when present).
    pub(crate) skills_context_full: Option<String>,
    /// Skill bodies only (no overview). Set when context mode is readOnDemand; use for Skills section to avoid duplicating the overview.
    pub(crate) skills_context_bodies: Option<String>,
    /// Skill context mode: "full" or "readOnDemand".
    pub(crate) context_mode: Option<String>,
    /// Merged tool definitions sent to the model (including read_skill when context mode is readOnDemand).
    pub(crate) tools: Option<String>,
    /// Per-agent pretty-printed tool JSON (orchestrator + each worker). Same strings as top-level **`tools`** for the orchestrator id.
    pub(crate) agent_tools: BTreeMap<String, String>,
    /// Discovery + allowlist merge for delegation / UI (see lib `build_orchestration_catalog`).
    pub(crate) orchestration_catalog: Vec<OrchestrationCatalogRow>,
    /// Worker rows from **`payload.agents.entries`** (**`role`** **`worker`**).
    pub(crate) workers: Vec<StatusWorkerRow>,
    /// Pretty-printed JSON for the full WebSocket **`res`** to the `status` request (type, id, ok, payload).
    pub(crate) status_response_json: Option<String>,
    /// Full **`payload.channels`** (active, configured, transport, errors, Matrix verification summary, …).
    pub(crate) channels_block: Option<serde_json::Value>,
}

