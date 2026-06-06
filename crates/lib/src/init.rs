//! Initialize `~/.chai`: profiles (`assistant`, `developer`), `active` symlink, shared `skills/`, per-profile config, `sandbox/` (seeded from templates), and `agents/<orchestratorId>/AGENT.md`.
//! Bundled defaults mirror **`~/.chai/profiles/<name>/`**: **`config/profiles/<name>/agents/orchestrator/AGENT.md`**, **`config/profiles/<name>/sandbox/`**.

use anyhow::{Context, Result};
use include_dir::{include_dir, Dir, DirEntry};
use std::path::{Path, PathBuf};

use crate::config;
use crate::profile;
use crate::skills::versioning;

static BUNDLED_SKILLS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/config/skills");

fn bundled_default_agents_md(profile_name: &str) -> Result<&'static str> {
    match profile_name {
        "assistant" => Ok(include_str!(
            "../config/profiles/assistant/agents/orchestrator/AGENT.md"
        )),
        "developer" => Ok(include_str!(
            "../config/profiles/developer/agents/orchestrator/AGENT.md"
        )),
        _ => anyhow::bail!(
            "no bundled AGENT.md template for profile {:?}",
            profile_name
        ),
    }
}

/// Bundled sandbox template files for a profile.
///
/// Returns `[(relative_path, contents)]` for each file in
/// `config/profiles/<name>/sandbox/`.
fn bundled_sandbox_templates(profile_name: &str) -> Result<Vec<(&'static str, &'static [u8])>> {
    match profile_name {
        "assistant" => Ok(vec![
            (
                "AGENTS.md",
                include_bytes!("../config/profiles/assistant/sandbox/AGENTS.md"),
            ),
            (
                "README.md",
                include_bytes!("../config/profiles/assistant/sandbox/README.md"),
            ),
        ]),
        "developer" => Ok(vec![
            (
                "AGENTS.md",
                include_bytes!("../config/profiles/developer/sandbox/AGENTS.md"),
            ),
            (
                "README.md",
                include_bytes!("../config/profiles/developer/sandbox/README.md"),
            ),
        ]),
        _ => anyhow::bail!(
            "no bundled sandbox templates for profile {:?}",
            profile_name
        ),
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
    let orchestrator_agents = orchestrator_dir.join("AGENT.md");
    if !orchestrator_agents.exists() {
        std::fs::write(&orchestrator_agents, default_agents_md).with_context(|| {
            format!(
                "writing default AGENT.md to {}",
                orchestrator_agents.display()
            )
        })?;
        log::info!(
            "wrote default AGENT.md to {}",
            orchestrator_agents.display()
        );
    }

    let sandbox_dir = profile_dir.join("sandbox");
    if !sandbox_dir.exists() {
        std::fs::create_dir_all(&sandbox_dir).with_context(|| {
            format!("creating sandbox directory {}", sandbox_dir.display())
        })?;
        log::info!("created sandbox directory at {}", sandbox_dir.display());
    }

    // Seed sandbox template files from the profile config if they do not
    // already exist.  This mirrors the AGENT.md seeding pattern above.
    for (rel_path, contents) in bundled_sandbox_templates(profile_name)? {
        let dest = sandbox_dir.join(rel_path);
        if !dest.exists() {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&dest, contents)
                .with_context(|| format!("writing sandbox template {}", dest.display()))?;
            log::info!("seeded sandbox template at {}", dest.display());
        }
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
    std::fs::create_dir_all(&skills_dir)
        .with_context(|| format!("creating skills directory {}", skills_dir.display()))?;
    // Sync bundled skills so that updates to bundled skill content (e.g. new tool
    // annotations, description changes) are available after `chai init`. The
    // extraction is hash-based and idempotent: existing version snapshots are never
    // re-written, and the active symlink is only set for fresh installations (no
    // existing active version). Re-running `chai init` will create the new bundled
    // version snapshot but will not change the active symlink, preserving any user
    // customizations (rollbacks, edits via skills_write_skill_md).
    extract_bundled_skills_versioned(&skills_dir)?;

    profile::switch_active_profile(&chai_home, "assistant")?;

    Ok(chai_home)
}

/// Extract bundled skills into content-addressed versioned layout.
///
/// For each skill directory in BUNDLED_SKILLS:
/// 1. Collect all files as (relative_path, contents) pairs
/// 2. Compute the content hash
/// 3. Write files into `skills/<name>/versions/<hash>/`
/// 4. Create `active` symlink pointing to the version
fn extract_bundled_skills_versioned(skills_dir: &Path) -> Result<()> {
    for skill_dir in BUNDLED_SKILLS.dirs() {
        // Only process top-level skill directories (not nested subdirs like scripts/)
        let skill_name = skill_dir.path().to_str().unwrap_or("unknown");
        if skill_name.contains('/') || skill_name.contains('\\') {
            continue;
        }

        // Collect all files in this skill as (relative_path, contents) pairs
        let mut file_entries = Vec::new();
        collect_bundled_files(skill_dir, skill_dir.path(), &mut file_entries);

        if file_entries.is_empty() {
            continue;
        }

        // Compute content hash from the bundled content
        let hash_entries: Vec<(&str, &[u8])> = file_entries
            .iter()
            .map(|(path, contents)| (path.as_str(), contents.as_slice()))
            .collect();
        let hash = versioning::compute_hash_from_entries(&hash_entries);

        let skill_root = skills_dir.join(skill_name);

        // Resolve the currently active version (if any). Used to decide whether to
        // update the active symlink and for log output.
        let active_hash = versioning::resolve_active_dir(&skill_root)
            .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()));
        let already_active = active_hash.as_deref() == Some(hash.as_str());

        // Write the version snapshot only when it does not already exist.  Snapshots
        // are immutable (same hash = same content) so re-writing is never needed.
        let snapshot_dir = skill_root.join("versions").join(&hash);
        if !snapshot_dir.exists() {
            std::fs::create_dir_all(&snapshot_dir)
                .with_context(|| format!("creating version dir {}", snapshot_dir.display()))?;

            for (rel_path, contents) in &file_entries {
                let dest = snapshot_dir.join(rel_path);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&dest, contents)
                    .with_context(|| format!("writing {}", dest.display()))?;

                // Set executable permission for scripts
                #[cfg(unix)]
                if rel_path.starts_with("scripts/") {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = std::fs::metadata(&dest)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&dest, perms)?;
                }
            }
        }

        // Only set the active symlink when one does not already exist. This preserves
        // user customizations (manual rollbacks, edits via skills_write_skill_md) while
        // still seeding the bundled version for fresh installations. The bundled version
        // snapshot is always created above, so the user can switch to it manually if
        // desired.
        if active_hash.is_none() {
            versioning::set_active_version(&skill_root, &hash)?;
            log::info!(
                "skill '{}' → bundled version {} (new)",
                skill_name,
                hash,
            );
        } else if already_active {
            log::debug!("skill '{}' already at bundled version {}", skill_name, hash);
        } else {
            log::debug!(
                "skill '{}' keeping active version {} (bundled: {})",
                skill_name,
                active_hash.as_deref().unwrap_or("none"),
                hash,
            );
        }
    }
    Ok(())
}

/// Recursively collect files from a bundled Dir as (relative_path, contents) pairs.
fn collect_bundled_files(dir: &Dir<'_>, root_path: &Path, out: &mut Vec<(String, Vec<u8>)>) {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(d) => {
                collect_bundled_files(d, root_path, out);
            }
            DirEntry::File(f) => {
                let rel = f
                    .path()
                    .strip_prefix(root_path)
                    .unwrap_or(f.path())
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, f.contents().to_vec()));
            }
        }
    }
}
