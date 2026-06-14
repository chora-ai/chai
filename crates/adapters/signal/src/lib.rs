//! Signal client for Chai: SSE receive loop and JSON-RPC send over a user-run signal-cli daemon.
//!
//! Depends only on `reqwest` + `futures-util` + `tokio` (no `lib` crate) so the workspace can
//! compile without Signal when `lib` disables the `signal` feature.

use futures_util::StreamExt;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const CHANNEL_ID: &str = "signal";

const STATUS_ERR_MAX: usize = 512;

/// Initial reconnect delay (seconds) for SSE and daemon-check failures.
const RECONNECT_BASE_SECS: u64 = 1;
/// Maximum reconnect delay cap (seconds).
const RECONNECT_MAX_SECS: u64 = 30;

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

/// Exponential backoff with jitter: `min(base * 2^attempt, max) ± jitter`.
fn reconnect_delay(attempt: u32) -> Duration {
    let exp = RECONNECT_BASE_SECS.saturating_mul(1u64.checked_shl(attempt).unwrap_or(u64::MAX));
    let capped = exp.min(RECONNECT_MAX_SECS);
    // Simple jitter: ±25% of capped value
    let jitter = (capped / 4).max(1);
    // Deterministic jitter based on attempt; no random crate needed.
    let offset = (attempt as u64 * 7) % (2 * jitter);
    let delay = capped + offset;
    Duration::from_secs(delay.min(RECONNECT_MAX_SECS))
}

/// Inbound payload for the gateway queue (same shape as `lib::InboundMessage`).
#[derive(Debug, Clone)]
pub struct RawInbound {
    pub channel_id: String,
    pub conversation_id: String,
    pub text: String,
}

/// Base URL of signal-cli HTTP daemon, e.g. `http://127.0.0.1:7583`.
#[derive(Clone)]
pub struct SignalDaemonConfig {
    pub http_base: String,
    /// When set, include in JSON-RPC `params` for multi-account daemon mode (`+E.164`).
    pub account: Option<String>,
}

/// Signal channel: SSE `/api/v1/events` for inbound `receive`, JSON-RPC `send` for replies.
#[derive(Clone)]
pub struct SignalInner {
    id: String,
    running: Arc<AtomicBool>,
    client: reqwest::Client,
    http_base: String,
    account: Option<String>,
    rpc_id: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<String>>>,
    daemon_check_ok: Arc<AtomicBool>,
}

impl SignalInner {
    pub fn new(cfg: SignalDaemonConfig) -> Self {
        Self {
            id: CHANNEL_ID.to_string(),
            running: Arc::new(AtomicBool::new(false)),
            client: reqwest::Client::new(),
            http_base: cfg.http_base,
            account: cfg.account,
            rpc_id: Arc::new(AtomicU64::new(1)),
            last_error: Arc::new(Mutex::new(None)),
            daemon_check_ok: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    fn next_rpc_id(&self) -> u64 {
        self.rpc_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Start SSE loop; forwards `receive` notifications with `dataMessage.message` text.
    pub fn start_inbound(
        self: Arc<Self>,
        inbound_tx: mpsc::Sender<RawInbound>,
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

    /// Sanitized snapshot for gateway `status.channels.signal` (no secrets).
    pub fn status_detail(&self) -> Value {
        json!({
            "transport": "sse",
            "daemonCheckOk": self.daemon_check_ok.load(Ordering::SeqCst),
            "lastError": self.last_error.lock().ok().and_then(|g| g.clone()),
        })
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

async fn run_events_loop(channel: Arc<SignalInner>, inbound_tx: mpsc::Sender<RawInbound>) {
    let events_url = format!("{}/api/v1/events", channel.http_base.trim_end_matches('/'));
    let mut attempt: u32 = 0;
    while channel.running.load(Ordering::SeqCst) {
        let res = match channel.client.get(&events_url).send().await {
            Ok(r) => r,
            Err(e) => {
                log::debug!("signal events connect error (attempt {}): {}", attempt + 1, e);
                set_signal_status_error(&channel.last_error, &format!("events connect: {}", e));
                let delay = reconnect_delay(attempt);
                log::debug!("signal: reconnecting in {:?}", delay);
                tokio::time::sleep(delay).await;
                attempt += 1;
                continue;
            }
        };
        if !res.status().is_success() {
            log::debug!(
                "signal events: bad status {} (attempt {})",
                res.status(),
                attempt + 1
            );
            set_signal_status_error(
                &channel.last_error,
                &format!("events stream: {}", res.status()),
            );
            let delay = reconnect_delay(attempt);
            log::debug!("signal: reconnecting in {:?}", delay);
            tokio::time::sleep(delay).await;
            attempt += 1;
            continue;
        }
        // Connection succeeded — reset backoff counter.
        attempt = 0;
        if let Ok(mut g) = channel.last_error.lock() {
            *g = None;
        }
        let mut stream = res.bytes_stream();
        let mut buf = String::new();
        while channel.running.load(Ordering::SeqCst) {
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
                        if let Some(inbound) = notification_to_raw_inbound(&v) {
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
        if !channel.running.load(Ordering::SeqCst) {
            break;
        }
        // Stream ended or errored — short delay before reconnecting (not a
        // connection failure, so use a small constant delay rather than the
        // exponential backoff path).
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    log::info!("signal channel: events loop stopped");
}

/// Map JSON-RPC `receive` notification to [`RawInbound`] if there is plain text for the agent.
///
/// Handles:
/// - Regular `dataMessage` with `message` text
/// - Edited messages via `dataMessage.message` (signal-cli sends the edited text in the same field)
/// - Attachments: when `dataMessage.attachments` is present, appends `[N attachment(s)]` to the text
///   so the agent is aware media was included even though it cannot be forwarded
fn notification_to_raw_inbound(v: &Value) -> Option<RawInbound> {
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

    // Resolve the text body. signal-cli sends edits in the same `message` field
    // with the updated content; no separate edit event type exists in the
    // JSON-RPC receive notification. The edit timestamp is available in
    // `dataMessage.timestamp` vs the original in the conversation, but both
    // arrive as regular `dataMessage` payloads.
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

    // Build final text, appending attachment count if present.
    let final_text = match data.get("attachments").and_then(|a| a.as_array()) {
        Some(attachments) if !attachments.is_empty() => {
            let count = attachments.len();
            format!("{} [{} attachment(s)]", trimmed, count)
        }
        _ => trimmed.to_string(),
    };

    let conversation_id = if let Some(gid) = data.get("groupId").and_then(|x| x.as_str()) {
        gid.to_string()
    } else {
        envelope
            .get("source")
            .or_else(|| envelope.get("sourceNumber"))?
            .as_str()?
            .to_string()
    };
    Some(RawInbound {
        channel_id: CHANNEL_ID.to_string(),
        conversation_id,
        text: final_text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_delay_increases_then_caps() {
        let d0 = reconnect_delay(0);
        let d1 = reconnect_delay(1);
        let d2 = reconnect_delay(2);
        assert!(d1 > d0, "backoff should increase: {:?} > {:?}", d1, d0);
        assert!(d2 > d1, "backoff should increase: {:?} > {:?}", d2, d1);
        // Cap at RECONNECT_MAX_SECS
        let d_max = reconnect_delay(20);
        assert!(
            d_max <= Duration::from_secs(RECONNECT_MAX_SECS),
            "should be capped at {:?}, got {:?}",
            Duration::from_secs(RECONNECT_MAX_SECS),
            d_max
        );
    }

    #[test]
    fn notification_with_attachment_appends_count() {
        let v = serde_json::json!({
            "method": "receive",
            "params": {
                "envelope": {
                    "source": "+1234567890",
                    "dataMessage": {
                        "message": "Here is a photo",
                        "attachments": [
                            { "contentType": "image/jpeg" },
                            { "contentType": "image/png" }
                        ]
                    }
                }
            }
        });
        let inbound = notification_to_raw_inbound(&v).unwrap();
        assert_eq!(inbound.text, "Here is a photo [2 attachment(s)]");
    }

    #[test]
    fn notification_without_attachment_unchanged() {
        let v = serde_json::json!({
            "method": "receive",
            "params": {
                "envelope": {
                    "source": "+1234567890",
                    "dataMessage": {
                        "message": "Hello"
                    }
                }
            }
        });
        let inbound = notification_to_raw_inbound(&v).unwrap();
        assert_eq!(inbound.text, "Hello");
    }

    #[test]
    fn notification_empty_attachment_array_no_suffix() {
        let v = serde_json::json!({
            "method": "receive",
            "params": {
                "envelope": {
                    "source": "+1234567890",
                    "dataMessage": {
                        "message": "Hello",
                        "attachments": []
                    }
                }
            }
        });
        let inbound = notification_to_raw_inbound(&v).unwrap();
        assert_eq!(inbound.text, "Hello");
    }

    #[test]
    fn notification_story_ignored() {
        let v = serde_json::json!({
            "method": "receive",
            "params": {
                "envelope": {
                    "source": "+1234567890",
                    "dataMessage": {
                        "message": "Story text",
                        "isStory": true
                    }
                }
            }
        });
        assert!(notification_to_raw_inbound(&v).is_none());
    }

    #[test]
    fn notification_group_message() {
        let v = serde_json::json!({
            "method": "receive",
            "params": {
                "envelope": {
                    "source": "+1234567890",
                    "dataMessage": {
                        "message": "Group hello",
                        "groupId": "abc123="
                    }
                }
            }
        });
        let inbound = notification_to_raw_inbound(&v).unwrap();
        assert_eq!(inbound.conversation_id, "abc123=");
        assert_eq!(inbound.text, "Group hello");
    }

    #[test]
    fn notification_sync_without_data_ignored() {
        let v = serde_json::json!({
            "method": "receive",
            "params": {
                "envelope": {
                    "source": "+1234567890",
                    "syncMessage": {
                        "sentMessage": {
                            "message": "sent from another device"
                        }
                    }
                }
            }
        });
        assert!(notification_to_raw_inbound(&v).is_none());
    }

    #[test]
    fn notification_non_receive_method_ignored() {
        let v = serde_json::json!({
            "method": "something_else",
            "params": {}
        });
        assert!(notification_to_raw_inbound(&v).is_none());
    }
}
