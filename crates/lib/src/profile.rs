//! Runtime profile layout under `~/.chai`: `profiles/<name>/`, `active` symlink, shared `skills/`.
//! Gateway lock at `~/.chai/gateway.lock` (profile name + PID) so `chai profile switch` can refuse while any gateway is up.
//! The running gateway holds an **advisory exclusive lock** (`flock` / `LockFileEx`) on that file so concurrent starts and stale-PID races are avoided.

use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Resolved Chai data roots for the active (or overridden) profile.
#[derive(Debug, Clone)]
pub struct ChaiPaths {
    pub chai_home: PathBuf,
    pub profile_name: String,
    pub profile_dir: PathBuf,
    pub config_path: PathBuf,
}

impl ChaiPaths {
    pub fn device_json(&self) -> PathBuf {
        self.profile_dir.join("device.json")
    }

    pub fn device_token_path(&self) -> PathBuf {
        self.profile_dir.join("device_token")
    }

    pub fn paired_json(&self) -> PathBuf {
        self.profile_dir.join("paired.json")
    }

    /// Per-profile write sandbox directory.
    pub fn sandbox_dir(&self) -> PathBuf {
        self.profile_dir.join("sandbox")
    }
}

/// `~/.chai` (or `$HOME/.chai`).
pub fn chai_home() -> Result<PathBuf> {
    dirs::home_dir()
        .map(|h| h.join(".chai"))
        .ok_or_else(|| anyhow::anyhow!("could not resolve home directory for ~/.chai"))
}

fn profiles_dir(chai_home: &Path) -> PathBuf {
    chai_home.join("profiles")
}

/// Sorted directory names under `~/.chai/profiles` (each must be a profile folder).
pub fn list_profile_names(chai_home: &Path) -> Result<Vec<String>> {
    let base = profiles_dir(chai_home);
    if !base.is_dir() {
        anyhow::bail!("no profiles directory (run `chai init` first)");
    }
    let mut names: Vec<String> = std::fs::read_dir(&base)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    Ok(names)
}

fn active_symlink_path(chai_home: &Path) -> PathBuf {
    chai_home.join("active")
}

fn gateway_lock_path(chai_home: &Path) -> PathBuf {
    chai_home.join("gateway.lock")
}

/// Profile directory `~/.chai/profiles/<name>`.
pub fn profile_dir(chai_home: &Path, profile_name: &str) -> PathBuf {
    profiles_dir(chai_home).join(profile_name)
}

fn normalize_profile_target(chai_home: &Path, raw: PathBuf) -> Result<PathBuf> {
    let dir = if raw.is_absolute() {
        raw
    } else {
        chai_home.join(raw)
    };
    let dir = std::fs::canonicalize(&dir).with_context(|| {
        format!(
            "active symlink target is not a valid path: {}",
            dir.display()
        )
    })?;
    let profiles_base = std::fs::canonicalize(profiles_dir(chai_home)).with_context(|| {
        format!(
            "profiles directory missing (run `chai init`): {}",
            profiles_dir(chai_home).display()
        )
    })?;
    if !dir.starts_with(&profiles_base) || !dir.is_dir() {
        anyhow::bail!(
            "active profile link must resolve to a directory under {}",
            profiles_base.display()
        );
    }
    Ok(dir)
}

/// Read `~/.chai/active` and return canonical profile directory.
pub fn read_persistent_profile_dir(chai_home: &Path) -> Result<PathBuf> {
    let link = active_symlink_path(chai_home);
    let target = std::fs::read_link(&link).with_context(|| {
        format!(
            "missing or invalid ~/.chai/active symlink (run `chai init` or `chai profile switch`): {}",
            link.display()
        )
    })?;
    normalize_profile_target(chai_home, target)
}

fn profile_name_from_dir(profile_dir: &Path) -> Result<String> {
    profile_dir
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("invalid profile directory path"))
}

/// Resolve which profile to use: CLI override > `CHAI_PROFILE` > `~/.chai/active`.
pub fn resolve_profile_dir(cli_profile: Option<&str>) -> Result<ChaiPaths> {
    let chai_home = chai_home()?;

    let profile_name = if let Some(name) = cli_profile {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("profile name must not be empty");
        }
        name.to_string()
    } else if let Ok(env_name) = std::env::var("CHAI_PROFILE") {
        let name = env_name.trim();
        if name.is_empty() {
            read_persistent_profile_dir(&chai_home).and_then(|d| profile_name_from_dir(&d))?
        } else {
            name.to_string()
        }
    } else {
        let dir = read_persistent_profile_dir(&chai_home)?;
        profile_name_from_dir(&dir)?
    };

    let profile_dir = profile_dir(&chai_home, &profile_name);
    if !profile_dir.is_dir() {
        anyhow::bail!(
            "profile {:?} has no directory at {}",
            profile_name,
            profile_dir.display()
        );
    }

    Ok(ChaiPaths {
        config_path: profile_dir.join("config.json"),
        profile_dir,
        profile_name,
        chai_home,
    })
}

/// Persistent profile name from `~/.chai/active` only (for `chai profile current`).
pub fn read_persistent_profile_name(chai_home: &Path) -> Result<String> {
    let dir = read_persistent_profile_dir(chai_home)?;
    profile_name_from_dir(&dir)
}

#[cfg(unix)]
fn set_active_symlink(chai_home: &Path, profile_name: &str) -> Result<()> {
    let link = active_symlink_path(chai_home);
    let target = Path::new("profiles").join(profile_name);
    if link.exists() || link.symlink_metadata().is_ok() {
        std::fs::remove_file(&link).with_context(|| format!("remove {}", link.display()))?;
    }
    std::os::unix::fs::symlink(&target, &link)
        .with_context(|| format!("symlink {} -> {}", link.display(), target.display()))?;
    Ok(())
}

#[cfg(windows)]
fn set_active_symlink(chai_home: &Path, profile_name: &str) -> Result<()> {
    let link = active_symlink_path(chai_home);
    let target = profiles_dir(chai_home).join(profile_name);
    if link.exists() || std::fs::symlink_metadata(&link).is_ok() {
        std::fs::remove_file(&link).with_context(|| format!("remove {}", link.display()))?;
    }
    std::os::windows::fs::symlink_dir(&target, &link)
        .with_context(|| format!("symlink {} -> {}", link.display(), target.display()))?;
    Ok(())
}

/// Point `~/.chai/active` at `profiles/<name>`. Caller must ensure gateway is not running.
pub fn switch_active_profile(chai_home: &Path, profile_name: &str) -> Result<()> {
    let dir = profile_dir(chai_home, profile_name);
    if !dir.is_dir() {
        anyhow::bail!("unknown profile {:?}: {}", profile_name, dir.display());
    }
    set_active_symlink(chai_home, profile_name)
}

/// True if another process holds an exclusive lock on `gateway.lock` (a gateway is running).
pub fn gateway_is_running(chai_home: &Path) -> bool {
    let path = gateway_lock_path(chai_home);
    let Ok(file) = OpenOptions::new().read(true).write(true).open(&path) else {
        return false;
    };
    match file.try_lock_exclusive() {
        Ok(()) => {
            let _ = file.unlock();
            false
        }
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => true,
        Err(e) => {
            log::warn!("gateway.lock: try_lock_exclusive failed: {}", e);
            false
        }
    }
}

/// Holds `gateway.lock` open with an exclusive advisory lock until dropped; then removes the file.
pub struct GatewayLockGuard {
    path: PathBuf,
    file: Option<std::fs::File>,
}

impl Drop for GatewayLockGuard {
    fn drop(&mut self) {
        if let Some(f) = self.file.take() {
            drop(f);
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Create `gateway.lock`, take an exclusive non-blocking advisory lock, and write profile name + PID.
/// Another process holding the lock causes an error (atomic vs TOCTOU on a plain PID file).
pub fn acquire_gateway_lock(chai_home: &Path, profile_name: &str) -> Result<GatewayLockGuard> {
    let path = gateway_lock_path(chai_home);
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    file.try_lock_exclusive().map_err(|e| {
        if e.kind() == io::ErrorKind::WouldBlock {
            anyhow::anyhow!(
                "another chai gateway is already running (stop it before starting another)"
            )
        } else {
            anyhow::Error::from(e).context(format!("lock {}", path.display()))
        }
    })?;
    file.set_len(0)
        .with_context(|| format!("truncate {}", path.display()))?;
    let pid = std::process::id();
    let content = format!("{}\n{}\n", profile_name, pid);
    file.write_all(content.as_bytes())
        .with_context(|| format!("write {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("sync {}", path.display()))?;
    Ok(GatewayLockGuard {
        path,
        file: Some(file),
    })
}

#[cfg(test)]
mod lock_tests {
    use super::*;

    #[test]
    fn gateway_lock_second_acquire_fails_until_first_dropped() {
        let dir =
            std::env::temp_dir().join(format!("chai-gateway-lock-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");

        let g1 = acquire_gateway_lock(&dir, "assistant").expect("first acquire");
        assert!(
            acquire_gateway_lock(&dir, "other").is_err(),
            "second acquire should fail while lock held"
        );
        assert!(gateway_is_running(&dir));
        drop(g1);
        assert!(!gateway_is_running(&dir));
        let _g2 = acquire_gateway_lock(&dir, "developer").expect("acquire after release");
    }
}
