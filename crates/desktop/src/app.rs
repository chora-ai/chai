//! Chai Desktop — egui app state and UI.

use eframe::egui;
use futures_util::{SinkExt, StreamExt};
use std::collections::{BTreeMap, HashMap};
use std::io::BufRead;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

mod screens;
mod state;
mod ui;

const CHAT_INPUT_HEIGHT: f32 = 130.0;
const CHAT_MESSAGES_MIN_HEIGHT: f32 = 80.0;

/// Short label for a session in the sessions list (id with optional channel/conversation).
fn session_label_display(
    session_id: &str,
    meta: Option<&(Option<String>, Option<String>)>,
) -> String {
    match meta {
        Some((Some(cid), Some(conv))) => format!("{} ({}:{})", session_id, cid, conv),
        Some((Some(cid), None)) => format!("{} ({})", session_id, cid),
        _ => session_id.to_string(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Screen {
    #[default]
    Info,
    Chat,
    Config,
    Context,
    Skills,
    Logs,
}

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
                details.default_backend = payload.get("defaultBackend").and_then(|v| v.as_str()).map(String::from);
                details.default_model = payload.get("defaultModel").and_then(|v| v.as_str()).map(String::from);
                details.ollama_models = payload
                    .get("ollamaModels")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                details.lm_studio_models = payload
                    .get("lmStudioModels")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                details.agent_context = payload.get("agentContext").and_then(|v| v.as_str()).map(String::from);
                details.system_context = payload.get("systemContext").and_then(|v| v.as_str()).map(String::from);
                details.date = payload.get("date").and_then(|v| v.as_str()).map(String::from);
                details.skills_context = payload.get("skillsContext").and_then(|v| v.as_str()).map(String::from);
                details.skills_context_full = payload.get("skillsContextFull").and_then(|v| v.as_str()).map(String::from);
                details.skills_context_bodies = payload.get("skillsContextBodies").and_then(|v| v.as_str()).map(String::from);
                details.context_mode = payload.get("contextMode").and_then(|v| v.as_str()).map(String::from);
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
    /// Resolved default backend: "ollama" or "lmstudio".
    pub default_backend: Option<String>,
    /// Resolved default model id (from config or backend fallback).
    pub default_model: Option<String>,
    /// Ollama model names from gateway discovery (empty if Ollama unreachable).
    pub ollama_models: Vec<String>,
    /// LM Studio model names from gateway discovery (empty if LM Studio unreachable).
    pub lm_studio_models: Vec<String>,
    /// Agent context loaded at gateway startup (e.g. AGENTS.md). None if not loaded.
    pub agent_context: Option<String>,
    /// Full system context sent to the model (agent context + skills). Empty if none.
    pub system_context: Option<String>,
    /// Current date (YYYY-MM-DD) from the gateway, for display in Context.
    pub date: Option<String>,
    /// Skills portion of system context (full or compact per context mode).
    pub skills_context: Option<String>,
    /// Full skill content for display (always full; use for UI when present).
    pub skills_context_full: Option<String>,
    /// Skill bodies only (no overview). Set when context mode is readOnDemand; use for Skills section to avoid duplicating the overview.
    pub skills_context_bodies: Option<String>,
    /// Skill context mode: "full" or "readOnDemand".
    pub context_mode: Option<String>,
}

pub struct ChaiApp {
    /// When Some, the gateway subprocess is running. Cleared when process exits or we stop it.
    gateway_process: Option<Child>,
    /// Last error from start gateway (e.g. spawn failed).
    gateway_error: Option<String>,
    /// True if the configured gateway address:port accepted a TCP connection (we or someone else).
    gateway_responds: bool,
    /// True once we have received at least one probe result (so we don't show "Gateway running" before probing).
    gateway_probe_completed: bool,
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
    /// User message we sent for the in-flight turn (used when reply creates a new session).
    pending_user_message: Option<String>,
    /// Live session messages from gateway events (keyed by session id).
    session_messages: BTreeMap<String, Vec<ChatMessage>>,
    /// Optional channel metadata for each session (channelId, conversationId).
    session_meta: HashMap<String, (Option<String>, Option<String>)>,
    /// When Some, a session events stream is in flight; we read gateway session.message events here.
    session_events_receiver: Option<mpsc::Receiver<SessionEvent>>,
    /// Currently selected backend override (None = use gateway default).
    current_backend: Option<String>,
    /// Currently selected model override (None = use gateway default).
    current_model: Option<String>,
    /// Default model from config (cached for display / fallback).
    default_model: Option<String>,
    /// Current screen (Info, Chat, Config, Context, Skills, Logs).
    current_screen: Screen,
    /// Session whose messages are shown in the chat area (None = "New session" / desktop buffer).
    selected_session_id: Option<String>,
    /// Session IDs in most-recently-active order (latest first) for the sidebar list.
    session_order: Vec<String>,
    /// Whether the gateway was running last frame (used to detect stop and clear messages).
    was_gateway_running: bool,
    /// Currently selected skill on the Skills screen (by name).
    selected_skill_name: Option<String>,
}

impl Default for ChaiApp {
    fn default() -> Self {
        Self {
            gateway_process: None,
            gateway_error: None,
            gateway_responds: false,
            gateway_probe_completed: false,
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
            pending_user_message: None,
            session_messages: BTreeMap::new(),
            session_meta: HashMap::new(),
            session_events_receiver: None,
            current_backend: None,
            current_model: None,
            default_model: None,
            current_screen: Screen::default(),
            selected_session_id: None,
            session_order: Vec::new(),
            was_gateway_running: false,
            selected_skill_name: None,
        }
    }
}

impl ChaiApp {
    /// Space between the main screen title (Info, Chat, Logs) and the content below.
    const SCREEN_TITLE_BOTTOM_SPACING: f32 = 18.0;
    /// Space between the bottom of the content and the window edge on Info, Logs, Chat, and Sessions.
    const SCREEN_FOOTER_SPACING: f32 = 48.0;

    fn start_new_session(&mut self) {
        self.chat_session_id = None;
        self.selected_session_id = None;
        self.chat_messages.clear();
        self.chat_error = None;
        self.chat_messages.push(ChatMessage::assistant(
            "Session restarted. Next message will start with a clean history.".to_string(),
            None,
        ));
    }

    /// Clear all session and message state when the gateway stops (it does not persist sessions).
    fn clear_session_and_messages(&mut self) {
        self.chat_session_id = None;
        self.chat_messages.clear();
        self.chat_error = None;
        self.chat_turn_receiver = None;
        self.pending_user_message = None;
        self.session_messages.clear();
        self.session_meta.clear();
        self.session_order.clear();
        self.selected_session_id = None;
        self.session_events_receiver = None;
    }
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        state::logs::init_logging();
        Self::default()
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

                        let entry = self
                            .session_messages
                            .entry(reply.session_id.clone())
                            .or_insert_with(Vec::new);
                        // Deduplicate: broadcast session events may have already added these messages.
                        if was_new_session {
                            if let Some(ref user_content) = self.pending_user_message {
                                let already = entry
                                    .last()
                                    .map(|m| m.role == "user" && m.content == *user_content)
                                    .unwrap_or(false);
                                if !already {
                                    entry.push(ChatMessage::user(user_content.clone()));
                                }
                            }
                        }
                        let assistant_msg = ChatMessage::assistant(
                            reply.reply.clone(),
                            if reply.tool_calls.is_empty() {
                                None
                            } else {
                                Some(reply.tool_calls.clone())
                            },
                        );
                        let already = entry
                            .last()
                            .map(|m| {
                                m.role == assistant_msg.role
                                    && m.content == assistant_msg.content
                                    && m.tool_calls == assistant_msg.tool_calls
                            })
                            .unwrap_or(false);
                        if !already {
                            entry.push(assistant_msg);
                        }
                        self.session_meta
                            .entry(reply.session_id.clone())
                            .or_insert((None, None));

                        self.pending_user_message = None;
                        self.chat_messages = self
                            .session_messages
                            .get(&reply.session_id)
                            .cloned()
                            .unwrap_or_default();
                        self.move_session_to_front(&reply.session_id);
                        if was_new_session {
                            self.selected_session_id = Some(reply.session_id);
                        }
                    }
                    Err(e) => {
                        self.pending_user_message = None;
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
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        match child {
            Ok(mut c) => {
                if let Some(stderr) = c.stderr.take() {
                    std::thread::spawn(move || {
                        let reader = std::io::BufReader::new(stderr);
                        for line in reader.lines() {
                            if let Ok(l) = line {
                                state::logs::push_log_line(format!("[gateway] {}", l));
                            }
                        }
                    });
                }
                if let Some(stdout) = c.stdout.take() {
                    std::thread::spawn(move || {
                        let reader = std::io::BufReader::new(stdout);
                        for line in reader.lines() {
                            if let Ok(l) = line {
                                state::logs::push_log_line(format!("[gateway] {}", l));
                            }
                        }
                    });
                }
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
        self.pending_user_message = Some(message.clone());

        // Handle special commands
        if message.eq_ignore_ascii_case("/new") {
            self.pending_user_message = None;
            self.start_new_session();
            return;
        }

        if message.eq_ignore_ascii_case("/help") {
            self.pending_user_message = None;
            self.chat_messages.push(ChatMessage::assistant(
                "available commands:\n\n/new - start a new session (clear conversation history)\n/help - show this help message".to_string(),
                None,
            ));
            return;
        }

        // Send to the current conversation session (chat_session_id), not the merely selected one.
        // None = new session; reply will set chat_session_id.
        let session_id = self.chat_session_id.clone();
        if let Some(ref sid) = session_id {
            let entry = self
                .session_messages
                .entry(sid.clone())
                .or_insert_with(Vec::new);
            entry.push(ChatMessage::user(message.clone()));
            self.session_meta
                .entry(sid.clone())
                .or_insert((None, None));
            self.move_session_to_front(sid);
            // Switch view to the session we're sending to so the message is visible (ui_chat shows selected_session_id).
            self.selected_session_id = Some(sid.clone());
        }
        // Keep chat_messages in sync when we're already viewing this session (e.g. for empty selected_session_id path).
        if self.selected_session_id == self.chat_session_id {
            self.chat_messages.push(ChatMessage::user(message.clone()));
        }
        // Send backend only when we know it (from UI override or gateway status). Do not hardcode
        // a fallback (e.g. "ollama") when status is unavailable—let the gateway use its config.
        let backend = self
            .current_backend
            .clone()
            .or_else(|| self.gateway_status.as_ref().and_then(|s| s.default_backend.clone()));
        let model = self.current_model.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = run_agent_turn(session_id, message, backend, model);
            let _ = tx.send(result);
        });
        self.chat_turn_receiver = Some(rx);
    }

    // screen-specific UI functions moved into app::screens::*
}

impl eframe::App for ChaiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_gateway_probe();
        self.poll_status_fetch();
        self.poll_chat_turn();
        let owned = self.gateway_owned();
        let running = owned || self.gateway_responds;
        if self.was_gateway_running && !running {
            self.clear_session_and_messages();
        }
        self.was_gateway_running = running;
        self.ensure_session_events_listener(running);
        self.poll_session_events();

        // Layout-level UI components
        let mut start_gateway = false;
        let mut stop_gateway = false;
        ui::header::header(
            ctx,
            running,
            owned,
            self.gateway_probe_completed,
            || {
                start_gateway = true;
            },
            || {
                stop_gateway = true;
            },
        );
        if start_gateway {
            self.start_gateway();
        }
        if stop_gateway {
            self.stop_gateway();
        }
        ui::sidebar::sidebar(&mut self.current_screen, ctx);
        ui::sessions::sessions_panel(self, ctx, running);

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.current_screen == Screen::Chat {
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                    .show(ui, |ui| {
                        ui.add_space(24.0);
                        ui.heading("Chat");
                        ui.add_space(Self::SCREEN_TITLE_BOTTOM_SPACING);
                        if !running {
                            ui.label("Start the gateway to chat with the model.");
                            ui.add_space(8.0);
                        }
                        screens::chat::ui_chat(self, ui, running);
                    });
            } else if self.current_screen == Screen::Info {
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                    .show(ui, |ui| {
                        screens::info::ui_info_screen(self, ui, running);
                    });
            } else if self.current_screen == Screen::Config {
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                    .show(ui, |ui| {
                        screens::config::ui_config_screen(self, ui);
                    });
            } else if self.current_screen == Screen::Context {
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                    .show(ui, |ui| {
                        screens::context::ui_context_screen(self, ui, running);
                    });
            } else if self.current_screen == Screen::Skills {
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                    .show(ui, |ui| {
                        screens::skills::ui_skills_screen(self, ui);
                    });
            } else if self.current_screen == Screen::Logs {
                // Logs screen has its own scroll area for the log lines; avoid double scrollbars
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                    .show(ui, |ui| {
                        screens::logs::ui_logs_screen(self, ui);
                    });
            }
        });
    }
}

/// Run one agent turn against the gateway: connect, send message, return reply and session id.
fn run_agent_turn(
    session_id: Option<String>,
    message: String,
    backend: Option<String>,
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
        if let Some(b) = &backend {
            agent_params["backend"] = serde_json::Value::String(b.clone());
        }
        if let Some(m) = &model {
            agent_params["model"] = serde_json::Value::String(m.clone());
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
