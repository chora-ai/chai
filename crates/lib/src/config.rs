//! Configuration types and loading.
//!
//! Config is loaded from a JSON file under the active profile (e.g. `~/.chai/profiles/assistant/config.json`) and environment.
//! Top-level keys include `gateway`, `channels` (Telegram, Matrix, Signal), `providers` (JSON array of `id` + `endpointType` entries
//! for model APIs), and `agents` (JSON array of `id` / `role` entries; omit the key for a single default orchestrator).
//! Skill **packages** are always loaded from **`~/.chai/skills`** (per-agent enablement is under **`agents`**).

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

    /// Agent definitions: JSON array of `id` + `role` (`orchestrator` \| `worker`). Omit for defaults (one orchestrator, id `orchestrator`).
    #[serde(default = "default_agents_config", with = "agents_config_de")]
    pub agents: AgentsConfig,

    /// How the gateway handles mismatches between the lockfile and active skill versions.
    /// `"strict"` (default): refuse to start. `"warn"`: log a warning and continue.
    #[serde(default)]
    pub skill_lock_mode: SkillLockMode,
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

/// How the gateway handles mismatches between the lockfile and active skill versions at startup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillLockMode {
    /// Refuse to start the gateway when any enabled skill's active version does not match its locked hash.
    #[default]
    Strict,
    /// Log a warning when the active version does not match the locked hash, but continue loading.
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

/// Resolved agents configuration: one orchestrator (flattened fields) plus optional worker presets for `delegate_task`.
///
/// In `config.json`, **`agents`** is a JSON **array** of entries with `id`, `role` (`orchestrator` \| `worker`),
/// and per-entry fields. Exactly one orchestrator and unique ids are required. Omit **`agents`** entirely
/// for built-in defaults (single orchestrator id `orchestrator`).
#[derive(Debug, Clone)]
pub struct AgentsConfig {
    /// Orchestrator entry's `id` from config (defaults to `orchestrator` when `agents` is omitted).
    pub orchestrator_id: Option<String>,
    /// Which default provider to use (a provider `id` from the `providers` array). When absent, defaults to "ollama" (if configured) or the first configured provider.
    pub default_provider: Option<String>,
    /// Model id for the selected provider. Use the id format the provider expects (e.g. for Ollama `llama3.2:3b`; for LM Studio `llama-3.2-3B-instruct`; for NIM `meta/llama-3.2-3b-instruct`). Not used for routing—provider is chosen by defaultProvider.
    pub default_model: Option<String>,
    /// Providers to fetch models from at startup (e.g. `["ollama", "lms"]`). Opt-in: when absent or empty, only the default provider (from defaultProvider) is discovered; when set, only listed providers are polled.
    pub enabled_providers: Option<Vec<String>>,
    /// Optional cap on the number of recent session messages sent to the model on each turn.
    /// When set, only the last N messages are included in the provider request; the full session
    /// history is still stored in memory. Default: unset (no cap).
    pub max_session_messages: Option<usize>,

    /// Skill package names enabled for the orchestrator (subset of packages under `~/.chai/skills`). Omitted or empty ⇒ no skills for the orchestrator.
    pub skills_enabled: Option<Vec<String>>,
    /// How orchestrator skill docs are inlined vs `read_skill`.
    pub context_mode: Option<SkillContextMode>,

    /// Optional cap on the number of `delegate_task` tool calls allowed per turn.
    /// When unset, delegation is limited only by the tool loop iteration cap.
    pub max_delegations_per_turn: Option<usize>,

    /// Worker presets for `delegate_task` `workerId` (from array entries with `role: worker`).
    ///
    /// Worker presets provide per-worker `defaultProvider` / `defaultModel` selections.
    pub workers: Option<Vec<WorkerConfig>>,

    /// Max successful **`delegate_task`** calls per session (orchestrator only). Omitted = no limit.
    pub max_delegations_per_session: Option<usize>,

    /// Optional per-provider caps on successful delegations per session (`nim` → 5). Canonical provider ids as keys.
    pub max_delegations_per_provider: Option<HashMap<String, usize>>,

    /// Maximum number of agent loop iterations (LLM round-trips) per turn. Each iteration is one
    /// call to the provider followed by tool call execution. The loop exits naturally when the model
    /// returns no tool calls; this limit is a safety net against runaway loops. Default: 500.
    pub max_tool_loop_iterations: Option<u32>,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            orchestrator_id: Some("orchestrator".to_string()),
            default_provider: None,
            default_model: None,
            enabled_providers: None,
            max_session_messages: None,
            skills_enabled: None,
            context_mode: None,
            max_delegations_per_turn: None,
            workers: None,
            max_delegations_per_session: None,
            max_delegations_per_provider: None,
            max_tool_loop_iterations: None,
        }
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
    skills_enabled: Option<Vec<String>>,
    #[serde(default)]
    context_mode: Option<SkillContextMode>,
    #[serde(default)]
    max_session_messages: Option<usize>,
    #[serde(default)]
    max_delegations_per_turn: Option<usize>,
    #[serde(default)]
    max_delegations_per_session: Option<usize>,
    #[serde(default)]
    max_delegations_per_provider: Option<HashMap<String, usize>>,
    #[serde(default)]
    max_tool_loop_iterations: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AgentRole {
    Orchestrator,
    Worker,
}

fn agents_to_definitions(agents: &AgentsConfig) -> Vec<AgentDefinition> {
    let oid = agents
        .orchestrator_id
        .as_deref()
        .unwrap_or("orchestrator")
        .to_string();
    let mut out = vec![AgentDefinition {
        id: oid,
        role: AgentRole::Orchestrator,
        default_provider: agents.default_provider.clone(),
        default_model: agents.default_model.clone(),
        enabled_providers: agents.enabled_providers.clone(),
        skills_enabled: agents.skills_enabled.clone(),
        context_mode: agents.context_mode,
        max_session_messages: agents.max_session_messages,
        max_delegations_per_turn: agents.max_delegations_per_turn,
        max_delegations_per_session: agents.max_delegations_per_session,
        max_delegations_per_provider: agents.max_delegations_per_provider.clone(),
        max_tool_loop_iterations: agents.max_tool_loop_iterations,
    }];
    if let Some(ws) = &agents.workers {
        for w in ws {
            out.push(AgentDefinition {
                id: w.id.clone(),
                role: AgentRole::Worker,
                default_provider: w.default_provider.clone(),
                default_model: w.default_model.clone(),
                enabled_providers: None,
                skills_enabled: w.skills_enabled.clone(),
                context_mode: w.context_mode,
                max_session_messages: None,
                max_delegations_per_turn: None,
                max_delegations_per_session: None,
                max_delegations_per_provider: None,
                max_tool_loop_iterations: None,
            });
        }
    }
    out
}

fn agents_from_array(entries: Vec<AgentDefinition>) -> Result<AgentsConfig, String> {
    struct OrchestratorFields {
        id: String,
        default_provider: Option<String>,
        default_model: Option<String>,
        enabled_providers: Option<Vec<String>>,
        skills_enabled: Option<Vec<String>>,
        context_mode: Option<SkillContextMode>,
        max_session_messages: Option<usize>,
        max_delegations_per_turn: Option<usize>,
        max_delegations_per_session: Option<usize>,
        max_delegations_per_provider: Option<HashMap<String, usize>>,
        max_tool_loop_iterations: Option<u32>,
    }

    let mut orchestrator: Option<OrchestratorFields> = None;
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
                if orchestrator.is_some() {
                    return Err("agents array must include exactly one orchestrator".to_string());
                }
                orchestrator = Some(OrchestratorFields {
                    id,
                    default_provider: e.default_provider,
                    default_model: e.default_model,
                    enabled_providers: e.enabled_providers,
                    skills_enabled: e.skills_enabled,
                    context_mode: e.context_mode,
                    max_session_messages: e.max_session_messages,
                    max_delegations_per_turn: e.max_delegations_per_turn,
                    max_delegations_per_session: e.max_delegations_per_session,
                    max_delegations_per_provider: e.max_delegations_per_provider,
                    max_tool_loop_iterations: e.max_tool_loop_iterations,
                });
            }
            AgentRole::Worker => {
                worker_rows.push(WorkerConfig {
                    id,
                    default_provider: e.default_provider,
                    default_model: e.default_model,
                    skills_enabled: e.skills_enabled,
                    context_mode: e.context_mode,
                });
            }
        }
    }

    let o = orchestrator.ok_or_else(|| {
        "agents array must include exactly one entry with role \"orchestrator\"".to_string()
    })?;

    Ok(AgentsConfig {
        orchestrator_id: Some(o.id),
        default_provider: o.default_provider,
        default_model: o.default_model,
        enabled_providers: o.enabled_providers,
        skills_enabled: o.skills_enabled,
        context_mode: o.context_mode,
        max_session_messages: o.max_session_messages,
        max_delegations_per_turn: o.max_delegations_per_turn,
        workers: if worker_rows.is_empty() {
            None
        } else {
            Some(worker_rows)
        },
        max_delegations_per_session: o.max_delegations_per_session,
        max_delegations_per_provider: o.max_delegations_per_provider,
        max_tool_loop_iterations: o.max_tool_loop_iterations,
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
    pub skills_enabled: Option<Vec<String>>,
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
            model_discovery: ModelDiscovery::Default,
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
    Default,
    /// LM Studio native model list: `GET /api/v1/models`, filter `type == "llm"`, use `key` as
    /// model id. Applicable to `openai-compat` endpoint type only.
    Lmstudio,
    /// Use the `staticModels` config field. No polling. Works for any endpoint type.
    Static,
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

/// True if model discovery should run for the given provider. Opt-in: when agents.enabled_providers
/// is absent or empty, only the default provider is discovered; when set, only providers in the
/// list are discovered.
pub fn provider_discovery_enabled(providers: &ProvidersConfig, agents: &AgentsConfig, provider_id: &str) -> bool {
    let id = match canonical_provider_id(providers, provider_id) {
        Some(id) => id,
        None => return false,
    };
    let use_default_only = match &agents.enabled_providers {
        None => true,
        Some(v) => v.is_empty(),
    };
    if use_default_only {
        let default_id = agents
            .default_provider
            .as_deref()
            .and_then(|s| canonical_provider_id(providers, s))
            .unwrap_or_else(|| {
                // If the default provider string doesn't match any configured provider,
                // fall back to "ollama" if it exists.
                if providers.has("ollama") { "ollama".to_string() } else { String::new() }
            });
        return id == default_id;
    }
    agents
        .enabled_providers
        .as_ref()
        .unwrap()
        .iter()
        .filter_map(|s| canonical_provider_id(providers, s))
        .any(|p| p == id)
}

/// Provider ids for which [`provider_discovery_enabled`] is true.
/// Matches which backends run model discovery at gateway startup and which **`status`** includes `*Models` for.
pub fn discovery_enabled_provider_ids(providers: &ProvidersConfig, agents: &AgentsConfig) -> Vec<String> {
    providers
        .ids()
        .into_iter()
        .filter(|id| provider_discovery_enabled(providers, agents, id))
        .collect()
}

/// Resolve effective default provider and model for display (e.g. in desktop when gateway status is not yet available).
/// Returns (provider_id, model_id). Invalid provider values fall back to the first configured provider
/// or "ollama" defaults if no providers are configured.
pub fn resolve_effective_provider_and_model(providers: &ProvidersConfig, agents: &AgentsConfig) -> (String, String) {
    let provider = agents
        .default_provider
        .as_deref()
        .and_then(|s| canonical_provider_id(providers, s))
        .or_else(|| {
            // Fall back to first configured provider.
            providers.entries.first().map(|p| p.id.trim().to_string())
        })
        .unwrap_or_else(|| "ollama".to_string());
    let model = agents
        .default_model
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| resolve_provider_default_model(providers, &provider));
    (provider, model)
}

/// Orchestrator **agent context directory** (on-disk home for **`AGENT.md`**): `<profile_dir>/agents/<orchestratorId>/`.
pub fn orchestrator_context_dir(config: &Config, profile_dir: &Path) -> PathBuf {
    let oid = config
        .agents
        .orchestrator_id
        .as_deref()
        .unwrap_or("orchestrator")
        .trim();
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
fn agent_context_dir(profile_dir: &Path, agent_id: &str) -> PathBuf {
    profile_dir.join("agents").join(agent_id)
}

/// Orchestrator skill context mode (default full).
pub fn orchestrator_context_mode(agents: &AgentsConfig) -> SkillContextMode {
    agents.context_mode.unwrap_or_default()
}

/// Worker skill context mode (default full).
pub fn worker_context_mode(worker: &WorkerConfig) -> SkillContextMode {
    worker.context_mode.unwrap_or_default()
}

/// Default maximum agent loop iterations per turn when `maxToolLoopIterations` is not set.
pub const DEFAULT_MAX_TOOL_LOOP_ITERATIONS: u32 = 500;

/// Resolve the maximum agent loop iterations per turn from config, falling back to
/// [`DEFAULT_MAX_TOOL_LOOP_ITERATIONS`] when not set.
pub fn resolve_max_tool_loop_iterations(agents: &AgentsConfig) -> u32 {
    agents
        .max_tool_loop_iterations
        .unwrap_or(DEFAULT_MAX_TOOL_LOOP_ITERATIONS)
}

/// Orchestrator enabled skill names (may be empty).
pub fn orchestrator_skills_enabled_list(agents: &AgentsConfig) -> &[String] {
    agents.skills_enabled.as_deref().unwrap_or(&[])
}

/// Worker enabled skill names (may be empty).
pub fn worker_skills_enabled_list(worker: &WorkerConfig) -> &[String] {
    worker.skills_enabled.as_deref().unwrap_or(&[])
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
        c.agents.orchestrator_id = Some("orch-id".to_string());
        let prof = Path::new("/home/u/.chai/profiles/p1");
        assert_eq!(
            orchestrator_context_dir(&c, prof),
            PathBuf::from("/home/u/.chai/profiles/p1/agents/orch-id")
        );
        let w = WorkerConfig {
            id: "w1".to_string(),
            default_provider: None,
            default_model: None,
            skills_enabled: None,
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
        assert_eq!(c.agents.orchestrator_id.as_deref(), Some("orchestrator"));
        assert!(c.agents.workers.is_none());
    }

    #[test]
    fn agents_array_one_orchestrator_and_worker() {
        let j = r#"{"agents":[
            {"id":"main","role":"orchestrator","defaultProvider":"ollama","defaultModel":"m"},
            {"id":"fast","role":"worker","defaultProvider":"lms","defaultModel":"w"}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(c.agents.orchestrator_id.as_deref(), Some("main"));
        assert_eq!(c.agents.default_provider.as_deref(), Some("ollama"));
        let w = c.agents.workers.as_ref().expect("workers");
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].id, "fast");
        assert_eq!(w[0].default_provider.as_deref(), Some("lms"));
    }

    #[test]
    fn agents_array_rejects_two_orchestrators() {
        let j = r#"{"agents":[
            {"id":"a","role":"orchestrator"},
            {"id":"b","role":"orchestrator"}
        ]}"#;
        let err = serde_json::from_str::<Config>(j).unwrap_err();
        assert!(
            err.to_string().contains("orchestrator"),
            "unexpected: {}",
            err
        );
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
    fn providers_array_round_trips() {
        let j = r#"{"providers":[{"id":"ollama","endpointType":"ollama"},{"id":"lms","endpointType":"openai-compat","modelDiscovery":"lmstudio","baseUrl":"http://127.0.0.1:9999/v1"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let lms = c.providers.get("lms").expect("lms");
        assert_eq!(lms.base_url.as_deref(), Some("http://127.0.0.1:9999/v1"));
        assert_eq!(lms.endpoint_type, EndpointType::OpenaiCompat);
        assert_eq!(lms.model_discovery, ModelDiscovery::Lmstudio);
        let out = serde_json::to_string(&c).expect("serialize");
        assert!(
            out.contains("\"lms\""),
            "expected lms id in {}",
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
        let j = r#"{"providers":[{"id":"ollama","endpointType":"ollama"},{"id":"lms","endpointType":"openai-compat"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(
            resolve_provider_default_model(&c.providers, "ollama"),
            "llama3.2:3b"
        );
        assert_eq!(
            resolve_provider_default_model(&c.providers, "lms"),
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
    fn providers_model_discovery_default() {
        let j = r#"{"providers":[{"id":"ollama","endpointType":"ollama"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let def = c.providers.get("ollama").expect("ollama");
        assert_eq!(def.model_discovery, ModelDiscovery::Default);
    }

    #[test]
    fn providers_model_discovery_lmstudio() {
        let j = r#"{"providers":[{"id":"lms","endpointType":"openai-compat","modelDiscovery":"lmstudio"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let def = c.providers.get("lms").expect("lms");
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
            {"id":"nim","endpointType":"openai-compat","baseUrl":"https://integrate.api.nvidia.com/v1","apiKey":null,"modelDiscovery":"static","staticModels":["meta/llama-3.1-8b-instruct","meta/llama-3.1-70b-instruct"]}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let def = c.providers.get("nim").expect("nim");
        assert_eq!(def.endpoint_type, EndpointType::OpenaiCompat);
        assert_eq!(def.model_discovery, ModelDiscovery::Static);
        assert_eq!(def.static_models.len(), 2);
    }

    #[test]
    fn providers_rejects_lms_endpoint_type() {
        let j = r#"{"providers":[{"id":"x","endpointType":"lms"}]}"#;
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
    fn resolve_provider_api_key_none_when_omitted() {
        let j = r#"{"providers":[{"id":"ollama","endpointType":"ollama"}]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        assert_eq!(resolve_provider_api_key(&c.providers, "ollama"), None);
    }
}
