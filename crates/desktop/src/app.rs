//! Chai Desktop — egui app state and UI.

use eframe::egui;
use std::collections::{BTreeMap, HashMap};
use std::io::BufRead;
use std::process::{Child, Stdio};
use std::sync::mpsc;

mod screens;
mod state;
mod ui;
mod types;

pub use types::{AgentReply, ChatMessage, GatewayStatusDetails, SessionEvent};

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

/// Frames between gateway probes (probe at ~1 Hz if 60 fps).
const PROBE_INTERVAL_FRAMES: u32 = 60;

/// Frames between WebSocket status fetches when gateway is running (~0.5 Hz).
const STATUS_INTERVAL_FRAMES: u32 = 120;

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
    /// Cached list of enabled backends for the chat backend dropdown (invalidated when Config screen is shown).
    cached_enabled_backends: Option<Vec<String>>,
}

/// Standard screen layout: title, optional subtitle, then body with a footer
/// gap at the bottom. Use for full-screen panels (Context, Skills, Info, Config).
pub fn ui_screen(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: Option<&str>,
    body: impl FnOnce(&mut egui::Ui),
) {
    // Top padding and title
    ui.add_space(24.0);
    ui.heading(title);
    ui.add_space(ChaiApp::SCREEN_TITLE_BOTTOM_SPACING);
    if let Some(text) = subtitle {
        ui.label(text);
        ui.add_space(6.0);
    }
    // Spacing before main body content
    ui.add_space(18.0);

    // Lay out a full-height body area with consistent footer spacing at the bottom.
    // The `body` closure receives a UI that fills the remaining vertical space
    // after reserving `SCREEN_FOOTER_SPACING` at the bottom.
    let available = ui.available_height();
    let content_height = (available - ChaiApp::SCREEN_FOOTER_SPACING).max(0.0);
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), content_height),
        egui::Layout::top_down(egui::Align::Min),
        body,
    );
    ui.add_space(ChaiApp::SCREEN_FOOTER_SPACING);
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
            cached_enabled_backends: None,
        }
    }
}

impl ChaiApp {
    /// Space between the main screen title and the content below on full‑screen panels.
    const SCREEN_TITLE_BOTTOM_SPACING: f32 = 9.0;
    /// Space between the bottom of the content and the window edge on full‑screen panels.
    const SCREEN_FOOTER_SPACING: f32 = 48.0;

    /// Returns the list of enabled backends for the chat dropdown. Cached until the Config screen is shown.
    pub fn enabled_backends(&mut self) -> Vec<String> {
        if let Some(ref list) = self.cached_enabled_backends {
            return list.clone();
        }
        let (config, _) = lib::config::load_config(None)
            .unwrap_or((lib::config::Config::default(), std::path::PathBuf::new()));
        // Start from enabledBackends when set; otherwise fall back to the effective default backend.
        let mut list: Vec<String> =
            if config
                .agents
                .enabled_backends
                .as_ref()
                .map(|v| v.is_empty())
                .unwrap_or(true)
            {
                let (default, _) =
                    lib::config::resolve_effective_backend_and_model(&config.agents);
                vec![default]
            } else {
                let mut seen = std::collections::HashSet::new();
                config
                    .agents
                    .enabled_backends
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .filter(|s| {
                        *s == "ollama"
                            || *s == "lmstudio"
                            || *s == "lm_studio"
                            || *s == "nim"
                            || *s == "nvidia_nim"
                    })
                    .map(|s| {
                        if s == "lm_studio" {
                            "lmstudio".to_string()
                        } else if s == "nvidia_nim" {
                            "nim".to_string()
                        } else {
                            s
                        }
                    })
                    .filter(|s| seen.insert(s.clone()))
                    .collect()
            };
        // Always include the effective default backend in the dropdown so the UI reflects
        // which backend the gateway will actually use when no override is provided.
        let (default_backend, _) =
            lib::config::resolve_effective_backend_and_model(&config.agents);
        if !list.contains(&default_backend) {
            list.push(default_backend);
        }
        self.cached_enabled_backends = Some(list.clone());
        list
    }

    /// Invalidates the enabled-backends cache (call when showing Config so next Chat use reloads).
    pub fn invalidate_enabled_backends_cache(&mut self) {
        self.cached_enabled_backends = None;
    }

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
                        let last_is_same = entry.last().map(|m| {
                            m.role == assistant_msg.role && m.content == assistant_msg.content
                        }).unwrap_or(false);
                        if last_is_same {
                            // Prefer the agent response's tool_calls (source of truth for this turn).
                            let last = entry.last_mut().unwrap();
                            last.tool_calls = assistant_msg.tool_calls;
                        } else {
                            entry.push(assistant_msg);
                        }
                        self.session_meta
                            .entry(reply.session_id.clone())
                            .or_insert((None, None));

                        self.pending_user_message = None;
                        if was_new_session {
                            // Retain only the most recent pre-session error so it doesn't disappear when we switch to the new session, without piling every past failure onto the new session.
                            let last_pre_error = self
                                .chat_messages
                                .iter()
                                .rev()
                                .find(|m| m.role == "error")
                                .cloned();
                            if let Some(err_msg) = last_pre_error {
                                let entry = self
                                    .session_messages
                                    .get_mut(&reply.session_id)
                                    .expect("entry exists");
                                entry.insert(0, err_msg);
                            }
                        }
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
                        let err_text = e.clone();
                        // Show the full error as an in-stream chat message.
                        self.chat_messages
                            .push(ChatMessage::error(err_text.clone()));
                        // Also attach to the current session's messages when we know the id.
                        if let Some(ref sid) = self.chat_session_id {
                            let entry = self
                                .session_messages
                                .entry(sid.clone())
                                .or_insert_with(Vec::new);
                            entry.push(ChatMessage::error(err_text));
                        }
                        self.chat_error = None;
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
        let binary = match state::gateway::resolve_chai_binary() {
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
            let result = state::gateway::run_agent_turn(session_id, message, backend, model);
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
                        let subtitle = if !running {
                            Some("Start the gateway to chat with the model.")
                        } else {
                            None
                        };
                        ui_screen(ui, "Chat", subtitle, |ui| {
                            screens::chat::ui_chat(self, ui, running);
                        });
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
