//! Initialize `~/.chai`: profiles (`assistant`, `developer`), `active` symlink, shared `skills/`, per-profile config, `sandbox/` (seeded from templates), `agents/<orchestratorId>/AGENT.md`, and `skills.lock` for newly seeded profiles.
//! Bundled defaults mirror **`~/.chai/profiles/<name>/`**: **`bundled/profiles/<name>/agents/orchestrator/AGENT.md`**, **`bundled/profiles/<name>/sandbox/`**.

use anyhow::{Context, Result};
use include_dir::{include_dir, Dir, DirEntry};
use std::path::{Path, PathBuf};

use crate::config;
use crate::profile;
use crate::skills::versioning;

static BUNDLED_SKILLS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/bundled/skills");

fn bundled_default_agents_md(profile_name: &str) -> Result<&'static str> {
    match profile_name {
        "assistant" => Ok(include_str!(
            "../bundled/profiles/assistant/agents/orchestrator/AGENT.md"
        )),
        "developer" => Ok(include_str!(
            "../bundled/profiles/developer/agents/orchestrator/AGENT.md"
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
/// `bundled/profiles/<name>/sandbox/`.
fn bundled_sandbox_templates(profile_name: &str) -> Result<Vec<(&'static str, &'static [u8])>> {
    match profile_name {
        "assistant" => Ok(vec![
            (
                "AGENTS.md",
                include_bytes!("../bundled/profiles/assistant/sandbox/AGENTS.md"),
            ),
        ]),
        "developer" => Ok(vec![
            (
                "AGENTS.md",
                include_bytes!("../bundled/profiles/developer/sandbox/AGENTS.md"),
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

fn seed_profile(profile_dir: &Path, profile_name: &str) -> Result<bool> {
    // If the profile directory already exists, skip seeding entirely.  Users
    // may rename the orchestrator agent (and its directory) within an existing
    // profile — re-running `chai init` must not overwrite those changes.
    if profile_dir.is_dir() {
        log::debug!(
            "profile directory {} already exists; skipping seed",
            profile_dir.display()
        );
        return Ok(false);
    }

    std::fs::create_dir_all(profile_dir)
        .with_context(|| format!("creating profile directory {}", profile_dir.display()))?;

    let config_path = profile_dir.join("config.json");
    std::fs::write(&config_path, b"{}")
        .with_context(|| format!("writing default config to {}", config_path.display()))?;
    log::info!("created default config at {}", config_path.display());

    let default_agents_md = bundled_default_agents_md(profile_name)?;
    let orchestrator_dir = profile_dir.join("agents").join("orchestrator");
    std::fs::create_dir_all(&orchestrator_dir).with_context(|| {
        format!(
            "creating orchestrator agent directory {}",
            orchestrator_dir.display()
        )
    })?;
    let orchestrator_agents = orchestrator_dir.join("AGENT.md");
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

    seed_sandbox(profile_dir, profile_name)?;

    Ok(true)
}

/// Re-create a missing sandbox directory for an existing profile and seed
/// template files. Returns `true` if the sandbox was re-created, `false` if
/// it already exists.
///
/// Called for every known default profile during `chai init` so that a
/// deleted sandbox directory can be recovered without deleting the entire
/// profile.
fn recover_sandbox(profile_dir: &Path, profile_name: &str) -> Result<bool> {
    let sandbox_dir = profile_dir.join("sandbox");
    if sandbox_dir.is_dir() {
        return Ok(false);
    }
    log::info!(
        "sandbox directory missing for existing profile at {}; re-creating",
        sandbox_dir.display()
    );
    seed_sandbox(profile_dir, profile_name)?;
    Ok(true)
}

/// Create the sandbox directory under a profile and seed template files.
/// Existing files within the sandbox are never overwritten.
fn seed_sandbox(profile_dir: &Path, profile_name: &str) -> Result<()> {
    let sandbox_dir = profile_dir.join("sandbox");
    std::fs::create_dir_all(&sandbox_dir).with_context(|| {
        format!("creating sandbox directory {}", sandbox_dir.display())
    })?;
    log::info!("created sandbox directory at {}", sandbox_dir.display());

    // Seed sandbox template files from the profile config.
    for (rel_path, contents) in bundled_sandbox_templates(profile_name)? {
        let dest = sandbox_dir.join(rel_path);
        if dest.exists() {
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, contents)
            .with_context(|| format!("writing sandbox template {}", dest.display()))?;
        log::info!("seeded sandbox template at {}", dest.display());
    }

    Ok(())
}

/// Create `~/.chai` layout: `profiles/assistant`, `profiles/developer`, `active` → assistant (on first init only), shared `skills/`, and `skills.lock` for newly seeded profiles.
///
/// When a profile directory does not yet exist and is seeded by this function, a `skills.lock` is
/// generated for it. This ensures the defensive-by-default `skills.lockMode=strict` takes effect
/// immediately after `chai init`, without requiring a separate `chai skill lock` step.
///
/// When re-run on an already-initialized directory:
/// - Existing profile directories (`assistant`, `developer`) are **not** re-seeded — the user may have renamed agent directories or customized files within the profile.
/// - Existing `skills.lock` files are **not** overwritten — only newly seeded profiles receive a lock.
/// - The `active` symlink is **not** overwritten — any profile the user has switched to is preserved.
///   Only a missing or broken `active` symlink triggers the default (`assistant`).
pub fn init_chai_home() -> Result<PathBuf> {
    let chai_home = profile::chai_home()?;
    std::fs::create_dir_all(&chai_home)
        .with_context(|| format!("creating {}", chai_home.display()))?;

    let profiles_base = chai_home.join("profiles");
    std::fs::create_dir_all(&profiles_base)
        .with_context(|| format!("creating {}", profiles_base.display()))?;

    let mut newly_seeded = Vec::new();
    for name in ["assistant", "developer"] {
        let profile_dir = profiles_base.join(name);
        if seed_profile(&profile_dir, name)? {
            newly_seeded.push(name);
        } else {
            // Profile directory already exists — recover sandbox if it was
            // deleted. This ensures `chai init` can fix a missing sandbox
            // without requiring the entire profile to be re-created.
            recover_sandbox(&profile_dir, name)?;
        }
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

    // Generate skills.lock for newly seeded profiles so the gateway can start.
    // Strict mode (the default) refuses to start without a lockfile, so `chai init`
    // must create one. Only profiles that were just created need a lock; existing
    // profiles retain whatever lock state they already have.
    if !newly_seeded.is_empty() {
        let all_entries = crate::skills::load_skills(&skills_dir)?;
        if !all_entries.is_empty() {
            for name in &newly_seeded {
                let profile_dir = profiles_base.join(name);
                let lock = crate::skills::lockfile::SkillsLock::from_entries(&all_entries)?;
                crate::skills::lockfile::write_lock(&profile_dir, &lock)?;
                log::info!(
                    "generated skills.lock for profile '{}' ({} skill(s), generation {})",
                    name,
                    lock.skills.len(),
                    lock.generation,
                );
            }
        }
    }

    // Only set the active profile symlink on first initialization. If the
    // symlink already exists and resolves to a valid profile directory, leave
    // it unchanged — the user may have switched profiles via `chai profile
    // switch` and re-running `chai init` should not reset that choice. This
    // mirrors the skills versioning logic above where the `active` symlink is
    // only set when no active version exists.
    let active_link = chai_home.join("active");
    let already_initialized = active_link.symlink_metadata().is_ok()
        && profile::read_persistent_profile_dir(&chai_home).is_ok();
    if already_initialized {
        let current = profile::read_persistent_profile_name(&chai_home)?;
        log::debug!(
            "active profile already set to {:?}; skipping symlink update",
            current,
        );
    } else {
        profile::switch_active_profile(&chai_home, "assistant")?;
    }

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
