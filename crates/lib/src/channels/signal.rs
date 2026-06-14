//! Signal channel: thin wrapper around [`signal_channel`] (signal-cli HTTP/SSE in a separate crate for optional builds).

use crate::channels::inbound::InboundMessage;
use crate::channels::registry::ChannelHandle;
use crate::config::Config;
use async_trait::async_trait;
use signal_channel::SignalInner;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Resolve signal-cli HTTP base URL from env `SIGNAL_CLI_HTTP` or `channels.signal.httpBase`.
pub fn resolve_signal_daemon_config(config: &Config) -> Option<signal_channel::SignalDaemonConfig> {
    let http_base = std::env::var("SIGNAL_CLI_HTTP")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .channels
                .signal
                .http_base
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })?;
    let base = http_base.trim_end_matches('/').to_string();
    let account = std::env::var("SIGNAL_CLI_ACCOUNT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .channels
                .signal
                .account
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    Some(signal_channel::SignalDaemonConfig {
        http_base: base,
        account,
    })
}

/// Newtype around [`SignalInner`] so this crate can implement [`ChannelHandle`] (orphan rule).
pub struct SignalChannel(pub signal_channel::SignalInner);

impl SignalChannel {
    pub fn new(cfg: signal_channel::SignalDaemonConfig) -> Self {
        Self(SignalInner::new(cfg))
    }

    /// SSE loop and event handlers; forwards text `receive` notifications to the gateway.
    pub fn start_inbound(
        self: Arc<Self>,
        inbound_tx: mpsc::Sender<InboundMessage>,
    ) -> JoinHandle<()> {
        let (raw_tx, mut raw_rx) = mpsc::channel::<signal_channel::RawInbound>(64);
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
        SignalInner::start_inbound(inner, raw_tx)
    }
}

#[async_trait]
impl ChannelHandle for SignalChannel {
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
