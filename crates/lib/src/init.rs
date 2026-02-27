//! Initialize the configuration directory: create ~/.chai, default config, workspace, and bundled skills.
//!
//! Layout mirrors `crates/lib/config/`: `config/skills/` → `~/.chai/skills/`, `config/workspace/AGENTS.md` → `~/.chai/workspace/AGENTS.md`.

use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use std::path::{Path, PathBuf};

use crate::config;

static BUNDLED_SKILLS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/config/skills");
static DEFAULT_AGENT_CTX: &str = include_str!("../config/workspace/AGENTS.md");

/// Ensure the configuration directory has been initialized (config file and skills directory exist).
/// Uses the primary skill root from config (or default) and checks that it exists.
pub fn require_initialized(config_path: &Path, config: &config::Config) -> Result<()> {
    if !config_path.exists() {
        anyhow::bail!(
            "configuration not initialized; run `chai init` first (config file not found: {})",
            config_path.display()
        );
    }
    let skills_dir = config::resolve_skills_dir(config, config_path);
    if !skills_dir.exists() {
        anyhow::bail!(
            "configuration not initialized; run `chai init` first (skills directory not found: {})",
            skills_dir.display()
        );
    }
    Ok(())
}

/// Create the config directory and default files if they do not exist.
/// - Creates the config directory (parent of config file path).
/// - Writes `config.json` with `{}` if missing.
/// - Creates the `workspace` subdirectory and seeds `AGENTS.md` from the default template if missing.
/// - Extracts bundled skills from the template into `skills` subdirectory if it does not exist.
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

    let skills_dir = config_dir.join("skills");
    if !skills_dir.exists() {
        std::fs::create_dir_all(&skills_dir)
            .with_context(|| format!("creating skills directory {}", skills_dir.display()))?;
        if let Err(e) = BUNDLED_SKILLS.extract(&skills_dir) {
            anyhow::bail!(
                "extracting bundled skills to {}: {}",
                skills_dir.display(),
                e
            );
        }
        log::info!("extracted bundled skills to {}", skills_dir.display());
    } else {
        log::debug!("skills directory already exists at {}, skipping", skills_dir.display());
    }

    Ok(config_dir.to_path_buf())
}
