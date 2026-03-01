//! Configuration types and loading.
//!
//! Config is loaded from a JSON file (e.g. `~/.chai/config.json`) and environment.
//! Kept minimal for short-term goals; extend as needed for gateway, channels, and skills.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
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

    /// Agent defaults (e.g. default model for Ollama).
    #[serde(default)]
    pub agents: AgentsConfig,

    /// Skills load paths and options.
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

/// Agent defaults (backend, model, workspace, enabled backends for discovery).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsConfig {
    /// Which LLM backend to use: "ollama" or "lmstudio". When absent, defaults to "ollama".
    #[serde(default)]
    pub default_backend: Option<String>,
    /// Model id for the selected backend. Use the id format the backend expects (e.g. for Ollama "llama3.2:latest"; for LM Studio "openai/gpt-oss-20b", "ibm/granite-4-micro"). Not used for routing—backend is chosen by defaultBackend.
    pub default_model: Option<String>,
    /// Backends to fetch models from at startup (e.g. `["ollama", "lmstudio"]`). Opt-in: when absent or empty, only the default backend (from defaultBackend) is discovered; when set, only listed backends are polled.
    #[serde(default)]
    pub enabled_backends: Option<Vec<String>>,
    /// Workspace root (default ~/.chai/workspace).
    pub workspace: Option<PathBuf>,
    /// Optional per-backend settings (base URLs, LM Studio endpoint type).
    #[serde(default)]
    pub backends: Option<BackendsConfig>,
}

/// Per-backend configuration (base URL, endpoint type where applicable).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendsConfig {
    #[serde(default)]
    pub ollama: Option<OllamaBackendEntry>,
    #[serde(default)]
    pub lm_studio: Option<LmStudioBackendEntry>,
}

/// Ollama backend entry (e.g. base URL override).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaBackendEntry {
    pub base_url: Option<String>,
}

/// LM Studio backend entry: base URL and endpoint type (openai vs native).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LmStudioBackendEntry {
    pub base_url: Option<String>,
    /// "openai" (OpenAI-compatible API) or "native" (LM Studio native /api/v1/chat). Default "openai".
    #[serde(default)]
    pub endpoint_type: Option<LmStudioEndpointType>,
}

/// LM Studio endpoint type: OpenAI-compatible API or native API. LM Studio does not expose Ollama endpoints.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LmStudioEndpointType {
    /// OpenAI-compatible /v1/models and /v1/chat/completions (supports tools).
    #[default]
    Openai,
    /// LM Studio native /api/v1/models and /api/v1/chat (no custom tools in this implementation).
    Native,
}

/// Resolve LM Studio base URL: agents.backends.lmStudio.baseUrl, else default.
pub fn resolve_lm_studio_base_url(agents: &AgentsConfig) -> String {
    agents
        .backends
        .as_ref()
        .and_then(|b| b.lm_studio.as_ref())
        .and_then(|e| e.base_url.as_ref())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:1234/v1".to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Resolve LM Studio endpoint type: agents.backends.lmStudio.endpointType, else openai.
pub fn resolve_lm_studio_endpoint_type(agents: &AgentsConfig) -> LmStudioEndpointType {
    agents
        .backends
        .as_ref()
        .and_then(|b| b.lm_studio.as_ref())
        .and_then(|e| e.endpoint_type)
        .unwrap_or_default()
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

/// Skills load config (dirs, enabled list, context mode).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsConfig {
    /// Override the default skill root. If set, skills are loaded from this directory instead of the config directory's `skills` subdirectory. Relative paths are resolved against the config file's parent. Omit or leave empty to use the default (~/.chai/skills when config is ~/.chai/config.json).
    #[serde(default)]
    pub directory: Option<PathBuf>,
    /// Extra skill directories (lowest precedence).
    #[serde(default)]
    pub extra_dirs: Vec<PathBuf>,
    /// Skill names to load. Only skills in this list are enabled; default is empty (no skills). Add names of skills you want (e.g. `["notesmd-cli-daily"]`).
    #[serde(default)]
    pub enabled: Vec<String>,
    /// How skill docs are given to the model: "full" (default) or "readOnDemand". Full injects all SKILL.md into the system message; readOnDemand uses a compact list and a read_skill tool.
    #[serde(default)]
    pub context_mode: SkillContextMode,
    /// When true, skills may reference scripts in their scripts/ directory (e.g. for resolveCommand). Scripts are run via sh; only files under the skill's scripts/ dir are executed. Default: false.
    #[serde(default)]
    pub allow_scripts: bool,
}

/// True if model discovery should run for the given backend. Opt-in: when agents.enabled_backends is absent or empty, only the default backend (from defaultBackend) is discovered; when set, only backends in the list are discovered (case-insensitive, "ollama" | "lmstudio").
pub fn backend_discovery_enabled(agents: &AgentsConfig, backend: &str) -> bool {
    let use_default_only = match &agents.enabled_backends {
        None => true,
        Some(v) => v.is_empty(),
    };
    if use_default_only {
        let default = agents
            .default_backend
            .as_deref()
            .unwrap_or("ollama")
            .trim()
            .to_lowercase();
        let default_name = if default == "lmstudio" || default == "lm_studio" {
            "lmstudio"
        } else {
            "ollama"
        };
        let normalized = backend.trim().to_lowercase();
        return normalized == default_name;
    }
    let list = agents.enabled_backends.as_ref().unwrap();
    let normalized = backend.trim().to_lowercase();
    list.iter().any(|b| b.trim().to_lowercase() == normalized)
}

/// Resolve effective default backend and model for display (e.g. in desktop when gateway status is not yet available).
/// Returns (backend_name, model_id) where backend_name is "ollama" or "lmstudio".
pub fn resolve_effective_backend_and_model(agents: &AgentsConfig) -> (String, String) {
    let b = agents
        .default_backend
        .as_deref()
        .unwrap_or("ollama")
        .trim()
        .to_lowercase();
    let backend = if b == "lmstudio" || b == "lm_studio" {
        "lmstudio"
    } else {
        "ollama"
    };
    let model = agents
        .default_model
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let model = model.unwrap_or_else(|| {
        if backend == "lmstudio" {
            "gpt-oss-20b".to_string()
        } else {
            "llama3.2:latest".to_string()
        }
    });
    (backend.to_string(), model)
}

/// Resolve config path from env or default.
pub fn default_config_path() -> PathBuf {
    std::env::var("CHAI_CONFIG_PATH").map(PathBuf::from).unwrap_or_else(|_| {
        dirs::home_dir()
            .map(|h| h.join(".chai").join("config.json"))
            .unwrap_or_else(|| PathBuf::from("config.json"))
    })
}

/// Resolve workspace directory for agent context (e.g. AGENTS.md).
pub fn resolve_workspace_dir(config: &Config) -> Option<PathBuf> {
    config
        .agents
        .workspace
        .clone()
        .or_else(|| dirs::home_dir().map(|h| h.join(".chai").join("workspace")))
}

/// Load config from the default path (or CHAI_CONFIG_PATH). Missing file => default config.
/// Returns the config and the path that was used (for resolving the config directory).
pub fn load_config(path: Option<PathBuf>) -> Result<(Config, PathBuf)> {
    let path = path.unwrap_or_else(default_config_path);
    let config = if !path.exists() {
        log::debug!("config file not found, using defaults: {}", path.display());
        Config::default()
    } else {
        let s = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config from {}", path.display()))?;
        serde_json::from_str(&s)
            .with_context(|| format!("parsing config from {}", path.display()))?
    };
    Ok((config, path))
}

/// Default skill root when no override is set: `skills` subdirectory of the config file's parent.
pub fn skills_dir(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join("skills")
}

/// Resolve the primary skill root: uses `config.skills.directory` if set (relative paths resolved against the config file's parent), otherwise the default `skills` subdirectory.
pub fn resolve_skills_dir(config: &Config, config_path: &Path) -> PathBuf {
    let config_parent = config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match &config.skills.directory {
        Some(d) if !d.as_os_str().is_empty() => {
            if d.is_absolute() {
                d.clone()
            } else {
                config_parent.join(d)
            }
        }
        _ => skills_dir(config_path),
    }
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
    fn resolve_skills_dir_default() {
        let config = Config::default();
        let path = Path::new("/home/user/.chai/config.json");
        assert_eq!(
            resolve_skills_dir(&config, path),
            PathBuf::from("/home/user/.chai/skills")
        );
    }

    #[test]
    fn resolve_skills_dir_override_relative() {
        let mut config = Config::default();
        config.skills.directory = Some(PathBuf::from("custom/skills"));
        let path = Path::new("/home/user/.chai/config.json");
        assert_eq!(
            resolve_skills_dir(&config, path),
            PathBuf::from("/home/user/.chai/custom/skills")
        );
    }

    #[test]
    fn resolve_skills_dir_override_absolute() {
        let mut config = Config::default();
        config.skills.directory = Some(PathBuf::from("/repo/skills"));
        let path = Path::new("/home/user/.chai/config.json");
        assert_eq!(
            resolve_skills_dir(&config, path),
            PathBuf::from("/repo/skills")
        );
    }
}
