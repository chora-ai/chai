use std::vec::Vec;

/// Chat message used across the app (chat screen, session timelines, logs).
#[derive(Clone)]
pub struct ChatMessage {
    pub(crate) role: String,
    pub(crate) content: String,
    pub(crate) tool_calls: Option<Vec<serde_json::Value>>,
}

impl ChatMessage {
    pub(crate) fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: text.into(),
            tool_calls: None,
        }
    }

    pub(crate) fn assistant(
        text: impl Into<String>,
        tool_calls: Option<Vec<serde_json::Value>>,
    ) -> Self {
        Self {
            role: "assistant".to_string(),
            content: text.into(),
            tool_calls,
        }
    }

    pub(crate) fn error(text: impl Into<String>) -> Self {
        Self {
            role: "error".to_string(),
            content: text.into(),
            tool_calls: None,
        }
    }
}

/// Reply from a single agent turn, as seen by the app.
#[derive(Clone)]
pub struct AgentReply {
    pub(crate) session_id: String,
    pub(crate) reply: String,
    pub(crate) tool_calls: Vec<serde_json::Value>,
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
}

/// Live gateway details from WebSocket `status` method.
#[derive(Clone, Default)]
pub struct GatewayStatusDetails {
    pub(crate) protocol: u32,
    pub(crate) port: u16,
    pub(crate) bind: String,
    pub(crate) auth: String,
    /// Resolved default backend: "ollama", "lmstudio", or "nim".
    pub(crate) default_backend: Option<String>,
    /// Resolved default model id (from config or backend fallback).
    pub(crate) default_model: Option<String>,
    /// Ollama model names from gateway discovery (empty if Ollama unreachable).
    pub(crate) ollama_models: Vec<String>,
    /// LM Studio model names from gateway discovery (empty if LM Studio unreachable).
    pub(crate) lm_studio_models: Vec<String>,
    /// NIM model ids (static catalog; API backend).
    pub(crate) nim_models: Vec<String>,
    /// Agent context loaded at gateway startup (e.g. AGENTS.md). None if not loaded.
    pub(crate) agent_context: Option<String>,
    /// Full system context sent to the model (agent context + skills). Empty if none.
    pub(crate) system_context: Option<String>,
    /// Current date (YYYY-MM-DD) from the gateway, for display in Context.
    pub(crate) date: Option<String>,
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
}

