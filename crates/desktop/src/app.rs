//! Chai Desktop — egui app state and UI.

use eframe::egui;
use futures_util::{SinkExt, StreamExt};
use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Child;
use std::sync::mpsc;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

const CHAT_WINDOW_HEIGHT: f32 = 260.0;
const CHAT_INPUT_HEIGHT: f32 = 130.0;
// Must match DEFAULT_MODEL_FALLBACK in `crates/lib/src/gateway/server.rs`.
const DEFAULT_MODEL_FALLBACK: &str = "llama3.2:latest";

#[derive(Clone)]
struct ChatMessage {
    role: String,
    content: String,
    tool_calls: Option<Vec<serde_json::Value>>,
}

impl ChatMessage {
    fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: text.into(),
            tool_calls: None,
        }
    }

    fn assistant(text: impl Into<String>, tool_calls: Option<Vec<serde_json::Value>>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: text.into(),
            tool_calls,
        }
    }
}

struct AgentReply {
    session_id: String,
    reply: String,
    tool_calls: Vec<serde_json::Value>,
}

#[derive(Clone)]
struct SessionEvent {
    session_id: String,
    role: String,
    content: String,
    channel_id: Option<String>,
    conversation_id: Option<String>,
}

/// Fetch gateway status via WebSocket (connect + status). Runs in a thread; use blocking.
fn fetch_gateway_status() -> Result<GatewayStatusDetails, String> {
    let (config, _) = lib::config::load_config(None).map_err(|e| e.to_string())?;
    let bind = config.gateway.bind.trim();
    let port = config.gateway.port;
    let token = lib::config::resolve_gateway_token(&config);
    let ws_url = format!("ws://{}:{}/ws", bind, port);

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .map_err(|e| e.to_string())?;

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

        let connect_params = if let Some(device_token) = lib::device::load_device_token() {
            serde_json::json!({ "auth": { "deviceToken": device_token } })
        } else {
            let identity = lib::device::DeviceIdentity::load(lib::device::default_device_path().as_path())
                .or_else(|| {
                    let id = lib::device::DeviceIdentity::generate().ok()?;
                    let _ = id.save(&lib::device::default_device_path());
                    Some(id)
                })
                .ok_or("failed to load or create device identity")?;
            let signed_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let token_str = token.as_deref().unwrap_or("");
            let scopes: Vec<String> = vec!["operator.read".into(), "operator.write".into()];
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

        let mut details = GatewayStatusDetails::default();
        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") {
                continue;
            }
            if res.get("id").and_then(|v| v.as_str()) == Some("1") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("connect failed");
                    return Err(err.to_string());
                }
                if let Some(auth) = res.get("payload").and_then(|p| p.get("auth")) {
                    if let Some(dt) = auth.get("deviceToken").and_then(|v| v.as_str()) {
                        let _ = lib::device::save_device_token(dt);
                    }
                }
                break;
            }
        }

        let status_req = serde_json::json!({
            "type": "req",
            "id": "2",
            "method": "status",
            "params": {}
        });
        ws.send(Message::Text(status_req.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") {
                continue;
            }
            if res.get("id").and_then(|v| v.as_str()) == Some("2") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("status failed");
                    return Err(err.to_string());
                }
                let payload = res.get("payload").ok_or("missing payload")?;
                details.protocol = payload.get("protocol").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                details.port = payload.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
                details.bind = payload
                    .get("bind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                details.auth = payload
                    .get("auth")
                    .and_then(|v| v.as_str())
                    .unwrap_or("none")
                    .to_string();
                details.ollama_models = payload
                    .get("ollamaModels")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                return Ok(details);
            }
        }
        Err("no status response".to_string())
    })
}

/// Resolve the chai CLI binary: same directory as this executable, or "chai" from PATH.
fn resolve_chai_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = if cfg!(windows) { "chai.exe" } else { "chai" };
    let candidate = dir.join(name);
    if candidate.exists() {
        return Some(candidate);
    }
    // Fallback: assume "chai" is on PATH
    Some(PathBuf::from("chai"))
}

/// Frames between gateway probes (probe at ~1 Hz if 60 fps).
const PROBE_INTERVAL_FRAMES: u32 = 60;

/// Frames between WebSocket status fetches when gateway is running (~0.5 Hz).
const STATUS_INTERVAL_FRAMES: u32 = 120;

/// Live gateway details from WebSocket `status` method.
#[derive(Clone, Default)]
pub struct GatewayStatusDetails {
    pub protocol: u32,
    pub port: u16,
    pub bind: String,
    pub auth: String,
    /// Ollama model names from gateway discovery (empty if Ollama unreachable).
    pub ollama_models: Vec<String>,
}

pub struct ChaiApp {
    /// When Some, the gateway subprocess is running. Cleared when process exits or we stop it.
    gateway_process: Option<Child>,
    /// Last error from start gateway (e.g. spawn failed).
    gateway_error: Option<String>,
    /// True if the configured gateway address:port accepted a TCP connection (we or someone else).
    gateway_responds: bool,
    /// When Some, a probe is in flight; we read the result here.
    probe_receiver: Option<mpsc::Receiver<bool>>,
    /// Frames since we last started a probe.
    frames_since_probe: u32,
    /// When Some, a status fetch is in flight; we read the result here.
    status_receiver: Option<mpsc::Receiver<Result<GatewayStatusDetails, String>>>,
    /// Frames since we last started a status fetch.
    frames_since_status: u32,
    /// Last successful gateway status (protocol, port, bind, auth). Cleared when gateway stops responding.
    gateway_status: Option<GatewayStatusDetails>,
    /// Current chat session id (created on first agent call).
    chat_session_id: Option<String>,
    /// In-memory chat transcript for the current session.
    chat_messages: Vec<ChatMessage>,
    /// Current input text for the chat box.
    chat_input: String,
    /// Last error from a chat turn, if any.
    chat_error: Option<String>,
    /// When Some, a chat turn is in flight; we read the result here.
    chat_turn_receiver: Option<mpsc::Receiver<Result<AgentReply, String>>>,
    /// Live session messages from gateway events (keyed by session id).
    session_messages: BTreeMap<String, Vec<ChatMessage>>,
    /// Optional channel metadata for each session (channelId, conversationId).
    session_meta: HashMap<String, (Option<String>, Option<String>)>,
    /// When Some, a session events stream is in flight; we read gateway session.message events here.
    session_events_receiver: Option<mpsc::Receiver<SessionEvent>>,
    /// Currently selected model override (None = use gateway default).
    current_model: Option<String>,
    /// Default model from config (cached for display / fallback).
    default_model: Option<String>,
}

impl Default for ChaiApp {
    fn default() -> Self {
        Self {
            gateway_process: None,
            gateway_error: None,
            gateway_responds: false,
            probe_receiver: None,
            frames_since_probe: 0,
            status_receiver: None,
            frames_since_status: 0,
            gateway_status: None,
            chat_session_id: None,
            chat_messages: Vec::new(),
            chat_input: String::new(),
            chat_error: None,
            chat_turn_receiver: None,
            session_messages: BTreeMap::new(),
            session_meta: HashMap::new(),
            session_events_receiver: None,
            current_model: None,
            default_model: None,
        }
    }
}

impl ChaiApp {
    fn start_new_session(&mut self) {
        self.chat_session_id = None;
        self.chat_messages.clear();
        self.chat_error = None;
        self.chat_messages.push(ChatMessage::assistant(
            "Session restarted. Next message will start with a clean history.".to_string(),
            None,
        ));
    }
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    /// Poll for probe result and optionally start a new probe. Call each frame.
    fn poll_gateway_probe(&mut self) {
        if let Some(rx) = &self.probe_receiver {
            if let Ok(ok) = rx.try_recv() {
                self.gateway_responds = ok;
                if !ok {
                    self.gateway_status = None;
                }
                self.probe_receiver = None;
            }
        }
        self.frames_since_probe = self.frames_since_probe.saturating_add(1);
        if self.probe_receiver.is_none() && self.frames_since_probe >= PROBE_INTERVAL_FRAMES {
            self.frames_since_probe = 0;
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let (config, _) = lib::config::load_config(None).unwrap_or((lib::config::Config::default(), PathBuf::new()));
                let addr_str = format!(
                    "{}:{}",
                    config.gateway.bind.trim(),
                    config.gateway.port
                );
                let ok = addr_str
                    .parse::<SocketAddr>()
                    .ok()
                    .and_then(|addr| {
                        std::net::TcpStream::connect_timeout(
                            &addr,
                            Duration::from_millis(800),
                        )
                        .ok()
                    })
                    .is_some();
                let _ = tx.send(ok);
            });
            self.probe_receiver = Some(rx);
        }
    }

    /// Poll for status fetch result and optionally start a new fetch when gateway is running. Call each frame.
    fn poll_status_fetch(&mut self) {
        if let Some(rx) = &self.status_receiver {
            if let Ok(result) = rx.try_recv() {
                self.gateway_status = result.ok();
                self.status_receiver = None;
            }
        }
        if !self.gateway_responds || self.status_receiver.is_some() {
            return;
        }
        self.frames_since_status = self.frames_since_status.saturating_add(1);
        if self.frames_since_status >= STATUS_INTERVAL_FRAMES {
            self.frames_since_status = 0;
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let result = fetch_gateway_status();
                let _ = tx.send(result);
            });
            self.status_receiver = Some(rx);
        }
    }

    /// Ensure the background session.events listener is running when the gateway is up.
    fn ensure_session_events_listener(&mut self, running: bool) {
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
                std::thread::sleep(std::time::Duration::from_secs(1));
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
                                eprintln!("session events listener error: {}, retrying in {}s (attempt {})", e, delay, retry_count);
                            }
                            std::thread::sleep(std::time::Duration::from_secs(delay));
                        }
                        Ok(()) => {
                            // Normal exit (connection closed), reset retry count and wait before retry
                            retry_count = 0;
                            std::thread::sleep(std::time::Duration::from_secs(2));
                        }
                    }
                }
            });
            self.session_events_receiver = Some(rx);
        }
    }

    /// Poll for session.message events from the gateway and update local session timelines.
    fn poll_session_events(&mut self) {
        if let Some(rx) = &self.session_events_receiver {
            loop {
                match rx.try_recv() {
                    Ok(ev) => {
                        let entry = self
                            .session_messages
                            .entry(ev.session_id.clone())
                            .or_insert_with(Vec::new);
                        entry.push(ChatMessage {
                            role: ev.role,
                            content: ev.content,
                            tool_calls: None,
                        });
                        self.session_meta
                            .insert(ev.session_id, (ev.channel_id, ev.conversation_id));
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // Listener thread exited or sender dropped, clear receiver so it can be restarted
                        self.session_events_receiver = None;
                        break;
                    }
                }
            }
        }
    }

    /// Poll for chat turn result and clear receiver when done. Call each frame.
    fn poll_chat_turn(&mut self) {
        if let Some(rx) = &self.chat_turn_receiver {
            if let Ok(result) = rx.try_recv() {
                self.chat_turn_receiver = None;
                match result {
                    Ok(reply) => {
                        let was_new_session = self.chat_session_id.is_none();
                        self.chat_session_id = Some(reply.session_id.clone());
                        self.chat_messages.push(ChatMessage::assistant(
                            reply.reply.clone(),
                            if reply.tool_calls.is_empty() {
                                None
                            } else {
                                Some(reply.tool_calls.clone())
                            },
                        ));
                        
                        // Add messages to session_messages for immediate UI update
                        let entry = self
                            .session_messages
                            .entry(reply.session_id.clone())
                            .or_insert_with(Vec::new);
                        
                        // If this was a new session, add the last user message (which triggered this turn)
                        if was_new_session && !self.chat_messages.is_empty() {
                            if let Some(last_user_msg) = self.chat_messages.iter().rev().find(|m| m.role == "user") {
                                entry.push(last_user_msg.clone());
                            }
                        }
                        
                        // Add the assistant reply
                        entry.push(ChatMessage::assistant(
                            reply.reply,
                            if reply.tool_calls.is_empty() {
                                None
                            } else {
                                Some(reply.tool_calls)
                            },
                        ));
                        self.session_meta
                            .entry(reply.session_id)
                            .or_insert((None, None));
                    }
                    Err(e) => {
                        self.chat_error = Some(e);
                    }
                }
            }
        }
    }

    /// True if we started the gateway and it is still running (we can stop it).
    fn gateway_owned(&mut self) -> bool {
        if let Some(ref mut child) = self.gateway_process {
            if child.try_wait().ok().flatten().is_some() {
                self.gateway_process = None;
                return false;
            }
            return true;
        }
        false
    }

    fn start_gateway(&mut self) {
        self.gateway_error = None;
        let (config, _) = match lib::config::load_config(None) {
            Ok(pair) => pair,
            Err(e) => {
                self.gateway_error = Some(format!("failed to load config: {}", e));
                return;
            }
        };
        let port = config.gateway.port;
        let binary = match resolve_chai_binary() {
            Some(p) => p,
            None => {
                self.gateway_error = Some("could not find chai binary".to_string());
                return;
            }
        };
        let child = std::process::Command::new(&binary)
            .args(["gateway", "--port", &port.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        match child {
            Ok(c) => {
                self.gateway_process = Some(c);
            }
            Err(e) => {
                self.gateway_error = Some(format!("failed to start gateway: {}", e));
            }
        }
    }

    fn stop_gateway(&mut self) {
        if let Some(mut child) = self.gateway_process.take() {
            let _ = child.kill();
        }
        self.gateway_error = None;
    }

    /// Start a chat turn in a background thread if possible.
    fn start_chat_turn(&mut self) {
        if self.chat_turn_receiver.is_some() {
            return;
        }
        let message = self.chat_input.trim().to_string();
        if message.is_empty() {
            return;
        }
        self.chat_error = None;
        self.chat_input.clear();
        
        // Handle special commands
        if message.eq_ignore_ascii_case("/new") {
            self.start_new_session();
            return;
        }
        
        if message.eq_ignore_ascii_case("/help") {
            self.chat_messages.push(ChatMessage::assistant(
                "available commands:\n\n/new - start a new session (clear conversation history)\n/help - show this help message".to_string(),
                None,
            ));
            return;
        }
        
        self.chat_messages.push(ChatMessage::user(message.clone()));
        
        // Also add to session_messages for immediate UI update
        // If we have a session_id, use it; otherwise messages will be added when we get the response
        if let Some(ref session_id) = self.chat_session_id {
            let entry = self
                .session_messages
                .entry(session_id.clone())
                .or_insert_with(Vec::new);
            entry.push(ChatMessage::user(message.clone()));
            self.session_meta
                .entry(session_id.clone())
                .or_insert((None, None));
        }
        
        let session_id = self.chat_session_id.clone();
        let model = self.current_model.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = run_agent_turn(session_id, message, model);
            let _ = tx.send(result);
        });
        self.chat_turn_receiver = Some(rx);
    }

    /// Render the chat UI (messages + input).
    fn ui_chat(&mut self, ui: &mut egui::Ui, gateway_running: bool) {
        // Chat window: fixed-height scroll area for messages
        egui::ScrollArea::vertical()
            .max_height(CHAT_WINDOW_HEIGHT)
            .show(ui, |ui| {
                ui.set_min_height(CHAT_WINDOW_HEIGHT);
                for m in &self.chat_messages {
                    let is_user = m.role == "user";
                    let frame = egui::Frame::none()
                        .fill(if is_user {
                            ui.style().visuals.extreme_bg_color
                        } else {
                            ui.style().visuals.panel_fill
                        })
                        .stroke(egui::Stroke::new(
                            1.0,
                            ui.style()
                                .visuals
                                .widgets
                                .noninteractive
                                .bg_stroke
                                .color,
                        ))
                        .rounding(egui::Rounding::same(8.0))
                        .inner_margin(egui::Margin::same(8.0));

                    frame.show(ui, |ui| {
                        // Let the chat bubble use the full available width (same as the send container).
                        if is_user {
                            ui.label(egui::RichText::new(&m.content).strong());
                        } else {
                            ui.label(&m.content);
                            if let Some(ref tool_calls) = m.tool_calls {
                                if !tool_calls.is_empty() {
                                    ui.add_space(8.0);
                                    ui.separator();
                                    ui.add_space(4.0);
                                    egui::CollapsingHeader::new(format!(
                                        "🔧 {} tool call(s)",
                                        tool_calls.len()
                                    ))
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        for (idx, tc) in tool_calls.iter().enumerate() {
                                            if idx > 0 {
                                                ui.add_space(4.0);
                                            }
                                            // Handle both serialized ToolCall structs and raw JSON
                                            let tool_name = tc
                                                .get("function")
                                                .and_then(|f| f.get("name"))
                                                .and_then(|n| n.as_str())
                                                .unwrap_or("unknown");
                                            let tool_args = tc
                                                .get("function")
                                                .and_then(|f| f.get("arguments"))
                                                .unwrap_or(&serde_json::Value::Null);
                                            let tool_type = tc
                                                .get("type")
                                                .and_then(|t| t.as_str())
                                                .unwrap_or("");
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "Tool: {}",
                                                    tool_name
                                                ))
                                                .strong(),
                                            );
                                            if !tool_type.is_empty() {
                                                ui.label(format!("Type: {}", tool_type));
                                            }
                                            ui.label(format!(
                                                "Arguments: {}",
                                                serde_json::to_string_pretty(tool_args)
                                                    .unwrap_or_else(|_| tool_args.to_string())
                                            ));
                                        }
                                    });
                                }
                            }
                        }
                    });
                    ui.add_space(8.0);
                }
            });
        ui.add_space(8.0);

        // Text box for sending messages: fixed height (about two thirds of chat window)
        let response = ui.add_sized(
            [ui.available_width(), CHAT_INPUT_HEIGHT],
            egui::TextEdit::multiline(&mut self.chat_input),
        );
        ui.add_space(8.0);

        // Fixed-height container row for the send button, right-aligned.
        let row_height = ui.spacing().interact_size.y + 8.0; // 4px top + 4px bottom padding
        let row_width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(row_width, row_height), egui::Sense::hover());
        let mut row_ui = ui.child_ui(rect, egui::Layout::right_to_left(egui::Align::Center));
        egui::Frame::none()
            .inner_margin(egui::Margin {
                left: 0.0,
                right: 8.0,
                top: 4.0,
                bottom: 4.0,
            })
            .show(&mut row_ui, |ui| {
                // Right-to-left layout:
                // - Send button on the far right
                // - Model dropdown to its left
                // - /new button to the left of the model dropdown
                let mut send_now = false;

                let send_button = ui.add_enabled(gateway_running, egui::Button::new("Send"));

                // Model dropdown: show even when gateway is stopped, using default model from config.
                let gateway_models: Vec<String> = self
                    .gateway_status
                    .as_ref()
                    .map(|s| s.ollama_models.clone())
                    .unwrap_or_default();
                let mut model_options: Vec<String> = Vec::new();
                if let Some(dm) = self.default_model.as_ref() {
                    model_options.push(dm.clone());
                }
                for m in gateway_models {
                    if !model_options.iter().any(|existing| existing == &m) {
                        model_options.push(m);
                    }
                }

                if !model_options.is_empty() {
                    ui.add_space(8.0);
                    let current_label = self
                        .current_model
                        .as_deref()
                        .or(self.default_model.as_deref())
                        .unwrap_or(DEFAULT_MODEL_FALLBACK);
                    egui::ComboBox::from_id_source("model_select")
                        .selected_text(current_label)
                        .show_ui(ui, |ui| {
                            for m in &model_options {
                                let selected = self
                                    .current_model
                                    .as_deref()
                                    .map(|cm| cm == m.as_str())
                                    .unwrap_or(false);
                                if ui.selectable_label(selected, m).clicked() {
                                    self.current_model = Some(m.clone());
                                }
                            }
                        });
                }

                ui.add_space(8.0);
                if ui.add_enabled(gateway_running, egui::Button::new("/new")).clicked() {
                    self.start_new_session();
                }

                if send_button.clicked() {
                    send_now = true;
                }
                if gateway_running && response.has_focus() {
                    let modifiers = ui.input(|i| i.modifiers);
                    if modifiers.command || modifiers.ctrl {
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            send_now = true;
                        }
                    }
                }
                if send_now {
                    self.start_chat_turn();
                }
            });

        if let Some(ref err) = self.chat_error {
            ui.add_space(8.0);
            ui.colored_label(egui::Color32::RED, err);
        }
    }
}

impl eframe::App for ChaiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_gateway_probe();
        self.poll_status_fetch();
        self.poll_chat_turn();
        let owned = self.gateway_owned();
        let running = owned || self.gateway_responds;
        self.ensure_session_events_listener(running);
        self.poll_session_events();

        // Header with title and gateway controls only
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                .show(ui, |ui| {
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        ui.heading("Chai — Multi-Agent Management System");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if running {
                                if owned {
                                    if ui.button("Stop gateway").clicked() {
                                        self.stop_gateway();
                                    }
                                } else {
                                    ui.add_enabled(false, egui::Button::new("Gateway running"));
                                }
                            } else {
                                if ui.button("Start gateway").clicked() {
                                    self.start_gateway();
                                }
                            }
                        });
                    });
                    ui.add_space(16.0);
                });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                    .show(ui, |ui| {
                        ui.add_space(24.0);
                        // Info section - Gateway status and Ollama
                        ui.heading("Info");
                        ui.add_space(8.0);
                        
                        let (config, _) = lib::config::load_config(None)
                            .unwrap_or((lib::config::Config::default(), std::path::PathBuf::new()));
                        if self.default_model.is_none() {
                            self.default_model = config
                                .agents
                                .default_model
                                .clone()
                                .or_else(|| Some(DEFAULT_MODEL_FALLBACK.to_string()));
                        }
                        let port = config.gateway.port;
                        let bind = config.gateway.bind.trim();
                        let auth_mode = match config.gateway.auth.mode {
                            lib::config::GatewayAuthMode::None => "none",
                            lib::config::GatewayAuthMode::Token => "token",
                        };
                        
                        // Use live status if available, otherwise use config values
                        let (protocol, status_port, status_bind, status_auth) = if let Some(ref s) = self.gateway_status {
                            (s.protocol, s.port, s.bind.clone(), s.auth.clone())
                        } else {
                            (1, port, bind.to_string(), auth_mode.to_string())
                        };
                        
                        ui.horizontal(|ui| {
                            ui.label(format!("Gateway: {}", if running { "running" } else { "stopped" }));
                            ui.separator();
                            ui.label(format!("Bind: {}", status_bind));
                            ui.separator();
                            ui.label(format!("Port: {}", status_port));
                            ui.separator();
                            ui.label(format!("Protocol: {}", protocol));
                            ui.separator();
                            ui.label(format!("Auth: {}", status_auth));
                        });
                        // Context mode (full vs readOnDemand)
                        let context_mode_str = match config.skills.context_mode {
                            lib::config::SkillContextMode::Full => "full",
                            lib::config::SkillContextMode::ReadOnDemand => "readOnDemand",
                        };
                        ui.add_space(4.0);
                        ui.label(format!("Context mode: {}", context_mode_str));

                        // Current model (effective model for next turn: override or default/fallback)
                        let current_model = self
                            .current_model
                            .as_deref()
                            .or(self.default_model.as_deref())
                            .unwrap_or(DEFAULT_MODEL_FALLBACK);
                        ui.add_space(4.0);
                        ui.label(format!("Current model: {}", current_model));
                        
                        // Ollama models line
                        if let Some(ref s) = self.gateway_status {
                            if !s.ollama_models.is_empty() {
                                ui.add_space(4.0);
                                ui.label(format!("Ollama: {} model(s) — {}", s.ollama_models.len(), s.ollama_models.join(", ")));
                            }
                        } else if self.status_receiver.is_some() {
                            ui.add_space(4.0);
                            ui.label("Ollama: fetching…");
                        } else {
                            ui.add_space(4.0);
                            ui.label("Ollama: (start gateway to see available models)");
                        }

                        if let Some(ref err) = self.gateway_error {
                            ui.add_space(4.0);
                            ui.colored_label(egui::Color32::RED, err);
                        }

                        ui.add_space(24.0);
                        ui.separator();
                        ui.add_space(24.0);
                    ui.heading("Chat");
                    ui.add_space(8.0);
                    if !running {
                        ui.label("Start the gateway to chat with the model.");
                        ui.add_space(8.0);
                    }
                    self.ui_chat(ui, running);

                        ui.add_space(24.0);
                        ui.separator();
                        ui.add_space(24.0);
                        ui.heading("Sessions");
                        ui.add_space(8.0);
                        if !running {
                            ui.label("No sessions available (gateway stopped).");
                        } else if self.session_messages.is_empty() {
                            ui.label("No sessions yet. Send a message to start one.");
                        } else {
                            for (session_id, messages) in &self.session_messages {
                                let label = if let Some((channel_id, conversation_id)) =
                                    self.session_meta.get(session_id)
                                {
                                    match (channel_id, conversation_id) {
                                        (Some(cid), Some(conv)) => {
                                            format!("{} ({}:{})", session_id, cid, conv)
                                        }
                                        (Some(cid), None) => {
                                            format!("{} ({})", session_id, cid)
                                        }
                                        _ => session_id.clone(),
                                    }
                                } else {
                                    session_id.clone()
                                };
                                ui.label(egui::RichText::new(label).strong());
                                if let Some(last) = messages.last() {
                                    ui.add_space(4.0);
                                    ui.label(format!("  {}: {}", last.role, last.content));
                                }
                                ui.add_space(12.0);
                            }
                        }
                        ui.add_space(48.0);
                    });
            });
        });
    }
}

/// Listen for session.message events from the gateway and forward them via an mpsc channel.
fn run_session_events_loop(tx: mpsc::Sender<SessionEvent>) -> Result<(), String> {
    let (config, _) = lib::config::load_config(None).map_err(|e| e.to_string())?;
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

        let connect_params = if let Some(device_token) = lib::device::load_device_token() {
            serde_json::json!({ "auth": { "deviceToken": device_token } })
        } else {
            let identity = lib::device::DeviceIdentity::load(
                lib::device::default_device_path().as_path(),
            )
            .or_else(|| {
                let id = lib::device::DeviceIdentity::generate().ok()?;
                let _ = id.save(&lib::device::default_device_path());
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
            "id": "session-events-connect",
            "method": "connect",
            "params": connect_params
        });
        ws.send(Message::Text(connect_req.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        // Wait for connect response before listening for events (with timeout)
        let mut connected = false;
        let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
        tokio::pin!(timeout);
        
        loop {
            tokio::select! {
                _ = &mut timeout => {
                    return Err("connect handshake timeout".to_string());
                }
                msg = ws.next() => {
                    let Some(msg) = msg else { break; };
                    let msg = msg.map_err(|e| e.to_string())?;
                    let Message::Text(text) = msg else { continue };
                    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                        continue;
                    };
                    
                    // Handle connect response
                    if value.get("type").and_then(|v| v.as_str()) == Some("res") {
                        if value.get("id").and_then(|v| v.as_str()) == Some("session-events-connect") {
                            if value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                                connected = true;
                                if let Some(auth) = value.get("payload").and_then(|p| p.get("auth")) {
                                    if let Some(dt) = auth.get("deviceToken").and_then(|v| v.as_str()) {
                                        let _ = lib::device::save_device_token(dt);
                                    }
                                }
                                break;
                            } else {
                                let err = value
                                    .get("error")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("connect failed");
                                return Err(err.to_string());
                            }
                        }
                    }
                }
            }
        }
        
        if !connected {
            return Err("connect handshake incomplete".to_string());
        }

        // Now listen for events
        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            if value.get("type").and_then(|v| v.as_str()) == Some("event") {
                if value
                    .get("event")
                    .and_then(|v| v.as_str())
                    == Some("session.message")
                {
                    if let Some(payload) = value.get("payload") {
                        if let Some(session_id) =
                            payload.get("sessionId").and_then(|v| v.as_str())
                        {
                            if let Some(role) =
                                payload.get("role").and_then(|v| v.as_str())
                            {
                                if let Some(content) =
                                    payload.get("content").and_then(|v| v.as_str())
                                {
                                    let channel_id = payload
                                        .get("channelId")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string());
                                    let conversation_id = payload
                                        .get("conversationId")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string());
                                    let ev = SessionEvent {
                                        session_id: session_id.to_string(),
                                        role: role.to_string(),
                                        content: content.to_string(),
                                        channel_id,
                                        conversation_id,
                                    };
                                    let _ = tx.send(ev);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    })
}

/// Run one agent turn against the gateway: connect, send message, return reply and session id.
fn run_agent_turn(
    session_id: Option<String>,
    message: String,
    model: Option<String>,
) -> Result<AgentReply, String> {
    let (config, _) = lib::config::load_config(None).map_err(|e| e.to_string())?;
    let bind = config.gateway.bind.trim();
    let port = config.gateway.port;
    let token = lib::config::resolve_gateway_token(&config);
    let ws_url = format!("ws://{}:{}/ws", bind, port);

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .map_err(|e| e.to_string())?;

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

        let connect_params = if let Some(device_token) = lib::device::load_device_token() {
            serde_json::json!({ "auth": { "deviceToken": device_token } })
        } else {
            let identity = lib::device::DeviceIdentity::load(
                lib::device::default_device_path().as_path(),
            )
            .or_else(|| {
                let id = lib::device::DeviceIdentity::generate().ok()?;
                let _ = id.save(&lib::device::default_device_path());
                Some(id)
            })
            .ok_or("failed to load or create device identity")?;
            let signed_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let token_str = token.as_deref().unwrap_or("");
            let scopes: Vec<String> = vec!["operator.read".into(), "operator.write".into()];
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

        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") {
                continue;
            }
            if res.get("id").and_then(|v| v.as_str()) == Some("1") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("connect failed");
                    return Err(err.to_string());
                }
                if let Some(auth) = res.get("payload").and_then(|p| p.get("auth")) {
                    if let Some(dt) = auth.get("deviceToken").and_then(|v| v.as_str()) {
                        let _ = lib::device::save_device_token(dt);
                    }
                }
                break;
            }
        }

        let mut agent_params = serde_json::json!({
            "message": message,
        });
        if let Some(id) = session_id {
            agent_params["sessionId"] = serde_json::Value::String(id);
        }
        if let Some(m) = model {
            agent_params["model"] = serde_json::Value::String(m);
        }

        let agent_req = serde_json::json!({
            "type": "req",
            "id": "2",
            "method": "agent",
            "params": agent_params
        });
        ws.send(Message::Text(agent_req.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") {
                continue;
            }
            if res.get("id").and_then(|v| v.as_str()) == Some("2") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("agent failed");
                    return Err(err.to_string());
                }
                let payload = res.get("payload").ok_or("missing payload")?;
                let session_id = payload
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .ok_or("missing sessionId in agent response")?
                    .to_string();
                let reply = payload
                    .get("reply")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let tool_calls = payload
                    .get("toolCalls")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.clone())
                    .unwrap_or_default();
                return Ok(AgentReply {
                    session_id,
                    reply,
                    tool_calls,
                });
            }
        }
        Err("no agent response".to_string())
    })
}
