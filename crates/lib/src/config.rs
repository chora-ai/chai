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

/// Agent defaults (model, workspace).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsConfig {
    /// Default Ollama model: use the exact name from `ollama list` (e.g. "llama3.2:latest", "smollm2:1.7b"). Do not add extra segments like ":latest" unless that tag exists for the model.
    pub default_model: Option<String>,
    /// Workspace root (default ~/.chai/workspace).
    pub workspace: Option<PathBuf>,
}

/// Skills load config (dirs, gating, disabled list).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsConfig {
    /// Extra skill directories (lowest precedence).
    #[serde(default)]
    pub extra_dirs: Vec<PathBuf>,
    /// Skill names to skip even when loaded from a directory (e.g. when both obsidian and notesmd-cli are on PATH but you want only one).
    #[serde(default)]
    pub disabled: Vec<String>,
}

/// Resolve config path from env or default.
pub fn default_config_path() -> PathBuf {
    std::env::var("CHAI_CONFIG_PATH").map(PathBuf::from).unwrap_or_else(|_| {
        dirs::home_dir()
            .map(|h| h.join(".chai").join("config.json"))
            .unwrap_or_else(|| PathBuf::from("config.json"))
    })
}

/// Resolve workspace directory for skills: config value or default ~/.chai/workspace.
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

/// Path to the bundled skills directory: `bundled` subdirectory of the config file's parent.
/// After `chai init`, default skills live here.
pub fn bundled_skills_dir(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join("bundled")
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
}
