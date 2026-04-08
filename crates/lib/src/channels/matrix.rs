//! Matrix channel: thin wrapper around [`matrix_channel`] (matrix-sdk in a separate crate for optional builds).

use crate::channels::inbound::InboundMessage;
use crate::channels::registry::ChannelHandle;
use crate::config::Config;
use async_trait::async_trait;
use matrix_channel::MatrixInner;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Re-export for gateway routes and docs.
pub use matrix_channel::PendingMatrixVerification;

/// matrix-sdk SQLite + E2EE state: always `<profile_dir>/matrix`.
fn matrix_store_path(profile_dir: &Path) -> PathBuf {
    profile_dir.join("matrix")
}

fn resolve_device_id_for_token(config: &Config, whoami_device: Option<String>) -> Option<String> {
    whoami_device
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var("MATRIX_DEVICE_ID")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        .or_else(|| {
            config
                .channels
                .matrix
                .device_id
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Newtype around [`MatrixInner`] so this crate can implement [`ChannelHandle`] (orphan rule).
pub struct MatrixChannel(pub matrix_channel::MatrixInner);

impl Deref for MatrixChannel {
    type Target = MatrixInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Connect and build a [`MatrixChannel`], or [`None`] if Matrix is not configured.
pub async fn connect_matrix_client(config: &Config, profile_dir: &Path) -> Option<MatrixChannel> {
    let homeserver = std::env::var("MATRIX_HOMESERVER")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .channels
                .matrix
                .homeserver
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })?;
    let base = homeserver.trim_end_matches('/').to_string();
    let store_path = matrix_store_path(profile_dir);

    let token = std::env::var("MATRIX_ACCESS_TOKEN")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .channels
                .matrix
                .access_token
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });

    if let Some(t) = token {
        let user_id_from_config = std::env::var("MATRIX_USER_ID")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                config
                    .channels
                    .matrix
                    .user_id
                    .as_ref()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            });

        let whoami = matrix_channel::fetch_whoami(&base, &t).await;

        let user_id_str =
            user_id_from_config.or_else(|| whoami.as_ref().map(|w| w.user_id.clone()));
        let user_id_str = match user_id_str {
            Some(u) => u,
            None => {
                log::warn!("matrix: set MATRIX_USER_ID / channels.matrix.userId or use a token that allows whoami");
                return None;
            }
        };

        let whoami_device = whoami.and_then(|w| w.device_id);
        let device_id_str = match resolve_device_id_for_token(config, whoami_device) {
            Some(d) => d,
            None => {
                log::warn!(
                    "matrix: access token login needs a device id (whoami, or MATRIX_DEVICE_ID, or channels.matrix.deviceId)"
                );
                return None;
            }
        };

        let params = matrix_channel::MatrixConnectParams {
            homeserver: base,
            store_path,
            login: matrix_channel::MatrixLogin::AccessToken {
                token: t,
                user_id: user_id_str,
                device_id: device_id_str,
            },
        };
        let client = matrix_channel::connect_with_params(params).await?;
        let allowlist = crate::config::resolve_matrix_room_allowlist(config);
        return Some(MatrixChannel(MatrixInner::new(client, allowlist)));
    }

    let password = std::env::var("MATRIX_PASSWORD")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .channels
                .matrix
                .password
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })?;
    let user = std::env::var("MATRIX_USER")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .channels
                .matrix
                .user
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })?;

    let params = matrix_channel::password_login_params(base, store_path, user, password)?;
    let client = matrix_channel::connect_with_params(params).await?;
    let allowlist = crate::config::resolve_matrix_room_allowlist(config);
    Some(MatrixChannel(MatrixInner::new(client, allowlist)))
}

impl MatrixChannel {
    /// Sync loop and event handlers; forwards text `m.room.message` to the gateway.
    pub fn start_inbound(
        self: Arc<Self>,
        inbound_tx: mpsc::Sender<InboundMessage>,
    ) -> JoinHandle<()> {
        let (raw_tx, mut raw_rx) = mpsc::channel::<matrix_channel::RawInbound>(64);
        let inbound_tx2 = inbound_tx.clone();
        tokio::spawn(async move {
            while let Some(raw) = raw_rx.recv().await {
                let msg = InboundMessage {
                    channel_id: raw.channel_id,
                    conversation_id: raw.conversation_id,
                    text: raw.text,
                };
                if inbound_tx2.send(msg).await.is_err() {
                    break;
                }
            }
        });
        let inner = Arc::new(self.0.clone());
        MatrixInner::start_inbound(inner, raw_tx)
    }
}

#[async_trait]
impl ChannelHandle for MatrixChannel {
    fn id(&self) -> &str {
        self.0.id()
    }

    fn stop(&self) {
        self.0.stop();
    }

    async fn send_message(&self, conversation_id: &str, text: &str) -> Result<(), String> {
        self.0.send_message(conversation_id, text).await
    }

    async fn status_detail(&self) -> serde_json::Value {
        self.0.status_detail()
    }
}
