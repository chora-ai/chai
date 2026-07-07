//! Chai Desktop — egui app state and UI.

use eframe::egui;
use std::collections::{BTreeMap, HashMap};
use std::io::BufRead;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

mod screens;
mod state;
mod types;
mod ui;

pub use types::{AgentReply, AgentSkillsRuntime, ChannelBinding, ChatMessage, GatewayStatusDetails, ProviderStatusInfo, SessionEvent, SessionHistory, SessionSummary};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Screen {
    #[default]
    Chat,
    Files,
    Gateway,
    Agent,
    Tools,
    Config,
    Skills,
    Logging,
    Settings,
}

/// **Gateway** screen: human-readable dashboard vs full `status` WebSocket response JSON.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum StatusViewMode {
    #[default]
    Dashboard,
    RawJson,
}

/// **Config** screen: human-readable dashboard vs on-disk **`config.json`**.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ConfigViewMode {
    #[default]
    Dashboard,
    RawJson,
}

/// **Settings** screen: human-readable dashboard vs on-disk **`desktop.json`**.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SettingsViewMode {
    #[default]
    Dashboard,
    RawJson,
}

/// Frames between gateway probes (probe at ~1 Hz if 60 fps).
const PROBE_INTERVAL_FRAMES: u32 = 60;

/// Frames between WebSocket status fetches when gateway is running (~0.5 Hz).
const STATUS_INTERVAL_FRAMES: u32 = 120;

/// Per-profile gateway state. Stored in `ChaiApp::gateways` keyed by profile name.
/// When the user switches profiles, the current gateway state is saved to the map
/// and the new profile's state is loaded. This allows multiple gateways to run
/// simultaneously on different profiles with independent session state.
struct GatewayState {
    /// When Some, the gateway subprocess owned by the desktop for this profile.
    process: Option<Child>,
    /// Whether this profile's gateway responded to the last TCP probe.
    responds: bool,
    /// Last gateway status fetched via WebSocket for this profile.
    status: Option<GatewayStatusDetails>,
    /// True once a status fetch has completed with an error since the gateway was last detected.
    status_fetch_ever_failed: bool,
    /// When Some, a status fetch is in flight for this profile's gateway.
    status_receiver: Option<mpsc::Receiver<Result<GatewayStatusDetails, String>>>,
    /// Frames since we last started a status fetch for this profile.
    frames_since_status: u32,
    /// When Some, a probe is in flight for this profile's gateway.
    probe_receiver: Option<mpsc::Receiver<bool>>,
    /// Frames since we last started a probe for this profile.
    frames_since_probe: u32,
    /// True once we have received at least one probe result for this profile.
    probe_completed: bool,
    /// When Some, a gateway log fetch is in flight for this profile's gateway.
    logs_receiver: Option<mpsc::Receiver<Result<(Vec<String>, u64), String>>>,
    /// Frames since we last started a gateway log fetch for this profile.
    frames_since_logs: u32,
    /// Sequence cursor for gateway log deduplication for this profile.
    logs_cursor: u64,
    /// When Some, a session events stream is in flight for this profile's gateway.
    session_events_receiver: Option<mpsc::Receiver<SessionEvent>>,
    /// Cancel flag for the session events listener thread. When Some, the
    /// background thread checks this flag before each reconnection attempt.
    /// Setting it to true signals the thread to exit cleanly instead of
    /// reconnecting indefinitely after a disconnect or gateway stop.
    session_events_cancel: Option<Arc<AtomicBool>>,
    /// Current chat session id for this profile (created on first agent call).
    chat_session_id: Option<String>,
    /// In-memory chat transcript for the current session in this profile.
    chat_messages: Vec<ChatMessage>,
    /// When Some, a chat turn is in flight for this profile's gateway.
    chat_turn_receiver: Option<mpsc::Receiver<Result<AgentReply, String>>>,
    /// When Some, a stop request is in flight for this profile's gateway.
    stop_receiver: Option<mpsc::Receiver<Result<bool, String>>>,
    /// True after the user requests a stop, until the chat turn actually completes.
    chat_stopping: bool,
    /// User message we sent for the in-flight turn (used when reply creates a new session).
    pending_user_message: Option<String>,
    /// True when the in-flight turn was started for a new (previously unbound) session.
    chat_turn_is_new_session: bool,
    /// Session messages for this profile's gateway, keyed by session id.
    session_messages: BTreeMap<String, Vec<ChatMessage>>,
    /// Summary metadata for each session in this profile.
    session_summaries: HashMap<String, SessionSummary>,
    /// Session IDs in most-recently-active order for this profile.
    session_order: Vec<String>,
    /// When Some, a `sessions.list` fetch is in flight for this profile.
    sessions_list_receiver: Option<mpsc::Receiver<Result<Vec<SessionSummary>, String>>>,
    /// True once the session list has been fetched for this profile's gateway.
    sessions_list_fetched: bool,
    /// When Some, a `sessions.history` fetch is in flight for this profile.
    sessions_history_receiver: Option<(String, mpsc::Receiver<Result<SessionHistory, String>>)>,
    /// Session id whose history is currently loading for this profile.
    loading_session_id: Option<String>,
    /// When Some, a `sessions.delete` fetch is in flight for this profile.
    sessions_delete_receiver: Option<(String, mpsc::Receiver<Result<bool, String>>)>,
    /// When Some, a `sessions.delete_all` fetch is in flight for this profile.
    sessions_delete_all_receiver: Option<mpsc::Receiver<Result<usize, String>>>,
    /// Whether the "Clear all" confirmation dialog is showing for this profile.
    show_clear_all_confirm: bool,
    /// Session whose messages are shown in the chat area for this profile.
    selected_session_id: Option<String>,
    /// Currently selected provider override for this profile.
    current_provider: Option<String>,
    /// Currently selected model override for this profile.
    current_model: Option<String>,
    /// Default model from config for this profile.
    default_model: Option<String>,
    /// Active orchestrator id for this profile's chat screen.
    active_orchestrator_id: Option<String>,
    /// Whether the gateway was running last frame (per-profile, used to detect stop).
    was_running: bool,
    /// True when the user explicitly stopped this profile's gateway via the Stop button.
    was_stopped_by_user: bool,
    /// True when the user explicitly disconnected a remote profile. Prevents
    /// the periodic TCP probe from re-detecting the remote gateway and
    /// automatically reconnecting. Cleared when the user clicks Connect.
    remote_disconnected: bool,
    /// On-demand per-agent detail cache for this profile's gateway.
    agent_detail_cache: BTreeMap<String, crate::app::types::AgentDetail>,
    /// Last error from an agent detail fetch for this profile.
    agent_detail_fetch_error: Option<(String, String)>,
    /// When Some, an `agentDetail` WS fetch is in flight for this profile.
    agent_detail_receiver: Option<(String, mpsc::Receiver<Result<crate::app::types::AgentDetail, String>>)>,
    /// Agent id that was last requested for detail fetch for this profile.
    agent_detail_requested_id: Option<String>,
    /// **Agent**, **Tools**, and **Skills**: which agent id is selected for this profile.
    dashboard_agent_id: Option<String>,
}

impl Default for GatewayState {
    fn default() -> Self {
        Self {
            process: None,
            responds: false,
            status: None,
            status_fetch_ever_failed: false,
            status_receiver: None,
            frames_since_status: 0,
            probe_receiver: None,
            frames_since_probe: 0,
            probe_completed: false,
            logs_receiver: None,
            frames_since_logs: 0,
            logs_cursor: 0,
            session_events_receiver: None,
            session_events_cancel: None,
            chat_session_id: None,
            chat_messages: Vec::new(),
            chat_turn_receiver: None,
            stop_receiver: None,
            chat_stopping: false,
            pending_user_message: None,
            chat_turn_is_new_session: false,
            session_messages: BTreeMap::new(),
            session_summaries: HashMap::new(),
            session_order: Vec::new(),
            sessions_list_receiver: None,
            sessions_list_fetched: false,
            sessions_history_receiver: None,
            loading_session_id: None,
            sessions_delete_receiver: None,
            sessions_delete_all_receiver: None,
            show_clear_all_confirm: false,
            selected_session_id: None,
            current_provider: None,
            current_model: None,
            default_model: None,
            active_orchestrator_id: None,
            was_running: false,
            was_stopped_by_user: false,
            remote_disconnected: false,
            agent_detail_cache: BTreeMap::new(),
            agent_detail_fetch_error: None,
            agent_detail_receiver: None,
            agent_detail_requested_id: None,
            dashboard_agent_id: None,
        }
    }
}

pub struct ChaiApp {
    /// Per-profile gateway state. Keyed by profile name.
    /// The active profile's GatewayState is the source of truth for all
    /// UI fields that were previously singular (gateway_status, chat_session_id, etc.).
    gateways: HashMap<String, GatewayState>,
    /// Last error from start gateway (e.g. spawn failed). Not per-profile — only
    /// the most recent start attempt can fail.
    gateway_error: Option<String>,
    /// Current input text for the chat box (not per-profile — it's the user's draft).
    chat_input: String,
    /// Current screen (Chat, Gateway, Agent, Tools, Config, Skills, Logging).
    current_screen: Screen,
    /// Currently selected skill on the Skills screen (by name).
    selected_skill_name: Option<String>,
    /// Cached list of enabled providers for the chat provider dropdown (invalidated when Config screen is shown).
    cached_enabled_providers: Option<Vec<String>>,
    /// **Gateway** screen: show parsed fields or the raw `status` response JSON.
    status_view_mode: StatusViewMode,
    /// Stable buffer for **Tools** screen `TextEdit` (updated when the effective tools JSON changes).
    tools_display_buffer: String,
    /// Config screen: dashboard vs raw file.
    config_view_mode: ConfigViewMode,
    /// **Config** raw view: file text (synced when content changes; avoids `TextEdit` flicker).
    config_raw_display_buffer: String,
    /// Settings screen: dashboard vs raw file.
    settings_view_mode: SettingsViewMode,
    /// **Settings** raw view: file text (synced when content changes; avoids `TextEdit` flicker).
    settings_raw_display_buffer: String,
    /// Profile names under `~/.chai/profiles` (from disk).
    profile_names: Vec<String>,
    /// Persistent active profile from `~/.chai/active`.
    profile_active: String,
    /// Profile switch or symlink read error (shown in header).
    profile_switch_error: Option<String>,
    /// When true, next frame reloads profile list and active name from disk.
    profiles_need_refresh: bool,
    /// Profile names discovered from per-profile gateway.lock scan while gateways are running
    /// (refreshed on probe cadence). Replaces the old singular `gateway_lock_profile`.
    running_profiles: Vec<String>,
    /// Previous frame's `running_profiles`; used to detect when the effective profile changes
    /// so config-dependent caches (providers, model) can be invalidated.
    prev_running_profiles: Vec<String>,

    /// Frames since we last refreshed `running_profiles` from disk.
    frames_since_lock_profile: u32,
    /// Cached config loaded from disk. Invalidated when the file's mtime changes.
    cached_config: Option<(lib::config::Config, lib::profile::ChaiPaths)>,
    /// Mtime of `config.json` when `cached_config` was last read (None = not yet read or file missing).
    cached_config_mtime: Option<SystemTime>,
    /// Cached desktop config loaded from disk. Invalidated when the file's mtime changes.
    cached_desktop_config: Option<lib::config::DesktopConfig>,
    /// Mtime of `desktop.json` when `cached_desktop_config` was last read (None = not yet read or file missing).
    cached_desktop_config_mtime: Option<SystemTime>,
    /// Cached skill entries loaded from the skills directory (on-demand: immediate when empty, periodic only on Skills/Agent screen).
    cached_skills: Option<Vec<lib::skills::SkillEntry>>,
    /// Last error from a skills fetch, shown on the Skills screen when cached_skills is None.
    skills_fetch_error: Option<String>,
    /// When Some, a skills fetch is in flight; we read the result here.
    skills_fetch_receiver: Option<mpsc::Receiver<Result<Vec<lib::skills::SkillEntry>, String>>>,
    /// Frames since we last started a skills fetch.
    frames_since_skills_fetch: u32,
}

/// Return the current time as an ISO 8601 string (e.g. "2025-06-10T12:34:56Z").
/// Used for session timestamps when a session is created locally (before the
/// gateway provides authoritative timestamps via `sessions.list`).
fn now_iso8601() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Compute date/time components from unix timestamp.
    // Simplified algorithm — valid for 1970–2099.
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;
    // Year computation.
    let mut remaining_days = days;
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    // Month computation.
    let month_days = [31, if is_leap_year(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u64;
    for &md in &month_days {
        if remaining_days < md {
            break;
        }
        remaining_days -= md;
        month += 1;
    }
    let day = remaining_days + 1;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Standard screen layout: title, optional subtitle, then body with a footer
/// gap at the bottom. Use for full-screen panels (Agent, Skills, Gateway, Config).
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
            gateways: HashMap::new(),
            gateway_error: None,
            chat_input: String::new(),
            current_screen: Screen::default(),
            selected_skill_name: None,
            cached_enabled_providers: None,
            status_view_mode: StatusViewMode::default(),
            tools_display_buffer: String::new(),
            config_view_mode: ConfigViewMode::default(),
            config_raw_display_buffer: String::new(),
            settings_view_mode: SettingsViewMode::default(),
            settings_raw_display_buffer: String::new(),
            profile_names: Vec::new(),
            profile_active: String::new(),
            profile_switch_error: None,
            profiles_need_refresh: true,
            running_profiles: Vec::new(),
            prev_running_profiles: Vec::new(),
            frames_since_lock_profile: 0,
            cached_config: None,
            cached_config_mtime: None,
            cached_desktop_config: None,
            cached_desktop_config_mtime: None,
            cached_skills: None,
            skills_fetch_error: None,
            skills_fetch_receiver: None,
            frames_since_skills_fetch: 0,
        }
    }
}

impl ChaiApp {
    /// Space between the main screen title and the content below on full‑screen panels.
    const SCREEN_TITLE_BOTTOM_SPACING: f32 = 9.0;
    /// Space between the bottom of the content and the window edge on full‑screen panels.
    const SCREEN_FOOTER_SPACING: f32 = 48.0;

    // ── Profile helpers ──

    /// The name of the currently active profile.
    pub fn active_profile(&self) -> &str {
        &self.profile_active
    }

    // ── GatewayState accessors (delegate to the active profile's entry) ──

    /// Get a mutable reference to the active profile's `GatewayState`, creating it if needed.
    fn gw(&mut self) -> &mut GatewayState {
        self.gateways.entry(self.profile_active.clone()).or_default()
    }

    /// Get a reference to the active profile's `GatewayState`, if one exists.
    fn gw_ref(&self) -> Option<&GatewayState> {
        self.gateways.get(&self.profile_active)
    }

    /// True if we started the gateway for the active profile and it is still running (we can stop it).
    fn gateway_owned(&mut self) -> bool {
        let Some(gw) = self.gateways.get_mut(&self.profile_active) else {
            return false;
        };
        if let Some(ref mut child) = gw.process {
            if child.try_wait().ok().flatten().is_some() {
                gw.process = None;
                return false;
            }
            return true;
        }
        false
    }

    /// Whether the active profile's gateway is running (owned or external).
    fn gateway_running(&self) -> bool {
        self.running_profiles.iter().any(|p| *p == self.profile_active)
    }

    /// Stop the background session events listener thread for the active profile.
    ///
    /// Signals the cancel flag so the thread exits instead of reconnecting,
    /// then drops the receiver. This must be called whenever the gateway is
    /// disconnected (remote profile disconnect, local gateway stop, or profile
    /// switch). Without this, the listener thread continues reconnecting to
    /// the gateway WebSocket indefinitely — a leaked thread that registers
    /// with the connection tracker and participates in kick churn even after
    /// the user has disconnected.
    fn stop_session_events_listener(&mut self) {
        let gw = self.gw();
        if let Some(cancel) = gw.session_events_cancel.take() {
            cancel.store(true, Ordering::SeqCst);
        }
        gw.session_events_receiver = None;
    }

    /// Whether the active profile's gateway has completed at least one probe.
    fn gateway_probe_completed(&self) -> bool {
        self.gw_ref().map_or(false, |gw| gw.probe_completed)
    }

    // ── GatewayState forwarding accessors ──
    // These methods expose the active profile's GatewayState fields through
    // method calls so screen files use `app.method()` instead of direct field access.
    // When the active profile has no GatewayState entry yet, Option fields return
    // None and scalar fields return their default (false).

    /// Active profile's gateway status.
    pub fn gateway_status(&self) -> Option<&GatewayStatusDetails> {
        self.gw_ref().and_then(|gw| gw.status.as_ref())
    }

    /// Active profile's chat session id.
    pub fn chat_session_id(&self) -> Option<&String> {
        self.gw_ref().and_then(|gw| gw.chat_session_id.as_ref())
    }

    /// Active profile's chat messages buffer.
    pub fn chat_messages(&self) -> Option<&Vec<ChatMessage>> {
        self.gw_ref().map(|gw| &gw.chat_messages)
    }

    /// Active profile's session messages.
    pub fn session_messages(&self) -> Option<&BTreeMap<String, Vec<ChatMessage>>> {
        self.gw_ref().map(|gw| &gw.session_messages)
    }

    /// Active profile's session summaries.
    pub fn session_summaries(&self) -> Option<&HashMap<String, SessionSummary>> {
        self.gw_ref().map(|gw| &gw.session_summaries)
    }

    /// Active profile's session order.
    pub fn session_order(&self) -> Option<&Vec<String>> {
        self.gw_ref().map(|gw| &gw.session_order)
    }

    /// Active profile's selected session id.
    pub fn selected_session_id(&self) -> Option<&String> {
        self.gw_ref().and_then(|gw| gw.selected_session_id.as_ref())
    }

    /// Active profile's loading session id.
    pub fn loading_session_id(&self) -> Option<&String> {
        self.gw_ref().and_then(|gw| gw.loading_session_id.as_ref())
    }

    /// Active profile's chat turn receiver.
    pub fn chat_turn_receiver(&self) -> Option<&mpsc::Receiver<Result<AgentReply, String>>> {
        self.gw_ref().and_then(|gw| gw.chat_turn_receiver.as_ref())
    }

    /// Active profile's chat stopping flag.
    pub fn chat_stopping(&self) -> bool {
        self.gw_ref().map_or(false, |gw| gw.chat_stopping)
    }

    /// Active profile's current provider.
    pub fn current_provider(&self) -> Option<&String> {
        self.gw_ref().and_then(|gw| gw.current_provider.as_ref())
    }

    /// Active profile's current model.
    pub fn current_model(&self) -> Option<&String> {
        self.gw_ref().and_then(|gw| gw.current_model.as_ref())
    }

    /// Active profile's current model (mutable).
    pub fn current_model_mut(&mut self) -> &mut Option<String> {
        &mut self.gw().current_model
    }

    /// Active profile's default model.
    pub fn default_model(&self) -> Option<&String> {
        self.gw_ref().and_then(|gw| gw.default_model.as_ref())
    }

    /// Active profile's default model (mutable).
    pub fn default_model_mut(&mut self) -> &mut Option<String> {
        &mut self.gw().default_model
    }

    /// Active profile's active orchestrator id.
    pub fn active_orchestrator_id(&self) -> Option<&String> {
        self.gw_ref().and_then(|gw| gw.active_orchestrator_id.as_ref())
    }

    /// Effective orchestrator IDs: from gateway status when available, otherwise from config.
    /// Used by the agent combobox to show meaningful options even before the status response
    /// arrives (e.g. after gateway restart).
    pub fn effective_orchestrator_ids(&mut self) -> Vec<String> {
        // Prefer live gateway status when available.
        if let Some(gs) = self.gateway_status() {
            if !gs.orchestrators.is_empty() {
                return gs.orchestrators.iter().map(|o| o.id.clone()).collect();
            }
        }
        // Fall back to config-based orchestrator IDs.
        self.load_config_cached()
            .map(|(c, _)| c.agents.orchestrators.iter().map(|o| o.id.clone()).collect())
            .unwrap_or_default()
    }

    /// Effective active orchestrator id: from runtime state, then status, then config default.
    /// Provides a consistent display label for the agent combobox regardless of loading state.
    pub fn effective_active_orchestrator_id(&mut self) -> String {
        // 1. Runtime state (set by user selection or reconcile_dashboard_agent_selection).
        if let Some(id) = self.active_orchestrator_id() {
            return id.clone();
        }
        // 2. Gateway status (first orchestrator from running gateway).
        if let Some(id) = self.gateway_status().and_then(|s| s.orchestrator_id()) {
            return id.to_string();
        }
        // 3. Config default (first orchestrator in config).
        self.load_config_cached()
            .map(|(c, _)| c.agents.default_orchestrator().id.clone())
            .unwrap_or_else(|_| "orchestrator".to_string())
    }

    /// Active profile's dashboard agent id.
    pub fn dashboard_agent_id(&self) -> Option<&String> {
        self.gw_ref().and_then(|gw| gw.dashboard_agent_id.as_ref())
    }

    /// Active profile's agent detail cache.
    pub fn agent_detail_cache(&self) -> Option<&BTreeMap<String, crate::app::types::AgentDetail>> {
        self.gw_ref().map(|gw| &gw.agent_detail_cache)
    }

    /// Active profile's agent detail fetch error.
    pub fn agent_detail_fetch_error(&self) -> Option<&(String, String)> {
        self.gw_ref().and_then(|gw| gw.agent_detail_fetch_error.as_ref())
    }

    /// Active profile's sessions list fetched flag.
    pub fn sessions_list_fetched(&self) -> bool {
        self.gw_ref().map_or(false, |gw| gw.sessions_list_fetched)
    }

    /// Active profile's show clear all confirm flag.
    pub fn show_clear_all_confirm(&self) -> bool {
        self.gw_ref().map_or(false, |gw| gw.show_clear_all_confirm)
    }

    /// Active profile's show clear all confirm flag (mutable).
    pub fn show_clear_all_confirm_mut(&mut self) -> &mut bool {
        &mut self.gw().show_clear_all_confirm
    }

    /// Active profile's current provider (mutable).
    pub fn current_provider_mut(&mut self) -> &mut Option<String> {
        &mut self.gw().current_provider
    }

    /// Active profile's selected session id (mutable).
    pub fn selected_session_id_mut(&mut self) -> &mut Option<String> {
        &mut self.gw().selected_session_id
    }

    /// Active profile's loading session id (mutable).
    pub fn loading_session_id_mut(&mut self) -> &mut Option<String> {
        &mut self.gw().loading_session_id
    }

    /// Active profile's dashboard agent id (mutable).
    pub fn dashboard_agent_id_mut(&mut self) -> &mut Option<String> {
        &mut self.gw().dashboard_agent_id
    }

    /// Active profile's sessions history receiver.
    pub fn sessions_history_receiver(&self) -> Option<&(String, mpsc::Receiver<Result<SessionHistory, String>>)> {
        self.gw_ref().and_then(|gw| gw.sessions_history_receiver.as_ref())
    }

    /// Active profile's sessions history receiver (mutable).
    pub fn sessions_history_receiver_mut(&mut self) -> &mut Option<(String, mpsc::Receiver<Result<SessionHistory, String>>)> {
        &mut self.gw().sessions_history_receiver
    }

    /// Active profile's sessions delete receiver.
    pub fn sessions_delete_receiver(&self) -> Option<&(String, mpsc::Receiver<Result<bool, String>>)> {
        self.gw_ref().and_then(|gw| gw.sessions_delete_receiver.as_ref())
    }

    /// Active profile's sessions delete receiver (mutable).
    pub fn sessions_delete_receiver_mut(&mut self) -> &mut Option<(String, mpsc::Receiver<Result<bool, String>>)> {
        &mut self.gw().sessions_delete_receiver
    }

    /// Active profile's sessions delete all receiver (mutable).
    pub fn sessions_delete_all_receiver_mut(&mut self) -> &mut Option<mpsc::Receiver<Result<usize, String>>> {
        &mut self.gw().sessions_delete_all_receiver
    }
    /// Load config from disk with mtime-based caching. Returns a reference to the
    /// cached `(Config, ChaiPaths)` pair, re-reading only when the file has changed.
    /// If the file doesn't exist, returns defaults (matching `load_config` behaviour).
    pub fn load_config_cached(&mut self) -> Result<&(lib::config::Config, lib::profile::ChaiPaths), String> {
        // Resolve paths for the active profile (no override — always use ~/.chai/active).
        let paths = lib::profile::resolve_profile_dir(None)
            .map_err(|e| e.to_string())?;
        let config_path = paths.config_path.clone();

        // Check mtime to decide whether the cache is still valid.
        let current_mtime = std::fs::metadata(&config_path)
            .ok()
            .and_then(|m| m.modified().ok());

        let cache_valid = match (self.cached_config_mtime, current_mtime) {
            (Some(cached), Some(current)) => cached == current,
            (None, None) => true, // both missing = file doesn't exist, cache is valid
            _ => false,
        };

        // Also check that the active profile hasn't changed (paths could differ).
        let profile_matches = self.cached_config.as_ref().map_or(false, |(_, p)| p.config_path == config_path);

        if cache_valid && profile_matches {
            return Ok(self.cached_config.as_ref().unwrap());
        }

        // Cache miss or stale — load from disk.
        // Ensure .env is loaded (no-op if already loaded), matching load_config behaviour.
        lib::config::load_profile_env(None);
        let config = if !config_path.exists() {
            lib::config::Config::default()
        } else {
            let s = std::fs::read_to_string(&config_path)
                .map_err(|e| format!("reading config from {}: {}", config_path.display(), e))?;
            serde_json::from_str(&s)
                .map_err(|e| format!("parsing config from {}: {}", config_path.display(), e))?
        };

        self.cached_config = Some((config, paths));
        self.cached_config_mtime = current_mtime;
        Ok(self.cached_config.as_ref().unwrap())
    }

    /// Invalidate the config cache, forcing a reload on next access.
    pub fn invalidate_config_cache(&mut self) {
        self.cached_config = None;
        self.cached_config_mtime = None;
    }

    /// Load desktop config from disk with mtime-based caching. Returns a reference
    /// to the cached `DesktopConfig`, re-reading only when the file has changed.
    pub fn load_desktop_config_cached(&mut self) -> Result<&lib::config::DesktopConfig, String> {
        let chai_home = lib::profile::chai_home().map_err(|e| e.to_string())?;
        let path = lib::config::DesktopConfig::path(&chai_home);

        let current_mtime = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());

        let cache_valid = match (self.cached_desktop_config_mtime, current_mtime) {
            (Some(cached), Some(current)) => cached == current,
            (None, None) => true,
            _ => false,
        };

        if cache_valid && self.cached_desktop_config.is_some() {
            return Ok(self.cached_desktop_config.as_ref().unwrap());
        }

        // Cache miss or stale — load from disk.
        let config = lib::config::load_desktop_config().map_err(|e| e.to_string())?;
        // Ensure remote profile directories exist so they appear in the ComboBox.
        if let Some(ref remote) = config.remote {
            if let Ok(chai_home) = lib::profile::chai_home() {
                for entry in remote {
                    let profile_dir = lib::profile::profile_dir(&chai_home, &entry.id);
                    if !profile_dir.is_dir() {
                        if let Err(e) = std::fs::create_dir_all(&profile_dir) {
                            log::warn!(
                                "failed to create remote profile directory for \"{}\": {}",
                                entry.id,
                                e
                            );
                        }
                    }
                }
            }
        }
        self.cached_desktop_config = Some(config);
        self.cached_desktop_config_mtime = current_mtime;
        Ok(self.cached_desktop_config.as_ref().unwrap())
    }

    /// Invalidate the desktop config cache, forcing a reload on next access.
    pub fn invalidate_desktop_config_cache(&mut self) {
        self.cached_desktop_config = None;
        self.cached_desktop_config_mtime = None;
    }

    /// Returns the list of enabled providers for the chat dropdown. Cached until the Config screen is shown
    /// or the active orchestrator changes.
    pub fn enabled_providers(&mut self) -> Vec<String> {
        if let Some(ref list) = self.cached_enabled_providers {
            return list.clone();
        }
        let config = self.load_config_cached()
            .map(|(c, _)| c.clone())
            .unwrap_or_default();
        // Resolve the active orchestrator from state or fall back to default.
        let active_orch_id = self.gw_ref()
            .and_then(|gw| gw.active_orchestrator_id.as_deref())
            .or_else(|| config.agents.orchestrators.first().map(|o| o.id.as_str()))
            .unwrap_or("orchestrator");
        let orch = config.agents.orchestrator(Some(active_orch_id))
            .unwrap_or_else(|_| config.agents.default_orchestrator());
        // Start from enabledProviders when set; otherwise fall back to the active
        // orchestrator's effective default provider (not the default orchestrator's).
        let mut list: Vec<String> = if orch
            .enabled_providers
            .as_ref()
            .map(|v| v.is_empty())
            .unwrap_or(true)
        {
            let default = lib::orchestration::resolve_orchestrator_provider_choice(&config.providers, orch)
                .as_str()
                .to_string();
            vec![default]
        } else {
            let mut seen = std::collections::HashSet::new();
            let configured_ids: std::collections::HashSet<String> = config
                .providers
                .entries
                .iter()
                .map(|p| p.id.trim().to_lowercase())
                .collect();
            orch
                .enabled_providers
                .as_ref()
                .unwrap()
                .iter()
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .filter(|s| configured_ids.contains(s.as_str()))
                .filter(|s| seen.insert(s.clone()))
                .collect()
        };
        // Always include the active orchestrator's effective default provider in the
        // dropdown so the UI reflects which provider the gateway will actually use
        // when no override is provided.
        let orch_default = lib::orchestration::resolve_orchestrator_provider_choice(&config.providers, orch)
            .as_str()
            .to_string();
        if !list.contains(&orch_default) {
            list.push(orch_default);
        }
        self.cached_enabled_providers = Some(list.clone());
        list
    }

    /// Invalidates the enabled-providers cache (call when showing Config so next Chat use reloads).
    pub fn invalidate_enabled_providers_cache(&mut self) {
        self.cached_enabled_providers = None;
    }

    /// Switch the active orchestrator on the chat screen. Invalidates provider/model
    /// caches, resets the session list, and clears the chat view so it reloads from
    /// the new orchestrator's data.
    pub(crate) fn switch_active_orchestrator(&mut self, new_id: String) {
        if self.gw_ref().and_then(|gw| gw.active_orchestrator_id.as_deref()) == Some(new_id.as_str()) {
            return;
        }
        self.cached_enabled_providers = None;
        let gw = self.gw();
        gw.active_orchestrator_id = Some(new_id);
        gw.current_provider = None;
        gw.current_model = None;
        gw.sessions_list_fetched = false;
        gw.session_order.clear();
        gw.session_summaries.clear();
        gw.session_messages.clear();
        gw.chat_session_id = None;
        gw.selected_session_id = None;
        gw.chat_messages.clear();
    }

    /// After **`status`** refresh, keep **Agent** / **Tools** / **Skills** agent selection valid.
    /// Also initializes `active_orchestrator_id` from the gateway status when not yet set.
    pub(crate) fn reconcile_dashboard_agent_selection(&mut self) {
        let details = self.gw_ref().and_then(|gw| gw.status.as_ref()).cloned();
        let Some(details) = details else {
            let gw = self.gw();
            gw.dashboard_agent_id = None;
            return;
        };
        if details.agent_skills.is_empty() {
            let gw = self.gw();
            gw.dashboard_agent_id = None;
            return;
        }
        let orch = details.orchestrator_id().unwrap_or("orchestrator");
        let valid = self.gw_ref()
            .and_then(|gw| gw.dashboard_agent_id.as_ref())
            .map(|id| details.agent_skills.contains_key(id))
            .unwrap_or(false);
        if !valid {
            let gw = self.gw();
            gw.dashboard_agent_id = Some(orch.to_string());
        }
        // Initialize active_orchestrator_id from gateway status when not yet set
        // (first status response after gateway starts).
        let gw = self.gw();
        if gw.active_orchestrator_id.is_none() {
            gw.active_orchestrator_id = Some(orch.to_string());
        }
        // If the active orchestrator no longer exists in the gateway, fall back to default.
        if let Some(ref active) = gw.active_orchestrator_id {
            if !details.orchestrators.iter().any(|o| o.id == *active) {
                gw.active_orchestrator_id = Some(orch.to_string());
                self.invalidate_enabled_providers_cache();
            }
        }
    }

    fn refresh_profiles_from_disk(&mut self) {
        let Ok(chai_home) = lib::profile::chai_home() else {
            self.profiles_need_refresh = false;
            return;
        };
        match lib::profile::list_profile_names(&chai_home) {
            Ok(names) => self.profile_names = names,
            Err(_) => self.profile_names = Vec::new(),
        }
        if let Ok(n) = lib::profile::read_persistent_profile_name(&chai_home) {
            self.profile_active = n;
        }
        // For remote profiles, start with remote_disconnected = true so the
        // user must explicitly click Connect. Without this, the periodic TCP
        // probe auto-detects the remote gateway and the UI shows a "connected"
        // state without the user choosing to connect. Profile switches also
        // set remote_disconnected in switch_profile_to(), but this handles
        // the initial startup load.
        if self.is_remote_profile() {
            self.gw().remote_disconnected = true;
        }
        self.profiles_need_refresh = false;
    }

    fn switch_profile_to(&mut self, name: String) {
        self.profile_switch_error = None;
        // Clear gateway error on profile switch — it's specific to the
        // previous profile's start attempt (e.g. port conflict) and would
        // be confusing when shown for the new profile.
        self.gateway_error = None;
        let Ok(chai_home) = lib::profile::chai_home() else {
            self.profile_switch_error = Some("could not resolve ~/.chai".to_string());
            return;
        };
        // With per-profile locks, switching the persistent profile is always
        // allowed — it just updates the ~/.chai/active symlink and reloads
        // config. The per-profile gateway lock already prevents starting a
        // second gateway on the same profile; there is no reason to block the
        // desktop from pointing at a profile with a running gateway (e.g. to
        // return to the profile where an agent is working).
        if let Err(e) = lib::profile::switch_active_profile(&chai_home, &name) {
            self.profile_switch_error = Some(e.to_string());
            return;
        }

        // Reload .env for the new profile: remove tracked variables from the previous
        // profile's .env and load the new profile's .env.
        let profile_dir = lib::profile::profile_dir(&chai_home, &name);
        if let Err(e) = state::env::load_profile_env_tracked(&profile_dir) {
            log::error!("failed to load .env for profile {}: {}", name, e);
        }

        self.profile_active = name;

        // Reset the new profile's gateway runtime state so that stale data
        // from a previous visit does not leak across profile switches.
        // In particular, if two profiles share the same port and the desktop
        // previously connected to the wrong gateway, the old status and
        // orchestrator ids would persist and show incorrect agents.
        // The only field intentionally preserved is `process` (the owned
        // gateway subprocess) — that is real infrastructure state, not
        // derived from the gateway connection.
        if let Some(gw) = self.gateways.get_mut(&self.profile_active) {
            gw.responds = false;
            gw.probe_completed = false;
            gw.probe_receiver = None;
            gw.frames_since_probe = 0;
            gw.status = None;
            gw.status_fetch_ever_failed = false;
            gw.status_receiver = None;
            gw.frames_since_status = 0;
            gw.logs_receiver = None;
            gw.frames_since_logs = 0;
            gw.logs_cursor = 0;
            gw.active_orchestrator_id = None;
            gw.dashboard_agent_id = None;
            gw.agent_detail_cache.clear();
            gw.agent_detail_fetch_error = None;
            gw.agent_detail_receiver = None;
            gw.agent_detail_requested_id = None;
            gw.was_running = false;
            gw.was_stopped_by_user = false;
            gw.remote_disconnected = false;
            gw.current_provider = None;
            gw.current_model = None;
            gw.default_model = None;
            if let Some(cancel) = gw.session_events_cancel.take() {
                cancel.store(true, Ordering::SeqCst);
            }
            gw.session_events_receiver = None;
            gw.sessions_list_fetched = false;
            gw.sessions_list_receiver = None;
            gw.sessions_delete_receiver = None;
            gw.sessions_delete_all_receiver = None;
            gw.sessions_history_receiver = None;
            gw.loading_session_id = None;
            gw.chat_session_id = None;
            gw.chat_messages.clear();
            gw.chat_turn_receiver = None;
            gw.chat_stopping = false;
            gw.pending_user_message = None;
            gw.chat_turn_is_new_session = false;
            gw.stop_receiver = None;
            gw.session_messages.clear();
            gw.session_summaries.clear();
            gw.session_order.clear();
            gw.selected_session_id = None;
            gw.show_clear_all_confirm = false;
        }

        self.invalidate_enabled_providers_cache();
        self.invalidate_config_cache();
        self.invalidate_desktop_config_cache();
        self.invalidate_skills_cache();
        self.profiles_need_refresh = true;

        // For remote profiles, start with remote_disconnected = true so the
        // user must explicitly click Connect. Without this, the periodic TCP
        // probe would auto-detect the remote gateway and the UI would show a
        // "connected" state without the user choosing to connect.
        // The cache was invalidated above, so reload it to check is_remote.
        let _ = self.load_desktop_config_cached();
        if self.is_remote_profile() {
            self.gw().remote_disconnected = true;
        }
    }

    fn start_new_session(&mut self) {
        let gw = self.gw();
        gw.chat_session_id = None;
        gw.selected_session_id = None;
        // Drop any in-flight agent RPC so a late reply cannot re-bind `chat_session_id` to the
        // previous server session (which would make the next send continue that history).
        gw.chat_turn_receiver = None;
        gw.stop_receiver = None;
        gw.chat_stopping = false;
        gw.pending_user_message = None;
        gw.chat_turn_is_new_session = false;
        gw.chat_messages.clear();
        gw.chat_messages.push(ChatMessage::system(
            "New Session. Next message will start with a clean history.".to_string(),
        ));
    }

    /// Remove a session from all local state structures so the sidebar
    /// updates immediately. Idempotent — safe to call even if the session
    /// is already absent from one or more structures.
    fn remove_session_local(&mut self, sid: &str) {
        let need_new_session;
        let need_clear_chat_session;
        {
            let gw = self.gw();
            gw.session_messages.remove(sid);
            gw.session_summaries.remove(sid);
            gw.session_order.retain(|id| id != sid);
            need_new_session = gw.selected_session_id.as_deref() == Some(sid);
            need_clear_chat_session = gw.chat_session_id.as_deref() == Some(sid);
        }
        if need_new_session {
            self.start_new_session();
        }
        if need_clear_chat_session {
            self.gw().chat_session_id = None;
        }
    }

    /// Disconnect from a remote gateway profile. Clears probe state, session
    /// data, and agent detail cache so the UI returns to the disconnected state.
    /// Sets `remote_disconnected` to prevent the periodic TCP probe from
    /// automatically re-detecting the remote gateway.
    fn disconnect_remote_profile(&mut self) {
        let gw = self.gw();
        gw.responds = false;
        gw.probe_completed = false;
        gw.probe_receiver = None;
        gw.frames_since_probe = 0;
        gw.status = None;
        gw.status_receiver = None;
        gw.frames_since_status = 0;
        gw.status_fetch_ever_failed = false;
        gw.was_running = false;
        gw.was_stopped_by_user = true;
        gw.remote_disconnected = true;
        gw.active_orchestrator_id = None;
        gw.dashboard_agent_id = None;
        gw.agent_detail_cache.clear();
        gw.agent_detail_fetch_error = None;
        gw.agent_detail_receiver = None;
        gw.agent_detail_requested_id = None;
        self.clear_session_and_messages();
        self.invalidate_skills_cache();
        self.skills_fetch_error = None;
        self.invalidate_config_cache();
        // Note: do NOT invalidate the desktop config cache here.
        // Disconnecting does not change desktop.json, and invalidating
        // the cache causes is_remote_profile() to return false on the
        // next frame (before any screen repopulates the cache), which
        // makes the header show a disabled "Start gateway" button
        // instead of "Connect".
        self.gateway_error = None;
        self.profiles_need_refresh = true;
    }

    /// Clear all session and message state for the active profile when the gateway stops.
    fn clear_session_and_messages(&mut self) {
        let gw = self.gw();
        gw.chat_session_id = None;
        gw.chat_messages.clear();
        gw.chat_turn_receiver = None;
        gw.stop_receiver = None;
        gw.chat_stopping = false;
        gw.pending_user_message = None;
        gw.chat_turn_is_new_session = false;
        gw.session_messages.clear();
        gw.session_summaries.clear();
        gw.session_order.clear();
        gw.selected_session_id = None;
        if let Some(cancel) = gw.session_events_cancel.take() {
            cancel.store(true, Ordering::SeqCst);
        }
        gw.session_events_receiver = None;
        gw.sessions_list_fetched = false;
        gw.sessions_list_receiver = None;
        gw.sessions_history_receiver = None;
        gw.loading_session_id = None;
        gw.sessions_delete_receiver = None;
        gw.sessions_delete_all_receiver = None;
        gw.show_clear_all_confirm = false;
        gw.active_orchestrator_id = None;
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        state::logs::init_logging();

        // Load desktop.json for appearance and log settings.
        let desktop_config = lib::config::load_desktop_config().unwrap_or_else(|e| {
            log::warn!("failed to load desktop.json, using defaults: {}", e);
            lib::config::DesktopConfig::default()
        });

        // Apply log buffer size from desktop.json.
        state::logs::set_log_buffer_max_lines(desktop_config.logs.buffer_size);

        // Apply theme from desktop.json.
        let ctx = &cc.egui_ctx;
        match desktop_config.appearance.theme.trim().to_lowercase().as_str() {
            "light" => {
                ctx.set_visuals(egui::Visuals::light());
            }
            _ => {
                ctx.set_visuals(egui::Visuals::dark());
            }
        }

        // Apply font size from desktop.json.
        let font_size = desktop_config.appearance.font_size as f32;
        let mut fonts = egui::FontDefinitions::default();
        if font_size != 14.0 {
            for (_key, font_data) in fonts.font_data.iter_mut() {
                font_data.tweak.scale = font_size / 14.0;
            }
        }
        ctx.set_fonts(fonts);

        // Create profile directories for remote entries so they appear in
        // the ComboBox before the user has ever connected.
        if let Some(ref remote) = desktop_config.remote {
            if let Ok(chai_home) = lib::profile::chai_home() {
                for entry in remote {
                    let profile_dir = lib::profile::profile_dir(&chai_home, &entry.id);
                    if !profile_dir.is_dir() {
                        if let Err(e) = std::fs::create_dir_all(&profile_dir) {
                            log::warn!(
                                "failed to create remote profile directory for \"{}\": {}",
                                entry.id,
                                e
                            );
                        }
                    }
                }
            }
        }

        let mut app = Self::default();
        // Store the loaded desktop config so is_remote_profile() works
        // from the first frame without requiring a settings screen visit.
        app.cached_desktop_config = Some(desktop_config);
        app.cached_desktop_config_mtime = std::fs::metadata(lib::config::DesktopConfig::path(
            &lib::profile::chai_home().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        ))
        .ok()
        .and_then(|m| m.modified().ok());
        app
    }

    /// Poll for chat turn result and clear receiver when done. Call each frame.
    fn poll_chat_turn(&mut self) {
        let gw = self.gw();
        if let Some(rx) = &gw.chat_turn_receiver {
            if let Ok(result) = rx.try_recv() {
                gw.chat_turn_receiver = None;
                // When the turn was stopped, keep chat_stopping true until the
                // turn_stopped banner appears via session events — the stop RPC
                // returns immediately but the agent is still finishing its
                // current iteration.
                let was_stopped = matches!(&result, Ok(r) if r.stopped);
                if !was_stopped {
                    gw.chat_stopping = false;
                }
                match result {
                    Ok(reply) => {
                        // Use chat_turn_is_new_session (set in start_chat_turn when
                        // chat_session_id was None) instead of checking chat_session_id
                        // here, because poll_session_events may have already bound it
                        // from the first streamed event.
                        let was_new_session = gw.chat_turn_is_new_session;
                        gw.chat_turn_is_new_session = false;
                        if gw.chat_session_id.is_none() {
                            gw.chat_session_id = Some(reply.session_id.clone());
                        }

                        // Collect pre-session error before borrowing session_messages.
                        let pre_session_error: Option<ChatMessage> = if was_new_session {
                            gw.chat_messages
                                .iter()
                                .rev()
                                .find(|m| m.role == "error")
                                .cloned()
                        } else {
                            None
                        };

                        let entry = gw
                            .session_messages
                            .entry(reply.session_id.clone())
                            .or_insert_with(Vec::new);
                        // Deduplicate: broadcast session events may have already added these messages.
                        if was_new_session {
                            if let Some(ref user_content) = gw.pending_user_message {
                                let already = entry
                                    .iter()
                                    .any(|m| m.role == "user" && m.content == *user_content);
                                if !already {
                                    // Prepend so the user line stays before any delegation rows that arrived from the WebSocket while the turn was running (we skip the gateway echo of this user message).
                                    entry.insert(0, ChatMessage::user(user_content.clone()));
                                }
                            }
                        }
                        let reply_is_empty = reply.reply.trim().is_empty();
                        // When the tool loop limit was reached or the turn was stopped, skip the
                        // assistant message when:
                        // (1) content is empty — the banner already communicates what happened,
                        //     and an empty frame adds no useful information.
                        // (2) an assistant_progress with the same content already exists — the
                        //     progress message shows the intermediate text and the banner
                        //     explains the interruption; a duplicate assistant frame is redundant.
                        let skip_assistant = if reply.loop_limit_reached || reply.stopped {
                            if reply_is_empty {
                                true
                            } else {
                                entry.iter().any(|m| {
                                    m.role == "assistant_progress" && m.content == reply.reply
                                })
                            }
                        } else {
                            false
                        };
                        if !skip_assistant {
                            let mut assistant_msg = ChatMessage::assistant(
                                reply.reply.clone(),
                                if reply.tool_calls.is_empty() {
                                    None
                                } else {
                                    Some(reply.tool_calls.clone())
                                },
                                if reply.tool_calls.is_empty() {
                                    None
                                } else {
                                    Some(reply.tool_results.clone())
                                },
                            );
                            // Check dedup before clearing tool_calls so the comparison
                            // matches against WebSocket entries that still have tool_calls.
                            let has_streamed_tools = state::chat::has_streamed_tools_this_turn(entry);
                            let last_is_same = state::chat::last_non_delegation(entry.as_slice())
                                .map(|m| {
                                    state::chat::is_duplicate_assistant_row(
                                        m,
                                        &assistant_msg.role,
                                        &assistant_msg.content,
                                        &assistant_msg.tool_calls,
                                    )
                                })
                                .unwrap_or(false);
                            if last_is_same {
                                // Find the actual assistant entry (not necessarily last — a
                                // tool_loop_limit banner or delegation row may have been appended after it).
                                if let Some(existing) = entry.iter_mut().find(|m| {
                                    m.role == "assistant" && m.content == assistant_msg.content
                                }) {
                                    existing.tool_calls = assistant_msg.tool_calls;
                                    existing.tool_results = assistant_msg.tool_results;
                                    // Clear inline tool calls when streamed events exist.
                                    if has_streamed_tools {
                                        existing.tool_calls = None;
                                        existing.tool_results = None;
                                    }
                                }
                            } else {
                                // Clear tool_calls/tool_results on the assistant message so the
                                // inline fallback rendering doesn't produce duplicates alongside streamed events.
                                if has_streamed_tools {
                                    assistant_msg.tool_calls = None;
                                    assistant_msg.tool_results = None;
                                }
                                entry.push(assistant_msg);
                            }
                        }
                        // When the tool loop iteration limit was reached, add a banner
                        // message so the user knows what happened.
                        if reply.loop_limit_reached {
                            let already_has_banner = entry.iter().any(|m| m.role == "tool_loop_limit");
                            if !already_has_banner {
                                entry.push(ChatMessage::tool_loop_limit(
                                    "tool loop iteration limit reached",
                                    reply.pending_tool_calls.clone(),
                                ));
                            }
                        }
                        // When the turn was stopped by the user, add a banner so the
                        // user knows the turn was paused and can send a new message.
                        if reply.stopped {
                            let last_user_idx = entry.iter().rposition(|m| m.role == "user");
                            let already_has_banner = entry.iter().skip(last_user_idx.unwrap_or(0)).any(|m| m.role == "turn_stopped");
                            if !already_has_banner {
                                entry.push(ChatMessage::turn_stopped());
                            }
                            // The turn_stopped banner is now visible — clear the
                            // stopping flag so the Stop button reverts to its idle state.
                            // This handles the RPC fallback path when the WebSocket
                            // turn_stopped event hasn't arrived yet.
                            gw.chat_stopping = false;
                        }
                        // Retain only the most recent pre-session error so it doesn't disappear
                        // when we switch to the new session, without piling every past failure
                        // onto the new session. Inserted after entry work is done to avoid a
                        // second mutable borrow of session_messages.
                        if let Some(err_msg) = pre_session_error {
                            let entry = gw
                                .session_messages
                                .get_mut(&reply.session_id)
                                .expect("entry exists");
                            entry.insert(0, err_msg);
                        }
                        gw.session_summaries
                            .entry(reply.session_id.clone())
                            .or_insert_with(|| {
                                let now = now_iso8601();
                                SessionSummary {
                                    id: reply.session_id.clone(),
                                    created_at: now.clone(),
                                    updated_at: now,
                                    ..Default::default()
                                }
                            });

                        gw.pending_user_message = None;
                        gw.chat_messages = gw
                            .session_messages
                            .get(&reply.session_id)
                            .cloned()
                            .unwrap_or_default();
                        let should_set_selected = was_new_session && gw.selected_session_id.is_none();
                        let reply_sid = reply.session_id.clone();
                        self.move_session_to_front(&reply_sid);
                        if should_set_selected {
                            self.gw().selected_session_id = Some(reply_sid);
                        }
                    }
                    Err(e) => {
                        gw.chat_stopping = false;
                        gw.pending_user_message = None;
                        gw.chat_turn_is_new_session = false;
                        let err_text = e.clone();
                        // Show the full error as an in-stream chat message.
                        gw.chat_messages
                            .push(ChatMessage::error(err_text.clone()));
                        // Also attach to the current session's messages when we know the id.
                        if let Some(ref sid) = gw.chat_session_id {
                            let entry = gw
                                .session_messages
                                .entry(sid.clone())
                                .or_insert_with(Vec::new);
                            entry.push(ChatMessage::error(err_text));
                        }
                    }
                }
            }
        }
    }

    fn start_gateway(&mut self) {
        self.gateway_error = None;
        let (config, paths) = match lib::config::load_config(None) {
            Ok(pair) => pair,
            Err(e) => {
                self.gateway_error = Some(format!("failed to load config: {}", e));
                return;
            }
        };
        let bind = config.gateway.bind.trim();
        let port = config.gateway.port;
        let bind_addr = format!("{}:{}", bind, port);

        // Pre-flight: check that the configured port is available before
        // spawning the gateway child process. This catches cross-profile
        // port conflicts (e.g. profile A is running on port 15151 and
        // profile B also defaults to 15151) and produces a clear error
        // instead of letting the child process fail with an opaque
        // "address already in use" error after expensive startup work.
        if let Ok(addr) = bind_addr.parse::<std::net::SocketAddr>() {
            if std::net::TcpListener::bind(addr).is_err() {
                // Port is already in use. Identify which running profile
                // holds it (if any) for a more helpful error message.
                let using_profile = self.running_profiles.iter().find(|p| **p != self.profile_active).and_then(|p| {
                    let Ok((c, _)) = lib::config::load_config(Some(p)) else { return None };
                    if c.gateway.port == port { Some(p.clone()) } else { None }
                });
                self.gateway_error = if let Some(other) = using_profile {
                    Some(format!(
                        "port {} already in use by gateway running profile \"{}\" — configure a different port for profile {} to run gateways for both profiles simultaneously",
                        port, other, self.profile_active
                    ))
                } else {
                    Some(format!(
                        "port {} is already in use — configure a different port for profile \"{}\"",
                        port, self.profile_active
                    ))
                };
                return;
            }
        }

        let binary = match state::gateway::resolve_chai_binary() {
            Some(p) => p,
            None => {
                self.gateway_error = Some("could not find chai binary (expected next to desktop binary or on PATH)".to_string());
                return;
            }
        };

        // Build a clean environment for the gateway child process based on the
        // active profile's .env. This prevents stale .env variables from a
        // previous profile from leaking into the gateway. The gateway will also
        // load its own .env via lib::config::load_profile_env, but that function
        // won't override variables already present in the inherited environment.
        let env_map = state::env::build_gateway_env(&paths.profile_dir);

        // Validate CHAI_BIN if set in .env. The gateway binary (started below)
        // uses resolve_chai_binary() which ignores CHAI_BIN, but the gateway's
        // tool executor reads CHAI_BIN from the child environment via
        // lib::exec::resolve_binary(). If CHAI_BIN points to a non-existent
        // binary, tool calls will fail silently — the gateway starts but tools
        // are broken. Catch this early so the user sees a clear error instead
        // of mysterious tool failures.
        if let Some(chai_bin) = env_map.get("CHAI_BIN") {
            if !chai_bin.is_empty() && !std::path::Path::new(chai_bin).exists() {
                self.gateway_error = Some(format!(
                    "CHAI_BIN={} does not exist (set in .env)",
                    chai_bin
                ));
                return;
            }
        }

        let mut cmd = std::process::Command::new(&binary);
        cmd.args(["gateway", "--port", &port.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear()
            .envs(&env_map);
        if !env_map.contains_key("RUST_LOG") {
            cmd.env("RUST_LOG", "lib=info,cli=info");
        }
        // Propagate the active profile so the spawned gateway uses the
        // same profile as the desktop. Use --profile flag which is unambiguous.
        cmd.arg("--profile").arg(&self.profile_active);
        let child = cmd.spawn();
        match child {
            Ok(mut c) => {
                if let Some(stderr) = c.stderr.take() {
                    std::thread::spawn(move || {
                        let reader = std::io::BufReader::new(stderr);
                        for line in reader.lines() {
                            if let Ok(l) = line {
                                // Gateway logger already formats lines as
                                // `[timestamp LEVEL gateway] msg`; push as-is.
                                state::logs::push_gateway_log_line(l);
                            }
                        }
                    });
                }
                if let Some(stdout) = c.stdout.take() {
                    std::thread::spawn(move || {
                        let reader = std::io::BufReader::new(stdout);
                        for line in reader.lines() {
                            if let Ok(l) = line {
                                state::logs::push_gateway_log_line(l);
                            }
                        }
                    });
                }
                let gw = self.gw();
                gw.process = Some(c);
            }
            Err(e) => {
                self.gateway_error = Some(format!("failed to start gateway: {}", e));
            }
        }
    }

    fn stop_gateway(&mut self) {
        let gw = self.gw();
        gw.was_stopped_by_user = true;
        if let Some(mut child) = gw.process.take() {
            let _ = child.kill();
            // Reap the child to avoid zombies; the gateway releases `gateway.lock` on exit (advisory lock).
            let _ = child.wait();
        }
        self.gateway_error = None;
        self.profiles_need_refresh = true;
    }

    /// Append the same text as the **`/help`** command to the active chat session.
    pub(crate) fn show_chat_help(&mut self) {
        const TEXT: &str = "Available commands:\n\n/new - start a new session (clear conversation history)\n/help - show this help message";
        let msg = ChatMessage::system(TEXT.to_string());
        let sid = self.gw_ref().and_then(|gw| gw.chat_session_id.clone());
        if let Some(sid) = sid {
            {
                let gw = self.gw();
                gw.session_messages
                    .entry(sid.clone())
                    .or_default()
                    .push(msg);
            }
            self.move_session_to_front(&sid);
            // Keep the visible transcript in sync when the active session is also selected.
            let selected = self.gw_ref().and_then(|gw| gw.selected_session_id.clone());
            if selected.as_deref() == Some(sid.as_str()) {
                let msgs = self.gw_ref().and_then(|gw| gw.session_messages.get(&sid).cloned()).unwrap_or_default();
                self.gw().chat_messages = msgs;
            }
        } else {
            self.gw().chat_messages.push(msg);
        }
    }

    /// Start a chat turn in a background thread if possible.
    fn start_chat_turn(&mut self) {
        // Check for in-flight turn and extract message before borrowing gw.
        let has_receiver = self.gw_ref().map_or(false, |gw| gw.chat_turn_receiver.is_some());
        if has_receiver {
            return;
        }
        let message = self.chat_input.trim().to_string();
        if message.is_empty() {
            return;
        }
        self.chat_input.clear();

        // Extract needed state from gw, then set pending_user_message.
        let (chat_session_id, active_orchestrator_id, current_provider, current_model, status) = {
            let gw = self.gw_ref();
            gw.map(|gw| (
                gw.chat_session_id.clone(),
                gw.active_orchestrator_id.clone(),
                gw.current_provider.clone(),
                gw.current_model.clone(),
                gw.status.clone(),
            )).unwrap_or_default()
        };
        let is_new_session = chat_session_id.is_none();

        {
            let gw = self.gw();
            gw.pending_user_message = Some(message.clone());
            gw.chat_turn_is_new_session = is_new_session;
        }

        // Handle special commands
        if message.eq_ignore_ascii_case("/new") {
            self.gw().pending_user_message = None;
            self.start_new_session();
            return;
        }

        if message.eq_ignore_ascii_case("/help") {
            self.gw().pending_user_message = None;
            self.show_chat_help();
            return;
        }

        // Send to the current conversation session (chat_session_id), not the merely selected one.
        // None = new session; reply will set chat_session_id.
        let session_id = chat_session_id;
        if let Some(ref sid) = session_id {
            {
                let gw = self.gw();
                let entry = gw
                    .session_messages
                    .entry(sid.clone())
                    .or_insert_with(Vec::new);
                entry.push(ChatMessage::user(message.clone()));
                gw.session_summaries.entry(sid.clone()).or_insert_with(|| {
                    let now = now_iso8601();
                    SessionSummary {
                        id: sid.clone(),
                        created_at: now.clone(),
                        updated_at: now,
                        ..Default::default()
                    }
                });
                // Switch view to the session we're sending to so the message is visible (ui_chat shows selected_session_id).
                gw.selected_session_id = Some(sid.clone());
                // Keep chat_messages in sync when we're already viewing this session (e.g. for empty selected_session_id path).
                if gw.selected_session_id == gw.chat_session_id {
                    gw.chat_messages.push(ChatMessage::user(message.clone()));
                }
            }
            self.move_session_to_front(sid);
        } else {
            // No session id — just sync chat_messages.
            let gw = self.gw();
            if gw.selected_session_id == gw.chat_session_id {
                gw.chat_messages.push(ChatMessage::user(message.clone()));
            }
        }
        // Send provider only when we know it (from UI override or gateway status). Do not hardcode
        // a fallback (e.g. "ollama") when status is unavailable—let the gateway use its config.
        let active_orch = active_orchestrator_id.as_deref();
        let provider = current_provider.or_else(|| {
            status
                .as_ref()
                .and_then(|s| s.default_provider_for(active_orch).map(String::from))
        });
        let model = current_model;
        let orchestrator_id = active_orchestrator_id;
        let profile_override = Some(self.profile_active.clone());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = state::gateway::run_agent_turn(profile_override.as_deref(), session_id, message, provider, model, orchestrator_id);
            let _ = tx.send(result);
        });
        self.gw().chat_turn_receiver = Some(rx);
    }

    /// Signal the gateway to stop the current agent turn for the active session.
    /// The agent finishes the current iteration, then pauses. The session transcript
    /// is preserved and the user can send a new message to continue.
    fn stop_chat_turn(&mut self) {
        let session_id = {
            let gw = self.gw();
            match &gw.chat_session_id {
                Some(id) => id.clone(),
                None => return,
            }
        };
        if self.gw().chat_stopping {
            return;
        }
        let profile_override = Some(self.profile_active.clone());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = state::gateway::send_stop(profile_override.as_deref(), &session_id);
            let _ = tx.send(result);
        });
        let gw = self.gw();
        gw.stop_receiver = Some(rx);
        gw.chat_stopping = true;
    }

    /// Poll for the stop request result. Call each frame.
    fn poll_stop(&mut self) {
        let gw = self.gw();
        if let Some(rx) = &gw.stop_receiver {
            if let Ok(_result) = rx.try_recv() {
                gw.stop_receiver = None;
            }
        }
    }

    /// Poll for `sessions.list` fetch result and trigger a fetch on gateway connect.
    /// Call each frame.
    fn poll_sessions_list(&mut self) {
        let gw = self.gw();
        if let Some(rx) = &gw.sessions_list_receiver {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(summaries) => {
                        // Populate session_order from the response (already sorted by updatedAt desc).
                        gw.session_order = summaries.iter().map(|s| s.id.clone()).collect();
                        gw.session_summaries.clear();
                        for s in summaries {
                            gw.session_summaries.insert(s.id.clone(), s);
                        }
                        // Ensure sessions already in session_messages are in session_order
                        // (they may have been created during the current app session before
                        // the list fetch completed).
                        for sid in gw.session_messages.keys() {
                            if !gw.session_order.contains(sid) {
                                gw.session_order.push(sid.clone());
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("sessions.list fetch failed: {}", e);
                    }
                }
                gw.sessions_list_fetched = true;
                gw.sessions_list_receiver = None;
            }
        }

        // Trigger a fetch when the gateway is running, we haven't fetched yet, and
        // we know the active orchestrator (resolved from the status response). Without
        // this guard, the fetch fires before active_orchestrator_id is set after a
        // gateway restart, sending sessions.list with no orchestratorId and returning
        // sessions for the wrong (default) orchestrator.
        let should_fetch = {
            let gw = self.gw();
            gw.responds
                && !gw.sessions_list_fetched
                && gw.sessions_list_receiver.is_none()
                && gw.active_orchestrator_id.is_some()
        };
        if should_fetch {
            let (tx, rx) = mpsc::channel();
            let profile_override = Some(self.profile_active.clone());
            let orchestrator_id = self.gw().active_orchestrator_id.clone();
            std::thread::spawn(move || {
                let result = state::gateway::fetch_sessions_list(profile_override.as_deref(), orchestrator_id.as_deref());
                let _ = tx.send(result);
            });
            self.gw().sessions_list_receiver = Some(rx);
        }
    }

    /// Poll for `sessions.history` fetch result. Call each frame.
    fn poll_sessions_history(&mut self) {
        let gw = self.gw();
        if let Some((ref sid, ref rx)) = gw.sessions_history_receiver {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(history) => {
                        // Populate session_messages with the loaded history.
                        gw.session_messages.insert(history.id.clone(), history.messages);
                        // Also update the summary with timestamps from the history response.
                        if let Some(summary) = gw.session_summaries.get_mut(&history.id) {
                            if !history.created_at.is_empty() {
                                summary.created_at = history.created_at;
                            }
                            if !history.updated_at.is_empty() {
                                summary.updated_at = history.updated_at;
                            }
                        }
                        // Sync chat_messages if this is the currently selected session.
                        if gw.selected_session_id.as_deref() == Some(history.id.as_str()) {
                            if let Some(msgs) = gw.session_messages.get(&history.id) {
                                gw.chat_messages = msgs.clone();
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("sessions.history fetch failed for {}: {}", sid, e);
                    }
                }
                gw.loading_session_id = None;
                gw.sessions_history_receiver = None;
            }
        }
    }

    /// Poll for `sessions.delete` fetch result. Call each frame.
    fn poll_sessions_delete(&mut self) {
        let (result, sid_opt) = {
            let gw = self.gw();
            let result = gw
                .sessions_delete_receiver
                .as_ref()
                .and_then(|(_, rx)| rx.try_recv().ok());
            let sid = gw
                .sessions_delete_receiver
                .as_ref()
                .map(|(id, _)| id.clone());
            (result, sid)
        };
        if let Some(result) = result {
            if let Some(sid) = &sid_opt {
                self.gw().sessions_delete_receiver = None;
                match &result {
                    Ok(true) => {
                        // Immediately remove from local state so the sidebar
                        // updates without waiting for the broadcast event.
                        self.remove_session_local(sid);
                    }
                    Ok(false) => {
                        log::warn!("sessions.delete returned false for {}", sid);
                    }
                    Err(e) => {
                        log::warn!("sessions.delete failed: {}", e);
                        // If the gateway says the session doesn't exist, clean up
                        // local state — the session is already gone server-side.
                        if e.contains("session not found") {
                            self.remove_session_local(sid);
                        }
                    }
                }
            }
        }
    }

    /// Poll for `sessions.delete_all` fetch result. Call each frame.
    fn poll_sessions_delete_all(&mut self) {
        let result = {
            let gw = self.gw();
            gw.sessions_delete_all_receiver
                .as_ref()
                .and_then(|rx| rx.try_recv().ok())
        };
        if let Some(result) = result {
            match result {
                Ok(count) => {
                    // Immediately clear all session state so the sidebar
                    // updates without waiting for the broadcast event.
                    {
                        let gw = self.gw();
                        gw.sessions_delete_all_receiver = None;
                        gw.session_messages.clear();
                        gw.session_summaries.clear();
                        gw.session_order.clear();
                    }
                    self.start_new_session();
                    self.gw().chat_session_id = None;
                    log::debug!("sessions.delete_all removed {} session(s)", count);
                }
                Err(e) => {
                    self.gw().sessions_delete_all_receiver = None;
                    log::warn!("sessions.delete_all failed: {}", e);
                }
            }
        }
    }

    // screen-specific UI functions moved into app::screens::*
}

impl eframe::App for ChaiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll all running gateways.
        self.poll_gateway_probe();

        // Ensure the desktop config cache is populated before computing
        // is_remote_profile(), which depends on it. The cache may have been
        // invalidated by a disconnect, gateway stop, or running-profiles
        // change — if it is not repopulated, is_remote_profile() returns
        // false and the header falls back to local-profile button labels
        // (disabled "Start gateway" instead of "Connect").
        let _ = self.load_desktop_config_cached();

        let owned = self.gateway_owned();
        // For remote profiles, "running" means the remote gateway responded to
        // the TCP probe (no local gateway.lock). For local profiles, "running"
        // means the gateway.lock is held by some process.
        let is_remote = self.is_remote_profile();
        let running = if is_remote {
            self.gw_ref().map_or(false, |gw| gw.responds)
        } else {
            owned || self.gateway_running()
        };

        // Detect gateway stop for the active profile.
        let was_running = self.gw_ref().map_or(false, |gw| gw.was_running);
        if was_running && !running {
            // If the gateway was not stopped by the user, it exited unexpectedly.
            let was_stopped_by_user = self.gw_ref().map_or(false, |gw| gw.was_stopped_by_user);
            if !was_stopped_by_user && self.gateway_error.is_none() {
                if let Some(msg) = state::logs::extract_gateway_error_message(10) {
                    self.gateway_error = Some(msg);
                } else {
                    self.gateway_error = Some("gateway exited unexpectedly (no log output captured)".to_string());
                }
            }
            self.gw().was_stopped_by_user = false;
            self.clear_session_and_messages();
            self.invalidate_skills_cache();
            self.skills_fetch_error = None;
            self.gw().agent_detail_fetch_error = None;
            self.invalidate_config_cache();
            self.invalidate_desktop_config_cache();
            self.profile_switch_error = None;
            self.profiles_need_refresh = true;
            self.gw().status_fetch_ever_failed = false;
            // Remove stale per-profile gateway.lock if present.
            if let Ok(chai_home) = lib::profile::chai_home() {
                let lock_path = lib::profile::profile_dir(&chai_home, &self.profile_active)
                    .join("gateway.lock");
                // Only remove if no process holds the lock (advisory flock check).
                if !lib::profile::gateway_is_running(&chai_home, &self.profile_active) {
                    let _ = std::fs::remove_file(&lock_path);
                }
            }
        }
        // Clear gateway error when the gateway starts running (either owned or
        // external).
        if running && !was_running {
            self.gateway_error = None;
        }
        let gw = self.gw();
        gw.was_running = running;

        // Resolve running profiles from per-profile gateway.lock files.
        // Refreshed on probe cadence (~1 Hz) instead of every frame to avoid 60 disk reads/sec.
        self.frames_since_lock_profile = self.frames_since_lock_profile.saturating_add(1);
        if self.frames_since_lock_profile >= PROBE_INTERVAL_FRAMES {
            self.frames_since_lock_profile = 0;
            if let Ok(chai_home) = lib::profile::chai_home() {
                self.running_profiles = lib::profile::find_running_gateway_profiles(&chai_home);
            }
        }

        // Invalidate config-dependent caches when the running profiles change
        // (e.g. gateway started externally and we now detect it, or gateway stopped).
        if self.running_profiles != self.prev_running_profiles {
            self.invalidate_enabled_providers_cache();
            self.invalidate_config_cache();
            self.invalidate_desktop_config_cache();
            self.invalidate_skills_cache();
            self.gw().default_model = None;
            self.prev_running_profiles = self.running_profiles.clone();
        }

        if self.profiles_need_refresh {
            self.refresh_profiles_from_disk();
        }

        // Now that running_profiles is up-to-date, poll for status and events.
        self.poll_status_fetch();
        self.poll_skills_fetch();
        self.poll_gateway_logs_fetch(owned);
        // Only fetch agent detail when Agent or Tools screen is active (on-demand).
        if matches!(self.current_screen, Screen::Agent | Screen::Tools) {
            self.poll_agent_detail();
        }
        self.ensure_session_events_listener(running, ctx.clone());
        self.poll_session_events();
        self.poll_sessions_list();
        self.poll_sessions_history();
        self.poll_sessions_delete();
        self.poll_sessions_delete_all();
        self.poll_chat_turn();
        self.poll_stop();

        // Layout-level UI components
        let mut start_gateway = false;
        let mut stop_gateway = false;
        let mut switch_profile_to: Option<String> = None;
        // With per-profile locks, the profile dropdown is always enabled.
        // switch_profile_to() updates the persistent symlink and reloads
        // config for any profile.
        let profile_dropdown_enabled = true;
        let profile_error = self.profile_switch_error.clone();
        let gateway_error = self.gateway_error.clone();
        let remote_disconnected = self.gw_ref().map_or(false, |gw| gw.remote_disconnected);
        ui::header::header(
            ctx,
            running,
            owned,
            self.gateway_probe_completed(),
            is_remote,
            remote_disconnected,
            &self.profile_names,
            &self.profile_active,
            profile_dropdown_enabled,
            profile_error.as_deref(),
            gateway_error.as_deref(),
            |name| {
                switch_profile_to = Some(name);
            },
            || {
                start_gateway = true;
            },
            || {
                stop_gateway = true;
            },
        );
        if let Some(name) = switch_profile_to {
            // When switching away from a remote profile, auto-disconnect first.
            if is_remote && running {
                self.disconnect_remote_profile();
            }
            self.switch_profile_to(name);
        }
        // For remote profiles, "Connect" just means the probe succeeded —
        // no local gateway is spawned. The desktop connects to the remote
        // gateway via WebSocket on each operation (status, chat, etc.).
        // For local profiles, start_gateway spawns a local gateway child process.
        if start_gateway {
            if is_remote {
                // Remote: clear any prior disconnect and trigger an immediate
                // probe to detect the remote gateway.
                self.gw().remote_disconnected = false;
                self.gw().was_stopped_by_user = false;
                self.gateway_error = None;
                self.gw().frames_since_probe = PROBE_INTERVAL_FRAMES;
            } else {
                self.start_gateway();
            }
        }
        if stop_gateway {
            // For remote profiles there is no subprocess to stop.
            // Disconnect means clearing the probe state and session data.
            // For local profiles, kill the owned gateway child.
            if is_remote {
                self.disconnect_remote_profile();
            } else {
                self.stop_gateway();
            }
        }
        ui::sidebar::sidebar(&mut self.current_screen, ctx);
        ui::sessions::sessions_panel(self, ctx, running);

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.current_screen == Screen::Chat {
                ui::layout::central_padded(ui, |ui| {
                    screens::chat::ui_chat_screen(self, ui, running);
                });
            } else if self.current_screen == Screen::Files {
                ui::layout::central_padded(ui, |ui| {
                    screens::files::ui_files_screen(self, ui, running);
                });
            } else if self.current_screen == Screen::Skills {
                ui::layout::central_padded(ui, |ui| {
                    screens::skills::ui_skills_screen(self, ui);
                });
            } else if self.current_screen == Screen::Agent {
                ui::layout::central_padded(ui, |ui| {
                    screens::agent::ui_agent_screen(self, ui, running);
                });
            } else if self.current_screen == Screen::Tools {
                ui::layout::central_padded(ui, |ui| {
                    screens::tools::ui_tools_screen(self, ui, running);
                });
            } else if self.current_screen == Screen::Config {
                ui::layout::central_padded(ui, |ui| {
                    screens::config::ui_config_screen(self, ui);
                });
            } else if self.current_screen == Screen::Gateway {
                ui::layout::central_padded(ui, |ui| {
                    screens::gateway::ui_gateway_screen(self, ui, running);
                });
            } else if self.current_screen == Screen::Logging {
                ui::layout::central_padded(ui, |ui| {
                    screens::logging::ui_logging_screen(self, ui);
                });
            } else if self.current_screen == Screen::Settings {
                ui::layout::central_padded(ui, |ui| {
                    screens::settings::ui_settings_screen(self, ui);
                });
            }
        });
    }
}
