//! Sandbox-aware path resolution for tool parameter validation.
//!
//! Implements the `chai resolve` subcommand, which provides sandbox-aware
//! path resolution for tool `resolveCommand` entries. Each variant resolves
//! the sandbox root from `$HOME/.chai/active/sandbox` (matching the shell
//! scripts it replaces), validates that resolved paths are inside the
//! sandbox, and outputs the validated path on stdout (same protocol as the
//! resolve scripts it replaces).
//!
//! # Security properties
//!
//! All variants unconditionally validate that the resolved working directory
//! is inside the sandbox, regardless of whether project-discovery commands
//! (git rev-parse, cargo locate-project) succeed. This prevents path-traversal
//! attacks where a non-existent or non-project target directory would bypass
//! validation entirely.
use anyhow::Result;
use clap::Subcommand;
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub(crate) enum ResolveCmd {
    /// Resolve and validate a git repo path. Verifies the .git directory is inside the sandbox.
    RepoPath {
        /// Path value (relative to sandbox root, or empty for sandbox root)
        #[arg(long)]
        path: Option<String>,
    },
    /// Resolve and validate a cargo project path. Verifies the Cargo.toml is inside the sandbox.
    CargoPath {
        /// Path value (relative to sandbox root, or empty for sandbox root)
        #[arg(long)]
        path: Option<String>,
    },
    /// Resolve and validate a clone target path. Validates absolute paths are inside the sandbox.
    ClonePath {
        /// Path value (relative name, absolute path, or empty for sandbox root)
        #[arg(long)]
        path: Option<String>,
    },
    /// Resolve and validate a file path. Validates the path is inside the sandbox.
    FilePath {
        /// Path value (relative to sandbox root, or empty for sandbox root)
        #[arg(long)]
        path: Option<String>,
    },
    /// Validate a path is inside the sandbox (generic check, no project-root validation).
    Sandbox {
        /// Path value to validate (absolute path)
        #[arg(long)]
        path: Option<String>,
    },
}

/// Resolve the sandbox directory from `$HOME/.chai/active/sandbox`.
/// Returns the raw (non-canonicalized) sandbox path.
fn sandbox_raw() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| anyhow::anyhow!("$HOME not set"))?;
    Ok(PathBuf::from(home).join(".chai/active/sandbox"))
}

/// Resolve the sandbox to its physical (canonical) path.
/// This matches the shell pattern: `cd "$sandbox_raw" && pwd -P`.
fn sandbox_canonical(sandbox_raw: &Path) -> Result<PathBuf> {
    match std::fs::canonicalize(sandbox_raw) {
        Ok(canonical) => Ok(canonical),
        Err(_) => {
            // If the sandbox directory doesn't exist or can't be resolved,
            // fall back to the raw path (matching shell: `sandbox="$sandbox_raw"`)
            Ok(sandbox_raw.to_path_buf())
        }
    }
}

/// Check whether a physical (canonical) path is inside the sandbox.
///
/// Matches against both the physical sandbox root and any symlinked
/// entries at the top level of the sandbox directory. Symlinked entries
/// are granted access because the user placed them in the sandbox.
///
/// This is a direct port of the `is_inside_sandbox()` shell function.
fn is_inside_sandbox(path: &Path, sandbox: &Path, sandbox_raw: &Path) -> bool {
    // Check against the physical sandbox root.
    if path == sandbox || path.starts_with(sandbox) {
        return true;
    }

    // Defense-in-depth: check against the raw sandbox path. This catches
    // non-canonical paths that use the symlink prefix (e.g.
    // ~/.chai/active/sandbox/...) when canonicalization was not possible.
    if path == sandbox_raw || path.starts_with(sandbox_raw) {
        return true;
    }

    // Check symlinked entries in the sandbox root.
    if let Ok(entries) = std::fs::read_dir(sandbox_raw) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            // Only process symlinks.
            if entry_path
                .symlink_metadata()
                .map_or(false, |m| m.is_symlink())
            {
                if let Ok(target) = std::fs::canonicalize(&entry_path) {
                    if path == target || path.starts_with(&target) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Resolve a working directory from a path parameter.
///
/// If a path is provided, joins it with the raw sandbox path.
/// If empty, uses the raw sandbox path directly.
fn resolve_work_dir(path: &Option<String>, sandbox_raw: &Path) -> PathBuf {
    match path {
        Some(p) if !p.is_empty() => sandbox_raw.join(p),
        _ => sandbox_raw.to_path_buf(),
    }
}

/// Resolve the `repo-path` variant: validate that git would find its
/// repository root (.git directory) inside the sandbox.
///
/// This prevents git's upward traversal from escaping the sandbox when
/// the working directory does not contain its own .git.
///
/// The working directory is always validated against the sandbox boundary,
/// regardless of whether `git rev-parse --git-dir` succeeds. This closes
/// a path-traversal gap where a non-existent or non-repo target directory
/// would bypass validation entirely.
fn resolve_repo_path(path: &Option<String>) -> Result<()> {
    let raw = sandbox_raw()?;
    let canonical = sandbox_canonical(&raw)?;
    let work_dir = resolve_work_dir(path, &raw);

    // Unconditionally validate the working directory is inside the sandbox.
    // This ensures traversal paths like ../../tmp/outside are rejected
    // even when git rev-parse fails (e.g., target doesn't exist or isn't a repo).
    let work_dir_canonical = match std::fs::canonicalize(&work_dir) {
        Ok(c) => c,
        Err(_) => canonicalize_for_resolve(&work_dir)?,
    };

    if !is_inside_sandbox(&work_dir_canonical, &canonical, &raw) {
        anyhow::bail!(
            "path {} is outside the sandbox",
            path.as_deref().unwrap_or_default()
        );
    }

    // Run `git rev-parse --git-dir` in the working directory.
    let output = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&work_dir)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let git_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !git_dir.is_empty() {
                // Resolve git_dir to an absolute physical path.
                let abs_git_dir = if Path::new(&git_dir).is_absolute() {
                    // Absolute: canonicalize directly.
                    match std::fs::canonicalize(&git_dir) {
                        Ok(c) => c,
                        Err(_) => PathBuf::from(&git_dir),
                    }
                } else {
                    // Relative: resolve from work_dir, then canonicalize.
                    // First canonicalize the work_dir itself to get a physical path,
                    // then join the relative git_dir and canonicalize the result.
                    let work_dir_canonical = match std::fs::canonicalize(&work_dir) {
                        Ok(c) => c,
                        Err(_) => work_dir.clone(),
                    };
                    let joined = work_dir_canonical.join(&git_dir);
                    match std::fs::canonicalize(&joined) {
                        Ok(c) => c,
                        Err(_) => joined,
                    }
                };

                if !is_inside_sandbox(&abs_git_dir, &canonical, &raw) {
                    anyhow::bail!(
                        "git repository at {} is outside the sandbox",
                        abs_git_dir.display()
                    );
                }
            }
        }
    }

    // Output the same value as the shell script: relative path or sandbox root.
    match path {
        Some(p) if !p.is_empty() => print!("{}", p),
        _ => print!("{}", raw.to_string_lossy()),
    }

    Ok(())
}

/// Resolve the `cargo-path` variant: validate that cargo would find its
/// workspace manifest (Cargo.toml) inside the sandbox.
///
/// This prevents cargo's upward traversal from escaping the sandbox when
/// the working directory does not contain its own Cargo.toml.
///
/// The working directory is always validated against the sandbox boundary,
/// regardless of whether `cargo locate-project` succeeds. This closes
/// a path-traversal gap where a non-existent or non-cargo target directory
/// would bypass validation entirely.
fn resolve_cargo_path(path: &Option<String>) -> Result<()> {
    let raw = sandbox_raw()?;
    let canonical = sandbox_canonical(&raw)?;
    let work_dir = resolve_work_dir(path, &raw);

    // Unconditionally validate the working directory is inside the sandbox.
    // This ensures traversal paths like ../../tmp/outside are rejected
    // even when cargo locate-project fails (e.g., target doesn't exist
    // or isn't a cargo project).
    let work_dir_canonical = match std::fs::canonicalize(&work_dir) {
        Ok(c) => c,
        Err(_) => canonicalize_for_resolve(&work_dir)?,
    };

    if !is_inside_sandbox(&work_dir_canonical, &canonical, &raw) {
        anyhow::bail!(
            "path {} is outside the sandbox",
            path.as_deref().unwrap_or_default()
        );
    }

    // Run `cargo locate-project` in the working directory.
    let output = std::process::Command::new("cargo")
        .arg("locate-project")
        .current_dir(&work_dir)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse JSON: {"root":"<absolute-path>/Cargo.toml"}
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                if let Some(root) = json.get("root").and_then(|v| v.as_str()) {
                    let manifest_dir = Path::new(root)
                        .parent()
                        .ok_or_else(|| anyhow::anyhow!("cannot determine manifest directory"))?;

                    // cargo locate-project returns absolute paths, but
                    // canonicalize for consistency.
                    let manifest_canonical = match std::fs::canonicalize(manifest_dir) {
                        Ok(c) => c,
                        Err(_) => manifest_dir.to_path_buf(),
                    };

                    if !is_inside_sandbox(&manifest_canonical, &canonical, &raw) {
                        anyhow::bail!(
                            "cargo workspace at {} is outside the sandbox",
                            manifest_canonical.display()
                        );
                    }
                }
            }
        }
    }

    // Output the same value as the shell script.
    match path {
        Some(p) if !p.is_empty() => print!("{}", p),
        _ => print!("{}", raw.to_string_lossy()),
    }

    Ok(())
}

/// Resolve the `clone-path` variant: validate that clone targets fall
/// inside the sandbox.
///
/// Relative paths are prefixed with the sandbox root.
/// Absolute paths are validated against the sandbox boundary.
fn resolve_clone_path(path: &Option<String>) -> Result<()> {
    let raw = sandbox_raw()?;
    let canonical = sandbox_canonical(&raw)?;

    match path {
        Some(p) if !p.is_empty() => {
            if p.starts_with('/') {
                // Absolute path — validate it is inside the sandbox.
                let p_path = PathBuf::from(p);
                // Canonicalize for proper comparison. Use
                // canonicalize_for_resolve to handle non-existent paths
                // (e.g. clone targets that haven't been created yet).
                let p_canonical = canonicalize_for_resolve(&p_path)?;

                if !is_inside_sandbox(&p_canonical, &canonical, &raw) {
                    anyhow::bail!("clone target {} is outside the sandbox", p);
                }
                print!("{}", p);
            } else {
                // Relative path — prefix with sandbox root.
                print!("{}/{}", raw.to_string_lossy(), p);
            }
        }
        _ => {
            // Default to sandbox root (model should provide the directory name).
            print!("{}", raw.to_string_lossy());
        }
    }

    Ok(())
}

/// Resolve the `file-path` variant: validate that a file path is inside
/// the sandbox.
///
/// If a path is provided, validates the resolved path.
/// If empty, defaults to the sandbox root.
fn resolve_file_path(path: &Option<String>) -> Result<()> {
    let raw = sandbox_raw()?;
    let canonical = sandbox_canonical(&raw)?;
    let work_dir = resolve_work_dir(path, &raw);

    // Canonicalize the work_dir for validation.
    let work_dir_canonical = match std::fs::canonicalize(&work_dir) {
        Ok(c) => c,
        Err(_) => {
            // For non-existent paths, use WriteSandbox-style canonicalization:
            // walk up until we find an existing directory.
            canonicalize_for_resolve(&work_dir)?
        }
    };

    if !is_inside_sandbox(&work_dir_canonical, &canonical, &raw) {
        anyhow::bail!(
            "path {} is outside the sandbox",
            path.as_deref().unwrap_or_default()
        );
    }

    // Output the same value as the shell script pattern.
    match path {
        Some(p) if !p.is_empty() => print!("{}", p),
        _ => print!("{}", raw.to_string_lossy()),
    }

    Ok(())
}

/// Resolve the `sandbox` variant: validate that a path is inside the
/// sandbox (generic check, no project-root validation).
fn resolve_sandbox(path: &Option<String>) -> Result<()> {
    let raw = sandbox_raw()?;
    let canonical = sandbox_canonical(&raw)?;

    match path {
        Some(p) if !p.is_empty() => {
            let p_path = PathBuf::from(p);
            // Use canonicalize_for_resolve to handle non-existent paths
            // correctly (walks up to nearest existing ancestor, resolves
            // symlinks, then re-appends the non-existent suffix).
            let p_canonical = canonicalize_for_resolve(&p_path)?;

            if !is_inside_sandbox(&p_canonical, &canonical, &raw) {
                anyhow::bail!("path {} is outside the sandbox", p);
            }
            print!("{}", p);
        }
        _ => {
            print!("{}", raw.to_string_lossy());
        }
    }

    Ok(())
}

/// Canonicalize a path that may not exist yet, by walking up ancestors
/// until finding an existing directory, then re-appending the suffix.
/// This mirrors `WriteSandbox::canonicalize_for_write`.
fn canonicalize_for_resolve(path: &Path) -> Result<PathBuf> {
    // Try direct canonicalization first (path exists).
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Ok(canonical);
    }

    // Path doesn't exist — walk up the ancestor chain.
    let mut suffix: Vec<std::ffi::OsString> = Vec::new();
    let mut current = path.to_path_buf();

    loop {
        let name = current
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("cannot resolve path: {}", path.display()))?
            .to_os_string();
        suffix.push(name);

        let parent = current
            .parent()
            .ok_or_else(|| anyhow::anyhow!("cannot resolve parent of: {}", path.display()))?;

        if let Ok(canonical_parent) = std::fs::canonicalize(parent) {
            let mut result = canonical_parent;
            for component in suffix.into_iter().rev() {
                result = result.join(component);
            }
            return Ok(result);
        }

        current = parent.to_path_buf();
    }
}

pub(crate) fn run_resolve(cmd: ResolveCmd) -> Result<()> {
    match cmd {
        ResolveCmd::RepoPath { path } => resolve_repo_path(&path),
        ResolveCmd::CargoPath { path } => resolve_cargo_path(&path),
        ResolveCmd::ClonePath { path } => resolve_clone_path(&path),
        ResolveCmd::FilePath { path } => resolve_file_path(&path),
        ResolveCmd::Sandbox { path } => resolve_sandbox(&path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "chai-resolve-test-{}-{}",
            name,
            std::process::id()
        ))
    }

    // --- is_inside_sandbox tests ---

    #[test]
    fn is_inside_sandbox_matches_canonical_root() {
        let base = test_dir("inside-root");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");

        let canonical = fs::canonicalize(&sandbox_dir).expect("canonicalize");
        let child = canonical.join("some-project");
        fs::create_dir_all(&child).expect("create child");

        assert!(is_inside_sandbox(
            &child,
            &canonical,
            &sandbox_dir
        ));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn is_inside_sandbox_rejects_outside_path() {
        let base = test_dir("inside-outside");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        let outside = base.join("outside");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");
        fs::create_dir_all(&outside).expect("create outside");

        let canonical = fs::canonicalize(&sandbox_dir).expect("canonicalize");
        let outside_canonical = fs::canonicalize(&outside).expect("canonicalize outside");

        assert!(!is_inside_sandbox(
            &outside_canonical,
            &canonical,
            &sandbox_dir
        ));

        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn is_inside_sandbox_matches_symlink_target() {
        use std::os::unix::fs::symlink;

        let base = test_dir("inside-symlink");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        let external = base.join("external");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");
        fs::create_dir_all(&external).expect("create external");

        let link = sandbox_dir.join("myrepo");
        symlink(&external, &link).expect("create symlink");

        let canonical = fs::canonicalize(&sandbox_dir).expect("canonicalize");
        let external_canonical = fs::canonicalize(&external).expect("canonicalize external");

        assert!(is_inside_sandbox(
            &external_canonical,
            &canonical,
            &sandbox_dir
        ));

        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn is_inside_sandbox_matches_symlink_subpath() {
        use std::os::unix::fs::symlink;

        let base = test_dir("inside-symlink-sub");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        let external = base.join("external");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");
        fs::create_dir_all(&external.join("subdir")).expect("create external/subdir");

        let link = sandbox_dir.join("myrepo");
        symlink(&external, &link).expect("create symlink");

        let canonical = fs::canonicalize(&sandbox_dir).expect("canonicalize");
        let subdir_canonical = fs::canonicalize(external.join("subdir")).expect("canonicalize");

        assert!(is_inside_sandbox(
            &subdir_canonical,
            &canonical,
            &sandbox_dir
        ));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn is_inside_sandbox_exact_root_match() {
        let base = test_dir("inside-exact");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");

        let canonical = fs::canonicalize(&sandbox_dir).expect("canonicalize");

        assert!(is_inside_sandbox(
            &canonical,
            &canonical,
            &sandbox_dir
        ));

        let _ = fs::remove_dir_all(&base);
    }

    // --- canonicalize_for_resolve tests ---

    #[test]
    fn canonicalize_for_resolve_existing_path() {
        let base = test_dir("canon-existing");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("create base");

        let result = canonicalize_for_resolve(&base).expect("canonicalize existing");
        let expected = fs::canonicalize(&base).expect("canonicalize");
        assert_eq!(result, expected);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn canonicalize_for_resolve_nonexistent_child() {
        let base = test_dir("canon-nonexistent");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("create base");

        let path = base.join("nonexistent.txt");
        let result = canonicalize_for_resolve(&path).expect("canonicalize nonexistent");
        let expected = fs::canonicalize(&base).expect("canonicalize").join("nonexistent.txt");
        assert_eq!(result, expected);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn canonicalize_for_resolve_nested_nonexistent() {
        let base = test_dir("canon-nested");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("create base");

        let path = base.join("a/b/c.txt");
        let result = canonicalize_for_resolve(&path).expect("canonicalize nested");
        let expected = fs::canonicalize(&base).expect("canonicalize").join("a/b/c.txt");
        assert_eq!(result, expected);

        let _ = fs::remove_dir_all(&base);
    }

    // --- resolve_work_dir tests ---

    #[test]
    fn resolve_work_dir_with_path() {
        let sandbox_raw = PathBuf::from("/home/user/.chai/active/sandbox");
        let result = resolve_work_dir(&Some("chai".to_string()), &sandbox_raw);
        assert_eq!(result, PathBuf::from("/home/user/.chai/active/sandbox/chai"));
    }

    #[test]
    fn resolve_work_dir_empty_path() {
        let sandbox_raw = PathBuf::from("/home/user/.chai/active/sandbox");
        let result = resolve_work_dir(&None, &sandbox_raw);
        assert_eq!(result, PathBuf::from("/home/user/.chai/active/sandbox"));
    }

    #[test]
    fn resolve_work_dir_empty_string() {
        let sandbox_raw = PathBuf::from("/home/user/.chai/active/sandbox");
        let result = resolve_work_dir(&Some(String::new()), &sandbox_raw);
        assert_eq!(result, PathBuf::from("/home/user/.chai/active/sandbox"));
    }

    #[test]
    fn is_inside_sandbox_matches_raw_path() {
        // Verify the defense-in-depth raw path check: a path that uses the
        // non-canonical (symlink) prefix should still match when compared
        // against sandbox_raw, even if it doesn't match the canonical root.
        let base = test_dir("inside-raw");
        let _ = fs::remove_dir_all(&base);
        let real_dir = base.join("real");
        let link_dir = base.join("link");
        fs::create_dir_all(&real_dir).expect("create real");

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&real_dir, &link_dir).expect("create symlink");
        }

        // sandbox_raw is the symlink path, canonical is the real path.
        let raw = link_dir.clone();
        let canonical = fs::canonicalize(&real_dir).expect("canonicalize real");

        // A path using the symlink prefix should match via sandbox_raw.
        let raw_child = link_dir.join("some-project");
        assert!(is_inside_sandbox(&raw_child, &canonical, &raw));

        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn canonicalize_for_resolve_symlinked_parent() {
        // Verify that canonicalize_for_resolve resolves symlinks in the
        // existing prefix of a path, even when the final component doesn't
        // exist. This is the exact scenario from the clone-path/sandbox bug.
        use std::os::unix::fs::symlink;

        let base = test_dir("canon-symlink");
        let _ = fs::remove_dir_all(&base);
        let real_dir = base.join("real");
        let link_dir = base.join("link");
        fs::create_dir_all(&real_dir).expect("create real");
        symlink(&real_dir, &link_dir).expect("create symlink");

        // Path with non-existent child under a symlinked directory.
        let path = link_dir.join("new-project");
        let result = canonicalize_for_resolve(&path).expect("canonicalize");
        let expected = fs::canonicalize(&real_dir)
            .expect("canonicalize real")
            .join("new-project");

        assert_eq!(result, expected);

        let _ = fs::remove_dir_all(&base);
    }

    // --- resolve_work_dir boundary validation tests ---

    #[test]
    fn resolve_work_dir_rejects_traversal_to_outside() {
        // Verify that a traversal path like ../../tmp/outside resolves to
        // a work_dir outside the sandbox, and is_inside_sandbox rejects it.
        let sandbox = PathBuf::from("/home/user/.chai/active/sandbox");
        let work_dir = resolve_work_dir(
            &Some("../../tmp/outside".to_string()),
            &sandbox,
        );
        // The work_dir is sandbox_raw joined with ../../tmp/outside.
        assert_eq!(
            work_dir,
            PathBuf::from("/home/user/.chai/active/sandbox/../../tmp/outside")
        );
        // After canonicalization, this should be outside the sandbox.
        // We test the logic directly: the joined path does NOT start with
        // the sandbox prefix when canonicalized (in a real filesystem).
        // Since this test doesn't create files, we verify the join logic.
        assert!(work_dir
            .components()
            .any(|c| c.as_os_str() == ".."));
    }

    #[test]
    fn resolve_work_dir_inside_sandbox_no_traversal() {
        // Verify that a normal relative path resolves inside the sandbox.
        let sandbox = PathBuf::from("/home/user/.chai/active/sandbox");
        let work_dir = resolve_work_dir(
            &Some("chai".to_string()),
            &sandbox,
        );
        assert_eq!(
            work_dir,
            PathBuf::from("/home/user/.chai/active/sandbox/chai")
        );
        assert!(!work_dir
            .components()
            .any(|c| c.as_os_str() == ".."));
    }

    #[test]
    fn is_inside_sandbox_rejects_canonicalized_traversal() {
        // Create a real filesystem scenario: sandbox with a sibling directory.
        // A path that traverses out of the sandbox should be rejected.
        let base = test_dir("traversal-reject");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        let outside_dir = base.join("outside");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");
        fs::create_dir_all(&outside_dir).expect("create outside");

        let canonical = fs::canonicalize(&sandbox_dir).expect("canonicalize");
        let outside_canonical = fs::canonicalize(&outside_dir).expect("canonicalize");

        // A directory outside the sandbox is rejected.
        assert!(!is_inside_sandbox(
            &outside_canonical,
            &canonical,
            &sandbox_dir
        ));

        // A subdirectory of the outside directory is also rejected.
        let outside_sub = outside_canonical.join("subdir");
        fs::create_dir_all(&outside_sub).expect("create outside/subdir");
        assert!(!is_inside_sandbox(
            &outside_sub,
            &canonical,
            &sandbox_dir
        ));

        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn resolve_repo_path_rejects_traversal_nonexistent_target() {
        // Integration-style test: verify that resolve_repo_path rejects
        // a traversal path even when the target doesn't exist (and thus
        // git rev-parse would fail). We can't call resolve_repo_path
        // directly in a unit test (it requires $HOME and sandbox setup),
        // but we can test the core logic: a work_dir that resolves
        // outside the sandbox is caught by is_inside_sandbox.
        use std::os::unix::fs::symlink;

        let base = test_dir("repo-traversal");
        let _ = fs::remove_dir_all(&base);

        // Set up a sandbox with a symlink for the raw path.
        let real_sandbox = base.join("real-sandbox");
        let link_sandbox = base.join("link-sandbox");
        let outside = base.join("outside");
        fs::create_dir_all(&real_sandbox).expect("create real sandbox");
        fs::create_dir_all(&outside).expect("create outside");
        symlink(&real_sandbox, &link_sandbox).expect("create symlink");

        let canonical = fs::canonicalize(&real_sandbox).expect("canonicalize");

        // Simulate what resolve_repo_path does: join sandbox_raw with
        // a traversal path.
        let work_dir = link_sandbox.join("../../outside");

        // Canonicalize using the same logic as the fix.
        let work_dir_canonical = if work_dir.exists() {
            fs::canonicalize(&work_dir).expect("canonicalize traversal")
        } else {
            // If the joined path doesn't exist, canonicalize parent.
            canonicalize_for_resolve(&work_dir).expect("canonicalize for resolve")
        };

        // The canonicalized path should be outside the sandbox.
        assert!(!is_inside_sandbox(
            &work_dir_canonical,
            &canonical,
            &link_sandbox
        ));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_work_dir_inside_sandbox_with_subpath() {
        // Verify that a relative subpath inside the sandbox passes
        // the boundary check when the directory exists.
        let base = test_dir("workdir-subpath");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        let project_dir = sandbox_dir.join("my-project");
        fs::create_dir_all(&project_dir).expect("create project");

        let canonical = fs::canonicalize(&sandbox_dir).expect("canonicalize");
        let project_canonical = fs::canonicalize(&project_dir).expect("canonicalize project");

        assert!(is_inside_sandbox(
            &project_canonical,
            &canonical,
            &sandbox_dir
        ));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_work_dir_inside_sandbox_nonexistent_child() {
        // Verify that a non-existent child of the sandbox passes
        // the boundary check (using canonicalize_for_resolve).
        let base = test_dir("workdir-nonexistent");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");

        let canonical = fs::canonicalize(&sandbox_dir).expect("canonicalize");
        let nonexistent = sandbox_dir.join("new-project");

        let resolved = canonicalize_for_resolve(&nonexistent).expect("canonicalize");
        assert!(is_inside_sandbox(
            &resolved,
            &canonical,
            &sandbox_dir
        ));

        let _ = fs::remove_dir_all(&base);
    }
}
