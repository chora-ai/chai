//! Telegram channel: long-poll getUpdates and sendMessage via Bot API.

use crate::channels::inbound::InboundMessage;
use crate::channels::registry::ChannelHandle;
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const TELEGRAM_API_BASE: &str = "https://api.telegram.org";
const LONG_POLL_TIMEOUT: u64 = 30;

#[derive(Debug, Deserialize)]
struct GetUpdatesResponse {
    ok: bool,
    #[serde(default)]
    result: Vec<TelegramUpdate>,
}

/// Telegram update payload (getUpdates result item or webhook POST body).
#[derive(Debug, Deserialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    #[serde(default)]
    pub message: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize)]
pub struct TelegramMessage {
    pub chat: TelegramChat,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
}

/// Telegram channel connector: long-polls for updates and sends replies via sendMessage.
pub struct TelegramChannel {
    id: String,
    token: Option<String>,
    running: AtomicBool,
    client: reqwest::Client,
}

impl TelegramChannel {
    pub fn new(token: Option<String>) -> Self {
        Self {
            id: "telegram".to_string(),
            token,
            running: AtomicBool::new(false),
            client: reqwest::Client::new(),
        }
    }

    fn running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Start the getUpdates long-poll loop and forward messages to the gateway. Returns a handle to await on shutdown.
    pub fn start_inbound(
        self: Arc<Self>,
        inbound_tx: mpsc::Sender<InboundMessage>,
    ) -> JoinHandle<()> {
        self.running.store(true, Ordering::SeqCst);
        log::info!("telegram channel: starting getUpdates long-poll loop");
        tokio::spawn(async move {
            run_get_updates_loop(self, inbound_tx).await;
        })
    }

    /// Call Telegram getUpdates (long poll). Returns (updates, next_offset).
    async fn get_updates(&self, offset: Option<i64>) -> Result<(Vec<TelegramUpdate>, Option<i64>), String> {
        let token = self.token.as_ref().ok_or("telegram bot token not configured")?;
        let url = format!(
            "{}/bot{}/getUpdates?timeout={}",
            TELEGRAM_API_BASE,
            token,
            LONG_POLL_TIMEOUT
        );
        let url = if let Some(off) = offset {
            format!("{}&offset={}", url, off)
        } else {
            url
        };
        let res = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("getUpdates failed: {} {}", status, body));
        }
        let data: GetUpdatesResponse = res.json().await.map_err(|e| e.to_string())?;
        if !data.ok {
            return Err("getUpdates returned ok: false".to_string());
        }
        let next_offset = data
            .result
            .iter()
            .map(|u| u.update_id)
            .max()
            .map(|id| id + 1);
        Ok((data.result, next_offset))
    }

    /// Set webhook URL (and optional secret). When set, Telegram POSTs updates to the URL instead of getUpdates.
    pub async fn set_webhook(&self, url: &str, secret: Option<&str>) -> Result<(), String> {
        let token = self
            .token
            .as_ref()
            .ok_or("telegram bot token not configured")?;
        let api_url = format!("{}/bot{}/setWebhook", TELEGRAM_API_BASE, token);
        let mut body = serde_json::json!({ "url": url });
        if let Some(s) = secret {
            body["secret_token"] = serde_json::Value::String(s.to_string());
        }
        let res = self
            .client
            .post(&api_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("setWebhook failed: {} {}", status, body));
        }
        Ok(())
    }

    /// Remove webhook so the bot can use getUpdates again.
    pub async fn delete_webhook(&self) -> Result<(), String> {
        let token = self
            .token
            .as_ref()
            .ok_or("telegram bot token not configured")?;
        let url = format!("{}/bot{}/deleteWebhook", TELEGRAM_API_BASE, token);
        let res = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("deleteWebhook failed: {} {}", status, body));
        }
        Ok(())
    }

    /// Send a text message to a chat via sendMessage API.
    pub async fn send_message(&self, chat_id: &str, text: &str) -> Result<(), String> {
        let token = self
            .token
            .as_ref()
            .ok_or("telegram bot token not configured")?;
        let url = format!("{}/bot{}/sendMessage", TELEGRAM_API_BASE, token);
        let body = serde_json::json!({ "chat_id": chat_id, "text": text });
        let res = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("sendMessage failed: {} {}", status, body));
        }
        Ok(())
    }
}

async fn run_get_updates_loop(channel: Arc<TelegramChannel>, inbound_tx: mpsc::Sender<InboundMessage>) {
    let mut offset: Option<i64> = None;
    while channel.running() {
        match channel.get_updates(offset).await {
            Ok((updates, next)) => {
                offset = next;
                for u in updates {
                    if let Some(ref msg) = u.message {
                        if let Some(ref text) = msg.text {
                            let chat_id = msg.chat.id.to_string();
                            let inbound = InboundMessage {
                                channel_id: channel.id.clone(),
                                conversation_id: chat_id,
                                text: text.clone(),
                            };
                            if inbound_tx.send(inbound).await.is_err() {
                                log::debug!("telegram: inbound channel closed, stopping loop");
                                return;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::debug!("telegram getUpdates error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }
    }
    log::info!("telegram channel: getUpdates loop stopped");
}

#[async_trait]
impl ChannelHandle for TelegramChannel {
    fn id(&self) -> &str {
        &self.id
    }

    fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    async fn send_message(&self, conversation_id: &str, text: &str) -> Result<(), String> {
        TelegramChannel::send_message(self, conversation_id, text).await
    }
}

/// Resolve Telegram bot API base URL (for tests or custom endpoints).
#[allow(dead_code)]
pub fn telegram_api_base() -> String {
    std::env::var("TELEGRAM_API_BASE").unwrap_or_else(|_| TELEGRAM_API_BASE.to_string())
}
