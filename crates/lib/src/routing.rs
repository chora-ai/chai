//! Channel–session binding for routing: (channel_id, conversation_id) <-> session_id.
//!
//! Inbound: message from channel (e.g. Telegram chat) is routed to a session (get or create).
//! Outbound: reply for a session can be delivered to the bound channel/conversation.
//!
//! When a `data_dir` is provided, bindings are persisted to `bindings.json`
//! in that directory. Write-through on every `bind()` and `remove_binding()`.
//! Loaded from disk at construction time via `with_data_dir()`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Key for channel-side of the binding (channel id + conversation id, e.g. telegram chat_id).
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChannelConvKey {
    pub channel_id: String,
    pub conversation_id: String,
}

/// A single binding record as stored in `bindings.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct BindingRecord {
    channel_id: String,
    conversation_id: String,
    session_id: String,
}

/// In-memory store: (channel_id, conversation_id) <-> session_id (bidirectional).
/// When `data_dir` is set, bindings are persisted to `bindings.json`.
pub struct SessionBindingStore {
    /// channel+conv -> session_id (inbound routing)
    to_session: Arc<RwLock<HashMap<ChannelConvKey, String>>>,
    /// session_id -> (channel_id, conversation_id) (outbound delivery)
    to_channel: Arc<RwLock<HashMap<String, ChannelConvKey>>>,
    /// Directory where `bindings.json` lives. `None` = in-memory only.
    data_dir: Option<PathBuf>,
}

impl Default for SessionBindingStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionBindingStore {
    /// Create an in-memory-only binding store (no disk I/O).
    pub fn new() -> Self {
        Self {
            to_session: Arc::new(RwLock::new(HashMap::new())),
            to_channel: Arc::new(RwLock::new(HashMap::new())),
            data_dir: None,
        }
    }

    /// Create a persistent binding store that writes to `data_dir/bindings.json`.
    /// Loads existing bindings from disk if the file exists.
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        let (to_session, to_channel) = Self::load_bindings_from_disk(&data_dir);
        Self {
            to_session: Arc::new(RwLock::new(to_session)),
            to_channel: Arc::new(RwLock::new(to_channel)),
            data_dir: Some(data_dir),
        }
    }

    /// Read `bindings.json` from disk and return populated maps.
    /// Missing file or corrupt file returns empty maps (corrupt file is logged).
    fn load_bindings_from_disk(
        dir: &PathBuf,
    ) -> (HashMap<ChannelConvKey, String>, HashMap<String, ChannelConvKey>) {
        let path = dir.join("bindings.json");
        if !path.exists() {
            return (HashMap::new(), HashMap::new());
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "could not read bindings file {}: {}",
                    path.display(),
                    e
                );
                return (HashMap::new(), HashMap::new());
            }
        };
        let records: Vec<BindingRecord> = match serde_json::from_str(&content) {
            Ok(r) => r,
            Err(e) => {
                log::warn!(
                    "corrupt bindings file {}, starting with empty bindings: {}",
                    path.display(),
                    e
                );
                return (HashMap::new(), HashMap::new());
            }
        };
        let mut to_session = HashMap::new();
        let mut to_channel = HashMap::new();
        for rec in records {
            let key = ChannelConvKey {
                channel_id: rec.channel_id,
                conversation_id: rec.conversation_id,
            };
            to_session.insert(key.clone(), rec.session_id.clone());
            to_channel.insert(rec.session_id, key);
        }
        (to_session, to_channel)
    }

    /// Persist the current in-memory bindings to `bindings.json` (atomic: .tmp then rename).
    async fn persist_to_disk(&self) {
        let Some(ref dir) = self.data_dir else {
            return;
        };
        let to_session = self.to_session.read().await;
        let records: Vec<BindingRecord> = to_session
            .iter()
            .map(|(key, session_id)| BindingRecord {
                channel_id: key.channel_id.clone(),
                conversation_id: key.conversation_id.clone(),
                session_id: session_id.clone(),
            })
            .collect();
        drop(to_session);

        let path = dir.join("bindings.json");
        let tmp_path = dir.join("bindings.json.tmp");
        match serde_json::to_string_pretty(&records) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&tmp_path, &json) {
                    log::warn!(
                        "failed to write bindings tmp file {}: {}",
                        tmp_path.display(),
                        e
                    );
                    return;
                }
                if let Err(e) = std::fs::rename(&tmp_path, &path) {
                    log::warn!(
                        "failed to rename bindings file {} -> {}: {}",
                        tmp_path.display(),
                        path.display(),
                        e
                    );
                }
            }
            Err(e) => {
                log::warn!("failed to serialize bindings: {}", e);
            }
        }
    }

    /// Bind (channel_id, conversation_id) to session_id. Overwrites any existing binding for either side.
    /// Writes to disk if `data_dir` is set.
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
        drop(to_session);
        drop(to_channel);
        self.persist_to_disk().await;
    }

    /// Remove a binding by session_id from both in-memory maps.
    /// Rewrites `bindings.json` to disk if `data_dir` is set.
    pub async fn remove_binding(&self, session_id: &str) {
        let mut to_session = self.to_session.write().await;
        let mut to_channel = self.to_channel.write().await;
        if let Some(key) = to_channel.remove(session_id) {
            to_session.remove(&key);
        }
        drop(to_session);
        drop(to_channel);
        self.persist_to_disk().await;
    }

    /// Resolve session_id for a channel conversation (inbound).
    pub async fn get_session_id(&self, channel_id: &str, conversation_id: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn channel_conv_key_serialization_round_trip() {
        let key = ChannelConvKey {
            channel_id: "telegram".to_string(),
            conversation_id: "123456".to_string(),
        };
        let json = serde_json::to_string(&key).expect("serialize");
        let back: ChannelConvKey = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.channel_id, "telegram");
        assert_eq!(back.conversation_id, "123456");
    }

    #[tokio::test]
    async fn binding_store_persist_and_reload() {
        let dir = TempDir::new().unwrap();

        // Store 1: bind and persist.
        {
            let store = SessionBindingStore::with_data_dir(dir.path().to_path_buf());
            store
                .bind("telegram", "123", "sess-abc")
                .await;
        }

        // Store 2: load from disk.
        let store2 = SessionBindingStore::with_data_dir(dir.path().to_path_buf());
        let sid = store2
            .get_session_id("telegram", "123")
            .await
            .expect("should find binding");
        assert_eq!(sid, "sess-abc");

        let (ch, conv) = store2
            .get_channel_binding("sess-abc")
            .await
            .expect("should find reverse binding");
        assert_eq!(ch, "telegram");
        assert_eq!(conv, "123");
    }

    #[tokio::test]
    async fn binding_store_missing_file_starts_empty() {
        let dir = TempDir::new().unwrap();
        let store = SessionBindingStore::with_data_dir(dir.path().to_path_buf());
        assert!(store.get_session_id("telegram", "123").await.is_none());
    }

    #[tokio::test]
    async fn binding_store_corrupt_file_starts_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bindings.json");
        std::fs::write(&path, "not json").expect("write corrupt file");

        let store = SessionBindingStore::with_data_dir(dir.path().to_path_buf());
        assert!(store.get_session_id("telegram", "123").await.is_none());
    }

    #[tokio::test]
    async fn binding_store_remove_binding() {
        let dir = TempDir::new().unwrap();
        let store = SessionBindingStore::with_data_dir(dir.path().to_path_buf());
        store
            .bind("telegram", "123", "sess-abc")
            .await;
        store.remove_binding("sess-abc").await;

        assert!(store.get_session_id("telegram", "123").await.is_none());
        assert!(store.get_channel_binding("sess-abc").await.is_none());

        // Verify persisted after removal.
        let store2 = SessionBindingStore::with_data_dir(dir.path().to_path_buf());
        assert!(store2.get_session_id("telegram", "123").await.is_none());
    }

    #[tokio::test]
    async fn binding_store_bind_overwrites() {
        let dir = TempDir::new().unwrap();
        let store = SessionBindingStore::with_data_dir(dir.path().to_path_buf());
        store
            .bind("telegram", "123", "sess-old")
            .await;
        store
            .bind("telegram", "123", "sess-new")
            .await;

        let sid = store.get_session_id("telegram", "123").await.unwrap();
        assert_eq!(sid, "sess-new");
        assert!(store.get_channel_binding("sess-old").await.is_none());
    }
}
