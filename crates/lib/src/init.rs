//! Initialize the configuration directory: create ~/.chai, default config, workspace, and default skills.

use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use std::path::{Path, PathBuf};

static DEFAULT_SKILLS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/skills");

/// Create the config directory and default files if they do not exist.
/// - Creates the config directory (parent of config file path).
/// - Writes `config.json` with `{}` if missing.
/// - Creates the `workspace` subdirectory.
/// - Copies default (bundled) skills into `skills` subdirectory if it does not exist.
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

    let skills_dir = config_dir.join("skills");
    if !skills_dir.exists() {
        std::fs::create_dir_all(&skills_dir)
            .with_context(|| format!("creating skills directory {}", skills_dir.display()))?;
        if let Err(e) = DEFAULT_SKILLS.extract(&skills_dir) {
            anyhow::bail!(
                "extracting default skills to {}: {}",
                skills_dir.display(),
                e
            );
        }
        log::info!("extracted default skills to {}", skills_dir.display());
    } else {
        log::debug!("skills directory already exists at {}, skipping", skills_dir.display());
    }

    Ok(config_dir.to_path_buf())
}
