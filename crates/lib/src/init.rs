//! Initialize `~/.chai`: profiles (`assistant`, `developer`), `active` symlink, shared `skills/`, per-profile config and `agents/<orchestratorId>/AGENTS.md`.
//! Bundled defaults mirror **`~/.chai/profiles/<name>/`**: **`config/profiles/<name>/agents/orchestrator/AGENTS.md`**.

use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use std::path::{Path, PathBuf};

use crate::config;
use crate::profile;

static BUNDLED_SKILLS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/config/skills");

fn bundled_default_agents_md(profile_name: &str) -> Result<&'static str> {
    match profile_name {
        "assistant" => Ok(include_str!(
            "../config/profiles/assistant/agents/orchestrator/AGENTS.md"
        )),
        "developer" => Ok(include_str!(
            "../config/profiles/developer/agents/orchestrator/AGENTS.md"
        )),
        _ => anyhow::bail!("no bundled AGENTS.md template for profile {:?}", profile_name),
    }
}

/// Ensure the active profile has config and shared skills exist.
pub fn require_initialized(paths: &profile::ChaiPaths) -> Result<()> {
    if !paths.config_path.exists() {
        anyhow::bail!(
            "configuration not initialized; run `chai init` first (config file not found: {})",
            paths.config_path.display()
        );
    }
    let skills_dir = config::default_skills_dir(&paths.chai_home);
    if !skills_dir.exists() {
        anyhow::bail!(
            "configuration not initialized; run `chai init` first (skills directory not found: {})",
            skills_dir.display()
        );
    }
    Ok(())
}

fn seed_profile(profile_dir: &Path, profile_name: &str) -> Result<()> {
    std::fs::create_dir_all(profile_dir)
        .with_context(|| format!("creating profile directory {}", profile_dir.display()))?;

    let config_path = profile_dir.join("config.json");
    if !config_path.exists() {
        std::fs::write(&config_path, b"{}")
            .with_context(|| format!("writing default config to {}", config_path.display()))?;
        log::info!("created default config at {}", config_path.display());
    }

    let default_agents_md = bundled_default_agents_md(profile_name)?;
    let orchestrator_dir = profile_dir.join("agents").join("orchestrator");
    std::fs::create_dir_all(&orchestrator_dir).with_context(|| {
        format!(
            "creating orchestrator agent directory {}",
            orchestrator_dir.display()
        )
    })?;
    let orchestrator_agents = orchestrator_dir.join("AGENTS.md");
    if !orchestrator_agents.exists() {
        std::fs::write(&orchestrator_agents, default_agents_md).with_context(|| {
            format!(
                "writing default AGENTS.md to {}",
                orchestrator_agents.display()
            )
        })?;
        log::info!(
            "wrote default AGENTS.md to {}",
            orchestrator_agents.display()
        );
    }

    Ok(())
}

/// Create `~/.chai` layout: `profiles/assistant`, `profiles/developer`, `active` → assistant, shared `skills/`.
pub fn init_chai_home() -> Result<PathBuf> {
    let chai_home = profile::chai_home()?;
    std::fs::create_dir_all(&chai_home)
        .with_context(|| format!("creating {}", chai_home.display()))?;

    let profiles_base = chai_home.join("profiles");
    std::fs::create_dir_all(&profiles_base)
        .with_context(|| format!("creating {}", profiles_base.display()))?;

    for name in ["assistant", "developer"] {
        seed_profile(&profiles_base.join(name), name)?;
    }

    let skills_dir = chai_home.join("skills");
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
        log::debug!(
            "skills directory already exists at {}, skipping",
            skills_dir.display()
        );
    }

    profile::switch_active_profile(&chai_home, "assistant")?;

    Ok(chai_home)
}
