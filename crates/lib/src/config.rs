//! Configuration types and loading.
//!
//! Config is loaded from a JSON file under the active profile (e.g. `~/.chai/profiles/assistant/config.json`) and environment.
//! Top-level keys include `gateway`, `channels` (Telegram, Matrix, Signal), `providers` (URLs/keys for model APIs), and `agents`
//! (JSON array of `id` / `role` entries; omit the key for a single default orchestrator). Skill **packages** are always loaded from **`~/.chai/skills`** (per-agent enablement is under **`agents`**).

use anyhow::{Context, Result};
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

    /// Model provider connection settings (base URLs, API keys). Sibling to `channels`: integration points for model APIs.
    #[serde(default)]
    pub providers: Option<ProvidersConfig>,

    /// Agent definitions: JSON array of `id` + `role` (`orchestrator` \| `worker`). Omit for defaults (one orchestrator, id `orchestrator`).
    #[serde(default = "default_agents_config", with = "agents_config_de")]
    pub agents: AgentsConfig,
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

/// Signal channel: user-run signal-cli HTTP daemon (BYO). See `.agents/adr/SIGNAL_CLI_INTEGRATION.md`.
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
    /// Directory for matrix-sdk SQLite state and E2EE keys (default `<profile>/matrix`). Overridden by `CHAI_MATRIX_STORE`.
    pub store_path: Option<String>,
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
    /// Optional secret for webhook verification (X-Telegram-Bot-Api-Secret-Token). Used only when webhook_url is set.
    pub webhook_secret: Option<String>,
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
    /// Which default provider to use: "ollama", "lms", "vllm", or "nim". When absent, defaults to "ollama".
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
    /// Worker presets let you constrain which provider targets are allowed and provide per-worker
    /// default provider/model selections.
    pub workers: Option<Vec<WorkerConfig>>,

    /// Optional allowlist of `(provider, model)` pairs permitted for **`delegate_task`** when no
    /// worker-specific non-empty [`WorkerConfig::delegate_allowed_models`] applies. Omitted or empty
    /// means only the orchestrator effective default provider/model pair is allowed (see
    /// [`resolve_effective_provider_and_model`]).
    pub delegate_allowed_models: Option<Vec<AllowedModelEntry>>,

    /// Max successful **`delegate_task`** calls per session (orchestrator only). Omitted = no limit.
    pub max_delegations_per_session: Option<usize>,

    /// Optional per-provider caps on successful delegations per session (`nim` → 5). Canonical provider ids as keys.
    pub max_delegations_per_provider: Option<HashMap<String, usize>>,

    /// Providers delegation cannot target (canonical: `ollama`, `lms`, `vllm`, `nim`).
    pub delegate_blocked_providers: Option<Vec<String>>,

    /// When **`instruction`** starts with a route’s prefix, merge missing **`workerId`** / **`provider`** / **`model`** from that route (first match wins).
    pub delegation_instruction_routes: Option<Vec<DelegationInstructionRoute>>,
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
            delegate_allowed_models: None,
            max_delegations_per_session: None,
            max_delegations_per_provider: None,
            delegate_blocked_providers: None,
            delegation_instruction_routes: None,
        }
    }
}

fn default_agents_config() -> AgentsConfig {
    AgentsConfig::default()
}

/// Route **`delegate_task`** by **`instruction`** prefix (orchestrator policy). First matching prefix wins.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DelegationInstructionRoute {
    /// When **`instruction`** starts with this string (after trim), apply the optional overrides below.
    pub instruction_prefix: String,
    #[serde(default)]
    pub worker_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

/// One allowed `(provider, model)` pair for delegation policy. Provider ids use the same canonical
/// names as elsewhere: `ollama`, `lms`, `vllm`, `nim`. Model must match the resolved id exactly
/// (after trim), including any `:` or `/` in the provider's model naming.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AllowedModelEntry {
    pub provider: String,
    pub model: String,
    /// Hint for policy UIs: model is expected to run locally or self-hosted.
    #[serde(default)]
    pub local: bool,
    /// Hint: model supports tool calling well enough for worker turns.
    #[serde(default)]
    pub tool_capable: Option<bool>,
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
    delegate_allowed_models: Option<Vec<AllowedModelEntry>>,
    #[serde(default)]
    max_delegations_per_session: Option<usize>,
    #[serde(default)]
    max_delegations_per_provider: Option<HashMap<String, usize>>,
    #[serde(default)]
    delegate_blocked_providers: Option<Vec<String>>,
    #[serde(default)]
    delegation_instruction_routes: Option<Vec<DelegationInstructionRoute>>,
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
        delegate_allowed_models: agents.delegate_allowed_models.clone(),
        max_delegations_per_session: agents.max_delegations_per_session,
        max_delegations_per_provider: agents.max_delegations_per_provider.clone(),
        delegate_blocked_providers: agents.delegate_blocked_providers.clone(),
        delegation_instruction_routes: agents.delegation_instruction_routes.clone(),
    }];
    if let Some(ws) = &agents.workers {
        for w in ws {
            out.push(AgentDefinition {
                id: w.id.clone(),
                role: AgentRole::Worker,
                default_provider: w.default_provider.clone(),
                default_model: w.default_model.clone(),
                enabled_providers: w.enabled_providers.clone(),
                skills_enabled: w.skills_enabled.clone(),
                context_mode: w.context_mode,
                max_session_messages: None,
                max_delegations_per_turn: None,
                delegate_allowed_models: w.delegate_allowed_models.clone(),
                max_delegations_per_session: None,
                max_delegations_per_provider: None,
                delegate_blocked_providers: None,
                delegation_instruction_routes: None,
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
        delegate_allowed_models: Option<Vec<AllowedModelEntry>>,
        max_delegations_per_session: Option<usize>,
        max_delegations_per_provider: Option<HashMap<String, usize>>,
        delegate_blocked_providers: Option<Vec<String>>,
        delegation_instruction_routes: Option<Vec<DelegationInstructionRoute>>,
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
                    delegate_allowed_models: e.delegate_allowed_models,
                    max_delegations_per_session: e.max_delegations_per_session,
                    max_delegations_per_provider: e.max_delegations_per_provider,
                    delegate_blocked_providers: e.delegate_blocked_providers,
                    delegation_instruction_routes: e.delegation_instruction_routes,
                });
            }
            AgentRole::Worker => {
                worker_rows.push(WorkerConfig {
                    id,
                    default_provider: e.default_provider,
                    default_model: e.default_model,
                    enabled_providers: e.enabled_providers,
                    skills_enabled: e.skills_enabled,
                    context_mode: e.context_mode,
                    delegate_allowed_models: e.delegate_allowed_models,
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
        delegate_allowed_models: o.delegate_allowed_models,
        max_delegations_per_session: o.max_delegations_per_session,
        max_delegations_per_provider: o.max_delegations_per_provider,
        delegate_blocked_providers: o.delegate_blocked_providers,
        delegation_instruction_routes: o.delegation_instruction_routes,
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
/// The main model can delegate to `workerId` to get per-worker defaults and an allowlist of enabled providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerConfig {
    /// Stable worker id used as `workerId` in the `delegate_task` tool call.
    pub id: String,
    /// Which default provider to use when the worker is selected and `provider` is omitted in the tool call.
    #[serde(default)]
    pub default_provider: Option<String>,
    /// Model id for the selected provider. Not used for routing.
    pub default_model: Option<String>,
    /// Providers allowed for this worker when delegating.
    ///
    /// If omitted or empty, the global `agents.enabledProviders` rules apply.
    #[serde(default)]
    pub enabled_providers: Option<Vec<String>>,

    /// Skill package names enabled for this worker. Omitted or empty ⇒ no skills.
    #[serde(default)]
    pub skills_enabled: Option<Vec<String>>,
    /// How this worker's skill docs are inlined vs `read_skill`.
    #[serde(default)]
    pub context_mode: Option<SkillContextMode>,

    /// Optional allowlist of `(provider, model)` for **`delegate_task`** when **`workerId`** matches
    /// this worker. When **non-empty**, only these pairs are allowed for that worker (orchestrator
    /// [`AgentsConfig::delegate_allowed_models`] is not applied for that worker). When omitted or
    /// empty, only this worker's effective default provider/model pair is allowed (same resolution
    /// as runtime `delegate_task` defaults).
    #[serde(default)]
    pub delegate_allowed_models: Option<Vec<AllowedModelEntry>>,
}

/// Per-provider configuration (base URL, API key where applicable).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ProvidersConfig {
    pub ollama: Option<OllamaProviderEntry>,
    /// JSON key **`lms`** (same string as **`agents.defaultProvider`** `"lms"`). LM Studio (OpenAI-compat).
    pub lms: Option<LmsProviderEntry>,
    pub nim: Option<NimProviderEntry>,
    pub vllm: Option<VllmProviderEntry>,
    /// OpenAI API (`https://api.openai.com/v1` by default). Optional base URL override (e.g. Azure OpenAI proxy).
    pub openai: Option<OpenAiProviderEntry>,
    /// Hugging Face Inference Endpoints or TGI with OpenAI-compatible routes; set **`baseUrl`** to the deployment URL including `/v1`.
    pub hf: Option<HfProviderEntry>,
}

/// Ollama provider entry (e.g. base URL override).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaProviderEntry {
    pub base_url: Option<String>,
}

/// LM Studio provider entry (`lms`): base URL only. We always use the OpenAI-compatible API (with native chat fallback when the server rejects the model param) so tools are supported.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LmsProviderEntry {
    pub base_url: Option<String>,
}

/// NVIDIA NIM hosted API provider entry. API key from config or NVIDIA_API_KEY env. Not a privacy option; data is sent to NVIDIA.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NimProviderEntry {
    pub api_key: Option<String>,
    /// Extra NIM model ids merged into gateway **`nimModels`** (and desktop) in addition to the built-in static catalog. Use exact ids from the NIM docs.
    #[serde(default)]
    pub extra_models: Option<Vec<String>>,
}

/// vLLM OpenAI-compatible server (`vllm serve`). Base URL should include `/v1` (e.g. `http://127.0.0.1:8000/v1`). Optional API key when the server uses `--api-key`.
/// LocalAI in OpenAI-compat mode can use the same settings: set **`agents.defaultProvider`** to **`"vllm"`** and point **`baseUrl`** at LocalAI's `/v1` base (Ollama-compatible LocalAI mode uses **`"ollama"`** and **`providers.ollama.baseUrl`** instead).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VllmProviderEntry {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

/// OpenAI API. API key: **`OPENAI_API_KEY`** env, else **`providers.openai.apiKey`**.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAiProviderEntry {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

/// Hugging Face OpenAI-compatible endpoint. API key: **`HF_API_KEY`** env, else **`providers.hf.apiKey`**. Set **`baseUrl`** to your Inference Endpoint or TGI URL (include `/v1`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HfProviderEntry {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

fn providers_config(config: &Config) -> Option<&ProvidersConfig> {
    config.providers.as_ref()
}

/// Resolve Ollama base URL override: `providers.ollama.baseUrl` when set, else `None` (client default).
pub fn resolve_ollama_base_url(config: &Config) -> Option<String> {
    providers_config(config)?
        .ollama
        .as_ref()
        .and_then(|e| e.base_url.as_ref())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_end_matches('/').to_string())
}

/// Resolve NVIDIA NIM API key: NVIDIA_API_KEY env, else `providers.nim.apiKey`.
pub fn resolve_nim_api_key(config: &Config) -> Option<String> {
    // Follow the same pattern as resolve_telegram_token / resolve_gateway_token:
    // environment variable takes precedence (easy to override at runtime),
    // then fall back to config when env is unset or empty.
    std::env::var("NVIDIA_API_KEY")
        .ok()
        .and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        })
        .or_else(|| {
            providers_config(config)?
                .nim
                .as_ref()
                .and_then(|e| e.api_key.as_ref())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Resolve LM Studio base URL: `providers.lms.baseUrl`, else default.
pub fn resolve_lms_base_url(config: &Config) -> String {
    providers_config(config)
        .and_then(|b| b.lms.as_ref())
        .and_then(|e| e.base_url.as_ref())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:1234/v1".to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Resolve vLLM base URL: `providers.vllm.baseUrl`, else default `http://127.0.0.1:8000/v1`.
pub fn resolve_vllm_base_url(config: &Config) -> String {
    providers_config(config)
        .and_then(|b| b.vllm.as_ref())
        .and_then(|e| e.base_url.as_ref())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8000/v1".to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Resolve vLLM API key: VLLM_API_KEY env, else `providers.vllm.apiKey`.
pub fn resolve_vllm_api_key(config: &Config) -> Option<String> {
    std::env::var("VLLM_API_KEY")
        .ok()
        .and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        })
        .or_else(|| {
            providers_config(config)?
                .vllm
                .as_ref()
                .and_then(|e| e.api_key.as_ref())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Resolve OpenAI base URL: `providers.openai.baseUrl`, else `https://api.openai.com/v1`.
pub fn resolve_openai_base_url(config: &Config) -> String {
    providers_config(config)
        .and_then(|b| b.openai.as_ref())
        .and_then(|e| e.base_url.as_ref())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Resolve OpenAI API key: OPENAI_API_KEY env, else `providers.openai.apiKey`.
pub fn resolve_openai_api_key(config: &Config) -> Option<String> {
    std::env::var("OPENAI_API_KEY")
        .ok()
        .and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        })
        .or_else(|| {
            providers_config(config)?
                .openai
                .as_ref()
                .and_then(|e| e.api_key.as_ref())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Resolve Hugging Face OpenAI-compat base URL: `providers.hf.baseUrl`, else `http://127.0.0.1:8080/v1` (set a real endpoint for Inference Endpoints or TGI).
pub fn resolve_hf_base_url(config: &Config) -> String {
    providers_config(config)
        .and_then(|b| b.hf.as_ref())
        .and_then(|e| e.base_url.as_ref())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8080/v1".to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Resolve Hugging Face API key: HF_API_KEY env, else `providers.hf.apiKey`.
pub fn resolve_hf_api_key(config: &Config) -> Option<String> {
    std::env::var("HF_API_KEY")
        .ok()
        .and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        })
        .or_else(|| {
            providers_config(config)?
                .hf
                .as_ref()
                .and_then(|e| e.api_key.as_ref())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Canonical provider id: "ollama", "lms", "vllm", "nim", "openai", and "hf" are valid (trimmed, lowercased). Returns None for any other value.
pub fn canonical_provider(s: &str) -> Option<&'static str> {
    match s.trim().to_lowercase().as_str() {
        "ollama" => Some("ollama"),
        "lms" => Some("lms"),
        "vllm" => Some("vllm"),
        "nim" => Some("nim"),
        "openai" => Some("openai"),
        "hf" => Some("hf"),
        _ => None,
    }
}

/// True if model discovery should run for the given provider. Opt-in: when agents.enabled_providers is absent or empty, only the default provider is discovered; when set, only providers in the list are discovered.
pub fn provider_discovery_enabled(agents: &AgentsConfig, provider: &str) -> bool {
    let provider_canonical = match canonical_provider(provider) {
        Some(b) => b,
        None => return false,
    };
    let use_default_only = match &agents.enabled_providers {
        None => true,
        Some(v) => v.is_empty(),
    };
    if use_default_only {
        let default_canonical = agents
            .default_provider
            .as_deref()
            .and_then(canonical_provider)
            .unwrap_or("ollama");
        return provider_canonical == default_canonical;
    }
    let list = agents.enabled_providers.as_ref().unwrap();
    list.iter()
        .filter_map(|b| canonical_provider(b))
        .any(|b| b == provider_canonical)
}

/// Canonical provider ids (`ollama`, `lms`, …) for which [`provider_discovery_enabled`] is true.
/// Matches which backends run model discovery at gateway startup and which **`status`** includes `*Models` for.
pub fn discovery_enabled_provider_ids(agents: &AgentsConfig) -> Vec<String> {
    ["ollama", "lms", "vllm", "nim", "openai", "hf"]
        .iter()
        .copied()
        .filter(|p| provider_discovery_enabled(agents, p))
        .map(str::to_string)
        .collect()
}

/// Resolve effective default provider and model for display (e.g. in desktop when gateway status is not yet available).
/// Returns (provider_id, model_id). Invalid provider values fall back to "ollama".
pub fn resolve_effective_provider_and_model(agents: &AgentsConfig) -> (String, String) {
    let provider = agents
        .default_provider
        .as_deref()
        .and_then(canonical_provider)
        .unwrap_or("ollama");
    let model = agents
        .default_model
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let model = model.unwrap_or_else(|| match provider {
        "lms" => "llama-3.2-3B-instruct".to_string(),
        "vllm" => "Qwen/Qwen2.5-7B-Instruct".to_string(),
        "nim" => "meta/llama-3.2-3b-instruct".to_string(),
        "openai" => "gpt-4o-mini".to_string(),
        "hf" => "meta-llama/Llama-3.1-8B-Instruct".to_string(),
        _ => "llama3.2:3b".to_string(),
    });
    (provider.to_string(), model)
}

/// Orchestrator **agent context directory** (on-disk home for **`AGENTS.md`**): `<profile_dir>/agents/<orchestratorId>/`.
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

/// `<profile_dir>/agents/<agent_id>/` — directory for that agent’s on-disk context (**`AGENTS.md`**).
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

/// Orchestrator enabled skill names (may be empty).
pub fn orchestrator_skills_enabled_list(agents: &AgentsConfig) -> &[String] {
    agents.skills_enabled.as_deref().unwrap_or(&[])
}

/// Worker enabled skill names (may be empty).
pub fn worker_skills_enabled_list(worker: &WorkerConfig) -> &[String] {
    worker.skills_enabled.as_deref().unwrap_or(&[])
}

/// Load config for the resolved profile (`CHAI_PROFILE`, `chai gateway --profile`, or `~/.chai/active`).
/// Missing `config.json` in the profile => default config.
pub fn load_config(cli_profile: Option<&str>) -> Result<(Config, crate::profile::ChaiPaths)> {
    let paths = crate::profile::resolve_profile_dir(cli_profile)?;
    let path = &paths.config_path;
    let config = if !path.exists() {
        log::debug!(
            "config file not found, using defaults: {}",
            path.display()
        );
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
            enabled_providers: None,
            skills_enabled: None,
            context_mode: None,
            delegate_allowed_models: None,
        };
        assert_eq!(
            worker_context_dir(&w, prof),
            Some(PathBuf::from("/home/u/.chai/profiles/p1/agents/w1"))
        );
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
    fn agents_delegate_allowed_models_round_trips() {
        let j = r#"{"agents":[
            {"id":"main","role":"orchestrator","defaultProvider":"ollama","defaultModel":"m",
             "delegateAllowedModels":[{"provider":"ollama","model":"llama3.2:latest","local":true}]},
            {"id":"fast","role":"worker","defaultProvider":"lms","defaultModel":"w",
             "delegateAllowedModels":[{"provider":"lms","model":"ibm/granite-4-micro"}]}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let orch = c.agents.delegate_allowed_models.as_ref().expect("orch catalog");
        assert_eq!(orch.len(), 1);
        assert_eq!(orch[0].provider, "ollama");
        assert_eq!(orch[0].model, "llama3.2:latest");
        assert!(orch[0].local);
        let w = &c.agents.workers.as_ref().expect("workers")[0];
        let wl = w.delegate_allowed_models.as_ref().expect("worker catalog");
        assert_eq!(wl[0].model, "ibm/granite-4-micro");
        let out = serde_json::to_string(&c).expect("serialize");
        assert!(out.contains("delegateAllowedModels"));
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
    fn providers_lms_key_round_trips() {
        let j = r#"{"providers":{"lms":{"baseUrl":"http://127.0.0.1:9999/v1"}}}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let p = c.providers.as_ref().expect("providers");
        let lm = p.lms.as_ref().expect("lms");
        assert_eq!(
            lm.base_url.as_deref(),
            Some("http://127.0.0.1:9999/v1")
        );
        let out = serde_json::to_string(&c).expect("serialize");
        assert!(
            out.contains("\"lms\""),
            "expected canonical key lms in {}",
            out
        );
    }

    #[test]
    fn providers_rejects_unknown_keys() {
        let j = r#"{"providers":{"lmstudio":{"baseUrl":"http://example/v1"}}}"#;
        assert!(serde_json::from_str::<Config>(j).is_err());
    }
}
