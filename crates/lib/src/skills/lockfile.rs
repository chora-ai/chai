//! Skill lockfile: per-profile `skills.lock` mapping skill names to content hashes.
//!
//! The lockfile records exact content hashes for each skill, enabling reproducible
//! gateway restarts and integrity verification. Each update increments a generation
//! counter.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::versioning;
use super::SkillEntry;
use crate::config::SkillLockMode;

const LOCK_VERSION: u32 = 1;

/// On-disk lockfile format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsLock {
    /// Schema version (currently 1).
    pub version: u32,
    /// Skill directory name → pinned content hash.
    pub skills: BTreeMap<String, SkillPin>,
    /// Monotonic generation counter, incremented on each lock update.
    pub generation: u64,
}

/// A single skill's lock entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPin {
    /// Content hash of the pinned version.
    pub hash: String,
}

impl SkillsLock {
    /// Create a new lockfile from the current active versions of the given skills.
    pub fn from_entries(entries: &[SkillEntry]) -> Result<Self> {
        let mut skills = BTreeMap::new();
        for entry in entries {
            let hash = active_hash_for_entry(entry)?;
            skills.insert(entry.name.clone(), SkillPin { hash });
        }
        Ok(SkillsLock {
            version: LOCK_VERSION,
            skills,
            generation: 1,
        })
    }

    /// Update the lockfile with current active versions, incrementing the generation.
    pub fn update(&mut self, entries: &[SkillEntry]) -> Result<()> {
        let mut skills = BTreeMap::new();
        for entry in entries {
            let hash = active_hash_for_entry(entry)?;
            skills.insert(entry.name.clone(), SkillPin { hash });
        }
        self.skills = skills;
        self.generation += 1;
        Ok(())
    }
}

/// Resolve the lockfile path for a profile: `<profile_dir>/skills.lock`.
pub fn lock_path(profile_dir: &Path) -> PathBuf {
    profile_dir.join("skills.lock")
}

/// Read the lockfile for a profile. Returns None if the file does not exist.
pub fn read_lock(profile_dir: &Path) -> Result<Option<SkillsLock>> {
    let path = lock_path(profile_dir);
    if !path.exists() {
        return Ok(None);
    }
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let lock: SkillsLock =
        serde_json::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(lock))
}

/// Write the lockfile for a profile. Preserves the previous generation as `skills.lock.<gen>`.
pub fn write_lock(profile_dir: &Path, lock: &SkillsLock) -> Result<()> {
    let path = lock_path(profile_dir);

    // Preserve previous generation if the file exists
    if path.exists() {
        if let Ok(prev_content) = std::fs::read_to_string(&path) {
            if let Ok(prev_lock) = serde_json::from_str::<SkillsLock>(&prev_content) {
                let backup = profile_dir.join(format!("skills.lock.{}", prev_lock.generation));
                let _ = std::fs::copy(&path, &backup);
            }
        }
    }

    let content = serde_json::to_string_pretty(lock).context("serializing skills.lock")?;
    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Verify enabled skills against the lockfile at gateway startup.
///
/// For each enabled skill that has a lock entry, checks whether the `active` symlink
/// target directory name matches the locked hash. Returns Ok(()) on success (all
/// verified or no lock). Returns Err in strict mode if any mismatch is found.
pub fn verify_at_startup(
    all_entries: &[SkillEntry],
    enabled_names: &[&str],
    profile_dir: &Path,
    mode: SkillLockMode,
) -> Result<()> {
    let lock = match read_lock(profile_dir)? {
        Some(l) => l,
        None => {
            log::debug!("no skills.lock found, skipping verification");
            return Ok(());
        }
    };

    log::info!(
        "skills.lock: generation {}, {} skill(s) pinned",
        lock.generation,
        lock.skills.len(),
    );

    let mut mismatches = Vec::new();

    for name in enabled_names {
        let pin = match lock.skills.get(*name) {
            Some(p) => p,
            None => continue, // Unlocked skills load normally
        };

        // Find the entry to get its path
        let entry = match all_entries.iter().find(|e| &e.name == name) {
            Some(e) => e,
            None => {
                log::warn!(
                    "skills.lock pins '{}' to {} but skill not found on disk",
                    name,
                    pin.hash,
                );
                mismatches.push(name.to_string());
                continue;
            }
        };

        // The entry.path already points to the resolved active dir (`versions/<hash>/`).
        let active_hash = entry
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if active_hash != pin.hash {
            log::warn!(
                "skill '{}': active version {} does not match lock pin {}",
                name,
                active_hash,
                pin.hash,
            );
            mismatches.push(name.to_string());
        } else {
            log::debug!("skill '{}': version {} verified", name, pin.hash);
        }
    }

    if !mismatches.is_empty() && mode == SkillLockMode::Strict {
        anyhow::bail!(
            "skills.lock verification failed in strict mode: {} skill(s) mismatched ({})",
            mismatches.len(),
            mismatches.join(", "),
        );
    }

    Ok(())
}

/// Rollback to a previous generation: restore the generation's lockfile and update
/// `active` symlinks for all skills in the restored lock.
///
/// `skills_dir` is the shared `~/.chai/skills/` root.
pub fn rollback(profile_dir: &Path, generation: u64, skills_dir: &Path) -> Result<()> {
    let backup_path = profile_dir.join(format!("skills.lock.{}", generation));
    if !backup_path.exists() {
        anyhow::bail!(
            "generation {} not found (no {} file)",
            generation,
            backup_path.display()
        );
    }

    let content = std::fs::read_to_string(&backup_path)
        .with_context(|| format!("reading {}", backup_path.display()))?;
    let restored: SkillsLock = serde_json::from_str(&content)
        .with_context(|| format!("parsing {}", backup_path.display()))?;

    // Update active symlinks for each skill in the restored lock
    for (name, pin) in &restored.skills {
        let skill_root = skills_dir.join(name);
        let version_dir = skill_root.join("versions").join(&pin.hash);
        if !version_dir.exists() {
            log::warn!(
                "rollback: skill '{}' version {} not found on disk, skipping symlink update",
                name,
                pin.hash,
            );
            continue;
        }
        versioning::set_active_version(&skill_root, &pin.hash)?;
    }

    // Write the restored lock as the current lockfile (preserving the current one as a backup)
    write_lock(profile_dir, &restored)?;

    Ok(())
}

/// List available generations for a profile (from `skills.lock.*` backup files).
/// Returns generation numbers in ascending order.
pub fn list_generations(profile_dir: &Path) -> Result<Vec<u64>> {
    let mut generations = Vec::new();

    // Include the current lockfile's generation
    if let Some(current) = read_lock(profile_dir)? {
        generations.push(current.generation);
    }

    // Scan for backup files: skills.lock.<N>
    if let Ok(read_dir) = std::fs::read_dir(profile_dir) {
        for entry in read_dir.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(suffix) = name.strip_prefix("skills.lock.") {
                if let Ok(gen) = suffix.parse::<u64>() {
                    if !generations.contains(&gen) {
                        generations.push(gen);
                    }
                }
            }
        }
    }

    generations.sort();
    Ok(generations)
}

/// Get the active version hash from the entry path (`.../versions/<hash>/`).
fn active_hash_for_entry(entry: &SkillEntry) -> Result<String> {
    let parent_is_versions = entry
        .path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        == Some("versions");
    if !parent_is_versions {
        anyhow::bail!(
            "skill '{}' path is not under versions/<hash>/: {}",
            entry.name,
            entry.path.display()
        );
    }
    Ok(entry
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn round_trip_lockfile() {
        let dir = tempdir("lock_roundtrip");
        let lock = SkillsLock {
            version: LOCK_VERSION,
            skills: BTreeMap::from([
                (
                    "git".to_string(),
                    SkillPin {
                        hash: "abc123def456".to_string(),
                    },
                ),
                (
                    "devtools".to_string(),
                    SkillPin {
                        hash: "789012345678".to_string(),
                    },
                ),
            ]),
            generation: 3,
        };
        write_lock(&dir, &lock).unwrap();

        let loaded = read_lock(&dir).unwrap().unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.generation, 3);
        assert_eq!(loaded.skills.len(), 2);
        assert_eq!(loaded.skills["git"].hash, "abc123def456");
    }

    #[test]
    fn write_lock_preserves_previous_generation() {
        let dir = tempdir("lock_preserve");
        let lock1 = SkillsLock {
            version: LOCK_VERSION,
            skills: BTreeMap::from([(
                "git".to_string(),
                SkillPin {
                    hash: "aaa".to_string(),
                },
            )]),
            generation: 1,
        };
        write_lock(&dir, &lock1).unwrap();

        let mut lock2 = lock1.clone();
        lock2.generation = 2;
        lock2.skills.insert(
            "devtools".to_string(),
            SkillPin {
                hash: "bbb".to_string(),
            },
        );
        write_lock(&dir, &lock2).unwrap();

        // Previous generation should be preserved
        assert!(dir.join("skills.lock.1").exists());
        let prev: SkillsLock =
            serde_json::from_str(&fs::read_to_string(dir.join("skills.lock.1")).unwrap()).unwrap();
        assert_eq!(prev.generation, 1);
        assert_eq!(prev.skills.len(), 1);
    }

    #[test]
    fn read_lock_returns_none_when_missing() {
        let dir = tempdir("lock_missing");
        assert!(read_lock(&dir).unwrap().is_none());
    }

    #[test]
    fn verify_passes_when_no_lockfile() {
        let dir = tempdir("verify_no_lock");
        let result = verify_at_startup(&[], &[], &dir, SkillLockMode::Strict);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_passes_when_hashes_match() {
        let dir = tempdir("verify_match");

        // Create a versioned skill entry path that looks like versions/abc123
        let skill_path = PathBuf::from("/tmp/chai_test_skills/git/versions/abc123def456");
        let entries = vec![SkillEntry {
            name: "git".to_string(),
            description: String::new(),
            path: skill_path,
            content: String::new(),
            tool_descriptor: None,
            capability_tier: None,
            model_variant_of: None,
        }];

        let lock = SkillsLock {
            version: LOCK_VERSION,
            skills: BTreeMap::from([(
                "git".to_string(),
                SkillPin {
                    hash: "abc123def456".to_string(),
                },
            )]),
            generation: 1,
        };
        write_lock(&dir, &lock).unwrap();

        let result = verify_at_startup(&entries, &["git"], &dir, SkillLockMode::Strict);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_fails_strict_on_mismatch() {
        let dir = tempdir("verify_mismatch");

        let skill_path = PathBuf::from("/tmp/chai_test_skills/git/versions/different_hash");
        let entries = vec![SkillEntry {
            name: "git".to_string(),
            description: String::new(),
            path: skill_path,
            content: String::new(),
            tool_descriptor: None,
            capability_tier: None,
            model_variant_of: None,
        }];

        let lock = SkillsLock {
            version: LOCK_VERSION,
            skills: BTreeMap::from([(
                "git".to_string(),
                SkillPin {
                    hash: "abc123def456".to_string(),
                },
            )]),
            generation: 1,
        };
        write_lock(&dir, &lock).unwrap();

        let result = verify_at_startup(&entries, &["git"], &dir, SkillLockMode::Strict);
        assert!(result.is_err());
    }

    #[test]
    fn verify_warns_on_mismatch_in_warn_mode() {
        let dir = tempdir("verify_warn");

        let skill_path = PathBuf::from("/tmp/chai_test_skills/git/versions/different_hash");
        let entries = vec![SkillEntry {
            name: "git".to_string(),
            description: String::new(),
            path: skill_path,
            content: String::new(),
            tool_descriptor: None,
            capability_tier: None,
            model_variant_of: None,
        }];

        let lock = SkillsLock {
            version: LOCK_VERSION,
            skills: BTreeMap::from([(
                "git".to_string(),
                SkillPin {
                    hash: "abc123def456".to_string(),
                },
            )]),
            generation: 1,
        };
        write_lock(&dir, &lock).unwrap();

        // Warn mode should succeed even with mismatch
        let result = verify_at_startup(&entries, &["git"], &dir, SkillLockMode::Warn);
        assert!(result.is_ok());
    }

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("chai_test_lockfile").join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
