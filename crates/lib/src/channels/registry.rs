//! Channel registry: register and lookup channels by id.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Handle to a running channel (stop, send message).
#[async_trait]
pub trait ChannelHandle: Send + Sync {
    /// Channel id (e.g. "telegram").
    fn id(&self) -> &str;
    /// Stop the channel connector.
    fn stop(&self);
    /// Send a text message to a conversation (e.g. Telegram chat_id). Default returns error.
    async fn send_message(&self, _conversation_id: &str, _text: &str) -> Result<(), String> {
        Err("send not implemented".to_string())
    }
}

/// Registry of channel ids to handles. Shared across gateway.
pub struct ChannelRegistry {
    inner: Arc<RwLock<HashMap<String, Arc<dyn ChannelHandle>>>>,
}

impl Default for ChannelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, id: String, handle: Arc<dyn ChannelHandle>) {
        let mut g = self.inner.write().await;
        if let Some(old) = g.insert(id.clone(), handle) {
            old.stop();
        }
    }

    pub async fn get(&self, id: &str) -> Option<Arc<dyn ChannelHandle>> {
        let g = self.inner.read().await;
        g.get(id).cloned()
    }

    pub async fn ids(&self) -> Vec<String> {
        let g = self.inner.read().await;
        g.keys().cloned().collect()
    }
}
