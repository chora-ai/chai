//! Initialize the configuration directory: create ~/.chai, default config, workspace, and bundled skills.
//!
//! Layout mirrors `crates/lib/config/`: `config/bundled/` → `~/.chai/bundled/`, `config/workspace/AGENTS.md` → `~/.chai/workspace/AGENTS.md`.

use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use std::path::{Path, PathBuf};

use crate::config;

static DEFAULT_BUNDLED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/config/bundled");
static DEFAULT_AGENT_CTX: &str = include_str!("../config/workspace/AGENTS.md");

/// Ensure the configuration directory has been initialized (config file and bundled skills exist).
/// Call before starting the gateway so bundled skills and layout are present.
pub fn require_initialized(config_path: &Path) -> Result<()> {
    if !config_path.exists() {
        anyhow::bail!(
            "configuration not initialized; run `chai init` first (config file not found: {})",
            config_path.display()
        );
    }
    let bundled_dir = config::bundled_skills_dir(config_path);
    if !bundled_dir.exists() {
        anyhow::bail!(
            "configuration not initialized; run `chai init` first (bundled skills directory not found: {})",
            bundled_dir.display()
        );
    }
    Ok(())
}

/// Create the config directory and default files if they do not exist.
/// - Creates the config directory (parent of config file path).
/// - Writes `config.json` with `{}` if missing.
/// - Creates the `workspace` subdirectory and seeds `AGENTS.md` from the default template if missing.
/// - Extracts default (bundled) skills from the template into `bundled` subdirectory if it does not exist.
pub fn init_config_dir(config_path: &Path) -> Result<PathBuf> {
    let config_dir = config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(config_dir)
        .with_context(|| format!("creating config directory {}", config_dir.display()))?;

    if !config_path.exists() {
        let default_config = b"{}";
        std::fs::write(config_path, default_config)
            .with_context(|| format!("writing default config to {}", config_path.display()))?;
        log::info!("created default config at {}", config_path.display());
    }

    let workspace = config_dir.join("workspace");
    if !workspace.exists() {
        std::fs::create_dir_all(&workspace)
            .with_context(|| format!("creating workspace directory {}", workspace.display()))?;
        log::info!("created workspace directory at {}", workspace.display());
    }
    // Seed a default AGENTS.md in the workspace if one does not exist yet.
    let workspace_agents = workspace.join("AGENTS.md");
    if !workspace_agents.exists() {
        std::fs::write(&workspace_agents, DEFAULT_AGENT_CTX)
            .with_context(|| format!("writing default AGENTS.md to {}", workspace_agents.display()))?;
        log::info!("wrote default AGENTS.md to {}", workspace_agents.display());
    }

    let bundled_dir = config_dir.join("bundled");
    if !bundled_dir.exists() {
        std::fs::create_dir_all(&bundled_dir)
            .with_context(|| format!("creating bundled skills directory {}", bundled_dir.display()))?;
        if let Err(e) = DEFAULT_BUNDLED.extract(&bundled_dir) {
            anyhow::bail!(
                "extracting default skills to {}: {}",
                bundled_dir.display(),
                e
            );
        }
        log::info!("extracted default skills to {}", bundled_dir.display());
    } else {
        log::debug!("bundled skills directory already exists at {}, skipping", bundled_dir.display());
    }

    Ok(config_dir.to_path_buf())
}
