//! Signal channel: JSON-RPC over HTTP to a user-run signal-cli daemon (`daemon --http`).
//!
//! See `.agents/adr/SIGNAL_CLI_INTEGRATION.md` — BYO signal-cli; no bundling.

use crate::channels::inbound::InboundMessage;
use crate::channels::registry::ChannelHandle;
use crate::config::Config;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use futures_util::StreamExt;

const CHANNEL_ID: &str = "signal";

const STATUS_ERR_MAX: usize = 512;

fn set_signal_status_error(slot: &Mutex<Option<String>>, msg: &str) {
    let t = msg.trim();
    let s = if t.len() > STATUS_ERR_MAX {
        format!("{}…", &t[..STATUS_ERR_MAX])
    } else {
        t.to_string()
    };
    if let Ok(mut g) = slot.lock() {
        *g = Some(s);
    }
}

/// Base URL of signal-cli HTTP daemon, e.g. `http://127.0.0.1:7583`.
#[derive(Clone)]
pub struct SignalDaemonConfig {
    pub http_base: String,
    /// When set, include in JSON-RPC `params` for multi-account daemon mode (`+E.164`).
    pub account: Option<String>,
}

/// Resolve signal-cli HTTP base URL from env `SIGNAL_CLI_HTTP` or `channels.signal.httpBase`.
pub fn resolve_signal_daemon_config(config: &Config) -> Option<SignalDaemonConfig> {
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
    Some(SignalDaemonConfig {
        http_base: base,
        account,
    })
}

/// Signal channel: SSE `/api/v1/events` for inbound `receive`, JSON-RPC `send` for replies.
pub struct SignalChannel {
    id: String,
    running: AtomicBool,
    client: reqwest::Client,
    http_base: String,
    account: Option<String>,
    rpc_id: AtomicU64,
    last_error: Mutex<Option<String>>,
    daemon_check_ok: AtomicBool,
}

impl SignalChannel {
    pub fn new(cfg: SignalDaemonConfig) -> Self {
        Self {
            id: CHANNEL_ID.to_string(),
            running: AtomicBool::new(false),
            client: reqwest::Client::new(),
            http_base: cfg.http_base,
            account: cfg.account,
            rpc_id: AtomicU64::new(1),
            last_error: Mutex::new(None),
            daemon_check_ok: AtomicBool::new(false),
        }
    }

    fn running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn next_rpc_id(&self) -> u64 {
        self.rpc_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Start SSE loop; forwards `receive` notifications with `dataMessage.message` text.
    pub fn start_inbound(
        self: Arc<Self>,
        inbound_tx: mpsc::Sender<InboundMessage>,
    ) -> JoinHandle<()> {
        self.running.store(true, Ordering::SeqCst);
        log::info!("signal channel: starting SSE events loop");
        let c = self.clone();
        tokio::spawn(async move {
            let check_url = format!("{}/api/v1/check", c.http_base.trim_end_matches('/'));
            match c.client.get(&check_url).send().await {
                Ok(r) if r.status().is_success() => {
                    c.daemon_check_ok.store(true, Ordering::SeqCst);
                    if let Ok(mut g) = c.last_error.lock() {
                        *g = None;
                    }
                    log::info!("signal: daemon check ok at {}", check_url);
                }
                Ok(r) => {
                    c.daemon_check_ok.store(false, Ordering::SeqCst);
                    set_signal_status_error(
                        &c.last_error,
                        &format!("daemon check: {} from {}", r.status(), check_url),
                    );
                    log::warn!("signal: daemon check: {} from {}", r.status(), check_url);
                }
                Err(e) => {
                    c.daemon_check_ok.store(false, Ordering::SeqCst);
                    set_signal_status_error(&c.last_error, &format!("daemon check failed: {}", e));
                    log::warn!("signal: daemon check failed: {}", e);
                }
            }
            run_events_loop(c, inbound_tx).await;
        })
    }

    /// Send via JSON-RPC `send` (1:1 `recipient` or group `groupId`).
    pub async fn send_message(&self, conversation_id: &str, text: &str) -> Result<(), String> {
        let rpc_url = format!("{}/api/v1/rpc", self.http_base.trim_end_matches('/'));
        let mut params = build_send_params(conversation_id, text)?;
        if let Some(ref acc) = self.account {
            if let Value::Object(ref mut m) = params {
                m.insert("account".to_string(), json!(acc));
            }
        }
        let body = json!({
            "jsonrpc": "2.0",
            "method": "send",
            "params": params,
            "id": self.next_rpc_id(),
        });
        let res = self
            .client
            .post(&rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(format!("signal send rpc failed: {} {}", status, err_body));
        }
        let v: Value = res.json().await.map_err(|e| e.to_string())?;
        if let Some(err) = v.get("error") {
            return Err(format!("signal send error: {}", err));
        }
        Ok(())
    }
}

fn build_send_params(conversation_id: &str, text: &str) -> Result<Value, String> {
    let cid = conversation_id.trim();
    if cid.is_empty() {
        return Err("empty conversation id".to_string());
    }
    // E.164: `+` prefix; digits-only numbers are treated as phone (recipient).
    if cid.starts_with('+') || cid.chars().all(|c| c.is_ascii_digit()) {
        let phone = if cid.starts_with('+') {
            cid.to_string()
        } else {
            format!("+{}", cid)
        };
        return Ok(json!({
            "recipient": [phone],
            "message": text,
        }));
    }
    Ok(json!({
        "groupId": cid,
        "message": text,
    }))
}

async fn run_events_loop(channel: Arc<SignalChannel>, inbound_tx: mpsc::Sender<InboundMessage>) {
    let events_url = format!("{}/api/v1/events", channel.http_base.trim_end_matches('/'));
    while channel.running() {
        let res = match channel.client.get(&events_url).send().await {
            Ok(r) => r,
            Err(e) => {
                log::debug!("signal events connect error: {}", e);
                set_signal_status_error(&channel.last_error, &format!("events connect: {}", e));
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };
        if !res.status().is_success() {
            log::debug!("signal events: bad status {}", res.status());
            set_signal_status_error(
                &channel.last_error,
                &format!("events stream: {}", res.status()),
            );
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }
        let mut stream = res.bytes_stream();
        let mut buf = String::new();
        while channel.running() {
            let chunk = tokio::time::timeout(Duration::from_secs(90), stream.next()).await;
            match chunk {
                Ok(Some(Ok(bytes))) => {
                    buf.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(pos) = buf.find('\n') {
                        let line = buf[..pos].trim_end_matches('\r').to_string();
                        buf.drain(..pos + 1);
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        let payload = if let Some(rest) = line.strip_prefix("data:") {
                            rest.trim()
                        } else {
                            continue;
                        };
                        if payload.is_empty() {
                            continue;
                        }
                        let v: Value = match serde_json::from_str(payload) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        if let Some(inbound) = notification_to_inbound(&v) {
                            if inbound_tx.send(inbound).await.is_err() {
                                log::debug!("signal: inbound channel closed, stopping events");
                                return;
                            }
                        }
                    }
                }
                Ok(Some(Err(e))) => {
                    log::debug!("signal sse read error: {}", e);
                    break;
                }
                Ok(None) => {
                    log::debug!("signal sse stream ended");
                    break;
                }
                Err(_) => continue,
            }
        }
        if !channel.running() {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    log::info!("signal channel: events loop stopped");
}

/// Map JSON-RPC `receive` notification to [`InboundMessage`] if there is plain text for the agent.
fn notification_to_inbound(v: &Value) -> Option<InboundMessage> {
    if v.get("method").and_then(|m| m.as_str()) != Some("receive") {
        return None;
    }
    let params = v.get("params")?;
    let envelope = params
        .get("envelope")
        .or_else(|| params.get("result").and_then(|r| r.get("envelope")))?;
    // Skip sync-only envelope (no incoming data message text).
    if envelope.get("syncMessage").is_some() && envelope.get("dataMessage").is_none() {
        return None;
    }
    let data = envelope.get("dataMessage")?;
    let text = data.get("message")?.as_str()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if data
        .get("isStory")
        .and_then(|x| x.as_bool())
        .unwrap_or(false)
    {
        return None;
    }
    let conversation_id = if let Some(gid) = data.get("groupId").and_then(|x| x.as_str()) {
        gid.to_string()
    } else {
        envelope
            .get("source")
            .or_else(|| envelope.get("sourceNumber"))?
            .as_str()?
            .to_string()
    };
    Some(InboundMessage {
        channel_id: CHANNEL_ID.to_string(),
        conversation_id,
        text: trimmed.to_string(),
    })
}

#[async_trait]
impl ChannelHandle for SignalChannel {
    fn id(&self) -> &str {
        &self.id
    }

    fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    async fn send_message(&self, conversation_id: &str, text: &str) -> Result<(), String> {
        SignalChannel::send_message(self, conversation_id, text).await
    }

    async fn status_detail(&self) -> serde_json::Value {
        json!({
            "transport": "sse",
            "daemonCheckOk": self.daemon_check_ok.load(Ordering::SeqCst),
            "lastError": self.last_error.lock().ok().and_then(|g| g.clone()),
        })
    }
}
