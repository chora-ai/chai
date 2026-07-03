//! Tracked `.env` management for the desktop process.
//!
//! Unlike `lib::config::load_profile_env` (which uses a `OnceLock` to load `.env` at most once),
//! the desktop needs to reload `.env` when the user switches profiles. This module tracks which
//! environment variables were set by the previous profile's `.env` so they can be removed before
//! loading the new profile's `.env`.
//!
//! **Security invariant:** Only variables that were **not already present** in the process
//! environment when `.env` was loaded are tracked and removed on switch. Shell/system variables
//! are never touched.
//!
//! The launch environment (before any `.env` is loaded) is snapshotted at init time. This
//! snapshot is used when building gateway child environments to ensure that stale `.env`
//! variables from a previous profile cannot leak into the child process.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// Tracks which environment variables were set by the loaded `.env` file (key → value).
/// Variables that were already present in the process environment when `.env` was loaded
/// are **not** tracked — they belong to the launch environment, not to any profile.
static DOTENV_TRACKED: std::sync::OnceLock<Mutex<HashMap<String, String>>> = std::sync::OnceLock::new();

/// Snapshot of the process environment captured before any `.env` file is loaded.
/// Used by `build_gateway_env` to construct a clean environment for gateway child
/// processes that excludes any `.env`-sourced variables from the current or previous
/// profiles.
static LAUNCH_ENV: std::sync::OnceLock<HashMap<String, String>> = std::sync::OnceLock::new();

fn tracked() -> &'static Mutex<HashMap<String, String>> {
    DOTENV_TRACKED.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Capture the current process environment as the "launch environment" snapshot.
///
/// Must be called exactly once, before any `.env` file is loaded. After this call,
/// `build_gateway_env` will use the snapshot instead of `std::env::vars()` so that
/// `.env` variables loaded into the desktop process do not leak into gateway children.
fn snapshot_launch_env() {
    LAUNCH_ENV.get_or_init(|| std::env::vars().collect());
}

/// Load `.env` from the given profile directory, tracking which variables were set.
///
/// If a previous profile's `.env` was loaded, removes all tracked variables from the
/// process environment before loading the new one. Variables already present in the
/// process environment (from the shell or system) are never removed or overridden.
///
/// Returns `Ok(())` if the `.env` was loaded (or didn't exist), or an error if the
/// file could not be parsed.
pub fn load_profile_env_tracked(profile_dir: &Path) -> Result<(), dotenvy::Error> {
    // Capture the launch environment on first call (before any .env is loaded).
    snapshot_launch_env();

    let env_path = profile_dir.join(".env");
    if !env_path.is_file() {
        // No .env for this profile — clear any previously tracked variables.
        clear_tracked();
        return Ok(());
    }

    // Parse the .env file entries without setting them, then set each one manually
    // so we can track which ones were actually new (not already in the environment).
    let entries: Vec<(String, String)> = dotenvy::from_path_iter(&env_path)?
        .filter_map(|entry| entry.ok())
        .collect();

    // Remove previously tracked variables before setting new ones.
    clear_tracked();

    let mut guard = tracked().lock().unwrap();
    for (key, value) in entries {
        // Only set (and track) variables not already present in the process environment.
        if std::env::var_os(&key).is_none() {
            std::env::set_var(&key, &value);
            guard.insert(key, value);
        }
    }

    log::info!(
        "loaded .env from {} ({} variables tracked)",
        env_path.display(),
        guard.len()
    );
    Ok(())
}

/// Remove all environment variables that were set by the previous profile's `.env`.
///
/// This is called automatically by `load_profile_env_tracked` before loading the new
/// profile's `.env`, and can also be called directly when a profile with no `.env`
/// is activated.
fn clear_tracked() {
    let mut guard = tracked().lock().unwrap();
    if guard.is_empty() {
        return;
    }
    let count = guard.len();
    for key in guard.keys() {
        std::env::remove_var(key);
    }
    guard.clear();
    log::info!("cleared {} tracked .env variables", count);
}

/// Build the environment map for a gateway child process based on the given profile.
///
/// This constructs a clean environment for the child process:
/// 1. Starts with the **launch environment** (the environment as it was before
///    any `.env` variables were loaded by the desktop). This includes shell variables,
///    system variables from when the desktop was launched.
/// 2. Overlays the profile's `.env` variables on top.
///
/// This is a defense-in-depth measure: even if the desktop process's own environment
/// tracking has a bug, the gateway child gets the correct environment for its profile.
/// Because the launch environment is snapshotted before any `.env` is loaded, stale
/// variables from a previous profile cannot leak through.
pub fn build_gateway_env(profile_dir: &Path) -> HashMap<String, String> {
    // Start from the launch environment (before any .env was loaded).
    let mut env_map = LAUNCH_ENV
        .get()
        .cloned()
        .unwrap_or_else(|| std::env::vars().collect());

    // Overlay the profile's .env variables. Shell/system vars from the launch
    // environment take precedence (same semantics as dotenvy::from_path).
    let env_path = profile_dir.join(".env");
    if env_path.is_file() {
        if let Ok(entries) = dotenvy::from_path_iter(&env_path) {
            for entry in entries.flatten() {
                if !env_map.contains_key(&entry.0) {
                    env_map.insert(entry.0, entry.1);
                }
            }
        }
    }

    env_map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Serializes tests that mutate process-global state (`std::env`, `DOTENV_TRACKED`).
    /// These tests cannot run in parallel because they share `std::env` and the
    /// `DOTENV_TRACKED` OnceLock; one test's `clear_tracked()` call would remove
    /// another test's variables.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap()
    }

    #[test]
    fn tracked_vars_are_set_in_env() {
        let _guard = test_lock();
        let dir = std::env::temp_dir().join("chai_test_env_tracked_set");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".env"), "CHAI_TEST_TRACKED_SET=hello\n").unwrap();

        // Ensure the var is not already set.
        std::env::remove_var("CHAI_TEST_TRACKED_SET");

        load_profile_env_tracked(&dir).unwrap();
        assert_eq!(std::env::var("CHAI_TEST_TRACKED_SET").unwrap(), "hello");

        // Clean up.
        clear_tracked();
        std::env::remove_var("CHAI_TEST_TRACKED_SET");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tracked_vars_are_removed_on_switch() {
        let _guard = test_lock();
        let dir_a = std::env::temp_dir().join("chai_test_env_tracked_a");
        let dir_b = std::env::temp_dir().join("chai_test_env_tracked_b");
        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        fs::write(dir_a.join(".env"), "CHAI_TEST_TRACKED_A=val_a\n").unwrap();
        // Profile B has no .env or a different variable.
        fs::write(dir_b.join(".env"), "CHAI_TEST_TRACKED_B=val_b\n").unwrap();

        std::env::remove_var("CHAI_TEST_TRACKED_A");
        std::env::remove_var("CHAI_TEST_TRACKED_B");

        // Load profile A.
        load_profile_env_tracked(&dir_a).unwrap();
        assert_eq!(std::env::var("CHAI_TEST_TRACKED_A").unwrap(), "val_a");
        assert!(std::env::var("CHAI_TEST_TRACKED_B").is_err());

        // Switch to profile B — A's variable should be removed.
        load_profile_env_tracked(&dir_b).unwrap();
        assert!(std::env::var("CHAI_TEST_TRACKED_A").is_err());
        assert_eq!(std::env::var("CHAI_TEST_TRACKED_B").unwrap(), "val_b");

        // Clean up.
        clear_tracked();
        std::env::remove_var("CHAI_TEST_TRACKED_A");
        std::env::remove_var("CHAI_TEST_TRACKED_B");
        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
    }

    #[test]
    fn shell_vars_are_not_removed_on_switch() {
        let _guard = test_lock();
        let dir_a = std::env::temp_dir().join("chai_test_env_shell_a");
        let dir_b = std::env::temp_dir().join("chai_test_env_shell_b");
        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        fs::write(dir_a.join(".env"), "CHAI_TEST_SHELL_VAR=from_dotenv\n").unwrap();
        fs::write(dir_b.join(".env"), "CHAI_TEST_SHELL_OTHER=other\n").unwrap();

        // Set the variable in the shell environment BEFORE loading .env.
        std::env::set_var("CHAI_TEST_SHELL_VAR", "from_shell");
        std::env::remove_var("CHAI_TEST_SHELL_OTHER");

        // Load profile A — the .env value should NOT override the shell value.
        load_profile_env_tracked(&dir_a).unwrap();
        assert_eq!(std::env::var("CHAI_TEST_SHELL_VAR").unwrap(), "from_shell");

        // Switch to profile B — the shell variable should still be there.
        load_profile_env_tracked(&dir_b).unwrap();
        assert_eq!(std::env::var("CHAI_TEST_SHELL_VAR").unwrap(), "from_shell");

        // Clean up.
        clear_tracked();
        std::env::remove_var("CHAI_TEST_SHELL_VAR");
        std::env::remove_var("CHAI_TEST_SHELL_OTHER");
        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
    }

    #[test]
    fn profile_with_no_dotenv_clears_tracked() {
        let _guard = test_lock();
        let dir_a = std::env::temp_dir().join("chai_test_env_no_dotenv_a");
        let dir_b = std::env::temp_dir().join("chai_test_env_no_dotenv_b");
        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        fs::write(dir_a.join(".env"), "CHAI_TEST_NO_DOTENV=val\n").unwrap();
        // dir_b has no .env file.

        std::env::remove_var("CHAI_TEST_NO_DOTENV");

        // Load profile A.
        load_profile_env_tracked(&dir_a).unwrap();
        assert_eq!(std::env::var("CHAI_TEST_NO_DOTENV").unwrap(), "val");

        // Switch to profile B (no .env) — A's variable should be removed.
        load_profile_env_tracked(&dir_b).unwrap();
        assert!(std::env::var("CHAI_TEST_NO_DOTENV").is_err());

        // Clean up.
        clear_tracked();
        std::env::remove_var("CHAI_TEST_NO_DOTENV");
        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
    }

    #[test]
    fn build_gateway_env_uses_launch_env_plus_profile_dotenv() {
        let _guard = test_lock();
        let dir_a = std::env::temp_dir().join("chai_test_env_gw_a");
        let dir_b = std::env::temp_dir().join("chai_test_env_gw_b");
        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        fs::write(dir_a.join(".env"), "CHAI_TEST_GW_A=gw_a\nCHAI_TEST_GW_SHARED=from_a\n").unwrap();
        fs::write(dir_b.join(".env"), "CHAI_TEST_GW_B=gw_b\nCHAI_TEST_GW_SHARED=from_b\n").unwrap();

        std::env::remove_var("CHAI_TEST_GW_A");
        std::env::remove_var("CHAI_TEST_GW_B");
        std::env::remove_var("CHAI_TEST_GW_SHARED");

        // Load profile A into the desktop process.
        load_profile_env_tracked(&dir_a).unwrap();

        // Build gateway env for profile B — should NOT contain A's variable,
        // should contain B's variable, and B's .env should override A's shared var.
        let env = build_gateway_env(&dir_b);
        assert!(!env.contains_key("CHAI_TEST_GW_A"), "A's exclusive var should not be in gateway env");
        assert_eq!(env.get("CHAI_TEST_GW_B").unwrap(), "gw_b", "B's var should be in gateway env");
        assert_eq!(env.get("CHAI_TEST_GW_SHARED").unwrap(), "from_b", "B's shared var should override A's");

        // Clean up.
        clear_tracked();
        std::env::remove_var("CHAI_TEST_GW_A");
        std::env::remove_var("CHAI_TEST_GW_B");
        std::env::remove_var("CHAI_TEST_GW_SHARED");
        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
    }
}
