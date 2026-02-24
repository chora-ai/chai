//! Conversation session and message history for the agent loop.
//!
//! Sessions are keyed by id and hold a list of messages (user/assistant/system).
//! Used by the gateway to run agent turns and optionally bind to channel conversations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Unique session identifier (opaque string).
pub type SessionId = String;

/// A single message in a session (role + content; assistant may have tool_calls, tool results have tool_name).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    /// When role is "assistant", optional tool calls from the model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<crate::llm::ToolCall>>,
    /// When role is "tool", the name of the tool this result is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

impl SessionMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_name: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_name: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_name: None,
        }
    }
}

/// A session: id and ordered message history.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub messages: Vec<SessionMessage>,
}

/// In-memory store for sessions (create, get, append).
pub struct SessionStore {
    inner: Arc<RwLock<HashMap<SessionId, Session>>>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new session with a generated id; returns the session id.
    pub async fn create(&self) -> SessionId {
        let id = format!("sess-{}", uuid::Uuid::new_v4());
        let session = Session {
            id: id.clone(),
            messages: Vec::new(),
        };
        self.inner.write().await.insert(id.clone(), session);
        id
    }

    /// Create a session with the given id if it does not exist; returns the id.
    pub async fn get_or_create(&self, id: impl Into<SessionId>) -> SessionId {
        let id = id.into();
        if self.inner.read().await.contains_key(&id) {
            return id;
        }
        let session = Session {
            id: id.clone(),
            messages: Vec::new(),
        };
        self.inner.write().await.insert(id.clone(), session);
        id
    }

    /// Return a clone of the session if it exists.
    pub async fn get(&self, id: &str) -> Option<Session> {
        self.inner.read().await.get(id).cloned()
    }

    /// Append a message to the session; returns error if session not found.
    pub async fn append_message(
        &self,
        id: &str,
        role: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<(), String> {
        self.append_message_full(id, role, content, None, None).await
    }

    /// Append a message with optional tool_calls (assistant) or tool_name (tool result).
    pub async fn append_message_full(
        &self,
        id: &str,
        role: impl Into<String>,
        content: impl Into<String>,
        tool_calls: Option<Vec<crate::llm::ToolCall>>,
        tool_name: Option<String>,
    ) -> Result<(), String> {
        let mut g = self.inner.write().await;
        let session = g.get_mut(id).ok_or_else(|| "session not found".to_string())?;
        session.messages.push(SessionMessage {
            role: role.into(),
            content: content.into(),
            tool_calls,
            tool_name,
        });
        Ok(())
    }
}
