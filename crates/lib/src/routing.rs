//! Channel–session binding for routing: (channel_id, conversation_id) <-> session_id.
//!
//! Inbound: message from channel (e.g. Telegram chat) is routed to a session (get or create).
//! Outbound: reply for a session can be delivered to the bound channel/conversation.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Key for channel-side of the binding (channel id + conversation id, e.g. telegram chat_id).
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ChannelConvKey {
    pub channel_id: String,
    pub conversation_id: String,
}

/// In-memory store: (channel_id, conversation_id) <-> session_id (bidirectional).
pub struct SessionBindingStore {
    /// channel+conv -> session_id (inbound routing)
    to_session: Arc<RwLock<HashMap<ChannelConvKey, String>>>,
    /// session_id -> (channel_id, conversation_id) (outbound delivery)
    to_channel: Arc<RwLock<HashMap<String, ChannelConvKey>>>,
}

impl Default for SessionBindingStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionBindingStore {
    pub fn new() -> Self {
        Self {
            to_session: Arc::new(RwLock::new(HashMap::new())),
            to_channel: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Bind (channel_id, conversation_id) to session_id. Overwrites any existing binding for either side.
    pub async fn bind(
        &self,
        channel_id: impl Into<String>,
        conversation_id: impl Into<String>,
        session_id: impl Into<String>,
    ) {
        let channel_id = channel_id.into();
        let conversation_id = conversation_id.into();
        let session_id = session_id.into();
        let key = ChannelConvKey {
            channel_id: channel_id.clone(),
            conversation_id: conversation_id.clone(),
        };
        let mut to_session = self.to_session.write().await;
        let mut to_channel = self.to_channel.write().await;
        if let Some(old_key) = to_channel.get(&session_id).cloned() {
            to_session.remove(&old_key);
        }
        if let Some(old_session) = to_session.insert(key.clone(), session_id.clone()) {
            to_channel.remove(&old_session);
        }
        to_channel.insert(session_id, key);
    }

    /// Resolve session_id for a channel conversation (inbound).
    pub async fn get_session_id(
        &self,
        channel_id: &str,
        conversation_id: &str,
    ) -> Option<String> {
        let key = ChannelConvKey {
            channel_id: channel_id.to_string(),
            conversation_id: conversation_id.to_string(),
        };
        self.to_session.read().await.get(&key).cloned()
    }

    /// Resolve (channel_id, conversation_id) for a session (outbound).
    pub async fn get_channel_binding(&self, session_id: &str) -> Option<(String, String)> {
        self.to_channel
            .read()
            .await
            .get(session_id)
            .map(|k| (k.channel_id.clone(), k.conversation_id.clone()))
    }
}
