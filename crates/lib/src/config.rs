//! Configuration types and loading.
//!
//! Config is loaded from a JSON file under the active profile (e.g. `~/.chai/profiles/assistant/config.json`) and environment.
//! Top-level keys include `gateway`, `channels` (Telegram, Matrix, Signal), `providers` (JSON array of `id` + `endpointType` entries
//! for model APIs), `sandbox` (sandbox enforcement settings), `agents` (JSON array of `id` / `role` entries; omit the key for a
//! single default orchestrator), and `skills` (lock mode and shared skill settings). Skill **packages** are always loaded from
//! **`~/.chai/skills`** (per-agent enablement is under **`agents`**).

use anyhow::{Context, Result};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Top-level application config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// Gateway server settings.
    #[serde(default)]
    pub gateway: GatewayConfig,

    /// Channel settings (e.g. Telegram).
    #[serde(default)]
    pub channels: ChannelsConfig,

    /// Model provider definitions: JSON array of `id` + `endpointType` entries. Omit for default (single Ollama provider).
    #[serde(default = "default_providers_config", with = "providers_config_serde")]
    pub providers: ProvidersConfig,

    /// Sandbox enforcement settings.
    #[serde(default)]
    pub sandbox: SandboxConfig,

    /// Agent definitions: JSON array of `id` + `role` (`orchestrator` \| `worker`). Omit for defaults (one orchestrator, id `orchestrator`).
    #[serde(default = "default_agents_config", with = "agents_config_de")]
    pub agents: AgentsConfig,

    /// Skill package settings: lock mode and shared skill configuration.
    #[serde(default)]
    pub skills: SkillsConfig,
}

/// Gateway bind, port, and auth settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayConfig {
    /// Port for HTTP and WebSocket (default 15151).
    #[serde(default = "default_gateway_port")]
    pub port: u16,

    /// Bind address (default "127.0.0.1").
    #[serde(default = "default_gateway_bind")]
    pub bind: String,

    /// Auth settings. When absent, defaults to no auth for loopback bind.
    #[serde(default)]
    pub auth: GatewayAuthConfig,
}

/// Sandbox enforcement settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxConfig {
    /// How the gateway handles the sandbox directory at startup.
    /// `"strict"` (default): refuse to start when the sandbox directory is missing.
    /// `"current"`: use the current working directory as the sole writable root when
    /// the sandbox directory is missing.
    /// `"unsafe"`: start without any sandbox; CWD confinement and path validation
    /// are disabled.
    #[serde(default)]
    pub mode: SandboxMode,
}

/// How the gateway handles the sandbox at startup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SandboxMode {
    /// Refuse to start the gateway when the sandbox directory for the active
    /// profile does not exist. This is the most secure option.
    #[default]
    Strict,
    /// When the sandbox directory is missing, use the current working directory
    /// as the sole writable root. CWD confinement and path validation remain
    /// active. When the sandbox directory exists, behaves identically to
    /// `Strict`.
    Current,
    /// Start without a sandbox directory. CWD confinement and path validation
    /// are disabled. This should only be used when the operator explicitly
    /// accepts the risk of running without path restrictions.
    Unsafe,
}

impl SandboxMode {
    /// String identifier for this mode (matches the serde value).
    pub fn as_str(&self) -> &'static str {
        match self {
            SandboxMode::Strict => "strict",
            SandboxMode::Current => "current",
            SandboxMode::Unsafe => "unsafe",
        }
    }
}

/// Gateway auth: token or none (loopback-only when none).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAuthConfig {
    /// "none" = no shared secret (only safe when bind is loopback). "token" = require connect.auth.token.
    #[serde(default)]
    pub mode: GatewayAuthMode,

    /// Shared secret for WebSocket connect. Overridden by CHAI_GATEWAY_TOKEN env.
    pub token: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GatewayAuthMode {
    /// No auth; allow only when bind is loopback.
    #[default]
    None,

    /// Require connect.auth.token to match configured token.
    Token,
}

fn default_gateway_port() -> u16 {
    15151
}

fn default_gateway_bind() -> String {
    "127.0.0.1".to_string()
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            port: default_gateway_port(),
            bind: default_gateway_bind(),
            auth: GatewayAuthConfig::default(),
        }
    }
}

/// Resolve the gateway token: env CHAI_GATEWAY_TOKEN overrides config.
pub fn resolve_gateway_token(config: &Config) -> Option<String> {
    std::env::var("CHAI_GATEWAY_TOKEN")
        .ok()
        .and_then(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        })
        .or_else(|| {
            config
                .gateway
                .auth
                .token
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Resolve the Telegram bot token: env TELEGRAM_BOT_TOKEN overrides config.
pub fn resolve_telegram_token(config: &Config) -> Option<String> {
    std::env::var("TELEGRAM_BOT_TOKEN")
        .ok()
        .and_then(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        })
        .or_else(|| {
            config
                .channels
                .telegram
                .bot_token
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Resolve the Telegram webhook secret: env `TELEGRAM_WEBHOOK_SECRET` overrides config when set and non-empty.
pub fn resolve_telegram_webhook_secret(config: &Config) -> Option<String> {
    std::env::var("TELEGRAM_WEBHOOK_SECRET")
        .ok()
        .and_then(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        })
        .or_else(|| {
            config
                .channels
                .telegram
                .webhook_secret
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// When [`Some`], only inbound Matrix messages in these rooms are processed. When [`None`], all joined rooms are allowed.
/// `MATRIX_ROOM_ALLOWLIST` (comma-separated) overrides `channels.matrix.roomIds` when set and non-empty.
pub fn resolve_matrix_room_allowlist(config: &Config) -> Option<HashSet<String>> {
    let mut rooms: Vec<String> = std::env::var("MATRIX_ROOM_ALLOWLIST")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if rooms.is_empty() {
        if let Some(ref v) = config.channels.matrix.room_ids {
            rooms = v
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    if rooms.is_empty() {
        return None;
    }
    Some(rooms.into_iter().collect())
}

/// Resolve the Matrix homeserver URL: env `MATRIX_HOMESERVER` overrides config when set and non-empty.
pub fn resolve_matrix_homeserver(config: &Config) -> Option<String> {
    std::env::var("MATRIX_HOMESERVER")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .channels
                .matrix
                .homeserver
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Resolve the Matrix access token: env `MATRIX_ACCESS_TOKEN` overrides config when set and non-empty.
pub fn resolve_matrix_access_token(config: &Config) -> Option<String> {
    std::env::var("MATRIX_ACCESS_TOKEN")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .channels
                .matrix
                .access_token
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Resolve the Matrix user (localpart or full MXID): env `MATRIX_USER` overrides config when set and non-empty.
pub fn resolve_matrix_user(config: &Config) -> Option<String> {
    std::env::var("MATRIX_USER")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .channels
                .matrix
                .user
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Resolve the Matrix password: env `MATRIX_PASSWORD` overrides config when set and non-empty.
pub fn resolve_matrix_password(config: &Config) -> Option<String> {
    std::env::var("MATRIX_PASSWORD")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .channels
                .matrix
                .password
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Resolve the Matrix user id (`@user:server`): env `MATRIX_USER_ID` overrides config when set and non-empty.
pub fn resolve_matrix_user_id(config: &Config) -> Option<String> {
    std::env::var("MATRIX_USER_ID")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .channels
                .matrix
                .user_id
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Resolve the Matrix device id: env `MATRIX_DEVICE_ID` overrides config when set and non-empty.
pub fn resolve_matrix_device_id(config: &Config) -> Option<String> {
    std::env::var("MATRIX_DEVICE_ID")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .channels
                .matrix
                .device_id
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// True when Matrix homeserver and credentials are present (env or file); does not imply the client connected.
/// Uses centralized resolve helpers for consistent env var handling.
pub fn matrix_channel_configured(config: &Config) -> bool {
    if resolve_matrix_homeserver(config).is_none() {
        return false;
    }
    if resolve_matrix_access_token(config).is_some() {
        return true;
    }
    resolve_matrix_user(config).is_some() && resolve_matrix_password(config).is_some()
}

/// True if the bind address is loopback (127.0.0.1, ::1, etc.).
pub fn is_loopback_bind(bind: &str) -> bool {
    let b = bind.trim();
    b == "127.0.0.1" || b == "::1" || b == "localhost"
}

/// Per-channel config (e.g. Telegram bot token).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelsConfig {
    #[serde(default)]
    pub telegram: TelegramChannelConfig,
    #[serde(default)]
    pub matrix: MatrixChannelConfig,
    #[serde(default)]
    pub signal: SignalChannelConfig,
}

/// Signal channel: user-run signal-cli HTTP daemon.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalChannelConfig {
    /// Base URL of `signal-cli daemon --http`, e.g. `http://127.0.0.1:7583`. Overridden by `SIGNAL_CLI_HTTP`.
    pub http_base: Option<String>,
    /// Multi-account daemon: account (`+E.164`) for JSON-RPC `params`. Overridden by `SIGNAL_CLI_ACCOUNT`.
    pub account: Option<String>,
}

/// Matrix channel: homeserver URL and access token or password login.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixChannelConfig {
    /// HTTPS base URL of the homeserver (e.g. `https://matrix.example.org`).
    pub homeserver: Option<String>,
    /// Client API access token. Overridden by `MATRIX_ACCESS_TOKEN` when set.
    pub access_token: Option<String>,
    /// Localpart or full MXID for `m.login.password`. Overridden by `MATRIX_USER`.
    pub user: Option<String>,
    /// Password for `m.login.password` when `access_token` is not used. Overridden by `MATRIX_PASSWORD`.
    pub password: Option<String>,
    /// `@user:server` for echo filtering when using `access_token` without a login response. Overridden by `MATRIX_USER_ID`.
    pub user_id: Option<String>,
    /// Device id for access-token restore when `/account/whoami` does not return one. Overridden by `MATRIX_DEVICE_ID`.
    pub device_id: Option<String>,
    /// When non-empty, only these room ids (`!room:server`) receive agent turns. Overridden by `MATRIX_ROOM_ALLOWLIST` (comma-separated).
    pub room_ids: Option<Vec<String>>,
}

/// Telegram channel config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelegramChannelConfig {
    /// Bot token from BotFather. Overridden by TELEGRAM_BOT_TOKEN env when set.
    pub bot_token: Option<String>,
    /// When set, use webhook mode: Telegram POSTs updates to this URL. If unset, long-poll getUpdates is used.
    pub webhook_url: Option<String>,
    /// Optional secret for webhook verification (X-Telegram-Bot-Api-Secret-Token). Overridden by `TELEGRAM_WEBHOOK_SECRET` when set. Used only when webhook_url is set.
    pub webhook_secret: Option<String>,
}

/// Skill package settings: lock mode and shared skill configuration.
///
/// In `config.json`, the **`skills`** block holds settings for the shared skill package
/// system. Per-agent skill enablement (`enabledSkills`, `enabledWorkers`, `contextMode`) lives on each
/// agent entry inside the **`agents`** array.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsConfig {
    /// How the gateway handles the lockfile and skill version verification.
    /// `"strict"` (default): refuse to start when the lockfile is missing, any enabled
    /// skill has no lock entry (unpinned), or any pinned skill's active version does not
    /// match its locked hash. `"warn"`: log warnings for unpinned and mismatched skills
    /// and continue; skip verification when no lockfile is present.
    #[serde(default)]
    pub lock_mode: SkillLockMode,
}

/// How the gateway handles the lockfile and skill version verification at startup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillLockMode {
    /// Refuse to start the gateway when the lockfile is missing, any enabled skill has
    /// no lock entry (unpinned), or any pinned skill's active version does not match
    /// its locked hash.
    #[default]
    Strict,
    /// Log warnings for unpinned and mismatched skills but continue loading.
    /// Skip verification entirely when no lockfile is present.
    Warn,
}

/// How skill documentation is provided to the agent: full (all SKILL.md in system message) or read-on-demand (compact list + read_skill tool).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillContextMode {
    /// All loaded skills' full SKILL.md content is injected into the system message each turn. Best for few skills and smaller local models.
    #[default]
    Full,
    /// System message contains only a compact list (name, description). The model uses the read_skill tool to load a skill's full SKILL.md when needed. Keeps prompt small and scales to many skills.
    ReadOnDemand,
}

/// Orchestrator configuration: one entry per orchestrator in the `agents` array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorConfig {
    /// Stable orchestrator id (unique within the `agents` array).
    pub id: String,
    /// Which default provider to use (a provider `id` from the `providers` array). When absent, defaults to "ollama" (if configured) or the first configured provider.
    #[serde(default)]
    pub default_provider: Option<String>,
    /// Model id for the selected provider.
    #[serde(default)]
    pub default_model: Option<String>,
    /// Providers to fetch models from at startup. Opt-in: when absent or empty, only the default provider is discovered; when set, only listed providers are polled.
    #[serde(default)]
    pub enabled_providers: Option<Vec<String>>,
    /// Skill package names enabled for this orchestrator. Omitted or empty ⇒ no skills.
    #[serde(default)]
    pub enabled_skills: Option<Vec<String>>,
    /// Worker ids this orchestrator can delegate to. Omitted ⇒ no workers; empty array ⇒ all profile workers are available.
    #[serde(default)]
    pub enabled_workers: Option<Vec<String>>,
    /// How this orchestrator's skill docs are inlined vs `read_skill`.
    #[serde(default)]
    pub context_mode: Option<SkillContextMode>,
    /// Optional cap on the number of `delegate_task` tool calls allowed per turn.
    #[serde(default)]
    pub max_delegations_per_turn: Option<usize>,
    /// Max successful `delegate_task` calls per session. Omitted = no limit.
    #[serde(default)]
    pub max_delegations_per_session: Option<usize>,
    /// Optional per-worker caps on successful delegations per session. Worker ids as keys.
    #[serde(default)]
    pub max_delegations_per_worker: Option<HashMap<String, usize>>,
    /// Maximum number of tool loops per turn. Omitted = no limit.
    #[serde(default)]
    pub max_tool_loops_per_turn: Option<u32>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            id: "orchestrator".to_string(),
            default_provider: None,
            default_model: None,
            enabled_providers: None,
            enabled_skills: None,
            enabled_workers: None,
            context_mode: None,
            max_delegations_per_turn: None,
            max_delegations_per_session: None,
            max_delegations_per_worker: None,
            max_tool_loops_per_turn: None,
        }
    }
}

impl OrchestratorConfig {
    /// This orchestrator's skill context mode (default full).
    pub fn context_mode(&self) -> SkillContextMode {
        self.context_mode.unwrap_or_default()
    }

    /// This orchestrator's enabled skill names (may be empty).
    pub fn enabled_skills_list(&self) -> &[String] {
        self.enabled_skills.as_deref().unwrap_or(&[])
    }

    /// This orchestrator's enabled worker ids. When `None`, no workers are enabled; when empty, all workers are available.
    pub fn enabled_workers_list(&self) -> &[String] {
        self.enabled_workers.as_deref().unwrap_or(&[])
    }
}

/// Resolved agents configuration: orchestrator entries plus optional worker presets for `delegate_task`.
///
/// In `config.json`, **`agents`** is a JSON **array** of entries with `id`, `role` (`orchestrator` \| `worker`),
/// and per-entry fields. At least one orchestrator and unique ids are required. Omit **`agents`** entirely
/// for built-in defaults (single orchestrator id `orchestrator`).
#[derive(Debug, Clone)]
pub struct AgentsConfig {
    /// Orchestrator entries (at least one required; first is default).
    pub orchestrators: Vec<OrchestratorConfig>,
    /// Worker presets for `delegate_task` `workerId` (from array entries with `role: worker`).
    pub workers: Option<Vec<WorkerConfig>>,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            orchestrators: vec![OrchestratorConfig::default()],
            workers: None,
        }
    }
}

impl AgentsConfig {
    /// The default (first) orchestrator. Always present (validation ensures ≥1).
    pub fn default_orchestrator(&self) -> &OrchestratorConfig {
        &self.orchestrators[0]
    }

    /// Look up an orchestrator by ID. Returns the default if `id` is None.
    pub fn orchestrator(&self, id: Option<&str>) -> Result<&OrchestratorConfig, String> {
        match id {
            None => Ok(self.default_orchestrator()),
            Some(id) => self
                .orchestrators
                .iter()
                .find(|o| o.id == id)
                .ok_or_else(|| format!("unknown orchestrator id: {id}")),
        }
    }

    /// All orchestrator ids.
    pub fn orchestrator_ids(&self) -> impl Iterator<Item = &str> {
        self.orchestrators.iter().map(|o| o.id.as_str())
    }
}

fn default_agents_config() -> AgentsConfig {
    AgentsConfig::default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentDefinition {
    id: String,
    role: AgentRole,
    #[serde(default)]
    default_provider: Option<String>,
    #[serde(default)]
    default_model: Option<String>,
    #[serde(default)]
    enabled_providers: Option<Vec<String>>,
    #[serde(default)]
    enabled_skills: Option<Vec<String>>,
    #[serde(default)]
    enabled_workers: Option<Vec<String>>,
    #[serde(default)]
    context_mode: Option<SkillContextMode>,
    #[serde(default)]
    max_delegations_per_turn: Option<usize>,
    #[serde(default)]
    max_delegations_per_session: Option<usize>,
    #[serde(default)]
    max_delegations_per_worker: Option<HashMap<String, usize>>,
    #[serde(default)]
    max_tool_loops_per_turn: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AgentRole {
    Orchestrator,
    Worker,
}

fn agents_to_definitions(agents: &AgentsConfig) -> Vec<AgentDefinition> {
    let mut out: Vec<AgentDefinition> = agents
        .orchestrators
        .iter()
        .map(|o| AgentDefinition {
            id: o.id.clone(),
            role: AgentRole::Orchestrator,
            default_provider: o.default_provider.clone(),
            default_model: o.default_model.clone(),
            enabled_providers: o.enabled_providers.clone(),
            enabled_skills: o.enabled_skills.clone(),
            enabled_workers: o.enabled_workers.clone(),
            context_mode: o.context_mode,
            max_delegations_per_turn: o.max_delegations_per_turn,
            max_delegations_per_session: o.max_delegations_per_session,
            max_delegations_per_worker: o.max_delegations_per_worker.clone(),
            max_tool_loops_per_turn: o.max_tool_loops_per_turn,
        })
        .collect();
    if let Some(ws) = &agents.workers {
        for w in ws {
            out.push(AgentDefinition {
                id: w.id.clone(),
                role: AgentRole::Worker,
                default_provider: w.default_provider.clone(),
                default_model: w.default_model.clone(),
                enabled_providers: None,
                enabled_skills: w.enabled_skills.clone(),
                enabled_workers: None,
                context_mode: w.context_mode,
                max_delegations_per_turn: None,
                max_delegations_per_session: None,
                max_delegations_per_worker: None,
                max_tool_loops_per_turn: None,
            });
        }
    }
    out
}

fn agents_from_array(entries: Vec<AgentDefinition>) -> Result<AgentsConfig, String> {
    let mut orchestrator_rows: Vec<OrchestratorConfig> = Vec::new();
    let mut worker_rows: Vec<WorkerConfig> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for e in entries {
        let id = e.id.trim().to_string();
        if id.is_empty() {
            return Err("agent id must not be empty".to_string());
        }
        if !seen.insert(id.clone()) {
            return Err(format!("duplicate agent id: {id}"));
        }

        match e.role {
            AgentRole::Orchestrator => {
                orchestrator_rows.push(OrchestratorConfig {
                    id,
                    default_provider: e.default_provider,
                    default_model: e.default_model,
                    enabled_providers: e.enabled_providers,
                    enabled_skills: e.enabled_skills,
                    enabled_workers: e.enabled_workers,
                    context_mode: e.context_mode,
                    max_delegations_per_turn: e.max_delegations_per_turn,
                    max_delegations_per_session: e.max_delegations_per_session,
                    max_delegations_per_worker: e.max_delegations_per_worker,
                    max_tool_loops_per_turn: e.max_tool_loops_per_turn,
                });
            }
            AgentRole::Worker => {
                // Orchestrator-only fields must not be set on worker entries.
                // Since backwards compatibility is not a concern before v0.1.0,
                // strict rejection is appropriate.
                if let Some(ref v) = e.enabled_providers {
                    if !v.is_empty() {
                        return Err(format!(
                            "worker \"{id}\" has \"enabledProviders\" — this field is orchestrator-only"
                        ));
                    }
                }
                if let Some(ref v) = e.enabled_workers {
                    if !v.is_empty() {
                        return Err(format!(
                            "worker \"{id}\" has \"enabledWorkers\" — this field is orchestrator-only"
                        ));
                    }
                }
                if e.max_delegations_per_turn.is_some() {
                    return Err(format!(
                        "worker \"{id}\" has \"maxDelegationsPerTurn\" — this field is orchestrator-only"
                    ));
                }
                if e.max_delegations_per_session.is_some() {
                    return Err(format!(
                        "worker \"{id}\" has \"maxDelegationsPerSession\" — this field is orchestrator-only"
                    ));
                }
                if let Some(ref m) = e.max_delegations_per_worker {
                    if !m.is_empty() {
                        return Err(format!(
                            "worker \"{id}\" has \"maxDelegationsPerWorker\" — this field is orchestrator-only"
                        ));
                    }
                }
                if e.max_tool_loops_per_turn.is_some() {
                    return Err(format!(
                        "worker \"{id}\" has \"maxToolLoopsPerTurn\" — this field is orchestrator-only (applies globally to both orchestrator and worker turns)"
                    ));
                }
                worker_rows.push(WorkerConfig {
                    id,
                    default_provider: e.default_provider,
                    default_model: e.default_model,
                    enabled_skills: e.enabled_skills,
                    context_mode: e.context_mode,
                });
            }
        }
    }

    if orchestrator_rows.is_empty() {
        return Err(
            "agents array must include at least one entry with role \"orchestrator\"".to_string(),
        );
    }

    // Validate enabled_workers references: each listed ID must exist as a worker entry.
    let worker_ids: std::collections::HashSet<&str> = worker_rows.iter().map(|w| w.id.as_str()).collect();
    for o in &orchestrator_rows {
        if let Some(ref ew) = o.enabled_workers {
            for wid in ew {
                if !worker_ids.contains(wid.as_str()) {
                    return Err(format!(
                        "orchestrator \"{}\" references unknown worker id \"{}\" in enabledWorkers",
                        o.id, wid
                    ));
                }
            }
        }
    }

    Ok(AgentsConfig {
        orchestrators: orchestrator_rows,
        workers: if worker_rows.is_empty() {
            None
        } else {
            Some(worker_rows)
        },
    })
}

mod agents_config_de {
    use super::{agents_from_array, agents_to_definitions, AgentDefinition, AgentsConfig};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(agents: &AgentsConfig, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        agents_to_definitions(agents).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<AgentsConfig, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<Vec<AgentDefinition>>::deserialize(deserializer)?;
        match opt {
            None => Ok(AgentsConfig::default()),
            Some(entries) => agents_from_array(entries).map_err(serde::de::Error::custom),
        }
    }
}

/// Worker preset definition for delegation (`delegate_task`).
///
/// Each worker has a single `(defaultProvider, defaultModel)` pair. The worker's `defaultProvider`
/// must be enabled at the orchestrator level via `agents.enabledProviders`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerConfig {
    /// Stable worker id used as `workerId` in the `delegate_task` tool call.
    pub id: String,
    /// Which default provider to use when the worker is selected. Falls back to the
    /// orchestrator's `defaultProvider` when omitted. Must be enabled at the orchestrator
    /// level via `agents.enabledProviders`.
    #[serde(default)]
    pub default_provider: Option<String>,
    /// Model id for the selected provider. Falls back to the orchestrator's `defaultModel`
    /// when omitted.
    pub default_model: Option<String>,

    /// Skill package names enabled for this worker. Omitted or empty ⇒ no skills.
    #[serde(default)]
    pub enabled_skills: Option<Vec<String>>,
    /// How this worker's skill docs are inlined vs `read_skill`.
    #[serde(default)]
    pub context_mode: Option<SkillContextMode>,
}

/// Per-provider configuration: JSON array of provider definitions with `id`, `endpointType` type, and connection settings.
///
/// In `config.json`, **`providers`** is a JSON **array** of entries with `id`, `endpointType`, and
/// optional connection settings. This type wraps `Vec<ProviderDefinition>` with custom serde
/// so the wire format is a direct array (`"providers": [...]`) rather than an object with
/// an `entries` field.
///
/// When the `providers` key is omitted or the array is empty, a single default Ollama provider
/// is used (id `"ollama"`, endpointType `"ollama"`). This aligns with the default agent, which uses
/// Ollama as its `defaultProvider`.
#[derive(Debug, Clone)]
pub struct ProvidersConfig {
    pub entries: Vec<ProviderDefinition>,
}

/// Default providers configuration: a single Ollama provider. Aligns with the default agent
/// (orchestrator with `defaultProvider: "ollama"`).
fn default_providers_config() -> ProvidersConfig {
    ProvidersConfig {
        entries: vec![ProviderDefinition {
            id: "ollama".to_string(),
            endpoint_type: EndpointType::Ollama,
            base_url: None,
            api_key: None,
            default_model: None,
            model_discovery: ModelDiscovery::Auto,
            static_models: Vec::new(),
        }],
    }
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        default_providers_config()
    }
}

mod providers_config_serde {
    use super::{default_providers_config, ProviderDefinition, ProvidersConfig};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(providers: &ProvidersConfig, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        providers.entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ProvidersConfig, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<Vec<ProviderDefinition>>::deserialize(deserializer)?;
        match opt {
            None => Ok(default_providers_config()),
            Some(entries) if entries.is_empty() => Ok(default_providers_config()),
            Some(entries) => Ok(ProvidersConfig { entries }),
        }
    }
}

/// Wire protocol / API family for a provider. Determines which client implementation is used
/// and which default base URL applies when `baseUrl` is unset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndpointType {
    /// Native Ollama API (`/api/chat`, `/api/tags`). Base URL default: `http://127.0.0.1:11434`.
    Ollama,
    /// OpenAI-compatible servers (`/v1/chat/completions`, `/v1/models`). Base URL default: `http://127.0.0.1:1234/v1`.
    OpenaiCompat,
}

impl EndpointType {
    /// Default base URL for this endpoint type when `baseUrl` is not configured.
    pub fn default_base_url(&self) -> Option<&'static str> {
        match self {
            EndpointType::Ollama => Some("http://127.0.0.1:11434"),
            EndpointType::OpenaiCompat => Some("http://127.0.0.1:1234/v1"),
        }
    }

    /// Default model id for this endpoint type when neither `defaultModel` nor the agent's
    /// `defaultModel` is set.
    pub fn default_model(&self) -> &'static str {
        match self {
            EndpointType::Ollama => "llama3.2:3b",
            EndpointType::OpenaiCompat => "llama-3.2-3B-instruct",
        }
    }

    /// String identifier for this endpoint type (matches the serde value).
    pub fn as_str(&self) -> &'static str {
        match self {
            EndpointType::Ollama => "ollama",
            EndpointType::OpenaiCompat => "openai-compat",
        }
    }
}

/// How a provider discovers its available models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelDiscovery {
    /// Use the endpoint type's standard discovery method (`GET /api/tags` for `ollama`,
    /// `GET /v1/models` for `openai-compat`).
    #[default]
    Auto,
    /// LM Studio native model list: `GET /api/v1/models`, filter `type == "llm"`, use `key` as
    /// model id. Applicable to `openai-compat` endpoint type only.
    Lmstudio,
    /// Use the `staticModels` config field. No polling. Works for any endpoint type.
    Static,
}

impl ModelDiscovery {
    /// String identifier for this discovery method (matches the serde value).
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelDiscovery::Auto => "auto",
            ModelDiscovery::Lmstudio => "lmstudio",
            ModelDiscovery::Static => "static",
        }
    }
}

/// One provider definition in the `providers` array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDefinition {
    /// Unique provider id referenced by agents (`defaultProvider`, `enabledProviders`).
    pub id: String,
    /// Wire protocol / API family for this provider.
    pub endpoint_type: EndpointType,
    /// Base URL override. When unset, the endpoint type default is used.
    #[serde(default)]
    pub base_url: Option<String>,
    /// API key. A literal key string, an environment variable reference in `<VAR_NAME>` form
    /// (resolved at runtime via `std::env::var`), or unset. When the value matches `<...>`,
    /// the named environment variable is read; if it is unset or empty, the key resolves to
    /// `None`. When `apiKey` is absent entirely, no key is sent.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Default model id for this provider. When unset, `endpoint_type.default_model()` is used.
    #[serde(default)]
    pub default_model: Option<String>,
    /// How to discover available models for this provider.
    #[serde(default)]
    pub model_discovery: ModelDiscovery,
    /// Static model list used when `modelDiscovery: "static"`. No polling.
    #[serde(default)]
    pub static_models: Vec<String>,
}

impl ProvidersConfig {
    /// Look up a provider definition by id. Returns `None` if no provider with that id exists.
    pub fn get(&self, id: &str) -> Option<&ProviderDefinition> {
        let id_trimmed = id.trim();
        self.entries.iter().find(|p| p.id.trim() == id_trimmed)
    }

    /// Return true if a provider with the given id exists in the array.
    pub fn has(&self, id: &str) -> bool {
        self.get(id).is_some()
    }

    /// Return the set of provider ids in this config.
    pub fn ids(&self) -> Vec<String> {
        self.entries.iter().map(|p| p.id.trim().to_string()).collect()
    }

    /// Validate: all ids are non-empty and unique.
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = HashSet::new();
        for p in &self.entries {
            let id = p.id.trim().to_string();
            if id.is_empty() {
                return Err("provider id must not be empty".to_string());
            }
            if !seen.insert(id.clone()) {
                return Err(format!("duplicate provider id: {id}"));
            }
        }
        Ok(())
    }
}

/// Resolve the base URL for a provider. Falls back to the endpoint type default when `baseUrl`
/// is unset or empty. Returns `None` when the provider id is not found in the config (all
/// endpoint types now have default base URLs).
pub fn resolve_provider_base_url(providers: &ProvidersConfig, id: &str) -> Option<String> {
    let def = providers.get(id)?;
    let base = def
        .base_url
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| def.endpoint_type.default_base_url().map(|s| s.to_string()));
    base.map(|s| s.trim_end_matches('/').to_string())
}

/// Resolve the API key for a provider. If the `apiKey` config value uses the `<VAR_NAME>`
/// syntax, the named environment variable is read at runtime. Literal key strings are returned
/// as-is. Returns `None` when the provider is not found, `apiKey` is unset, the env var
/// reference points to an unset/empty variable, or the resolved value is empty.
pub fn resolve_provider_api_key(providers: &ProvidersConfig, id: &str) -> Option<String> {
    let def = providers.get(id)?;
    def.api_key
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| resolve_env_ref(&s))
        .filter(|s| !s.is_empty())
}

/// Resolve a `<VAR_NAME>` environment variable reference. If `s` starts with `<` and ends
/// with `>`, the content between the angle brackets is treated as an environment variable
/// name and its value is returned (trimmed, non-empty). Otherwise `s` is returned unchanged.
fn resolve_env_ref(s: &str) -> String {
    if s.starts_with('<') && s.ends_with('>') && s.len() > 2 {
        let var_name = &s[1..s.len() - 1];
        match std::env::var(var_name) {
            Ok(v) => {
                let t = v.trim().to_string();
                if !t.is_empty() {
                    return t;
                }
            }
            Err(_) => {}
        }
        // Env var not set or empty — nothing to send as a key.
        String::new()
    } else {
        s.to_string()
    }
}

/// Resolve the default model for a provider. Returns the provider's `defaultModel` if set,
/// otherwise the endpoint type default.
pub fn resolve_provider_default_model(providers: &ProvidersConfig, id: &str) -> String {
    if let Some(def) = providers.get(id) {
        if let Some(ref m) = def.default_model {
            let t = m.trim().to_string();
            if !t.is_empty() {
                return t;
            }
        }
        return def.endpoint_type.default_model().to_string();
    }
    // Fallback when no provider matches (should not happen in normal use).
    "llama3.2:3b".to_string()
}

/// Validate that a provider id references an existing provider in the config.
/// Returns the trimmed id as Some if valid, None otherwise.
pub fn canonical_provider_id(providers: &ProvidersConfig, s: &str) -> Option<String> {
    let trimmed = s.trim().to_string();
    if providers.has(&trimmed) {
        Some(trimmed)
    } else {
        None
    }
}

/// True if model discovery should run for the given provider. Uses the **union** of all
/// orchestrators' `enabled_providers`: if any orchestrator might use a provider, discovery
/// runs for it so that switching orchestrators doesn't require a gateway restart.
pub fn provider_discovery_enabled(providers: &ProvidersConfig, agents: &AgentsConfig, provider_id: &str) -> bool {
    let id = match canonical_provider_id(providers, provider_id) {
        Some(id) => id,
        None => return false,
    };
    // Union approach: collect enabled_providers across all orchestrators.
    // If any orchestrator has an empty/absent enabled_providers, fall back to
    // default-only for that orchestrator and compute the union.
    let mut union_enabled: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut any_uses_default_only = false;
    for o in &agents.orchestrators {
        match &o.enabled_providers {
            None => any_uses_default_only = true,
            Some(v) if v.is_empty() => any_uses_default_only = true,
            Some(v) => {
                for s in v {
                    if let Some(canonical) = canonical_provider_id(providers, s) {
                        union_enabled.insert(canonical);
                    }
                }
            }
        }
    }
    if any_uses_default_only {
        // At least one orchestrator uses default-only discovery.
        // Collect each such orchestrator's default provider.
        for o in &agents.orchestrators {
            let use_default_only = match &o.enabled_providers {
                None => true,
                Some(v) => v.is_empty(),
            };
            if use_default_only {
                let default_id = o
                    .default_provider
                    .as_deref()
                    .and_then(|s| canonical_provider_id(providers, s))
                    .unwrap_or_else(|| {
                        if providers.has("ollama") { "ollama".to_string() } else { String::new() }
                    });
                if !default_id.is_empty() {
                    union_enabled.insert(default_id);
                }
            }
        }
    }
    if union_enabled.is_empty() {
        // No orchestrator specified any enabled_providers and none had a default provider.
        // Fall back to "ollama" if it exists.
        return id == "ollama" || providers.has("ollama") && id == providers.entries[0].id.trim();
    }
    union_enabled.contains(&id)
}

/// Provider ids for which [`provider_discovery_enabled`] is true.
/// Matches which backends run model discovery at gateway startup and which **`status`** includes `*Models` for.
/// Uses the **union** of all orchestrators' `enabled_providers`.
pub fn discovery_enabled_provider_ids(providers: &ProvidersConfig, agents: &AgentsConfig) -> Vec<String> {
    providers
        .ids()
        .into_iter()
        .filter(|id| provider_discovery_enabled(providers, agents, id))
        .collect()
}

/// Provider ids that a single orchestrator's `enabledProviders` resolves to.
/// When `enabledProviders` is absent or empty, returns only the orchestrator's default provider.
pub fn orchestrator_discovery_provider_ids(
    providers: &ProvidersConfig,
    orch: &OrchestratorConfig,
) -> Vec<String> {
    match &orch.enabled_providers {
        Some(v) if !v.is_empty() => v
            .iter()
            .filter_map(|s| canonical_provider_id(providers, s))
            .collect(),
        _ => {
            // Default-only: the orchestrator's default provider.
            let default_id = orch
                .default_provider
                .as_deref()
                .and_then(|s| canonical_provider_id(providers, s))
                .or_else(|| {
                    if providers.has("ollama") {
                        Some("ollama".to_string())
                    } else {
                        None
                    }
                });
            match default_id {
                Some(id) => vec![id],
                None => vec![],
            }
        }
    }
}

/// Resolve effective default provider and model for display (e.g. in desktop when gateway status is not yet available).
/// Returns (provider_id, model_id). Uses the default (first) orchestrator's settings.
/// Invalid provider values fall back to the first configured provider
/// or "ollama" defaults if no providers are configured.
pub fn resolve_effective_provider_and_model(providers: &ProvidersConfig, agents: &AgentsConfig) -> (String, String) {
    let orch = agents.default_orchestrator();
    let provider = orch
        .default_provider
        .as_deref()
        .and_then(|s| canonical_provider_id(providers, s))
        .or_else(|| {
            // Fall back to first configured provider.
            providers.entries.first().map(|p| p.id.trim().to_string())
        })
        .unwrap_or_else(|| "ollama".to_string());
    let model = orch
        .default_model
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| resolve_provider_default_model(providers, &provider));
    (provider, model)
}

/// Orchestrator **agent context directory** (on-disk home for **`AGENT.md`**): `<profile_dir>/agents/<orchestratorId>/`.
/// Uses the default (first) orchestrator's id.
pub fn orchestrator_context_dir(config: &Config, profile_dir: &Path) -> PathBuf {
    let oid = config.agents.default_orchestrator().id.trim();
    let oid = if oid.is_empty() { "orchestrator" } else { oid };
    agent_context_dir(profile_dir, oid)
}

/// Worker **agent context directory**: `<profile_dir>/agents/<worker id>/`. **`None`** if **`id`** is empty.
pub fn worker_context_dir(worker: &WorkerConfig, profile_dir: &Path) -> Option<PathBuf> {
    let wid = worker.id.trim();
    if wid.is_empty() {
        return None;
    }
    Some(agent_context_dir(profile_dir, wid))
}

/// `<profile_dir>/agents/<agent_id>/` — directory for that agent's on-disk context (**`AGENT.md`**).
pub fn agent_context_dir(profile_dir: &Path, agent_id: &str) -> PathBuf {
    profile_dir.join("agents").join(agent_id)
}

/// Sessions directory for an agent: `<profile_dir>/agents/<agent_id>/sessions/`.
pub fn sessions_dir(profile_dir: &Path, agent_id: &str) -> PathBuf {
    agent_context_dir(profile_dir, agent_id).join("sessions")
}

/// Orchestrator skill context mode (default full). Uses the default (first) orchestrator.
pub fn orchestrator_context_mode(agents: &AgentsConfig) -> SkillContextMode {
    agents.default_orchestrator().context_mode()
}

/// Worker skill context mode (default full).
pub fn worker_context_mode(worker: &WorkerConfig) -> SkillContextMode {
    worker.context_mode.unwrap_or_default()
}

/// Orchestrator enabled skill names (may be empty). Uses the default (first) orchestrator.
pub fn orchestrator_enabled_skills_list(agents: &AgentsConfig) -> &[String] {
    agents.default_orchestrator().enabled_skills_list()
}

/// Worker enabled skill names (may be empty).
pub fn worker_enabled_skills_list(worker: &WorkerConfig) -> &[String] {
    worker.enabled_skills.as_deref().unwrap_or(&[])
}

/// Load `.env` from the resolved profile directory into the process environment.
///
/// Variables from `.env` are set **only if not already present** in the process
/// environment — shell/environment variables always take precedence. This function
/// is idempotent: the `.env` file is loaded at most once per process; subsequent
/// calls are no-ops.
///
/// Call this early (before logger initialization) so that environment-driven
/// configuration like `RUST_LOG` takes effect.
pub fn load_profile_env(cli_profile: Option<&str>) {
    static DOTENV_LOADED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    DOTENV_LOADED.get_or_init(|| {
        let paths = match crate::profile::resolve_profile_dir(cli_profile) {
            Ok(p) => p,
            Err(_) => return,
        };
        let env_path = paths.profile_dir.join(".env");
        if env_path.is_file() {
            match dotenvy::from_path(&env_path) {
                Ok(_) => info!("loaded .env from {}", env_path.display()),
                Err(e) => error!("failed to load .env at {}: {}", env_path.display(), e),
            }
        }
    });
}

/// Load config for the resolved profile (`CHAI_PROFILE`, `chai gateway --profile`, or `~/.chai/active`).
/// Missing `config.json` in the profile => default config.
///
/// Also loads the profile's `.env` file (via [`load_profile_env`]) if not already loaded.
/// See [`load_profile_env`] for details on `.env` semantics.
pub fn load_config(cli_profile: Option<&str>) -> Result<(Config, crate::profile::ChaiPaths)> {
    let paths = crate::profile::resolve_profile_dir(cli_profile)?;

    // Ensure .env is loaded (no-op if already loaded by an earlier call to load_profile_env).
    load_profile_env(cli_profile);

    let path = &paths.config_path;
    let config = if !path.exists() {
        log::debug!("config file not found, using defaults: {}", path.display());
        Config::default()
    } else {
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("reading config from {}", path.display()))?;
        serde_json::from_str(&s)
            .with_context(|| format!("parsing config from {}", path.display()))?
    };
    Ok((config, paths))
}

/// Shared skill package root: `<chai_home>/skills` (typically `~/.chai/skills`).
pub fn default_skills_dir(chai_home: &Path) -> PathBuf {
    chai_home.join("skills")
}

/// Desktop application configuration loaded from `~/.chai/desktop.json`.
///
/// This file is separate from per-profile `config.json`: it holds client-side
/// settings (appearance, log buffer size) that are machine-local and
/// user-specific, not tied to any profile.
///
/// All fields are optional. When the file is absent, all values use their
/// defaults — no change from current behavior.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopConfig {
    /// Desktop appearance settings (theme, font size).
    #[serde(default)]
    pub appearance: AppearanceConfig,

    /// Log buffer settings.
    #[serde(default)]
    pub logs: LogsConfig,
}

impl DesktopConfig {
    /// Path to `desktop.json` at the chai home root.
    pub fn path(chai_home: &Path) -> PathBuf {
        chai_home.join("desktop.json")
    }
}

/// Appearance settings for the desktop application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceConfig {
    /// Color theme: `"dark"` or `"light"`.
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Base font size in points.
    #[serde(default = "default_font_size")]
    pub font_size: u32,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            font_size: default_font_size(),
        }
    }
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_font_size() -> u32 {
    14
}

/// Log buffer settings for the desktop application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsConfig {
    /// Maximum number of log lines retained in memory per buffer.
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,
}

impl Default for LogsConfig {
    fn default() -> Self {
        Self {
            buffer_size: default_buffer_size(),
        }
    }
}

fn default_buffer_size() -> usize {
    1000
}

/// Load `desktop.json` from `~/.chai/desktop.json`.
///
/// Returns default values when the file is absent. Rejects invalid values
/// (bad theme, non-positive fontSize/bufferSize) at load time.
pub fn load_desktop_config() -> Result<DesktopConfig> {
    let chai_home = crate::profile::chai_home()?;
    let path = DesktopConfig::path(&chai_home);

    if !path.exists() {
        return Ok(DesktopConfig::default());
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let config: DesktopConfig = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;

    // Validate.
    let theme = config.appearance.theme.trim().to_lowercase();
    if theme != "dark" && theme != "light" {
        anyhow::bail!(
            "invalid appearance.theme {:?}: must be \"dark\" or \"light\"",
            config.appearance.theme
        );
    }
    if config.appearance.font_size == 0 {
        anyhow::bail!("invalid appearance.fontSize: must be a positive integer");
    }
    if config.logs.buffer_size == 0 {
        anyhow::bail!("invalid logs.bufferSize: must be a positive integer");
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_gateway_port_and_bind() {
        let g = GatewayConfig::default();
        assert_eq!(g.port, 15151);
        assert_eq!(g.bind, "127.0.0.1");
    }

    #[test]
    fn default_sandbox_mode_strict() {
        let c: Config = serde_json::from_str("{}").expect("parse");
        assert_eq!(c.sandbox.mode, SandboxMode::Strict);
    }

    #[test]
    fn sandbox_mode_strict_explicit() {
        let j = r#"{"sandbox":{"mode":"strict"}}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(c.sandbox.mode, SandboxMode::Strict);
    }

    #[test]
    fn sandbox_mode_current_from_json() {
        let j = r#"{"sandbox":{"mode":"current"}}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(c.sandbox.mode, SandboxMode::Current);
    }

    #[test]
    fn sandbox_mode_unsafe_from_json() {
        let j = r#"{"sandbox":{"mode":"unsafe"}}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(c.sandbox.mode, SandboxMode::Unsafe);
    }

    #[test]
    fn sandbox_mode_as_str() {
        assert_eq!(SandboxMode::Strict.as_str(), "strict");
        assert_eq!(SandboxMode::Current.as_str(), "current");
        assert_eq!(SandboxMode::Unsafe.as_str(), "unsafe");
    }

    #[test]
    fn sandbox_mode_rejects_unknown_value() {
        let j = r#"{"sandbox":{"mode":"permissive"}}"#;
        assert!(serde_json::from_str::<Config>(j).is_err());
    }

    #[test]
    fn default_skills_dir_under_chai_home() {
        let chai = Path::new("/home/user/.chai");
        assert_eq!(
            default_skills_dir(chai),
            PathBuf::from("/home/user/.chai/skills")
        );
    }

    #[test]
    fn agent_context_dirs_under_profile() {
        let mut c = Config::default();
        c.agents.orchestrators[0].id = "orch-id".to_string();
        let prof = Path::new("/home/u/.chai/profiles/p1");
        assert_eq!(
            orchestrator_context_dir(&c, prof),
            PathBuf::from("/home/u/.chai/profiles/p1/agents/orch-id")
        );
        let w = WorkerConfig {
            id: "w1".to_string(),
            default_provider: None,
            default_model: None,
            enabled_skills: None,
            context_mode: None,
        };
        assert_eq!(
            worker_context_dir(&w, prof),
            Some(PathBuf::from("/home/u/.chai/profiles/p1/agents/w1"))
        );
    }

    /// Restores `TELEGRAM_WEBHOOK_SECRET` after the test so parallel runs do not leak env.
    struct TelegramWebhookSecretEnvGuard {
        previous: Option<std::ffi::OsString>,
    }

    impl TelegramWebhookSecretEnvGuard {
        fn set(value: Option<&str>) -> Self {
            const KEY: &str = "TELEGRAM_WEBHOOK_SECRET";
            let previous = std::env::var_os(KEY);
            match value {
                Some(v) => std::env::set_var(KEY, v),
                None => std::env::remove_var(KEY),
            }
            Self { previous }
        }
    }

    impl Drop for TelegramWebhookSecretEnvGuard {
        fn drop(&mut self) {
            const KEY: &str = "TELEGRAM_WEBHOOK_SECRET";
            match &self.previous {
                Some(v) => std::env::set_var(KEY, v),
                None => std::env::remove_var(KEY),
            }
        }
    }

    #[test]
    fn resolve_telegram_webhook_secret_from_config_trims() {
        let _g = TelegramWebhookSecretEnvGuard::set(None);
        let mut c = Config::default();
        c.channels.telegram.webhook_secret = Some("  sec  ".to_string());
        assert_eq!(resolve_telegram_webhook_secret(&c).as_deref(), Some("sec"));
    }

    #[test]
    fn resolve_telegram_webhook_secret_none_when_unset() {
        let _g = TelegramWebhookSecretEnvGuard::set(None);
        assert!(resolve_telegram_webhook_secret(&Config::default()).is_none());
    }

    #[test]
    fn agents_missing_key_uses_default_orchestrator() {
        let c: Config = serde_json::from_str("{}").expect("parse");
        assert_eq!(c.agents.orchestrators.len(), 1);
        assert_eq!(c.agents.default_orchestrator().id, "orchestrator");
        assert!(c.agents.workers.is_none());
    }

    #[test]
    fn agents_array_one_orchestrator_and_worker() {
        let j = r#"{"agents":[
            {"id":"main","role":"orchestrator","defaultProvider":"ollama","defaultModel":"m"},
            {"id":"fast","role":"worker","defaultProvider":"lmstudio","defaultModel":"w"}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(c.agents.orchestrators.len(), 1);
        assert_eq!(c.agents.default_orchestrator().id, "main");
        assert_eq!(c.agents.default_orchestrator().default_provider.as_deref(), Some("ollama"));
        let w = c.agents.workers.as_ref().expect("workers");
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].id, "fast");
        assert_eq!(w[0].default_provider.as_deref(), Some("lmstudio"));
    }

    #[test]
    fn agents_array_allows_two_orchestrators() {
        let j = r#"{"agents":[
            {"id":"a","role":"orchestrator"},
            {"id":"b","role":"orchestrator"}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(c.agents.orchestrators.len(), 2);
        assert_eq!(c.agents.default_orchestrator().id, "a");
        assert_eq!(c.agents.orchestrator(Some("b")).unwrap().id, "b");
    }

    #[test]
    fn agents_array_rejects_duplicate_ids() {
        let j = r#"{"agents":[
            {"id":"x","role":"orchestrator"},
            {"id":"x","role":"worker"}
        ]}"#;
        let err = serde_json::from_str::<Config>(j).unwrap_err();
        assert!(err.to_string().contains("duplicate"), "{}", err);
    }

    #[test]
    fn agents_rejects_object_instead_of_array() {
        let j = r#"{"agents":{"defaultProvider":"ollama"}}"#;
        assert!(serde_json::from_str::<Config>(j).is_err());
    }

    #[test]
    fn agents_empty_array_errors() {
        let j = r#"{"agents":[]}"#;
        assert!(serde_json::from_str::<Config>(j).is_err());
    }

    #[test]
    fn agents_worker_rejects_enabled_providers() {
        let j = r#"{"agents":[
            {"id":"main","role":"orchestrator"},
            {"id":"fast","role":"worker","enabledProviders":["ollama"]}
        ]}"#;
        let err = serde_json::from_str::<Config>(j).unwrap_err();
        assert!(
            err.to_string().contains("enabledProviders") && err.to_string().contains("orchestrator-only"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn agents_worker_rejects_max_delegations_per_turn() {
        let j = r#"{"agents":[
            {"id":"main","role":"orchestrator"},
            {"id":"fast","role":"worker","maxDelegationsPerTurn":3}
        ]}"#;
        let err = serde_json::from_str::<Config>(j).unwrap_err();
        assert!(
            err.to_string().contains("maxDelegationsPerTurn") && err.to_string().contains("orchestrator-only"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn agents_worker_rejects_max_delegations_per_session() {
        let j = r#"{"agents":[
            {"id":"main","role":"orchestrator"},
            {"id":"fast","role":"worker","maxDelegationsPerSession":10}
        ]}"#;
        let err = serde_json::from_str::<Config>(j).unwrap_err();
        assert!(
            err.to_string().contains("maxDelegationsPerSession") && err.to_string().contains("orchestrator-only"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn agents_worker_rejects_max_delegations_per_worker() {
        let j = r#"{"agents":[
            {"id":"main","role":"orchestrator"},
            {"id":"fast","role":"worker","maxDelegationsPerWorker":{"search":5}}
        ]}"#;
        let err = serde_json::from_str::<Config>(j).unwrap_err();
        assert!(
            err.to_string().contains("maxDelegationsPerWorker") && err.to_string().contains("orchestrator-only"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn agents_worker_rejects_max_tool_loops_per_turn() {
        let j = r#"{"agents":[
            {"id":"main","role":"orchestrator"},
            {"id":"fast","role":"worker","maxToolLoopsPerTurn":100}
        ]}"#;
        let err = serde_json::from_str::<Config>(j).unwrap_err();
        assert!(
            err.to_string().contains("maxToolLoopsPerTurn") && err.to_string().contains("orchestrator-only"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn agents_worker_with_valid_fields_passes() {
        let j = r#"{"agents":[
            {"id":"main","role":"orchestrator"},
            {"id":"fast","role":"worker","defaultProvider":"lmstudio","defaultModel":"qwen3:8b","enabledSkills":["files"],"contextMode":"full"}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let w = c.agents.workers.as_ref().expect("workers");
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].id, "fast");
        assert_eq!(w[0].default_provider.as_deref(), Some("lmstudio"));
        assert_eq!(w[0].default_model.as_deref(), Some("qwen3:8b"));
    }

    #[test]
    fn agents_worker_rejects_enabled_workers() {
        let j = r#"{"agents":[
            {"id":"main","role":"orchestrator"},
            {"id":"fast","role":"worker","enabledWorkers":["other"]}
        ]}"#;
        let err = serde_json::from_str::<Config>(j).unwrap_err();
        assert!(
            err.to_string().contains("enabledWorkers") && err.to_string().contains("orchestrator-only"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn agents_enabled_workers_validation_unknown_id() {
        let j = r#"{"agents":[
            {"id":"main","role":"orchestrator","enabledWorkers":["fast","missing"]},
            {"id":"fast","role":"worker"}
        ]}"#;
        let err = serde_json::from_str::<Config>(j).unwrap_err();
        assert!(
            err.to_string().contains("unknown worker id") && err.to_string().contains("missing"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn agents_enabled_workers_valid() {
        let j = r#"{"agents":[
            {"id":"main","role":"orchestrator","enabledWorkers":["fast"]},
            {"id":"fast","role":"worker"},
            {"id":"slow","role":"worker"}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(c.agents.default_orchestrator().enabled_workers.as_deref().unwrap().len(), 1);
        assert_eq!(c.agents.default_orchestrator().enabled_workers.as_deref().unwrap()[0], "fast");
    }

    #[test]
    fn agents_enabled_workers_absent_means_none() {
        let j = r#"{"agents":[
            {"id":"main","role":"orchestrator"},
            {"id":"fast","role":"worker"}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert!(c.agents.default_orchestrator().enabled_workers.is_none());
    }

    #[test]
    fn agents_multi_orchestrator_round_trip() {
        let j = r#"{"agents":[
            {"id":"dev","role":"orchestrator","defaultProvider":"ollama","enabledWorkers":["engineer"]},
            {"id":"rev","role":"orchestrator","defaultProvider":"lmstudio","enabledWorkers":["reader"]},
            {"id":"engineer","role":"worker"},
            {"id":"reader","role":"worker"}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(c.agents.orchestrators.len(), 2);
        assert_eq!(c.agents.orchestrators[0].id, "dev");
        assert_eq!(c.agents.orchestrators[1].id, "rev");
        assert_eq!(c.agents.orchestrators[0].enabled_workers.as_deref().unwrap().len(), 1);
        assert_eq!(c.agents.orchestrators[1].enabled_workers.as_deref().unwrap()[0], "reader");
        // Round-trip through serde
        let json = serde_json::to_string(&c).expect("serialize");
        let c2: Config = serde_json::from_str(&json).expect("re-parse");
        assert_eq!(c2.agents.orchestrators.len(), 2);
        assert_eq!(c2.agents.orchestrators[0].id, "dev");
        assert_eq!(c2.agents.orchestrators[1].id, "rev");
    }

    #[test]
    fn providers_array_round_trips() {
        let j = r#"{"providers":[{"id":"ollama","endpointType":"ollama"},{"id":"lmstudio","endpointType":"openai-compat","modelDiscovery":"lmstudio","baseUrl":"http://127.0.0.1:9999/v1"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let lmstudio = c.providers.get("lmstudio").expect("lmstudio");
        assert_eq!(lmstudio.base_url.as_deref(), Some("http://127.0.0.1:9999/v1"));
        assert_eq!(lmstudio.endpoint_type, EndpointType::OpenaiCompat);
        assert_eq!(lmstudio.model_discovery, ModelDiscovery::Lmstudio);
        let out = serde_json::to_string(&c).expect("serialize");
        assert!(
            out.contains("\"lmstudio\""),
            "expected lmstudio id in {}",
            out
        );
    }

    #[test]
    fn providers_rejects_duplicate_ids() {
        let j = r#"{"providers":[{"id":"dup","endpointType":"ollama"},{"id":"dup","endpointType":"openai-compat"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert!(c.providers.validate().is_err());
    }

    #[test]
    fn providers_rejects_unknown_endpoint_type() {
        let j = r#"{"providers":[{"id":"x","endpointType":"unknown"}]}"#;
        assert!(serde_json::from_str::<Config>(j).is_err());
    }

    #[test]
    fn providers_default_base_url() {
        let j = r#"{"providers":[{"id":"ollama","endpointType":"ollama"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(
            resolve_provider_base_url(&c.providers, "ollama"),
            Some("http://127.0.0.1:11434".to_string())
        );
    }

    #[test]
    fn providers_openai_compat_default_base_url() {
        let j = r#"{"providers":[{"id":"my-openai","endpointType":"openai-compat"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(
            resolve_provider_base_url(&c.providers, "my-openai"),
            Some("http://127.0.0.1:1234/v1".to_string())
        );
    }

    #[test]
    fn providers_openai_compat_explicit_base_url() {
        let j = r#"{"providers":[{"id":"my-openai","endpointType":"openai-compat","baseUrl":"https://api.openai.com/v1"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(
            resolve_provider_base_url(&c.providers, "my-openai"),
            Some("https://api.openai.com/v1".to_string())
        );
    }

    #[test]
    fn providers_default_model_per_endpoint_type() {
        let j = r#"{"providers":[{"id":"ollama","endpointType":"ollama"},{"id":"lmstudio","endpointType":"openai-compat"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(
            resolve_provider_default_model(&c.providers, "ollama"),
            "llama3.2:3b"
        );
        assert_eq!(
            resolve_provider_default_model(&c.providers, "lmstudio"),
            "llama-3.2-3B-instruct"
        );
    }

    #[test]
    fn providers_custom_default_model() {
        let j = r#"{"providers":[{"id":"ollama","endpointType":"ollama","defaultModel":"qwen3:8b"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(
            resolve_provider_default_model(&c.providers, "ollama"),
            "qwen3:8b"
        );
    }

    #[test]
    fn providers_model_discovery_auto() {
        let j = r#"{"providers":[{"id":"ollama","endpointType":"ollama"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let def = c.providers.get("ollama").expect("ollama");
        assert_eq!(def.model_discovery, ModelDiscovery::Auto);
    }

    #[test]
    fn providers_model_discovery_lmstudio() {
        let j = r#"{"providers":[{"id":"lmstudio","endpointType":"openai-compat","modelDiscovery":"lmstudio"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let def = c.providers.get("lmstudio").expect("lmstudio");
        assert_eq!(def.model_discovery, ModelDiscovery::Lmstudio);
    }

    #[test]
    fn providers_model_discovery_static() {
        let j = r#"{"providers":[{"id":"custom","endpointType":"openai-compat","modelDiscovery":"static","staticModels":["a","b"]}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let def = c.providers.get("custom").expect("custom");
        assert_eq!(def.model_discovery, ModelDiscovery::Static);
        assert_eq!(def.static_models, vec!["a", "b"]);
    }

    #[test]
    fn providers_nim_like_config() {
        // A NIM-style provider using static model discovery.
        let j = r#"{"providers":[
            {"id":"nvidia","endpointType":"openai-compat","baseUrl":"https://integrate.api.nvidia.com/v1","apiKey":null,"modelDiscovery":"static","staticModels":["meta/llama-3.1-8b-instruct","meta/llama-3.1-70b-instruct"]}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let def = c.providers.get("nvidia").expect("nvidia");
        assert_eq!(def.endpoint_type, EndpointType::OpenaiCompat);
        assert_eq!(def.model_discovery, ModelDiscovery::Static);
        assert_eq!(def.static_models.len(), 2);
    }

    #[test]
    fn model_discovery_as_str() {
        assert_eq!(ModelDiscovery::Auto.as_str(), "auto");
        assert_eq!(ModelDiscovery::Lmstudio.as_str(), "lmstudio");
        assert_eq!(ModelDiscovery::Static.as_str(), "static");
    }

    #[test]
    fn providers_rejects_lmstudio_endpoint_type() {
        let j = r#"{"providers":[{"id":"x","endpointType":"lmstudio"}]}"#;
        assert!(serde_json::from_str::<Config>(j).is_err());
    }

    #[test]
    fn providers_missing_key_uses_default_ollama() {
        let c: Config = serde_json::from_str("{}").expect("parse");
        assert_eq!(c.providers.entries.len(), 1);
        assert_eq!(c.providers.entries[0].id, "ollama");
        assert_eq!(c.providers.entries[0].endpoint_type, EndpointType::Ollama);
    }

    #[test]
    fn providers_empty_array_uses_default_ollama() {
        let j = r#"{"providers":[]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(c.providers.entries.len(), 1);
        assert_eq!(c.providers.entries[0].id, "ollama");
        assert_eq!(c.providers.entries[0].endpoint_type, EndpointType::Ollama);
    }

    #[test]
    fn providers_default_has_ollama_resolveable() {
        let c = Config::default();
        assert!(c.providers.has("ollama"));
        assert_eq!(
            resolve_provider_base_url(&c.providers, "ollama"),
            Some("http://127.0.0.1:11434".to_string())
        );
        assert_eq!(
            resolve_provider_default_model(&c.providers, "ollama"),
            "llama3.2:3b"
        );
    }

    #[test]
    fn providers_empty_json_resolves_effective() {
        let c: Config = serde_json::from_str("{}").expect("parse");
        let (provider, model) = resolve_effective_provider_and_model(&c.providers, &c.agents);
        assert_eq!(provider, "ollama");
        assert_eq!(model, "llama3.2:3b");
    }

    // --- resolve_env_ref tests ---

    #[test]
    fn resolve_env_ref_literal_key() {
        assert_eq!(resolve_env_ref("sk-abc123"), "sk-abc123");
    }

    #[test]
    fn resolve_env_ref_env_var_set() {
        struct EnvGuard {
            key: String,
            previous: Option<std::ffi::OsString>,
        }
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                match &self.previous {
                    Some(v) => std::env::set_var(&self.key, v),
                    None => std::env::remove_var(&self.key),
                }
            }
        }
        let key = "CHAI_TEST_RESOLVE_ENV_REF";
        let guard = EnvGuard {
            key: key.to_string(),
            previous: std::env::var_os(key),
        };
        std::env::set_var(key, "resolved-key-value");
        assert_eq!(resolve_env_ref(&format!("<{}>", key)), "resolved-key-value");
        drop(guard);
    }

    #[test]
    fn resolve_env_ref_env_var_unset_returns_empty() {
        let key = "CHAI_TEST_DEFINITELY_NOT_SET_XYZ";
        let _guard = {
            struct G(String, Option<std::ffi::OsString>);
            impl Drop for G {
                fn drop(&mut self) {
                    match &self.1 {
                        Some(v) => std::env::set_var(&self.0, v),
                        None => std::env::remove_var(&self.0),
                    }
                }
            }
            let prev = std::env::var_os(key);
            std::env::remove_var(key);
            G(key.to_string(), prev)
        };
        assert_eq!(resolve_env_ref(&format!("<{}>", key)), "");
    }

    #[test]
    fn resolve_env_ref_empty_angle_brackets_untouched() {
        // `<>` is too short to be an env ref — treated as literal.
        assert_eq!(resolve_env_ref("<>"), "<>");
    }

    #[test]
    fn resolve_env_ref_partial_angle_brackets_untouched() {
        // Starts with `<` but does not end with `>` — not an env ref.
        assert_eq!(resolve_env_ref("<not-a-ref"), "<not-a-ref");
    }

    // --- resolve_provider_api_key with <ENV_VAR> ---

    #[test]
    fn resolve_provider_api_key_env_ref_resolves() {
        struct EnvGuard {
            key: String,
            previous: Option<std::ffi::OsString>,
        }
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                match &self.previous {
                    Some(v) => std::env::set_var(&self.key, v),
                    None => std::env::remove_var(&self.key),
                }
            }
        }
        let key = "CHAI_TEST_API_KEY_RESOLVE";
        let guard = EnvGuard {
            key: key.to_string(),
            previous: std::env::var_os(key),
        };
        std::env::set_var(key, "sk-from-env");

        let j = format!(
            r#"{{"providers":[{{"id":"nearai","endpointType":"openai-compat","baseUrl":"https://cloud-api.near.ai/v1","apiKey":"<{}>"}}]}}"#,
            key
        );
        let c: Config = serde_json::from_str(&j).expect("parse");
        assert_eq!(
            resolve_provider_api_key(&c.providers, "nearai"),
            Some("sk-from-env".to_string())
        );
        drop(guard);
    }

    #[test]
    fn resolve_provider_api_key_env_ref_unset_returns_none() {
        let key = "CHAI_TEST_API_KEY_NOT_SET";
        let _guard = {
            struct G(String, Option<std::ffi::OsString>);
            impl Drop for G {
                fn drop(&mut self) {
                    match &self.1 {
                        Some(v) => std::env::set_var(&self.0, v),
                        None => std::env::remove_var(&self.0),
                    }
                }
            }
            let prev = std::env::var_os(key);
            std::env::remove_var(key);
            G(key.to_string(), prev)
        };

        let j = format!(
            r#"{{"providers":[{{"id":"nearai","endpointType":"openai-compat","baseUrl":"https://cloud-api.near.ai/v1","apiKey":"<{}>"}}]}}"#,
            key
        );
        let c: Config = serde_json::from_str(&j).expect("parse");
        assert_eq!(resolve_provider_api_key(&c.providers, "nearai"), None);
    }

    #[test]
    fn resolve_provider_api_key_literal_value_still_works() {
        let j = r#"{"providers":[{"id":"nearai","endpointType":"openai-compat","baseUrl":"https://cloud-api.near.ai/v1","apiKey":"sk-literal-123"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(
            resolve_provider_api_key(&c.providers, "nearai"),
            Some("sk-literal-123".to_string())
        );
    }

    #[test]
    fn provider_discovery_enabled_union_across_orchestrators() {
        // Two orchestrators, each with a different defaultProvider and no
        // enabledProviders set. Model discovery must run for both providers
        // (union approach) so switching orchestrators doesn't require a restart.
        let j = r#"{"providers":[
            {"id":"nearai","endpointType":"openai-compat","baseUrl":"https://cloud-api.near.ai/v1"},
            {"id":"ollama","endpointType":"ollama"}
        ],"agents":[
            {"id":"developer","role":"orchestrator","defaultProvider":"nearai"},
            {"id":"reviewer","role":"orchestrator","defaultProvider":"ollama"}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        // Both providers should be discovered since each is the default for
        // an orchestrator that has no explicit enabledProviders.
        assert!(
            provider_discovery_enabled(&c.providers, &c.agents, "nearai"),
            "nearai should be discovered (default for developer)"
        );
        assert!(
            provider_discovery_enabled(&c.providers, &c.agents, "ollama"),
            "ollama should be discovered (default for reviewer)"
        );
    }

    #[test]
    fn orchestrator_discovery_provider_ids_per_orchestrator() {
        // Verify that orchestrator_discovery_provider_ids returns the correct
        // single provider for each orchestrator when enabledProviders is absent.
        let j = r#"{"providers":[
            {"id":"nearai","endpointType":"openai-compat","baseUrl":"https://cloud-api.near.ai/v1"},
            {"id":"ollama","endpointType":"ollama"}
        ],"agents":[
            {"id":"developer","role":"orchestrator","defaultProvider":"nearai"},
            {"id":"reviewer","role":"orchestrator","defaultProvider":"ollama"}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let dev = c.agents.orchestrator(Some("developer")).unwrap();
        let rev = c.agents.orchestrator(Some("reviewer")).unwrap();
        let dev_ids = orchestrator_discovery_provider_ids(&c.providers, dev);
        let rev_ids = orchestrator_discovery_provider_ids(&c.providers, rev);
        assert_eq!(dev_ids, vec!["nearai"]);
        assert_eq!(rev_ids, vec!["ollama"]);
    }

    #[test]
    fn resolve_provider_api_key_none_when_omitted() {
        let j = r#"{"providers":[{"id":"ollama","endpointType":"ollama"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(resolve_provider_api_key(&c.providers, "ollama"), None);
    }
}
