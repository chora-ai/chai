use std::sync::mpsc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use lib::orchestration::{
    EVENT_DELEGATE_COMPLETE, EVENT_DELEGATE_ERROR, EVENT_DELEGATE_REJECTED, EVENT_DELEGATE_START,
};

use super::super::{ChaiApp, ChatMessage, SessionEvent};

/// Last timeline row that is not an orchestration delegation line (used so RPC + WebSocket do not duplicate the same assistant turn).
pub(crate) fn last_non_delegation(messages: &[ChatMessage]) -> Option<&ChatMessage> {
    messages.iter().rev().find(|m| m.role != "delegation")
}

/// Same assistant turn as already shown (same content and tool_calls), ignoring delegation rows in between.
pub(crate) fn is_duplicate_assistant_row(
    prev: &ChatMessage,
    role: &str,
    content: &str,
    tool_calls: &Option<Vec<serde_json::Value>>,
) -> bool {
    role == "assistant"
        && prev.role == "assistant"
        && prev.content == content
        && prev.tool_calls.as_ref() == tool_calls.as_ref()
}

/// Human-readable line for a gateway `orchestration.delegate.*` payload.
fn format_delegation_line(event_name: &str, data: &serde_json::Value) -> String {
    let worker = data
        .get("workerId")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let provider = data.get("provider").and_then(|v| v.as_str()).unwrap_or("");
    let model = data.get("model").and_then(|v| v.as_str()).unwrap_or("");
    let pm = if provider.is_empty() && model.is_empty() {
        String::new()
    } else if model.is_empty() {
        provider.to_string()
    } else if provider.is_empty() {
        model.to_string()
    } else {
        format!("{} / {}", provider, model)
    };

    if event_name == EVENT_DELEGATE_START {
        let mut s = String::from("Delegation starting");
        if let Some(w) = worker {
            s.push_str(&format!(" · worker `{}`", w));
        }
        if !pm.is_empty() {
            s.push_str(&format!(" · {}", pm));
        }
        return s;
    }
    if event_name == EVENT_DELEGATE_COMPLETE {
        let n_calls = data
            .get("workerToolCalls")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let mut s = String::from("Delegation finished");
        if let Some(w) = worker {
            s.push_str(&format!(" · worker `{}`", w));
        }
        if !pm.is_empty() {
            s.push_str(&format!(" · {}", pm));
        }
        s.push_str(&format!(" · {} tool call(s)", n_calls));
        return s;
    }
    if event_name == EVENT_DELEGATE_ERROR {
        let err = data
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        let mut s = format!("Delegation failed: {}", err);
        if !pm.is_empty() {
            s.push_str(&format!(" ({})", pm));
        }
        return s;
    }
    if event_name == EVENT_DELEGATE_REJECTED {
        let reason = data
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("rejected");
        if reason == "max_delegations_per_turn" {
            let max = data
                .get("maxDelegationsPerTurn")
                .and_then(|v| v.as_u64())
                .map(|n| n.to_string())
                .unwrap_or_else(|| "?".to_string());
            let mut s = format!(
                "Delegation rejected: max delegations per turn exceeded (max {})",
                max
            );
            if let Some(w) = worker {
                s.push_str(&format!(" · worker `{}`", w));
            }
            return s;
        }
        if reason == "max_delegations_per_session" {
            return "Delegation rejected: max delegations per session reached".to_string();
        }
        if reason == "max_delegations_per_provider" {
            return "Delegation rejected: max delegations to this provider for the session reached"
                .to_string();
        }
        return format!("Delegation rejected: {}", reason);
    }
    format!("Delegation: {}", event_name)
}

impl ChaiApp {
    /// Move a session to the front of session_order (most recently active first).
    pub(crate) fn move_session_to_front(&mut self, session_id: &str) {
        self.session_order.retain(|id| id != session_id);
        self.session_order.insert(0, session_id.to_string());
    }

    /// Poll for session.message events from the gateway and update local session timelines.
    /// For all events we add them in gateway order (user → assistant); if the last message in the session has the same role and
    /// content (e.g. echo of our own turn from start_chat_turn + poll_chat_turn), we skip to avoid duplicate.
    pub(crate) fn poll_session_events(&mut self) {
        loop {
            let ev = match &self.session_events_receiver {
                Some(rx) => match rx.try_recv() {
                    Ok(e) => Some(e),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.session_events_receiver = None;
                        break;
                    }
                },
                None => break,
            };
            let ev = match ev {
                Some(e) => e,
                None => break,
            };
            let session_id = ev.session_id.clone();
            let entry = self
                .session_messages
                .entry(session_id.clone())
                .or_insert_with(Vec::new);
            // Skip duplicate user line (gateway echo after poll_chat_turn already prepended the same user for a new session).
            if ev.role == "user"
                && ev.delegation_event.is_none()
                && entry
                    .iter()
                    .any(|m| m.role == "user" && m.content == ev.content)
            {
                self.session_meta
                    .insert(session_id.clone(), (ev.channel_id, ev.conversation_id));
                self.move_session_to_front(&session_id);
                continue;
            }
            // Skip if this is a duplicate of the last message (e.g. echo of our own turn from start_chat_turn + poll_chat_turn).
            if let Some(last) = entry.last() {
                if last.role == ev.role && last.content == ev.content {
                    self.session_meta
                        .insert(session_id.clone(), (ev.channel_id, ev.conversation_id));
                    self.move_session_to_front(&session_id);
                    continue;
                }
            }
            // Assistant from session.message broadcast can duplicate the RPC reply when delegation
            // rows were appended in between (last != assistant). Match against last non-delegation row.
            if ev.role == "assistant" && ev.delegation_event.is_none() {
                if let Some(prev) = last_non_delegation(entry.as_slice()) {
                    if is_duplicate_assistant_row(prev, &ev.role, &ev.content, &ev.tool_calls) {
                        if let Some(existing) = entry.iter_mut().find(|m| {
                            m.role == "assistant"
                                && m.content == ev.content
                                && m.tool_calls == ev.tool_calls
                        }) {
                            let fill_results = existing
                                .tool_results
                                .as_ref()
                                .map(|v| v.is_empty())
                                .unwrap_or(true);
                            if fill_results {
                                if let Some(ref tr) = ev.tool_results {
                                    if !tr.is_empty() {
                                        existing.tool_results = Some(tr.clone());
                                    }
                                }
                            }
                        }
                        self.session_meta
                            .insert(session_id.clone(), (ev.channel_id, ev.conversation_id));
                        self.move_session_to_front(&session_id);
                        continue;
                    }
                }
            }
            entry.push(crate::app::ChatMessage {
                role: ev.role.clone(),
                content: ev.content.clone(),
                tool_calls: ev.tool_calls.clone(),
                tool_results: ev.tool_results.clone(),
                delegation_event: ev.delegation_event.clone(),
            });
            self.session_meta
                .insert(session_id.clone(), (ev.channel_id, ev.conversation_id));
            self.move_session_to_front(&session_id);
        }
    }

    /// Ensure the background session.events listener is running when the gateway is up.
    pub(crate) fn ensure_session_events_listener(&mut self, running: bool) {
        if !running {
            self.session_events_receiver = None;
            return;
        }
        // Only start listener if gateway is actually responding (not just starting)
        if self.session_events_receiver.is_none() && self.gateway_responds {
            let (tx, rx) = mpsc::channel();
            let tx_clone = tx.clone();
            std::thread::spawn(move || {
                // Wait a bit for gateway to be fully ready
                std::thread::sleep(Duration::from_secs(1));
                // Retry loop: if connection fails, wait a bit and retry
                let mut retry_count = 0;
                loop {
                    match run_session_events_loop(tx_clone.clone()) {
                        Err(e) => {
                            retry_count += 1;
                            // Exponential backoff, max 10 seconds
                            let delay = std::cmp::min(2_u64.pow(retry_count.min(3)), 10);
                            // Only log errors occasionally to avoid spam
                            if retry_count <= 3 || retry_count % 10 == 0 {
                                eprintln!(
                                    "session events listener error: {}, retrying in {}s (attempt {})",
                                    e, delay, retry_count
                                );
                            }
                            std::thread::sleep(Duration::from_secs(delay));
                        }
                        Ok(()) => {
                            // Normal exit (connection closed), reset retry count and wait before retry
                            retry_count = 0;
                            std::thread::sleep(Duration::from_secs(2));
                        }
                    }
                }
            });
            self.session_events_receiver = Some(rx);
        }
    }
}

/// Listen for session.message events from the gateway and forward them via an mpsc channel.
fn run_session_events_loop(tx: mpsc::Sender<SessionEvent>) -> Result<(), String> {
    let (config, paths) = lib::config::load_config(None).map_err(|e| e.to_string())?;
    let bind = config.gateway.bind.trim();
    let port = config.gateway.port;
    let token = lib::config::resolve_gateway_token(&config);
    let ws_url = format!("ws://{}:{}/ws", bind, port);

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let (mut ws, _) = match tokio_tungstenite::connect_async(&ws_url).await {
            Ok(pair) => pair,
            Err(e) => return Err(e.to_string()),
        };

        let first = ws
            .next()
            .await
            .ok_or("no first frame")?
            .map_err(|e| e.to_string())?;
        let Message::Text(challenge_text) = first else {
            return Err("expected text challenge frame".to_string());
        };
        let challenge: serde_json::Value =
            serde_json::from_str(&challenge_text).map_err(|e| e.to_string())?;
        let nonce = challenge
            .get("payload")
            .and_then(|p| p.get("nonce").and_then(|n| n.as_str()))
            .ok_or("expected connect.challenge event with nonce")?
            .to_string();

        let connect_params = if let Some(device_token) =
            lib::device::load_device_token_from(&paths.device_token_path())
        {
            serde_json::json!({ "auth": { "deviceToken": device_token } })
        } else {
            let identity = lib::device::DeviceIdentity::load(paths.device_json().as_path())
                .or_else(|| {
                    let id = lib::device::DeviceIdentity::generate().ok()?;
                    let _ = id.save(&paths.device_json());
                    Some(id)
                })
                .ok_or("failed to load or create device identity")?;
            let signed_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let token_str = token.as_deref().unwrap_or("");
            let scopes: Vec<String> = vec!["operator.read".into()];
            let payload_str = lib::device::build_connect_payload(
                &identity.device_id,
                "chai-desktop",
                "operator",
                "operator",
                &scopes,
                signed_at,
                token_str,
                &nonce,
            );
            let signature = identity.sign(&payload_str).map_err(|e| e.to_string())?;
            let mut params = serde_json::json!({
                "client": { "id": "chai-desktop", "mode": "operator" },
                "role": "operator",
                "scopes": scopes,
                "device": {
                    "id": identity.device_id,
                    "publicKey": identity.public_key,
                    "signature": signature,
                    "signedAt": signed_at,
                    "nonce": nonce
                }
            });
            if let Some(ref t) = token {
                params["auth"] = serde_json::json!({ "token": t });
            } else {
                params["auth"] = serde_json::json!({});
            }
            params
        };

        let connect_req = serde_json::json!({
            "type": "req",
            "id": "1",
            "method": "connect",
            "params": connect_params
        });
        ws.send(Message::Text(connect_req.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        // Wait for hello-ok with a timeout to avoid hanging indefinitely.
        let hello = tokio::select! {
            msg = ws.next() => msg,
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                return Err("hello-ok timeout".to_string());
            }
        };
        let hello = hello
            .ok_or("no hello-ok frame")?
            .map_err(|e| e.to_string())?;
        let Message::Text(hello_text) = hello else {
            return Err("expected text hello-ok frame".to_string());
        };
        let hello_val: serde_json::Value =
            serde_json::from_str(&hello_text).map_err(|e| e.to_string())?;
        if !hello_val
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return Err("hello-ok not ok".to_string());
        }
        // Persist device token from hello-ok, if provided, so future connects can
        // reuse it instead of regenerating device identity every time.
        if let Some(auth) = hello_val.get("payload").and_then(|p| p.get("auth")) {
            if let Some(dt) = auth.get("deviceToken").and_then(|v| v.as_str()) {
                let _ = lib::device::save_device_token_to(&paths.device_token_path(), dt);
            }
        }

        // Gateway broadcasts session.message (and other events) to all connected clients
        // after connect; no separate subscribe method exists.
        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            if let Message::Text(text) = msg {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if val.get("type") != Some(&serde_json::Value::String("event".into())) {
                        continue;
                    }
                    let Some(event_name) = val.get("event").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    if event_name == "session.message" {
                        if let Some(payload) = val.get("payload") {
                            // Session fields are in payload; support optional nested `data` for
                            // compatibility with older formats.
                            let data = payload.get("data").unwrap_or(payload);
                            let session_id_opt = data.get("sessionId").and_then(|v| v.as_str());
                            let role_opt = data.get("role").and_then(|v| v.as_str());
                            let content_opt = data.get("content").and_then(|v| v.as_str());

                            // Skip events missing any required field or with empty/whitespace-only values.
                            let (session_id, role, content) =
                                if let (Some(session_id), Some(role), Some(content)) =
                                    (session_id_opt, role_opt, content_opt)
                                {
                                    if session_id.trim().is_empty()
                                        || role.trim().is_empty()
                                        || content.trim().is_empty()
                                    {
                                        continue;
                                    }
                                    (
                                        session_id.to_string(),
                                        role.to_string(),
                                        content.to_string(),
                                    )
                                } else {
                                    continue;
                                };

                            let channel_id = data
                                .get("channelId")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let conversation_id = data
                                .get("conversationId")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let tool_calls = data
                                .get("toolCalls")
                                .and_then(|v| v.as_array())
                                .map(|arr| arr.clone());
                            let tool_results = data
                                .get("toolResults")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                        .collect::<Vec<_>>()
                                });
                            let tool_results = tool_results.filter(|v| !v.is_empty());
                            let ev = SessionEvent {
                                session_id,
                                role,
                                content,
                                channel_id,
                                conversation_id,
                                tool_calls,
                                tool_results,
                                delegation_event: None,
                            };
                            let _ = tx.send(ev);
                        }
                    } else if matches!(
                        event_name,
                        EVENT_DELEGATE_START
                            | EVENT_DELEGATE_COMPLETE
                            | EVENT_DELEGATE_ERROR
                            | EVENT_DELEGATE_REJECTED
                    ) {
                        if let Some(payload) = val.get("payload") {
                            let data = payload.get("data").unwrap_or(payload);
                            let Some(session_id) = data
                                .get("sessionId")
                                .and_then(|v| v.as_str())
                                .map(|s| s.trim())
                                .filter(|s| !s.is_empty())
                            else {
                                continue;
                            };
                            let content = format_delegation_line(event_name, data);
                            let ev = SessionEvent {
                                session_id: session_id.to_string(),
                                role: "delegation".to_string(),
                                content,
                                channel_id: None,
                                conversation_id: None,
                                tool_calls: None,
                                tool_results: None,
                                delegation_event: Some(event_name.to_string()),
                            };
                            let _ = tx.send(ev);
                        }
                    }
                }
            }
        }

        Ok(())
    })
}
