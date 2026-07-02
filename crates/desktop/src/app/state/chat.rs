use std::sync::mpsc;
use std::time::Duration;

use eframe::egui;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use lib::orchestration::{
    EVENT_DELEGATE_COMPLETE, EVENT_DELEGATE_ERROR, EVENT_DELEGATE_REJECTED, EVENT_DELEGATE_START,
};

use super::super::{ChannelBinding, ChaiApp, ChatMessage, SessionEvent, SessionSummary};

/// Last timeline row that is not an orchestration delegation line, tool event, or assistant thinking row
/// (used so RPC + WebSocket do not duplicate the same assistant turn).
pub(crate) fn last_non_delegation(messages: &[ChatMessage]) -> Option<&ChatMessage> {
    messages.iter().rev().find(|m| !matches!(m.role.as_str(), "delegation" | "tool_call" | "tool_result" | "assistant_progress" | "tool_loop_limit" | "turn_stopped"))
}

/// Same assistant turn as already shown (same content), ignoring delegation rows in between.
pub(crate) fn is_duplicate_assistant_row(
    prev: &ChatMessage,
    role: &str,
    content: &str,
    _tool_calls: &Option<Vec<serde_json::Value>>,
) -> bool {
    role == "assistant"
        && prev.role == "assistant"
        && prev.content == content
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
        if reason == "max_delegations_per_worker" {
            return "Delegation rejected: max delegations to this worker for the session reached"
                .to_string();
        }
        return format!("Delegation rejected: {}", reason);
    }
    format!("Delegation: {}", event_name)
}

/// Compute the index of the first message in the current turn (after the last
/// user message). Tool indices reset per turn, so scoping to the current turn
/// prevents false matches against previous turns.
fn turn_start_index(entry: &[ChatMessage]) -> usize {
    entry
        .iter()
        .rposition(|m| m.role == "user")
        .map(|i| i + 1)
        .unwrap_or(0)
}

/// Check whether any streamed `tool_call` entries exist in the current turn.
pub(crate) fn has_streamed_tools_this_turn(entry: &[ChatMessage]) -> bool {
    let turn_start = turn_start_index(entry);
    entry[turn_start..].iter().any(|m| m.role == "tool_call")
}

impl ChaiApp {
    /// Move a session to the front of session_order (most recently active first).
    pub(crate) fn move_session_to_front(&mut self, session_id: &str) {
        let gw = self.gw();
        gw.session_order.retain(|id| id != session_id);
        gw.session_order.insert(0, session_id.to_string());
    }

    /// Update channel metadata for a session in `session_summaries`.
    pub(crate) fn update_session_channel_meta(
        &mut self,
        session_id: &str,
        channel_id: Option<String>,
        conversation_id: Option<String>,
    ) {
        let gw = self.gw();
        let summary = gw
            .session_summaries
            .entry(session_id.to_string())
            .or_insert_with(|| {
                let now = super::super::now_iso8601();
                SessionSummary {
                    id: session_id.to_string(),
                    created_at: now.clone(),
                    updated_at: now,
                    ..Default::default()
                }
            });
        if channel_id.is_some() || conversation_id.is_some() {
            summary.channel_binding = Some(ChannelBinding {
                channel_id: channel_id.unwrap_or_default(),
                conversation_id: conversation_id.unwrap_or_default(),
            });
        }
    }

    /// Poll for session.message events from the gateway and update local session timelines.
    pub(crate) fn poll_session_events(&mut self) {
        loop {
            let ev = {
                let gw = self.gw_ref();
                match &gw.session_events_receiver {
                    Some(rx) => match rx.try_recv() {
                        Ok(e) => Some(e),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            drop(gw);
                            self.gw().session_events_receiver = None;
                            break;
                        }
                    },
                    None => break,
                }
            };
            let ev = match ev {
                Some(e) => e,
                None => break,
            };
            // Handle gateway config-changed events.
            if ev.role == "config_changed" {
                self.request_status_refetch();
                continue;
            }
            // Handle session deletion events.
            if ev.role == "session_deleted" {
                let should_handle = ev.orchestrator_id.as_deref() == self.gw_ref().active_orchestrator_id.as_deref()
                    || ev.orchestrator_id.is_none();
                if should_handle {
                    let sid = ev.session_id.clone();
                    if !sid.is_empty() {
                        self.remove_session_local(&sid);
                    }
                }
                continue;
            }
            // Handle sessions-cleared events.
            if ev.role == "sessions_cleared" {
                let should_clear = ev.orchestrator_id.as_deref() == self.gw_ref().active_orchestrator_id.as_deref()
                    || ev.orchestrator_id.is_none();
                if should_clear {
                    let gw = self.gw();
                    gw.session_messages.clear();
                    gw.session_summaries.clear();
                    gw.session_order.clear();
                    self.start_new_session();
                    gw.chat_session_id = None;
                }
                continue;
            }
            // Filter session events by active orchestrator.
            if let Some(ref ev_oid) = ev.orchestrator_id {
                if Some(ev_oid.as_str()) != self.gw_ref().active_orchestrator_id.as_deref() {
                    continue;
                }
            }
            let session_id = ev.session_id.clone();
            let gw = self.gw();
            let entry = gw
                .session_messages
                .entry(session_id.clone())
                .or_insert_with(Vec::new);
            // When the first streamed event arrives for a new session while the RPC is
            // still in flight, bind chat_session_id and selected_session_id immediately.
            if gw.chat_session_id.is_none() && gw.pending_user_message.is_some() {
                gw.chat_session_id = Some(session_id.clone());
                gw.selected_session_id = Some(session_id.clone());
                if let Some(ref user_content) = gw.pending_user_message {
                    let already = entry
                        .iter()
                        .any(|m| m.role == "user" && m.content == *user_content);
                    if !already {
                        entry.insert(0, crate::app::ChatMessage::user(user_content.clone()));
                    }
                }
                gw.chat_messages = entry.clone();
                log::debug!(
                    "poll_session_events: bound new session_id={}, selected_session_id={}",
                    session_id,
                    session_id
                );
            }
            // Skip duplicate user line.
            if ev.role == "user"
                && ev.delegation_event.is_none()
                && entry
                    .iter()
                    .any(|m| m.role == "user" && m.content == ev.content)
            {
                drop(gw);
                self.update_session_channel_meta(&session_id, ev.channel_id.clone(), ev.conversation_id.clone());
                self.move_session_to_front(&session_id);
                continue;
            }
            // Handle streamed tool events.
            if ev.role == "tool_call" {
                let turn_start = turn_start_index(entry);
                let is_dup = entry[turn_start..].iter().any(|m| {
                    m.role == "tool_call"
                        && m.tool_index == ev.tool_index
                        && m.tool_name == ev.tool_name
                        && m.source == ev.source
                });
                if !is_dup {
                    entry.push(crate::app::ChatMessage {
                        role: ev.role.clone(),
                        content: ev.content.clone(),
                        tool_calls: ev.tool_calls.clone(),
                        tool_results: ev.tool_results.clone(),
                        delegation_event: ev.delegation_event.clone(),
                        tool_name: ev.tool_name.clone(),
                        tool_args: ev.tool_args.clone(),
                        tool_result: None,
                        tool_index: ev.tool_index,
                        source: ev.source.clone(),
                        pending_tool_calls: ev.pending_tool_calls.clone(),
                    });
                }
                drop(gw);
                self.update_session_channel_meta(&session_id, ev.channel_id.clone(), ev.conversation_id.clone());
                self.move_session_to_front(&session_id);
                continue;
            }
            if ev.role == "tool_result" {
                if let Some(idx) = ev.tool_index {
                    let turn_start = turn_start_index(entry);
                    let found = entry[turn_start..].iter_mut().rev().find(|m| {
                        m.role == "tool_call"
                            && m.tool_index == Some(idx)
                            && m.tool_name == ev.tool_name
                            && m.source == ev.source
                    });
                    if let Some(tc) = found {
                        tc.tool_result = ev.tool_result.clone();
                        drop(gw);
                        self.update_session_channel_meta(&session_id, ev.channel_id.clone(), ev.conversation_id.clone());
                        self.move_session_to_front(&session_id);
                        continue;
                    }
                }
                entry.push(crate::app::ChatMessage {
                    role: ev.role.clone(),
                    content: ev.content.clone(),
                    tool_calls: ev.tool_calls.clone(),
                    tool_results: ev.tool_results.clone(),
                    delegation_event: ev.delegation_event.clone(),
                    tool_name: ev.tool_name.clone(),
                    tool_args: None,
                    tool_result: ev.tool_result.clone(),
                    tool_index: ev.tool_index,
                    source: ev.source.clone(),
                    pending_tool_calls: ev.pending_tool_calls.clone(),
                });
                drop(gw);
                self.update_session_channel_meta(&session_id, ev.channel_id.clone(), ev.conversation_id.clone());
                self.move_session_to_front(&session_id);
                continue;
            }
            // Handle tool loop limit event.
            if ev.role == "tool_loop_limit" {
                let pending = ev.pending_tool_calls.clone().unwrap_or_default();
                let msg = crate::app::ChatMessage::tool_loop_limit(
                    "tool loop iteration limit reached",
                    pending,
                );
                entry.push(msg);
                drop(gw);
                self.update_session_channel_meta(&session_id, ev.channel_id.clone(), ev.conversation_id.clone());
                self.move_session_to_front(&session_id);
                continue;
            }
            // Handle turn stopped event.
            if ev.role == "turn_stopped" {
                let last_user_idx = entry.iter().rposition(|m| m.role == "user");
                let already_has_banner = entry.iter().skip(last_user_idx.unwrap_or(0)).any(|m| m.role == "turn_stopped");
                if !already_has_banner {
                    entry.push(crate::app::ChatMessage::turn_stopped());
                }
                self.gw().chat_stopping = false;
                drop(gw);
                self.update_session_channel_meta(&session_id, ev.channel_id.clone(), ev.conversation_id.clone());
                self.move_session_to_front(&session_id);
                continue;
            }
            // Skip if this is a duplicate of the last message.
            if let Some(last) = entry.last() {
                if last.role == ev.role && last.content == ev.content {
                    drop(gw);
                    self.update_session_channel_meta(&session_id, ev.channel_id.clone(), ev.conversation_id.clone());
                    self.move_session_to_front(&session_id);
                    continue;
                }
            }
            // Assistant dedup logic.
            if ev.role == "assistant" && ev.delegation_event.is_none() {
                let has_loop_limit = entry.iter().any(|m| m.role == "tool_loop_limit");
                let has_turn_stopped = entry.iter().any(|m| m.role == "turn_stopped");
                if (has_loop_limit || has_turn_stopped)
                    && entry.iter().any(|m| {
                        m.role == "assistant_progress" && m.content == ev.content
                    })
                {
                    drop(gw);
                    self.update_session_channel_meta(&session_id, ev.channel_id.clone(), ev.conversation_id.clone());
                    self.move_session_to_front(&session_id);
                    continue;
                }
                let has_streamed_tools = has_streamed_tools_this_turn(entry);
                if let Some(prev) = last_non_delegation(entry.as_slice()) {
                    if is_duplicate_assistant_row(prev, &ev.role, &ev.content, &ev.tool_calls) {
                        if let Some(existing) = entry.iter_mut().find(|m| {
                            m.role == "assistant"
                                && m.content == ev.content
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
                            if has_streamed_tools {
                                existing.tool_calls = None;
                                existing.tool_results = None;
                            }
                        }
                        drop(gw);
                        self.update_session_channel_meta(&session_id, ev.channel_id.clone(), ev.conversation_id.clone());
                        self.move_session_to_front(&session_id);
                        continue;
                    }
                }
            }
            // Push the event as a chat message.
            let mut ev_msg = crate::app::ChatMessage {
                role: ev.role.clone(),
                content: ev.content.clone(),
                tool_calls: ev.tool_calls.clone(),
                tool_results: ev.tool_results.clone(),
                delegation_event: ev.delegation_event.clone(),
                tool_name: ev.tool_name.clone(),
                tool_args: ev.tool_args.clone(),
                tool_result: ev.tool_result.clone(),
                tool_index: ev.tool_index,
                source: ev.source.clone(),
                pending_tool_calls: ev.pending_tool_calls.clone(),
            };
            if ev.role == "assistant" && has_streamed_tools_this_turn(entry) {
                ev_msg.tool_calls = None;
                ev_msg.tool_results = None;
            }
            entry.push(ev_msg);
            drop(gw);
            self.update_session_channel_meta(&session_id, ev.channel_id.clone(), ev.conversation_id.clone());
            self.move_session_to_front(&session_id);
        }
    }

    /// Ensure the background session.events listener is running when the gateway is up.
    pub(crate) fn ensure_session_events_listener(&mut self, running: bool, ctx: egui::Context) {
        if !running {
            self.gw().session_events_receiver = None;
            return;
        }
        // Only start listener if gateway is actually responding (not just starting)
        let should_start = self.gw_ref().session_events_receiver.is_none() && self.gw_ref().responds;
        if should_start {
            let (tx, rx) = mpsc::channel();
            let tx_clone = tx.clone();
            let profile_override = self.cached_gateway_profile.clone();
            std::thread::spawn(move || {
                // Wait a bit for gateway to be fully ready
                std::thread::sleep(Duration::from_secs(1));
                // Retry loop: if connection fails, wait a bit and retry
                let mut retry_count = 0;
                loop {
                    match run_session_events_loop(tx_clone.clone(), ctx.clone(), profile_override.as_deref()) {
                        Err(e) => {
                            retry_count += 1;
                            let delay = std::cmp::min(2_u64.pow(retry_count.min(3)), 10);
                            if retry_count <= 3 || retry_count % 10 == 0 {
                                log::error!(
                                    "session events listener error: {}, retrying in {}s (attempt {})",
                                    e, delay, retry_count
                                );
                            }
                            std::thread::sleep(Duration::from_secs(delay));
                        }
                        Ok(()) => {
                            retry_count = 0;
                            std::thread::sleep(Duration::from_secs(2));
                        }
                    }
                }
            });
            self.gw().session_events_receiver = Some(rx);
        }
    }
}

/// Listen for session.message events from the gateway and forward them via an mpsc channel.
fn run_session_events_loop(tx: mpsc::Sender<SessionEvent>, ctx: egui::Context, profile_override: Option<&str>) -> Result<(), String> {
    let (config, paths) = lib::config::load_config(profile_override).map_err(|e| e.to_string())?;
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

        let connect_params = super::gateway::build_connect_params(&paths, token.as_deref(), &nonce)?;
        let connect_req = serde_json::json!({
            "type": "req",
            "id": "1",
            "method": "connect",
            "params": connect_params
        });
        ws.send(Message::Text(connect_req.to_string().into()))
            .await
            .map_err(|e| e.to_string())?;

        // Wait for hello-ok with a timeout.
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
            let err = hello_val
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("hello-ok not ok");
            if err == "invalid device token" {
                let _ = std::fs::remove_file(paths.device_token_path());
            }
            return Err(err.to_string());
        }
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
                            let data = payload.get("data").unwrap_or(payload);
                            let session_id_opt = data.get("sessionId").and_then(|v| v.as_str());
                            let role_opt = data.get("role").and_then(|v| v.as_str());
                            let content_opt = data.get("content").and_then(|v| v.as_str());

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
                            let event_orchestrator_id = data
                                .get("orchestratorId")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let ev = SessionEvent {
                                session_id,
                                role,
                                content,
                                channel_id,
                                conversation_id,
                                tool_calls,
                                tool_results,
                                delegation_event: None,
                                tool_name: None,
                                tool_args: None,
                                tool_result: None,
                                tool_index: None,
                                source: None,
                                pending_tool_calls: None,
                                orchestrator_id: event_orchestrator_id,
                            };
                            let _ = tx.send(ev);
                            ctx.request_repaint();
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
                            let event_orchestrator_id = data
                                .get("orchestratorId")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let content = format_delegation_line(event_name, data);

                            if event_name == EVENT_DELEGATE_COMPLETE {
                                if let Some(reply) = data
                                    .get("reply")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.trim())
                                    .filter(|s| !s.is_empty())
                                {
                                    let worker_id = data
                                        .get("workerId")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.trim())
                                        .filter(|s| !s.is_empty())
                                        .unwrap_or("worker");
                                    let worker_ev = SessionEvent {
                                        session_id: session_id.to_string(),
                                        role: "worker".to_string(),
                                        content: reply.to_string(),
                                        channel_id: None,
                                        conversation_id: None,
                                        tool_calls: None,
                                        tool_results: None,
                                        delegation_event: None,
                                        tool_name: None,
                                        tool_args: None,
                                        tool_result: None,
                                        tool_index: None,
                                        source: Some(worker_id.to_string()),
                                        pending_tool_calls: None,
                                        orchestrator_id: event_orchestrator_id.clone(),
                                    };
                                    let _ = tx.send(worker_ev);
                                }
                            }

                            let ev = SessionEvent {
                                session_id: session_id.to_string(),
                                role: "delegation".to_string(),
                                content,
                                channel_id: None,
                                conversation_id: None,
                                tool_calls: None,
                                tool_results: None,
                                delegation_event: Some(event_name.to_string()),
                                tool_name: None,
                                tool_args: None,
                                tool_result: None,
                                tool_index: None,
                                source: None,
                                pending_tool_calls: None,
                                orchestrator_id: event_orchestrator_id,
                            };
                            let _ = tx.send(ev);
                            ctx.request_repaint();
                        }
                    } else if event_name == "session.tool_call" || event_name == "session.tool_result" {
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
                            let tool_name = data
                                .get("toolName")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let tool_args = data.get("toolArgs").cloned();
                            let tool_result = data
                                .get("toolResult")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let tool_index = data
                                .get("index")
                                .and_then(|v| v.as_u64())
                                .map(|i| i as usize);
                            let source = data
                                .get("source")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let event_orchestrator_id = data
                                .get("orchestratorId")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let role = if event_name == "session.tool_call" {
                                "tool_call"
                            } else {
                                "tool_result"
                            };
                            let ev = SessionEvent {
                                session_id: session_id.to_string(),
                                role: role.to_string(),
                                content: String::new(),
                                channel_id: None,
                                conversation_id: None,
                                tool_calls: None,
                                tool_results: None,
                                delegation_event: None,
                                tool_name,
                                tool_args,
                                tool_result,
                                tool_index,
                                source,
                                pending_tool_calls: None,
                                orchestrator_id: event_orchestrator_id,
                            };
                            let _ = tx.send(ev);
                            ctx.request_repaint();
                        }
                    } else if event_name == "session.assistant_progress" {
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
                            let content = data
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            if content.trim().is_empty() {
                                continue;
                            }
                            let source = data
                                .get("source")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let event_orchestrator_id = data
                                .get("orchestratorId")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let ev = SessionEvent {
                                session_id: session_id.to_string(),
                                role: "assistant_progress".to_string(),
                                content,
                                channel_id: None,
                                conversation_id: None,
                                tool_calls: None,
                                tool_results: None,
                                delegation_event: None,
                                tool_name: None,
                                tool_args: None,
                                tool_result: None,
                                tool_index: None,
                                source,
                                pending_tool_calls: None,
                                orchestrator_id: event_orchestrator_id,
                            };
                            let _ = tx.send(ev);
                            ctx.request_repaint();
                        }
                    } else if event_name == "session.tool_loop_limit" {
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
                            let pending_tool_calls = data
                                .get("pendingToolCalls")
                                .and_then(|v| v.as_array())
                                .map(|arr| arr.clone());
                            let event_orchestrator_id = data
                                .get("orchestratorId")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let ev = SessionEvent {
                                session_id: session_id.to_string(),
                                role: "tool_loop_limit".to_string(),
                                content: String::new(),
                                channel_id: None,
                                conversation_id: None,
                                tool_calls: None,
                                tool_results: None,
                                delegation_event: None,
                                tool_name: None,
                                tool_args: None,
                                tool_result: None,
                                tool_index: None,
                                source: None,
                                pending_tool_calls,
                                orchestrator_id: event_orchestrator_id,
                            };
                            let _ = tx.send(ev);
                            ctx.request_repaint();
                        }
                    } else if event_name == "session.turn_stopped" {
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
                            let event_orchestrator_id = data
                                .get("orchestratorId")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let ev = SessionEvent {
                                session_id: session_id.to_string(),
                                role: "turn_stopped".to_string(),
                                content: String::new(),
                                channel_id: None,
                                conversation_id: None,
                                tool_calls: None,
                                tool_results: None,
                                delegation_event: None,
                                tool_name: None,
                                tool_args: None,
                                tool_result: None,
                                tool_index: None,
                                source: None,
                                pending_tool_calls: None,
                                orchestrator_id: event_orchestrator_id,
                            };
                            let _ = tx.send(ev);
                            ctx.request_repaint();
                        }
                    } else if event_name == "session.deleted" {
                        if let Some(payload) = val.get("payload") {
                            let data = payload.get("data").unwrap_or(payload);
                            let session_id = data
                                .get("sessionId")
                                .and_then(|v| v.as_str())
                                .map(|s| s.trim())
                                .filter(|s| !s.is_empty())
                                .unwrap_or("")
                                .to_string();
                            let event_orchestrator_id = data
                                .get("orchestratorId")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let ev = SessionEvent {
                                session_id,
                                role: "session_deleted".to_string(),
                                content: String::new(),
                                channel_id: None,
                                conversation_id: None,
                                tool_calls: None,
                                tool_results: None,
                                delegation_event: None,
                                tool_name: None,
                                tool_args: None,
                                tool_result: None,
                                tool_index: None,
                                source: None,
                                pending_tool_calls: None,
                                orchestrator_id: event_orchestrator_id,
                            };
                            let _ = tx.send(ev);
                            ctx.request_repaint();
                        }
                    } else if event_name == "sessions.cleared" {
                        let cleared_orchestrator_id = val
                            .get("payload")
                            .and_then(|p| p.get("orchestratorId"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let ev = SessionEvent {
                            session_id: String::new(),
                            role: "sessions_cleared".to_string(),
                            content: String::new(),
                            channel_id: None,
                            conversation_id: None,
                            tool_calls: None,
                            tool_results: None,
                            delegation_event: None,
                            tool_name: None,
                            tool_args: None,
                            tool_result: None,
                            tool_index: None,
                            source: None,
                            pending_tool_calls: None,
                            orchestrator_id: cleared_orchestrator_id,
                        };
                        let _ = tx.send(ev);
                        ctx.request_repaint();
                    } else if event_name == "gateway.config.changed" {
                        let ev = SessionEvent {
                            session_id: String::new(),
                            role: "config_changed".to_string(),
                            content: String::new(),
                            channel_id: None,
                            conversation_id: None,
                            tool_calls: None,
                            tool_results: None,
                            delegation_event: None,
                            tool_name: None,
                            tool_args: None,
                            tool_result: None,
                            tool_index: None,
                            source: None,
                            pending_tool_calls: None,
                            orchestrator_id: None,
                        };
                        let _ = tx.send(ev);
                        ctx.request_repaint();
                    }
                }
            }
        }

        Ok(())
    })
}
