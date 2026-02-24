//! Pairing store: persisted device IDs and device tokens for connect auth.
//!
//! When a device connects with valid signing and is not yet paired, the gateway can
//! auto-approve (e.g. when gateway token was provided) and issue a device token,
//! stored here and returned in hello-ok.auth.deviceToken.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::sync::RwLock;

/// One paired device: deviceId, role, scopes, and the issued device token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedEntry {
    pub device_id: String,
    pub role: String,
    pub scopes: Vec<String>,
    pub device_token: String,
}

/// In-memory store of paired devices; can load/save from a JSON file.
pub struct PairingStore {
    path: std::path::PathBuf,
    entries: RwLock<Vec<PairedEntry>>,
}

impl PairingStore {
    /// Load store from path; if file missing or invalid, starts empty.
    pub async fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let entries = match tokio::fs::read_to_string(&path).await {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| Vec::new()),
            Err(_) => Vec::new(),
        };
        Self {
            path,
            entries: RwLock::new(entries),
        }
    }

    async fn save(&self) -> std::io::Result<()> {
        let entries = self.entries.read().await;
        let json = serde_json::to_string_pretty(&*entries).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&self.path, json).await
    }

    /// Look up by device ID. Returns the entry if found.
    pub async fn get_by_device_id(&self, device_id: &str) -> Option<PairedEntry> {
        let entries = self.entries.read().await;
        entries.iter().find(|e| e.device_id == device_id).cloned()
    }

    /// Look up by device token. Returns the entry if found.
    pub async fn get_by_token(&self, token: &str) -> Option<PairedEntry> {
        let entries = self.entries.read().await;
        entries.iter().find(|e| e.device_token == token).cloned()
    }

    /// Add or replace entry for this device_id and persist to disk.
    pub async fn add_or_update(&self, device_id: String, role: String, scopes: Vec<String>, device_token: String) -> anyhow::Result<()> {
        let mut entries = self.entries.write().await;
        if let Some(e) = entries.iter_mut().find(|e| e.device_id == device_id) {
            e.role = role;
            e.scopes = scopes;
            e.device_token = device_token.clone();
        } else {
            entries.push(PairedEntry {
                device_id,
                role,
                scopes,
                device_token,
            });
        }
        drop(entries);
        self.save().await.map_err(anyhow::Error::from)
    }
}
