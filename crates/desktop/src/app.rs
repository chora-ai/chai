//! Chai Desktop — egui app state and UI.

use eframe::egui;
use std::collections::{BTreeMap, HashMap};
use std::io::BufRead;
use std::process::{Child, Stdio};
use std::sync::mpsc;
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
    /// True once a status fetch has completed with an error since the gateway was last detected.
    /// Used to suppress `need_immediate` so failed fetches don't trigger a tight retry loop.
    status_fetch_ever_failed: bool,
    /// True when the user explicitly stopped the gateway via the Stop button.
    /// Used to distinguish a user-initiated stop from an unexpected gateway exit/crash.
    gateway_was_stopped_by_user: bool,
    /// Current chat session id (created on first agent call).
    chat_session_id: Option<String>,
    /// In-memory chat transcript for the current session.
    chat_messages: Vec<ChatMessage>,
    /// Current input text for the chat box.
    chat_input: String,
    /// When Some, a chat turn is in flight; we read the result here.
    chat_turn_receiver: Option<mpsc::Receiver<Result<AgentReply, String>>>,
    /// When Some, a stop request is in flight; we read the result here.
    stop_receiver: Option<mpsc::Receiver<Result<bool, String>>>,
    /// True after the user requests a stop, until the chat turn actually completes.
    /// Persists across frames even after the stop RPC completes so the UI shows "Stopping…".
    chat_stopping: bool,
    /// User message we sent for the in-flight turn (used when reply creates a new session).
    pending_user_message: Option<String>,
    /// True when the in-flight turn was started for a new (previously unbound) session.
    /// Set in start_chat_turn when chat_session_id is None; read in poll_chat_turn.
    chat_turn_is_new_session: bool,
    session_messages: BTreeMap<String, Vec<ChatMessage>>,
    /// Summary metadata for each session (id, timestamps, message count, channel binding).
    /// Populated from `sessions.list` on gateway connect and updated via session events.
    session_summaries: HashMap<String, SessionSummary>,
    /// When Some, a session events stream is in flight; we read gateway session.message events here.
    session_events_receiver: Option<mpsc::Receiver<SessionEvent>>,
    /// Currently selected provider override (None = use gateway default).
    current_provider: Option<String>,
    /// Currently selected model override (None = use gateway default).
    current_model: Option<String>,
    /// Default model from config (cached for display / fallback).
    default_model: Option<String>,
    /// Current screen (Chat, Gateway, Agent, Tools, Config, Skills, Logging).
    current_screen: Screen,
    /// Session whose messages are shown in the chat area (None = "New session" / desktop buffer).
    selected_session_id: Option<String>,
    /// Session IDs in most-recently-active order (latest first) for the sidebar list.
    session_order: Vec<String>,
    /// When Some, a `sessions.list` fetch is in flight.
    sessions_list_receiver: Option<mpsc::Receiver<Result<Vec<SessionSummary>, String>>>,
    /// True once the session list has been fetched after the gateway was detected.
    /// Reset to false when the gateway stops so a fresh fetch occurs on reconnect.
    sessions_list_fetched: bool,
    /// When Some, a `sessions.history` fetch is in flight for the given session id.
    sessions_history_receiver: Option<(String, mpsc::Receiver<Result<SessionHistory, String>>)>,
    /// Session id whose history is currently loading (shown as a loading indicator in the chat area).
    loading_session_id: Option<String>,
    /// When Some, a `sessions.delete` fetch is in flight for the given session id.
    sessions_delete_receiver: Option<(String, mpsc::Receiver<Result<bool, String>>)>,
    /// When Some, a `sessions.delete_all` fetch is in flight.
    sessions_delete_all_receiver: Option<mpsc::Receiver<Result<usize, String>>>,
    /// Whether the "Clear all" confirmation dialog is showing.
    show_clear_all_confirm: bool,
    /// Whether the gateway was running last frame (used to detect stop and clear messages).
    was_gateway_running: bool,
    /// Currently selected skill on the Skills screen (by name).
    selected_skill_name: Option<String>,
    /// Cached list of enabled providers for the chat provider dropdown (invalidated when Config screen is shown).
    cached_enabled_providers: Option<Vec<String>>,
    /// **Gateway** screen: show parsed fields or the raw `status` response JSON.
    status_view_mode: StatusViewMode,
    /// Stable buffer for **Tools** screen `TextEdit` (updated when the effective tools JSON changes).
    tools_display_buffer: String,
    /// **Agent**, **Tools**, and **Skills**: which agent id is selected (orchestrator or worker).
    dashboard_agent_id: Option<String>,
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
    /// `CHAI_PROFILE` environment variable value (set once at startup). When present, the profile
    /// selector is disabled and the header shows an amber hint.
    env_profile: Option<String>,
    /// Profile name read from `gateway.lock` while a gateway is running (refreshed on probe cadence).
    gateway_lock_profile: Option<String>,
    /// Previous frame's `gateway_lock_profile`; used to detect when the effective profile changes
    /// so config-dependent caches (providers, model) can be invalidated.
    prev_gateway_lock_profile: Option<String>,
    /// Cached owned copy of the effective profile override. Updated whenever `env_profile` or
    /// `gateway_lock_profile` changes, so background thread spawns can clone this instead of
    /// calling `effective_profile_override().map(String::from)` every frame.
    cached_profile_override: Option<String>,
    /// Frames since we last refreshed `gateway_lock_profile` from disk.
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
    /// When Some, a gateway log fetch is in flight; we read the result here.
    /// Used for external (non-owned) gateways to pull logs via the `logs` WS method.
    gateway_logs_receiver: Option<mpsc::Receiver<Result<(Vec<String>, u64), String>>>,
    /// Frames since we last started a gateway log fetch.
    frames_since_gateway_logs: u32,
    /// Sequence cursor for gateway log deduplication. Tracks the `maxSeq` from
    /// the last successful `logs` WS response so subsequent fetches only get new lines.
    gateway_logs_cursor: u64,
    /// On-demand per-agent detail cache, keyed by agent id. Populated when the
    /// Agent or Tools screen is active via the `agentDetail` WS method. Cleared
    /// when the gateway stops, or when a status refresh detects that the agent
    /// roster, skill lock generation, or a cached agent's context mode has changed.
    agent_detail_cache: BTreeMap<String, crate::app::types::AgentDetail>,
    /// Last error from an agent detail fetch, shown on the Agent/Tools screen.
    /// Stores (agent_id, error_message).
    agent_detail_fetch_error: Option<(String, String)>,
    /// When Some, an `agentDetail` WS fetch is in flight for the given agent id.
    agent_detail_receiver: Option<(String, mpsc::Receiver<Result<crate::app::types::AgentDetail, String>>)>,
    /// Agent id that was last requested for detail fetch. Used to detect when the
    /// user switches agents so a new fetch can be triggered.
    agent_detail_requested_id: Option<String>,
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
            gateway_process: None,
            gateway_error: None,
            gateway_responds: false,
            gateway_probe_completed: false,
            probe_receiver: None,
            frames_since_probe: 0,
            status_receiver: None,
            frames_since_status: 0,
            gateway_status: None,
            status_fetch_ever_failed: false,
            gateway_was_stopped_by_user: false,
            chat_session_id: None,
            chat_messages: Vec::new(),
            chat_input: String::new(),
            chat_turn_receiver: None,
            stop_receiver: None,
            chat_stopping: false,
            pending_user_message: None,
            chat_turn_is_new_session: false,
            session_messages: BTreeMap::new(),
            session_summaries: HashMap::new(),
            session_events_receiver: None,
            current_provider: None,
            current_model: None,
            default_model: None,
            current_screen: Screen::default(),
            selected_session_id: None,
            session_order: Vec::new(),
            sessions_list_receiver: None,
            sessions_list_fetched: false,
            sessions_history_receiver: None,
            loading_session_id: None,
            sessions_delete_receiver: None,
            sessions_delete_all_receiver: None,
            show_clear_all_confirm: false,
            was_gateway_running: false,
            selected_skill_name: None,
            cached_enabled_providers: None,
            status_view_mode: StatusViewMode::default(),
            tools_display_buffer: String::new(),
            dashboard_agent_id: None,
            config_view_mode: ConfigViewMode::default(),
            config_raw_display_buffer: String::new(),
            settings_view_mode: SettingsViewMode::default(),
            settings_raw_display_buffer: String::new(),
            profile_names: Vec::new(),
            profile_active: String::new(),
            profile_switch_error: None,
            profiles_need_refresh: true,
            env_profile: std::env::var("CHAI_PROFILE").ok().filter(|s| !s.trim().is_empty()),
            gateway_lock_profile: None,
            prev_gateway_lock_profile: None,
            cached_profile_override: std::env::var("CHAI_PROFILE").ok().filter(|s| !s.trim().is_empty()),
            frames_since_lock_profile: 0,
            cached_config: None,
            cached_config_mtime: None,
            cached_desktop_config: None,
            cached_desktop_config_mtime: None,
            cached_skills: None,
            skills_fetch_error: None,
            skills_fetch_receiver: None,
            frames_since_skills_fetch: 0,
            gateway_logs_receiver: None,
            frames_since_gateway_logs: 0,
            gateway_logs_cursor: 0,
            agent_detail_cache: BTreeMap::new(),
            agent_detail_fetch_error: None,
            agent_detail_receiver: None,
            agent_detail_requested_id: None,
        }
    }
}

impl ChaiApp {
    /// Space between the main screen title and the content below on full‑screen panels.
    const SCREEN_TITLE_BOTTOM_SPACING: f32 = 9.0;
    /// Space between the bottom of the content and the window edge on full‑screen panels.
    const SCREEN_FOOTER_SPACING: f32 = 48.0;

    /// Returns the CLI profile override that should be passed to `load_config` so the desktop
    /// connects to the same profile the gateway is using. Resolution order:
    /// 1. `CHAI_PROFILE` env var (set at desktop startup)
    /// 2. Profile from `gateway.lock` (when an external gateway is detected)
    /// 3. `None` (use `~/.chai/active` symlink)
    fn effective_profile_override(&self) -> Option<&str> {
        if let Some(ref env) = self.env_profile {
            Some(env.as_str())
        } else if let Some(ref gw) = self.gateway_lock_profile {
            Some(gw.as_str())
        } else {
            None
        }
    }

    /// Recompute `cached_profile_override` from the current `env_profile` and
    /// `gateway_lock_profile`. Call after either field changes.
    fn refresh_cached_profile_override(&mut self) {
        self.cached_profile_override = self.effective_profile_override().map(String::from);
    }

    /// Load config from disk with mtime-based caching. Returns a reference to the
    /// cached `(Config, ChaiPaths)` pair, re-reading only when the file has changed.
    /// If the file doesn't exist, returns defaults (matching `load_config` behaviour).
    pub fn load_config_cached(&mut self) -> Result<&(lib::config::Config, lib::profile::ChaiPaths), String> {
        // Resolve paths first, then release the borrow before any &mut self writes.
        let (paths, profile_override_owned) = {
            let profile_override = self.effective_profile_override();
            let paths = lib::profile::resolve_profile_dir(profile_override)
                .map_err(|e| e.to_string())?;
            let profile_owned = self.cached_profile_override.clone();
            (paths, profile_owned)
        };
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

        // Also check that the profile override hasn't changed (paths could differ).
        let profile_matches = self.cached_config.as_ref().map_or(false, |(_, p)| p.config_path == config_path);

        if cache_valid && profile_matches {
            return Ok(self.cached_config.as_ref().unwrap());
        }

        // Cache miss or stale — load from disk.
        // Ensure .env is loaded (no-op if already loaded), matching load_config behaviour.
        lib::config::load_profile_env(profile_override_owned.as_deref());
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
        self.cached_desktop_config = Some(config);
        self.cached_desktop_config_mtime = current_mtime;
        Ok(self.cached_desktop_config.as_ref().unwrap())
    }

    /// Invalidate the desktop config cache, forcing a reload on next access.
    pub fn invalidate_desktop_config_cache(&mut self) {
        self.cached_desktop_config = None;
        self.cached_desktop_config_mtime = None;
    }

    /// Returns the list of enabled providers for the chat dropdown. Cached until the Config screen is shown.
    pub fn enabled_providers(&mut self) -> Vec<String> {
        if let Some(ref list) = self.cached_enabled_providers {
            return list.clone();
        }
        let config = self.load_config_cached()
            .map(|(c, _)| c.clone())
            .unwrap_or_default();
        // Start from enabledProviders when set; otherwise fall back to the effective default provider.
        let mut list: Vec<String> = if config
            .agents
            .enabled_providers
            .as_ref()
            .map(|v| v.is_empty())
            .unwrap_or(true)
        {
            let (default, _) = lib::config::resolve_effective_provider_and_model(&config.providers, &config.agents);
            vec![default]
        } else {
            let mut seen = std::collections::HashSet::new();
            let configured_ids: std::collections::HashSet<String> = config
                .providers
                .entries
                .iter()
                .map(|p| p.id.trim().to_lowercase())
                .collect();
            config
                .agents
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
        // Always include the effective default provider in the dropdown so the UI reflects
        // which provider the gateway will actually use when no override is provided.
        let (default_provider, _) =
            lib::config::resolve_effective_provider_and_model(&config.providers, &config.agents);
        if !list.contains(&default_provider) {
            list.push(default_provider);
        }
        self.cached_enabled_providers = Some(list.clone());
        list
    }

    /// Invalidates the enabled-providers cache (call when showing Config so next Chat use reloads).
    pub fn invalidate_enabled_providers_cache(&mut self) {
        self.cached_enabled_providers = None;
    }

    /// After **`status`** refresh, keep **Agent** / **Tools** / **Skills** agent selection valid.
    pub(crate) fn reconcile_dashboard_agent_selection(&mut self) {
        let Some(details) = self.gateway_status.as_ref() else {
            self.dashboard_agent_id = None;
            return;
        };
        if details.agent_skills.is_empty() {
            self.dashboard_agent_id = None;
            return;
        }
        let orch = details.orchestrator_id.as_deref().unwrap_or("orchestrator");
        let valid = self
            .dashboard_agent_id
            .as_ref()
            .map(|id| details.agent_skills.contains_key(id))
            .unwrap_or(false);
        if !valid {
            self.dashboard_agent_id = Some(orch.to_string());
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
        self.profiles_need_refresh = false;
    }

    fn switch_profile_to(&mut self, name: String) {
        self.profile_switch_error = None;
        let Ok(chai_home) = lib::profile::chai_home() else {
            self.profile_switch_error = Some("could not resolve ~/.chai".to_string());
            return;
        };
        if lib::profile::gateway_is_running(&chai_home) {
            self.profile_switch_error =
                Some("gateway is running; stop it before switching profile".to_string());
            return;
        }
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
        self.invalidate_enabled_providers_cache();
        self.invalidate_config_cache();
        self.invalidate_desktop_config_cache();
        self.invalidate_skills_cache();
        self.profiles_need_refresh = true;
    }

    fn start_new_session(&mut self) {
        self.chat_session_id = None;
        self.selected_session_id = None;
        // Drop any in-flight agent RPC so a late reply cannot re-bind `chat_session_id` to the
        // previous server session (which would make the next send continue that history).
        self.chat_turn_receiver = None;
        self.stop_receiver = None;
        self.chat_stopping = false;
        self.pending_user_message = None;
        self.chat_turn_is_new_session = false;
        self.chat_messages.clear();
        self.chat_messages.push(ChatMessage::system(
            "New Session. Next message will start with a clean history.".to_string(),
        ));
    }

    /// Clear all session and message state when the gateway stops (it does not persist sessions).
    fn clear_session_and_messages(&mut self) {
        self.chat_session_id = None;
        self.chat_messages.clear();
        self.chat_turn_receiver = None;
        self.stop_receiver = None;
        self.chat_stopping = false;
        self.pending_user_message = None;
        self.chat_turn_is_new_session = false;
        self.session_messages.clear();
        self.session_summaries.clear();
        self.session_order.clear();
        self.selected_session_id = None;
        self.session_events_receiver = None;
        self.sessions_list_fetched = false;
        self.sessions_list_receiver = None;
        self.sessions_history_receiver = None;
        self.loading_session_id = None;
        self.sessions_delete_receiver = None;
        self.sessions_delete_all_receiver = None;
        self.show_clear_all_confirm = false;
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

        Self::default()
    }

    /// Poll for chat turn result and clear receiver when done. Call each frame.
    fn poll_chat_turn(&mut self) {
        if let Some(rx) = &self.chat_turn_receiver {
            if let Ok(result) = rx.try_recv() {
                self.chat_turn_receiver = None;
                // When the turn was stopped, keep chat_stopping true until the
                // turn_stopped banner appears via session events — the stop RPC
                // returns immediately but the agent is still finishing its
                // current iteration.
                let was_stopped = matches!(&result, Ok(r) if r.stopped);
                if !was_stopped {
                    self.chat_stopping = false;
                }
                match result {
                    Ok(reply) => {
                        // Use chat_turn_is_new_session (set in start_chat_turn when
                        // chat_session_id was None) instead of checking chat_session_id
                        // here, because poll_session_events may have already bound it
                        // from the first streamed event.
                        let was_new_session = self.chat_turn_is_new_session;
                        self.chat_turn_is_new_session = false;
                        if self.chat_session_id.is_none() {
                            self.chat_session_id = Some(reply.session_id.clone());
                        }

                        // Collect pre-session error before borrowing session_messages.
                        let pre_session_error: Option<ChatMessage> = if was_new_session {
                            self.chat_messages
                                .iter()
                                .rev()
                                .find(|m| m.role == "error")
                                .cloned()
                        } else {
                            None
                        };

                        let entry = self
                            .session_messages
                            .entry(reply.session_id.clone())
                            .or_insert_with(Vec::new);
                        // Deduplicate: broadcast session events may have already added these messages.
                        if was_new_session {
                            if let Some(ref user_content) = self.pending_user_message {
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
                            log::debug!(
                                "poll_chat_turn: session={}, was_new_session={}, last_is_same={}, entry_len={}, last_role={:?}, last_non_del_role={:?}",
                                reply.session_id,
                                was_new_session,
                                last_is_same,
                                entry.len(),
                                entry.last().map(|m| m.role.as_str()),
                                state::chat::last_non_delegation(entry.as_slice()).map(|m| m.role.as_str()),
                            );
                            if last_is_same {
                                // Find the actual assistant entry (not necessarily last — a
                                // tool_loop_limit banner or delegation row may have been appended after it).
                                if let Some(existing) = entry.iter_mut().find(|m| {
                                    m.role == "assistant" && m.content == assistant_msg.content
                                }) {
                                    log::debug!(
                                        "poll_chat_turn dedup: overwriting assistant entry tool_calls={:?}, tool_results={:?}",
                                        existing.tool_calls.as_ref().map(|v| v.len()),
                                        existing.tool_results.as_ref().map(|v| v.len()),
                                    );
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
                        // message so the user knows what happened. The WebSocket event
                        // may have already added one, but dedup is handled by the
                        // tool_loop_limit event handler; the RPC fallback ensures the
                        // banner appears even when the event was missed or arrived early.
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
                        // Only check for an existing banner in recent messages (since
                        // the last user message) so that multiple stops in the same
                        // session each get their own banner.
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
                            self.chat_stopping = false;
                        }
                        // Retain only the most recent pre-session error so it doesn't disappear
                        // when we switch to the new session, without piling every past failure
                        // onto the new session. Inserted after entry work is done to avoid a
                        // second mutable borrow of session_messages.
                        if let Some(err_msg) = pre_session_error {
                            let entry = self
                                .session_messages
                                .get_mut(&reply.session_id)
                                .expect("entry exists");
                            entry.insert(0, err_msg);
                        }
                        self.session_summaries
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

                        self.pending_user_message = None;
                        self.chat_messages = self
                            .session_messages
                            .get(&reply.session_id)
                            .cloned()
                            .unwrap_or_default();
                        self.move_session_to_front(&reply.session_id);
                        if was_new_session && self.selected_session_id.is_none() {
                            self.selected_session_id = Some(reply.session_id);
                        }
                    }
                    Err(e) => {
                        self.chat_stopping = false;
                        self.pending_user_message = None;
                        self.chat_turn_is_new_session = false;
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
        let (config, paths) = match lib::config::load_config(self.effective_profile_override()) {
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
                self.gateway_error = Some("could not find chai binary (expected next to desktop binary or on PATH)".to_string());
                return;
            }
        };

        // Build a clean environment for the gateway child process based on the
        // effective profile's .env. This prevents stale .env variables from a
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
        // Propagate the effective profile override so the spawned gateway uses the
        // same profile as the desktop. Use --profile flag which is unambiguous (vs
        // env var which may affect child processes differently).
        if let Some(profile) = self.effective_profile_override() {
            cmd.arg("--profile").arg(profile);
        }
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
                self.gateway_process = Some(c);
            }
            Err(e) => {
                self.gateway_error = Some(format!("failed to start gateway: {}", e));
            }
        }
    }

    fn stop_gateway(&mut self) {
        self.gateway_was_stopped_by_user = true;
        if let Some(mut child) = self.gateway_process.take() {
            let _ = child.kill();
            // Reap the child to avoid zombies; the gateway releases `gateway.lock` on exit (advisory lock).
            let _ = child.wait();
        }
        self.gateway_error = None;
        self.profiles_need_refresh = true;
    }

    /// Append the same text as the **`/help`** command to the active chat session.
    pub(crate) fn show_chat_help(&mut self) {
        const TEXT: &str = "available commands:\n\n/new - start a new session (clear conversation history)\n/help - show this help message";
        let msg = ChatMessage::system(TEXT.to_string());
        if let Some(sid) = self.chat_session_id.clone() {
            self.session_messages
                .entry(sid.clone())
                .or_default()
                .push(msg);
            self.move_session_to_front(&sid);
            // Keep the visible transcript in sync when the active session is also selected.
            if self.selected_session_id.as_deref() == Some(sid.as_str()) {
                self.chat_messages = self.session_messages.get(&sid).cloned().unwrap_or_default();
            }
        } else {
            self.chat_messages.push(msg);
        }
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
        self.chat_input.clear();
        self.pending_user_message = Some(message.clone());
        self.chat_turn_is_new_session = self.chat_session_id.is_none();

        // Handle special commands
        if message.eq_ignore_ascii_case("/new") {
            self.pending_user_message = None;
            self.start_new_session();
            return;
        }

        if message.eq_ignore_ascii_case("/help") {
            self.pending_user_message = None;
            self.show_chat_help();
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
            self.session_summaries.entry(sid.clone()).or_insert_with(|| {
                let now = now_iso8601();
                SessionSummary {
                    id: sid.clone(),
                    created_at: now.clone(),
                    updated_at: now,
                    ..Default::default()
                }
            });
            self.move_session_to_front(sid);
            // Switch view to the session we're sending to so the message is visible (ui_chat shows selected_session_id).
            self.selected_session_id = Some(sid.clone());
        }
        // Keep chat_messages in sync when we're already viewing this session (e.g. for empty selected_session_id path).
        if self.selected_session_id == self.chat_session_id {
            self.chat_messages.push(ChatMessage::user(message.clone()));
        }
        // Send provider only when we know it (from UI override or gateway status). Do not hardcode
        // a fallback (e.g. "ollama") when status is unavailable—let the gateway use its config.
        let provider = self.current_provider.clone().or_else(|| {
            self.gateway_status
                .as_ref()
                .and_then(|s| s.default_provider.clone())
        });
        let model = self.current_model.clone();
        let profile_override = self.cached_profile_override.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = state::gateway::run_agent_turn(profile_override.as_deref(), session_id, message, provider, model);
            let _ = tx.send(result);
        });
        self.chat_turn_receiver = Some(rx);
    }

    /// Signal the gateway to stop the current agent turn for the active session.
    /// The agent finishes the current iteration, then pauses. The session transcript
    /// is preserved and the user can send a new message to continue.
    fn stop_chat_turn(&mut self) {
        let session_id = match self.chat_session_id {
            Some(ref id) => id.clone(),
            None => return,
        };
        if self.chat_stopping {
            return;
        }
        let profile_override = self.cached_profile_override.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = state::gateway::send_stop(profile_override.as_deref(), &session_id);
            let _ = tx.send(result);
        });
        self.stop_receiver = Some(rx);
        self.chat_stopping = true;
    }

    /// Poll for the stop request result. Call each frame.
    fn poll_stop(&mut self) {
        if let Some(rx) = &self.stop_receiver {
            if let Ok(_result) = rx.try_recv() {
                self.stop_receiver = None;
            }
        }
    }

    /// Poll for `sessions.list` fetch result and trigger a fetch on gateway connect.
    /// Call each frame.
    fn poll_sessions_list(&mut self) {
        if let Some(rx) = &self.sessions_list_receiver {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(summaries) => {
                        // Populate session_order from the response (already sorted by updatedAt desc).
                        self.session_order = summaries.iter().map(|s| s.id.clone()).collect();
                        self.session_summaries.clear();
                        for s in summaries {
                            self.session_summaries.insert(s.id.clone(), s);
                        }
                        // Ensure sessions already in session_messages are in session_order
                        // (they may have been created during the current app session before
                        // the list fetch completed).
                        for sid in self.session_messages.keys() {
                            if !self.session_order.contains(sid) {
                                self.session_order.push(sid.clone());
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("sessions.list fetch failed: {}", e);
                    }
                }
                self.sessions_list_fetched = true;
                self.sessions_list_receiver = None;
            }
        }

        // Trigger a fetch when the gateway is running and we haven't fetched yet.
        if self.gateway_responds
            && !self.sessions_list_fetched
            && self.sessions_list_receiver.is_none()
        {
            let (tx, rx) = mpsc::channel();
            let profile_override = self.cached_profile_override.clone();
            std::thread::spawn(move || {
                let result = state::gateway::fetch_sessions_list(profile_override.as_deref());
                let _ = tx.send(result);
            });
            self.sessions_list_receiver = Some(rx);
        }
    }

    /// Poll for `sessions.history` fetch result. Call each frame.
    fn poll_sessions_history(&mut self) {
        if let Some((ref sid, ref rx)) = self.sessions_history_receiver {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(history) => {
                        // Populate session_messages with the loaded history.
                        self.session_messages.insert(history.id.clone(), history.messages);
                        // Also update the summary with timestamps from the history response.
                        if let Some(summary) = self.session_summaries.get_mut(&history.id) {
                            if !history.created_at.is_empty() {
                                summary.created_at = history.created_at;
                            }
                            if !history.updated_at.is_empty() {
                                summary.updated_at = history.updated_at;
                            }
                        }
                        // Sync chat_messages if this is the currently selected session.
                        if self.selected_session_id.as_deref() == Some(history.id.as_str()) {
                            if let Some(msgs) = self.session_messages.get(&history.id) {
                                self.chat_messages = msgs.clone();
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("sessions.history fetch failed for {}: {}", sid, e);
                    }
                }
                self.loading_session_id = None;
                self.sessions_history_receiver = None;
            }
        }
    }

    /// Poll for `sessions.delete` fetch result. Call each frame.
    /// Note: the authoritative cleanup happens via the `session.deleted` broadcast
    /// event in poll_session_events; this poller just clears the receiver.
    fn poll_sessions_delete(&mut self) {
        if let Some((_, ref rx)) = self.sessions_delete_receiver {
            if let Ok(result) = rx.try_recv() {
                if let Err(e) = result {
                    log::warn!("sessions.delete failed: {}", e);
                }
                self.sessions_delete_receiver = None;
            }
        }
    }

    /// Poll for `sessions.delete_all` fetch result. Call each frame.
    /// Note: the authoritative cleanup happens via the `sessions.cleared` broadcast
    /// event in poll_session_events; this poller just clears the receiver.
    fn poll_sessions_delete_all(&mut self) {
        if let Some(rx) = &self.sessions_delete_all_receiver {
            if let Ok(result) = rx.try_recv() {
                if let Err(e) = result {
                    log::warn!("sessions.delete_all failed: {}", e);
                }
                self.sessions_delete_all_receiver = None;
            }
        }
    }

    // screen-specific UI functions moved into app::screens::*
}

impl eframe::App for ChaiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_gateway_probe();

        // Resolve running state and gateway profile before any fetch/listener
        // that uses effective_profile_override(), so they get the correct profile
        // on the same frame where the gateway is first detected.
        let owned = self.gateway_owned();
        let running = owned || self.gateway_responds;
        if self.was_gateway_running && !running {
            // If the gateway was not stopped by the user, it exited unexpectedly.
            // Extract the actual error message from the gateway log buffer (e.g.
            // "sandbox directory not found at...") rather than showing raw log
            // lines, which belong on the Logging screen.
            if !self.gateway_was_stopped_by_user && self.gateway_error.is_none() {
                if let Some(msg) = state::logs::extract_gateway_error_message(10) {
                    self.gateway_error = Some(msg);
                } else {
                    self.gateway_error = Some("gateway exited unexpectedly (no log output captured)".to_string());
                }
            }
            self.gateway_was_stopped_by_user = false;
            self.clear_session_and_messages();
            self.invalidate_skills_cache();
            self.skills_fetch_error = None;
            self.agent_detail_fetch_error = None;
            self.invalidate_config_cache();
            self.invalidate_desktop_config_cache();
            self.profile_switch_error = None;
            self.profiles_need_refresh = true;
            self.status_fetch_ever_failed = false;
            // Remove stale gateway.lock if present. When the gateway is killed
            // (e.g. user clicks Stop) the OS releases the advisory lock but the
            // gateway process may not clean up the file. A stale file with the
            // previous profile name causes a spurious profile-mismatch hint on
            // the next gateway start, before the new gateway acquires the lock
            // and overwrites the file.
            if let Ok(chai_home) = lib::profile::chai_home() {
                let lock_path = chai_home.join("gateway.lock");
                // Only remove if no process holds the lock (advisory flock check).
                if !lib::profile::gateway_is_running(&chai_home) {
                    let _ = std::fs::remove_file(&lock_path);
                }
            }
        }
        // Clear gateway error when the gateway starts running (either owned or
        // external). This handles the case where an external gateway comes online
        // after a previous crash, and also ensures the error from start_gateway()
        // failures is cleared once the gateway is actually running.
        if running && !self.was_gateway_running {
            self.gateway_error = None;
        }
        self.was_gateway_running = running;

        // Resolve the gateway's profile from gateway.lock when a gateway is running.
        // Must happen before poll_status_fetch / poll_skills_fetch / ensure_session_events_listener
        // so that effective_profile_override() returns the correct profile on the same frame.
        // Refreshed on probe cadence (~1 Hz) instead of every frame to avoid 60 disk reads/sec.
        self.frames_since_lock_profile = self.frames_since_lock_profile.saturating_add(1);
        if !running {
            if self.gateway_lock_profile.is_some() {
                self.gateway_lock_profile = None;
                self.refresh_cached_profile_override();
            }
            self.frames_since_lock_profile = 0;
        } else if self.frames_since_lock_profile >= PROBE_INTERVAL_FRAMES {
            self.frames_since_lock_profile = 0;
            self.gateway_lock_profile = lib::profile::chai_home()
                .ok()
                .and_then(|h| lib::profile::read_gateway_lock_profile(&h));
        }

        // Invalidate config-dependent caches when the effective profile changes
        // (e.g. gateway started externally with CHAI_PROFILE and we now detect it
        // via gateway.lock, or gateway stopped and profile reverts to persistent).
        if self.gateway_lock_profile != self.prev_gateway_lock_profile {
            self.invalidate_enabled_providers_cache();
            self.invalidate_config_cache();
            self.invalidate_desktop_config_cache();
            self.invalidate_skills_cache();
            self.default_model = None;
            self.refresh_cached_profile_override();
            self.prev_gateway_lock_profile = self.gateway_lock_profile.clone();
        }

        if self.profiles_need_refresh {
            self.refresh_profiles_from_disk();
        }

        // Now that gateway_lock_profile is up-to-date, poll for status and events.
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
        // Use the already-computed `running` state instead of a separate flock()
        // syscall every frame. The `running` boolean is derived from gateway_owned() and
        // gateway_responds (probe result), which is sufficient to determine whether the
        // profile dropdown should be locked.
        let profile_switch_locked = running;
        let profile_dropdown_enabled = !profile_switch_locked;
        // The effective profile is: env override > gateway lock profile > persistent symlink.
        let effective_profile = self
            .env_profile
            .as_deref()
            .or(self.gateway_lock_profile.as_deref())
            .unwrap_or(self.profile_active.as_str());
        // Compute a profile-mismatch hint label when the running gateway uses a
        // different profile than the desktop's effective profile.
        let profile_mismatch_label = if self.env_profile.is_some() {
            // CHAI_PROFILE is set in the desktop's environment.
            if let Some(ref gw_profile) = self.gateway_lock_profile {
                if gw_profile != effective_profile {
                    Some(format!(
                        "gateway using profile {} (CHAI_PROFILE={})",
                        gw_profile, effective_profile
                    ))
                } else {
                    Some(format!("gateway using CHAI_PROFILE={}", effective_profile))
                }
            } else {
                Some(format!("gateway using CHAI_PROFILE={}", effective_profile))
            }
        } else if let Some(ref gw_profile) = self.gateway_lock_profile {
            // Gateway lock profile differs from the persistent symlink.
            if *gw_profile != self.profile_active {
                Some(format!(
                    "gateway using profile {} (CHAI_PROFILE={})",
                    gw_profile, gw_profile
                ))
            } else {
                None
            }
        } else {
            None
        };
        let profile_error = self.profile_switch_error.clone();
        let gateway_error = self.gateway_error.clone();
        ui::header::header(
            ctx,
            running,
            owned,
            self.gateway_probe_completed,
            &self.profile_names,
            effective_profile,
            profile_dropdown_enabled,
            profile_error.as_deref(),
            gateway_error.as_deref(),
            profile_mismatch_label.as_deref(),
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
            self.switch_profile_to(name);
        }
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
