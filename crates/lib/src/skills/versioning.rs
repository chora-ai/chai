//! Content-addressed skill package versioning.
//!
//! Each skill package can store immutable versioned snapshots under `versions/<hash>/`,
//! with an `active` symlink selecting the current version. The hash is a truncated SHA-256
//! of the canonical skill content (sorted file paths + raw bytes).

use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Number of hex characters to use for version directory names.
/// 12 hex chars = 48 bits → collision probability < 1e-7 at 10,000 versions per skill.
const HASH_TRUNCATION: usize = 12;

/// Compute a content hash for a skill directory (or versioned snapshot directory).
///
/// Canonical form: sorted relative file paths, each entry as `<relative-path>\0<file-bytes>`,
/// concatenated and hashed with SHA-256. Only regular files are included (symlinks and
/// directories are structural, not content). The `versions/` and `active` entries within
/// the skill root are excluded so the hash reflects the skill content, not the versioning
/// metadata.
pub fn compute_content_hash(dir: &Path) -> Result<String> {
    let mut entries = collect_files(dir, dir)?;
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (rel_path, contents) in &entries {
        hasher.update(rel_path.as_bytes());
        hasher.update(b"\0");
        hasher.update(contents);
    }
    let hash = hasher.finalize();
    Ok(hex_encode(&hash)[..HASH_TRUNCATION].to_string())
}

/// Collect all regular files under `dir` as (relative_path, contents) pairs.
/// Skips the `versions/` directory and `active` symlink at the top level.
fn collect_files(dir: &Path, root: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    let read_dir =
        std::fs::read_dir(dir).with_context(|| format!("reading directory {}", dir.display()))?;
    for entry in read_dir {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        // Skip versioning metadata at the skill root level
        if dir == root && (file_name_str == "versions" || file_name_str == "active") {
            continue;
        }

        let ft = entry.file_type()?;
        if ft.is_dir() {
            out.extend(collect_files(&path, root)?);
        } else if ft.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let contents =
                std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            out.push((rel, contents));
        }
        // Symlinks are skipped (structural, not content)
    }
    Ok(out)
}

/// Resolve the active skill content directory for a given skill package root.
///
/// Returns `Some` only when `skill_root/active` is a symlink to a directory that contains
/// `SKILL.md` (typically `versions/<hash>/`). Otherwise `None`.
pub fn resolve_active_dir(skill_root: &Path) -> Option<PathBuf> {
    let active = skill_root.join("active");
    if !active.is_symlink() {
        return None;
    }
    let target = std::fs::read_link(&active).ok()?;
    let resolved = if target.is_relative() {
        skill_root.join(&target)
    } else {
        target
    };
    if resolved.join("SKILL.md").is_file() {
        Some(resolved)
    } else {
        None
    }
}

/// Create a new content-addressed version snapshot from the current skill content.
///
/// Copies all skill files (SKILL.md, tools.json, scripts/) into `versions/<hash>/`
/// and updates the `active` symlink. Returns the hash of the new version.
///
/// If a version with the same hash already exists, the copy is skipped but the
/// `active` symlink is still updated (idempotent).
pub fn create_version_snapshot(skill_root: &Path) -> Result<String> {
    let hash = compute_content_hash(skill_root)?;
    let versions_dir = skill_root.join("versions");
    let snapshot_dir = versions_dir.join(&hash);

    if !snapshot_dir.exists() {
        std::fs::create_dir_all(&snapshot_dir)
            .with_context(|| format!("creating version directory {}", snapshot_dir.display()))?;
        copy_skill_content(skill_root, &snapshot_dir)?;
    }

    update_active_symlink(skill_root, &hash)?;
    Ok(hash)
}

/// Create a versioned layout from extracted content already in a snapshot directory.
///
/// Used by `chai init` where bundled skills are extracted directly into `versions/<hash>/`.
/// Only creates the `active` symlink pointing to the given hash.
pub fn set_active_version(skill_root: &Path, hash: &str) -> Result<()> {
    std::fs::create_dir_all(skill_root)
        .with_context(|| format!("creating skill root {}", skill_root.display()))?;
    update_active_symlink(skill_root, hash)
}

/// Copy skill content files (everything except `versions/` and `active`) from src to dst.
fn copy_skill_content(src: &Path, dst: &Path) -> Result<()> {
    copy_dir_contents(src, dst, src)
}

fn copy_dir_contents(src_dir: &Path, dst_dir: &Path, root: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip versioning metadata at root level
        if src_dir == root && (name_str == "versions" || name_str == "active") {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst_dir.join(&name);
        let ft = entry.file_type()?;

        if ft.is_dir() {
            std::fs::create_dir_all(&dst_path)?;
            copy_dir_contents(&src_path, &dst_path, root)?;
        } else if ft.is_file() {
            std::fs::copy(&src_path, &dst_path)?;
            // Preserve executable permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let src_perms = std::fs::metadata(&src_path)?.permissions();
                let mut dst_perms = std::fs::metadata(&dst_path)?.permissions();
                dst_perms.set_mode(src_perms.mode());
                std::fs::set_permissions(&dst_path, dst_perms)?;
            }
        }
    }
    Ok(())
}

/// Atomically update the `active` symlink to point to `versions/<hash>`.
fn update_active_symlink(skill_root: &Path, hash: &str) -> Result<()> {
    let active = skill_root.join("active");
    let target = PathBuf::from("versions").join(hash);

    // Atomic symlink update: create a temp symlink then rename over the old one.
    // This avoids a window where `active` doesn't exist.
    let tmp = skill_root.join(".active_tmp");
    let _ = std::fs::remove_file(&tmp);

    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &tmp)
        .with_context(|| format!("creating symlink {} -> {}", tmp.display(), target.display()))?;

    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&target, &tmp)
        .with_context(|| format!("creating symlink {} -> {}", tmp.display(), target.display()))?;

    std::fs::rename(&tmp, &active)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), active.display()))?;

    Ok(())
}

/// Create a new version from the current active version with one file replaced.
///
/// Used by CLI write commands: copies the current active version to a staging dir,
/// applies the file update, computes the hash, and moves to `versions/<hash>/`.
/// Returns the hash of the new version.
///
/// `rel_path` is the relative path within the skill (e.g. "SKILL.md", "tools.json",
/// "scripts/parse.sh"). Parent directories are created as needed.
pub fn write_and_snapshot(skill_root: &Path, rel_path: &str, new_content: &[u8]) -> Result<String> {
    let staging = skill_root.join(".staging_tmp");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;

    // Copy current active version to staging
    let active_dir = resolve_active_dir(skill_root).ok_or_else(|| {
        anyhow!(
            "skill at {} has no valid `active` symlink to a version directory containing SKILL.md",
            skill_root.display()
        )
    })?;
    copy_skill_content(&active_dir, &staging)?;

    // Apply the update
    let dest = staging.join(rel_path);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&dest, new_content)?;

    // Set executable permission for scripts
    #[cfg(unix)]
    if rel_path.starts_with("scripts/") {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms)?;
    }

    // Compute hash and create version
    let hash = compute_content_hash(&staging)?;
    let versions_dir = skill_root.join("versions");
    let snapshot_dir = versions_dir.join(&hash);

    if !snapshot_dir.exists() {
        std::fs::create_dir_all(&versions_dir)?;
        std::fs::rename(&staging, &snapshot_dir)
            .with_context(|| format!("moving staging to {}", snapshot_dir.display()))?;
    } else {
        let _ = std::fs::remove_dir_all(&staging);
    }

    update_active_symlink(skill_root, &hash)?;
    Ok(hash)
}

/// Compute a content hash for files provided as (relative_path, contents) pairs.
/// Used by `chai init` to hash bundled skill content without writing to disk first.
pub fn compute_hash_from_entries(entries: &[(&str, &[u8])]) -> String {
    let mut sorted: Vec<_> = entries.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (rel_path, contents) in &sorted {
        hasher.update(rel_path.as_bytes());
        hasher.update(b"\0");
        hasher.update(contents);
    }
    let hash = hasher.finalize();
    hex_encode(&hash)[..HASH_TRUNCATION].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn content_hash_is_deterministic() {
        let dir = tempdir("hash_deterministic");
        fs::write(dir.join("SKILL.md"), "---\nname: test\n---\n").unwrap();
        fs::write(dir.join("tools.json"), "{}").unwrap();

        let h1 = compute_content_hash(&dir).unwrap();
        let h2 = compute_content_hash(&dir).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), HASH_TRUNCATION);
    }

    #[test]
    fn content_hash_changes_on_modification() {
        let dir = tempdir("hash_changes");
        fs::write(dir.join("SKILL.md"), "v1").unwrap();
        let h1 = compute_content_hash(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "v2").unwrap();
        let h2 = compute_content_hash(&dir).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn content_hash_ignores_versions_dir() {
        let dir = tempdir("hash_ignores_versions");
        fs::write(dir.join("SKILL.md"), "test").unwrap();
        let h1 = compute_content_hash(&dir).unwrap();

        // Add a versions directory — hash should not change
        let versions = dir.join("versions").join("abc123");
        fs::create_dir_all(&versions).unwrap();
        fs::write(versions.join("SKILL.md"), "old version").unwrap();
        let h2 = compute_content_hash(&dir).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn resolve_active_dir_requires_symlink() {
        let dir = tempdir("no_symlink");
        fs::write(dir.join("SKILL.md"), "test").unwrap();
        assert!(resolve_active_dir(&dir).is_none());
    }

    #[test]
    #[cfg(unix)]
    fn resolve_active_dir_versioned_layout() {
        let dir = tempdir("versioned_layout");
        let versions = dir.join("versions").join("abc123");
        fs::create_dir_all(&versions).unwrap();
        fs::write(versions.join("SKILL.md"), "test").unwrap();

        std::os::unix::fs::symlink("versions/abc123", dir.join("active")).unwrap();

        let resolved = resolve_active_dir(&dir).unwrap();
        assert_eq!(resolved, dir.join("versions/abc123"));
    }

    #[test]
    fn create_version_snapshot_round_trip() {
        let dir = tempdir("snapshot_roundtrip");
        fs::write(dir.join("SKILL.md"), "---\nname: test\n---\n").unwrap();
        fs::write(dir.join("tools.json"), "{\"tools\":[]}").unwrap();
        let scripts = dir.join("scripts");
        fs::create_dir_all(&scripts).unwrap();
        fs::write(scripts.join("test.sh"), "#!/bin/sh\necho hi").unwrap();

        let hash = create_version_snapshot(&dir).unwrap();
        assert_eq!(hash.len(), HASH_TRUNCATION);

        // Verify the active symlink resolves to the snapshot
        let active = resolve_active_dir(&dir).unwrap();
        assert!(active.join("SKILL.md").exists());
        assert!(active.join("tools.json").exists());
        assert!(active.join("scripts").join("test.sh").exists());

        // Verify snapshot content matches original
        let snapshot_content = fs::read_to_string(active.join("SKILL.md")).unwrap();
        assert_eq!(snapshot_content, "---\nname: test\n---\n");
    }

    #[test]
    fn create_version_snapshot_idempotent() {
        let dir = tempdir("snapshot_idempotent");
        fs::write(dir.join("SKILL.md"), "test").unwrap();

        let h1 = create_version_snapshot(&dir).unwrap();
        let h2 = create_version_snapshot(&dir).unwrap();
        assert_eq!(h1, h2);

        // Only one version directory should exist
        let versions: Vec<_> = fs::read_dir(dir.join("versions"))
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(versions.len(), 1);
    }

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("chai_test_versioning").join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
