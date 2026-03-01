//! Chai Desktop — egui app state and UI.

use eframe::egui;
use futures_util::{SinkExt, StreamExt};
use std::collections::VecDeque;
use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::io::BufRead;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

const CHAT_INPUT_HEIGHT: f32 = 130.0;
const CHAT_MESSAGES_MIN_HEIGHT: f32 = 80.0;
const LOG_BUFFER_MAX_LINES: usize = 2000;

/// Ring buffer of log lines for the Logs screen. Written by DesktopLogger and gateway stderr reader.
static LOG_LINES: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

fn log_buffer() -> &'static Mutex<VecDeque<String>> {
    LOG_LINES.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn push_log_line(line: String) {
    if let Ok(mut buf) = log_buffer().lock() {
        buf.push_back(line);
        while buf.len() > LOG_BUFFER_MAX_LINES {
            buf.pop_front();
        }
    }
}

/// Logger that appends to LOG_LINES for display in the Logs screen.
struct DesktopLogger;

impl log::Log for DesktopLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let line = format!(
            "{} [{}] {}",
            chrono_lite(),
            record.level(),
            record.args()
        );
        push_log_line(line);
    }

    fn flush(&self) {}
}

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

fn chrono_lite() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = t.as_secs();
    let millis = t.subsec_millis();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, millis)
}

static LOGGER: DesktopLogger = DesktopLogger;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Screen {
    #[default]
    Info,
    Chat,
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
    /// Current screen (Info, Chat, Logs).
    current_screen: Screen,
    /// Session whose messages are shown in the chat area (None = "New session" / desktop buffer).
    selected_session_id: Option<String>,
    /// Session IDs in most-recently-active order (latest first) for the sidebar list.
    session_order: Vec<String>,
    /// Whether the gateway was running last frame (used to detect stop and clear messages).
    was_gateway_running: bool,
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
        let _ = LOG_LINES.get_or_init(|| Mutex::new(VecDeque::new()));
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Debug);
        log::info!("desktop started");
        Self::default()
    }

    /// Poll for probe result and optionally start a new probe. Call each frame.
    fn poll_gateway_probe(&mut self) {
        if let Some(rx) = &self.probe_receiver {
            if let Ok(ok) = rx.try_recv() {
                self.gateway_probe_completed = true;
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

    /// When gateway status is received, ensure current model is in the available list for the backend; if not, switch to gateway default or first available.
    fn reconcile_model_with_status(&mut self) {
        let Some(ref details) = self.gateway_status else { return };
        let backend = details.default_backend.as_deref().unwrap_or("ollama");
        let models: &[String] = if backend == "lmstudio" {
            &details.lm_studio_models
        } else {
            &details.ollama_models
        };
        if models.is_empty() {
            return;
        }
        let effective = self
            .current_model
            .as_deref()
            .or(details.default_model.as_deref())
            .or(self.default_model.as_deref());
        let in_list = effective.map(|m| models.iter().any(|x| x == m)).unwrap_or(false);
        if !in_list {
            self.current_model = details
                .default_model
                .clone()
                .filter(|m| models.contains(m))
                .or_else(|| models.first().cloned());
        }
    }

    /// Poll for status fetch result and optionally start a new fetch when gateway is running. Call each frame.
    fn poll_status_fetch(&mut self) {
        if let Some(rx) = &self.status_receiver {
            if let Ok(result) = rx.try_recv() {
                self.gateway_status = result.ok();
                self.reconcile_model_with_status();
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

    /// Move a session to the front of session_order (most recently active first).
    fn move_session_to_front(&mut self, session_id: &str) {
        self.session_order.retain(|id| id != session_id);
        self.session_order.insert(0, session_id.to_string());
    }

    /// Poll for session.message events from the gateway and update local session timelines.
    /// Skip events for our desktop session (chat_session_id) so we don't duplicate messages
    /// that we already add via start_chat_turn + poll_chat_turn.
    fn poll_session_events(&mut self) {
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
            if self.chat_session_id.as_deref() == Some(ev.session_id.as_str()) {
                continue;
            }
            // When we're waiting for a new-session reply, skip events for sessions we don't have yet
            // so we don't duplicate the first user message (gateway echoes it before our reply arrives).
            if self.chat_turn_receiver.is_some()
                && self.chat_session_id.is_none()
                && !self.session_messages.contains_key(&ev.session_id)
            {
                continue;
            }
            let session_id = ev.session_id.clone();
            let entry = self
                .session_messages
                .entry(session_id.clone())
                .or_insert_with(Vec::new);
            entry.push(ChatMessage {
                role: ev.role,
                content: ev.content,
                tool_calls: None,
            });
            self.session_meta
                .insert(session_id.clone(), (ev.channel_id, ev.conversation_id));
            self.move_session_to_front(&session_id);
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

                        let entry = self
                            .session_messages
                            .entry(reply.session_id.clone())
                            .or_insert_with(Vec::new);
                        if was_new_session {
                            if let Some(ref user_content) = self.pending_user_message {
                                entry.push(ChatMessage::user(user_content.clone()));
                            }
                        }
                        entry.push(ChatMessage::assistant(
                            reply.reply.clone(),
                            if reply.tool_calls.is_empty() {
                                None
                            } else {
                                Some(reply.tool_calls.clone())
                            },
                        ));
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
                                push_log_line(format!("[gateway] {}", l));
                            }
                        }
                    });
                }
                if let Some(stdout) = c.stdout.take() {
                    std::thread::spawn(move || {
                        let reader = std::io::BufReader::new(stdout);
                        for line in reader.lines() {
                            if let Ok(l) = line {
                                push_log_line(format!("[gateway] {}", l));
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
        
        let session_id = self.selected_session_id.clone();
        let is_current_session = session_id == self.chat_session_id;
        if is_current_session {
            self.chat_messages.push(ChatMessage::user(message.clone()));
        }
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
        }
        // Send effective backend so the request matches the UI (default from status when not explicitly set).
        let backend = self
            .current_backend
            .clone()
            .or_else(|| self.gateway_status.as_ref().and_then(|s| s.default_backend.clone()))
            .or_else(|| Some("ollama".to_string()));
        let model = self.current_model.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = run_agent_turn(session_id, message, backend, model);
            let _ = tx.send(result);
        });
        self.chat_turn_receiver = Some(rx);
    }

    /// Renders a single chat message in the same style as the chat screen (frame, role-based fill, content, tool calls).
    fn render_chat_message(ui: &mut egui::Ui, m: &ChatMessage) {
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
                                    egui::RichText::new(format!("Tool: {}", tool_name)).strong(),
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
    }

    /// Render the chat UI (messages + input). Messages area is flexible (fills space) with stick-to-bottom; input and controls are fixed at bottom.
    fn ui_chat(&mut self, ui: &mut egui::Ui, gateway_running: bool) {
        let can_send = gateway_running
            && (self.selected_session_id == self.chat_session_id
                || (self.selected_session_id.is_none() && self.session_messages.is_empty()));

        let row_height = ui.spacing().interact_size.y + 8.0;
        let bottom_section_height = CHAT_INPUT_HEIGHT + 8.0 + row_height + Self::SCREEN_FOOTER_SPACING;
        let available = ui.available_height();
        let messages_height = (available - bottom_section_height).max(CHAT_MESSAGES_MIN_HEIGHT);

        let messages_width = ui.available_width();
        let messages_rect = ui.allocate_exact_size(
            egui::vec2(messages_width, messages_height),
            egui::Sense::hover(),
        ).0;
        let mut messages_ui = ui.child_ui(messages_rect, egui::Layout::top_down(egui::Align::Min));
        // Always use session_messages for the selected session when present to avoid duplicates from chat_messages diverging.
        let messages_to_show: Vec<ChatMessage> = if let Some(ref id) = self.selected_session_id {
            self.session_messages.get(id).cloned().unwrap_or_default()
        } else {
            self.chat_messages.clone()
        };
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(&mut messages_ui, |ui| {
                // Force scroll content to be at least viewport width so the scrollbar stays on the right
                let content_width = ui.available_width();
                ui.allocate_exact_size(egui::vec2(content_width, 0.0), egui::Sense::hover());
                for m in &messages_to_show {
                    Self::render_chat_message(ui, m);
                    ui.add_space(8.0);
                }
            });

        ui.add_space(8.0);

        let text_response = ui.add_enabled_ui(can_send, |ui| {
            ui.add_sized(
                [ui.available_width(), CHAT_INPUT_HEIGHT],
                egui::TextEdit::multiline(&mut self.chat_input),
            )
        });
        let response = text_response.inner;
        ui.add_space(8.0);

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
                // Right-to-left layout: first added = rightmost. We want left-to-right: Backend, Model, /new, Send.
                let mut send_now = false;

                let send_button = ui.add_enabled(can_send, egui::Button::new("Send"));

                let effective_backend = self
                    .current_backend
                    .as_deref()
                    .or_else(|| self.gateway_status.as_ref().and_then(|s| s.default_backend.as_deref()))
                    .unwrap_or("ollama")
                    .to_string();
                // Only models for the selected backend.
                let gateway_models: Vec<String> = self.gateway_status.as_ref().map(|s| {
                    if effective_backend == "lmstudio" {
                        s.lm_studio_models.clone()
                    } else {
                        s.ollama_models.clone()
                    }
                }).unwrap_or_default();
                let effective_default_model = self.gateway_status.as_ref().and_then(|s| s.default_model.clone()).or_else(|| self.default_model.clone());

                // Model dropdown: only models for the selected backend.
                let model_options: Vec<String> = gateway_models;
                if !model_options.is_empty() {
                    ui.add_space(8.0);
                    let current_label = self
                        .current_model
                        .as_deref()
                        .or(effective_default_model.as_deref())
                        .unwrap_or("—")
                        .to_string();
                    ui.add_enabled_ui(can_send, |ui| {
                        egui::ComboBox::from_id_source("model_select")
                            .selected_text(current_label.as_str())
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
                    });
                }

                // Backend dropdown: only show enabled backends (from config).
                ui.add_space(8.0);
                let enabled_backends_list: Vec<String> = {
                    let (config, _) = lib::config::load_config(None)
                        .unwrap_or((lib::config::Config::default(), std::path::PathBuf::new()));
                    if config.agents.enabled_backends.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
                        let (default, _) = lib::config::resolve_effective_backend_and_model(&config.agents);
                        vec![default]
                    } else {
                        let mut seen = std::collections::HashSet::new();
                        config.agents.enabled_backends.as_ref().unwrap()
                            .iter()
                            .map(|s| s.trim().to_lowercase())
                            .filter(|s| !s.is_empty())
                            .filter(|s| *s == "ollama" || *s == "lmstudio" || *s == "lm_studio")
                            .map(|s| if s == "lm_studio" { "lmstudio".to_string() } else { s })
                            .filter(|s| seen.insert(s.clone()))
                            .collect()
                    }
                };
                if !enabled_backends_list.is_empty() {
                    let selected = if enabled_backends_list.contains(&effective_backend) {
                        effective_backend.clone()
                    } else {
                        enabled_backends_list.first().cloned().unwrap_or_else(|| "—".to_string())
                    };
                    ui.add_enabled_ui(can_send, |ui| {
                        egui::ComboBox::from_id_source("backend_select")
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                for b in &enabled_backends_list {
                                    if ui.selectable_label(effective_backend == b.as_str(), b).clicked() {
                                        self.current_backend = Some(b.clone());
                                        self.current_model = None;
                                    }
                                }
                            });
                    });
                }

                ui.add_space(8.0);
                if ui.add_enabled(can_send, egui::Button::new("/new")).clicked() {
                    self.start_new_session();
                }

                if send_button.clicked() {
                    send_now = true;
                }
                if can_send && response.has_focus() {
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
        ui.add_space(Self::SCREEN_FOOTER_SPACING);
    }

    fn ui_info_screen(&mut self, ui: &mut egui::Ui, running: bool) {
        const INFO_LINE_SPACING: f32 = 6.0;
        const INFO_SUBSECTION_SPACING: f32 = 18.0;
        ui.add_space(24.0);
        ui.heading("Info");
        ui.add_space(Self::SCREEN_TITLE_BOTTOM_SPACING);
        let (config, _) = lib::config::load_config(None)
            .unwrap_or((lib::config::Config::default(), std::path::PathBuf::new()));
        if self.default_model.is_none() {
            let (_, model) = lib::config::resolve_effective_backend_and_model(&config.agents);
            self.default_model = Some(model);
        }

        let port = config.gateway.port;
        let bind = config.gateway.bind.trim();
        let auth_mode = match config.gateway.auth.mode {
            lib::config::GatewayAuthMode::None => "none",
            lib::config::GatewayAuthMode::Token => "token",
        };
        let (protocol, status_port, status_bind, status_auth) = if let Some(ref s) = self.gateway_status {
            (s.protocol, s.port, s.bind.clone(), s.auth.clone())
        } else {
            (1, port, bind.to_string(), auth_mode.to_string())
        };

        let available = ui.available_height();
        let content_height = (available - Self::SCREEN_FOOTER_SPACING).max(0.0);
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), content_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
        ui.columns(2, |columns| {
            // Left column: Gateway, Channels, Agents, Skills
            {
                let ui_left = &mut columns[0];
                ui_left.label(egui::RichText::new("Gateway").strong());
                ui_left.add_space(INFO_LINE_SPACING);
                ui_left.label(format!("Status: {}", if running { "running" } else { "stopped" }));
                ui_left.add_space(INFO_LINE_SPACING);
                ui_left.label(format!("Bind: {}", status_bind));
                ui_left.add_space(INFO_LINE_SPACING);
                ui_left.label(format!("Port: {}", status_port));
                ui_left.add_space(INFO_LINE_SPACING);
                ui_left.label(format!("Protocol: {}", protocol));
                ui_left.add_space(INFO_LINE_SPACING);
                ui_left.label(format!("Auth: {}", status_auth));
                ui_left.add_space(INFO_LINE_SPACING);
                if let Some(ref err) = self.gateway_error {
                    ui_left.colored_label(egui::Color32::RED, err);
                    ui_left.add_space(INFO_LINE_SPACING);
                }
                ui_left.add_space(INFO_SUBSECTION_SPACING);

                ui_left.label(egui::RichText::new("Channels").strong());
                ui_left.add_space(INFO_LINE_SPACING);
                let telegram_configured = config.channels.telegram.bot_token.is_some()
                    || config.channels.telegram.webhook_url.is_some();
                if telegram_configured {
                    if let Some(ref t) = config.channels.telegram.bot_token {
                        ui_left.label(format!("Telegram bot token: {}", if t.trim().is_empty() { "(empty)" } else { "set" }));
                        ui_left.add_space(INFO_LINE_SPACING);
                    }
                    if let Some(ref w) = config.channels.telegram.webhook_url {
                        ui_left.label(format!("Telegram webhook: {}", w));
                        ui_left.add_space(INFO_LINE_SPACING);
                    }
                } else {
                    ui_left.label("Not configured.");
                    ui_left.add_space(INFO_LINE_SPACING);
                }
                ui_left.add_space(INFO_SUBSECTION_SPACING);

                ui_left.label(egui::RichText::new("Agents").strong());
                ui_left.add_space(INFO_LINE_SPACING);
                let (backend_label, current_model) = if let Some(ref s) = self.gateway_status {
                    let backend = s.default_backend.as_deref().unwrap_or("ollama").to_string();
                    let model = self
                        .current_model
                        .clone()
                        .or_else(|| s.default_model.clone())
                        .or_else(|| self.default_model.clone())
                        .unwrap_or_else(|| "—".to_string());
                    (backend, model)
                } else {
                    let (backend, model) = lib::config::resolve_effective_backend_and_model(&config.agents);
                    let model = self
                        .current_model
                        .clone()
                        .or(Some(model))
                        .unwrap_or_else(|| "—".to_string());
                    (backend, model)
                };
                ui_left.label(format!("Current backend: {}", backend_label));
                ui_left.add_space(INFO_LINE_SPACING);
                ui_left.label(format!("Current model: {}", current_model));
                ui_left.add_space(INFO_LINE_SPACING);
                let enabled_backends_display = if config.agents.enabled_backends.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
                    let (default, _) = lib::config::resolve_effective_backend_and_model(&config.agents);
                    default
                } else {
                    let v = config.agents.enabled_backends.as_ref().unwrap();
                    let s = v.iter().map(|s| s.as_str()).filter(|s| !s.trim().is_empty()).collect::<Vec<_>>().join(", ");
                    if s.is_empty() {
                        let (default, _) = lib::config::resolve_effective_backend_and_model(&config.agents);
                        default
                    } else {
                        s
                    }
                };
                ui_left.label(format!("Enabled backends: {}", enabled_backends_display));
                ui_left.add_space(INFO_LINE_SPACING);
                if let Some(ref w) = config.agents.workspace {
                    ui_left.label(format!("Workspace: {}", w.display()));
                    ui_left.add_space(INFO_LINE_SPACING);
                }
                if let Some(ref b) = config.agents.backends {
                    if b.ollama.as_ref().and_then(|o| o.base_url.as_ref()).map(|u| !u.trim().is_empty()).unwrap_or(false) {
                        ui_left.label(format!("Ollama base URL: {}", b.ollama.as_ref().unwrap().base_url.as_ref().unwrap()));
                        ui_left.add_space(INFO_LINE_SPACING);
                    }
                    if b.lm_studio.as_ref().and_then(|l| l.base_url.as_ref()).map(|u| !u.trim().is_empty()).unwrap_or(false) {
                        ui_left.label(format!("LM Studio base URL: {}", b.lm_studio.as_ref().unwrap().base_url.as_ref().unwrap()));
                        ui_left.add_space(INFO_LINE_SPACING);
                    }
                }
                if let Some(ref s) = self.gateway_status {
                    if !s.ollama_models.is_empty() {
                        ui_left.label(format!("Ollama models: {}", s.ollama_models.join(", ")));
                        ui_left.add_space(INFO_LINE_SPACING);
                    }
                    if !s.lm_studio_models.is_empty() {
                        ui_left.label(format!("LM Studio models: {}", s.lm_studio_models.join(", ")));
                        ui_left.add_space(INFO_LINE_SPACING);
                    }
                } else if running {
                    ui_left.label("(loading available models)");
                    ui_left.add_space(INFO_LINE_SPACING);
                } else {
                    ui_left.label("(start gateway to see available models)");
                    ui_left.add_space(INFO_LINE_SPACING);
                }
                ui_left.add_space(INFO_SUBSECTION_SPACING);

                ui_left.label(egui::RichText::new("Skills").strong());
                ui_left.add_space(INFO_LINE_SPACING);
                let skills_configured = config.skills.directory.is_some()
                    || !config.skills.extra_dirs.is_empty()
                    || !config.skills.enabled.is_empty()
                    || config.skills.context_mode != lib::config::SkillContextMode::Full
                    || config.skills.allow_scripts;
                if skills_configured {
                    if let Some(ref d) = config.skills.directory {
                        ui_left.label(format!("Directory: {}", d.display()));
                        ui_left.add_space(INFO_LINE_SPACING);
                    }
                    if !config.skills.extra_dirs.is_empty() {
                        ui_left.label(format!("Extra dirs: {}", config.skills.extra_dirs.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join(", ")));
                        ui_left.add_space(INFO_LINE_SPACING);
                    }
                    if !config.skills.enabled.is_empty() {
                        ui_left.label(format!("Enabled: {}", config.skills.enabled.join(", ")));
                        ui_left.add_space(INFO_LINE_SPACING);
                    }
                    let context_mode_str = match config.skills.context_mode {
                        lib::config::SkillContextMode::Full => "full",
                        lib::config::SkillContextMode::ReadOnDemand => "readOnDemand",
                    };
                    ui_left.label(format!("Context mode: {}", context_mode_str));
                    ui_left.add_space(INFO_LINE_SPACING);
                    if config.skills.allow_scripts {
                        ui_left.label("Allow scripts: true");
                        ui_left.add_space(INFO_LINE_SPACING);
                    }
                } else {
                    ui_left.label("Not configured.");
                    ui_left.add_space(INFO_LINE_SPACING);
                }
            }

            // Right column: Context and Skills (aligned with Gateway in left column)
            {
                let ui_right = &mut columns[1];
                let loading = !running || self.gateway_status.is_none() || self.status_receiver.is_some();

                // Context: date + agent context
                ui_right.label(egui::RichText::new("Context").strong());
                ui_right.add_space(INFO_LINE_SPACING);
                let context_text = self.gateway_status.as_ref().and_then(|s| {
                    let mut out = String::new();
                    if let Some(ref d) = s.date {
                        out.push_str("Date: ");
                        out.push_str(d);
                        out.push_str("\n\n");
                    }
                    if let Some(ref a) = s.agent_context {
                        if !a.trim().is_empty() {
                            out.push_str(a);
                        }
                    }
                    if out.trim().is_empty() {
                        None
                    } else {
                        Some(out)
                    }
                }).or_else(|| self.gateway_status.as_ref().and_then(|s| s.system_context.clone()));
                if let Some(text) = context_text {
                    let available = ui_right.available_height();
                    let context_height = (available * 0.4).max(40.0);
                    egui::ScrollArea::vertical()
                        .id_source("info_context_scroll")
                        .max_height(context_height)
                        .show(ui_right, |ui| {
                            egui::Frame::none()
                                .inner_margin(egui::Margin {
                                    left: 0.0,
                                    right: 16.0,
                                    top: 0.0,
                                    bottom: 0.0,
                                })
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(text.as_str()).family(egui::FontFamily::Monospace),
                                    );
                                });
                        });
                } else if !running {
                    ui_right.label("(start gateway to see context)");
                } else if loading {
                    ui_right.label("(loading context)");
                } else {
                    ui_right.label("No context loaded.");
                }

                ui_right.add_space(INFO_SUBSECTION_SPACING);
                ui_right.label(egui::RichText::new("Skills").strong());
                ui_right.add_space(INFO_LINE_SPACING);
                let is_read_on_demand = self
                    .gateway_status
                    .as_ref()
                    .and_then(|s| s.context_mode.as_deref())
                    .map(|m| m == "readOnDemand")
                    .unwrap_or(false);
                if is_read_on_demand {
                    ui_right.label("When read-on-demand is enabled, full skill docs are loaded on demand via the read_skill tool.");
                    ui_right.add_space(INFO_LINE_SPACING);
                }
                let skills_text = self
                    .gateway_status
                    .as_ref()
                    .and_then(|s| s.skills_context.as_deref())
                    .filter(|s| !s.trim().is_empty());
                if let Some(text) = skills_text {
                    let height = ui_right.available_height();
                    egui::ScrollArea::vertical()
                        .id_source("info_skills_scroll")
                        .max_height(height)
                        .show(ui_right, |ui| {
                            egui::Frame::none()
                                .inner_margin(egui::Margin {
                                    left: 0.0,
                                    right: 16.0,
                                    top: 0.0,
                                    bottom: 0.0,
                                })
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(text).family(egui::FontFamily::Monospace),
                                    );
                                });
                        });
                } else if !running {
                    ui_right.label("(start gateway to see context)");
                } else if loading {
                    ui_right.label("(loading context)");
                } else {
                    ui_right.label("No skills loaded.");
                }
            }
        });
            },
        );
        ui.add_space(Self::SCREEN_FOOTER_SPACING);
    }

    fn ui_logs_screen(&self, ui: &mut egui::Ui) {
        ui.add_space(24.0);
        ui.heading("Logs");
        ui.add_space(Self::SCREEN_TITLE_BOTTOM_SPACING);

        let lines: Vec<String> = log_buffer()
            .lock()
            .map(|b| b.iter().cloned().collect())
            .unwrap_or_default();

        let available = ui.available_height();
        let scroll_height = (available - Self::SCREEN_FOOTER_SPACING).max(0.0);
        egui::ScrollArea::vertical()
            .max_height(scroll_height)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &lines {
                    ui.label(
                        egui::RichText::new(line.as_str()).family(egui::FontFamily::Monospace),
                    );
                }
                if lines.is_empty() {
                    ui.label("No log output yet.");
                }
            });
        ui.add_space(Self::SCREEN_FOOTER_SPACING);
    }
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

        // Header with title and gateway controls only
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                .show(ui, |ui| {
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        ui.heading("Chai");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !self.gateway_probe_completed {
                                ui.add_enabled(false, egui::Button::new("Start gateway"));
                            } else if running {
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

        let current_screen = &mut self.current_screen;
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(140.0)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                    .show(ui, |ui| {
                        ui.add_space(24.0);
                        if ui.selectable_label(*current_screen == Screen::Info, "Info").clicked() {
                            *current_screen = Screen::Info;
                        }
                        ui.add_space(12.0);
                        if ui.selectable_label(*current_screen == Screen::Chat, "Chat").clicked() {
                            *current_screen = Screen::Chat;
                        }
                        ui.add_space(12.0);
                        if ui.selectable_label(*current_screen == Screen::Logs, "Logs").clicked() {
                            *current_screen = Screen::Logs;
                        }
                    });
            });

        // Right sidebar: sessions list when on Chat (select which session's messages to show)
        if self.current_screen == Screen::Chat {
            // Default selected session to current chat session when none selected
            if self.selected_session_id.is_none() && self.chat_session_id.is_some() {
                self.selected_session_id = self.chat_session_id.clone();
            }
            egui::SidePanel::right("sessions_panel")
                .resizable(false)
                .exact_width(220.0)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                        .show(ui, |ui| {
                            ui.add_space(24.0);
                            ui.heading("Sessions");
                            ui.add_space(Self::SCREEN_TITLE_BOTTOM_SPACING);
                            if !running {
                                ui.label("Start the gateway to see sessions.");
                            } else {
                                if self.chat_session_id.is_none() {
                                    if ui.button("New session").clicked() {
                                        self.selected_session_id = None;
                                    }
                                    ui.add_space(8.0);
                                }
                                for session_id in self.session_order.iter().filter(|id| self.session_messages.contains_key(*id)).cloned().collect::<Vec<_>>() {
                                    let is_selected = self.selected_session_id.as_deref() == Some(session_id.as_str());
                                    let display = session_label_display(
                                        &session_id,
                                        self.session_meta.get(&session_id),
                                    );
                                    if ui.selectable_label(is_selected, display).clicked() {
                                        self.selected_session_id = Some(session_id);
                                    }
                                }
                                if self.session_messages.is_empty() {
                                    ui.label("No sessions yet. Send a message to start one.");
                                }
                            }
                            ui.add_space(Self::SCREEN_FOOTER_SPACING);
                        });
                });
        }

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
                        self.ui_chat(ui, running);
                    });
            } else if self.current_screen == Screen::Logs {
                // Logs screen has its own scroll area for the log lines; avoid double scrollbars
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                    .show(ui, |ui| {
                        self.ui_logs_screen(ui);
                    });
            } else if self.current_screen == Screen::Info {
                // Info screen has its own scroll area in the System Context column; avoid outer scrollbar
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                    .show(ui, |ui| {
                        self.ui_info_screen(ui, running);
                    });
            } else {
                egui::ScrollArea::vertical().show(ui, |_ui| {
                    egui::Frame::none()
                        .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                        .show(_ui, |_| {
                            match self.current_screen {
                                Screen::Info => {}
                                Screen::Logs => {}
                                Screen::Chat => {}
                            }
                        });
                });
            }
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
