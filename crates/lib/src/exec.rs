//! Safe execution layer: run allowlisted binaries with allowlisted subcommands only.
//! No shell is used; arguments are passed as a list to avoid injection.
//!
//! Also provides `WriteSandbox` for path boundary enforcement: validates that
//! write-target arguments fall within per-profile writable roots before execution.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Allowlist: binary name -> set of allowed subcommands (e.g. "notesmd-cli" -> ["search", "create", ...]).
#[derive(Debug, Clone, Default)]
pub struct Allowlist {
    /// Binary name (e.g. "notesmd-cli") -> allowed subcommands.
    bins: HashMap<String, Vec<String>>,
}

impl Allowlist {
    pub fn new() -> Self {
        Self {
            bins: HashMap::new(),
        }
    }

    /// Allow a binary to run only the given subcommands (e.g. "notesmd-cli" and ["search", "search-content", "create"]).
    pub fn allow(&mut self, binary: impl Into<String>, subcommands: Vec<&'static str>) {
        self.bins.insert(
            binary.into(),
            subcommands.into_iter().map(String::from).collect(),
        );
    }

    /// Allow a binary with subcommands given as owned strings (e.g. from a tool descriptor).
    pub fn allow_subcommands(&mut self, binary: impl Into<String>, subcommands: Vec<String>) {
        self.bins.insert(binary.into(), subcommands);
    }

    /// Check whether a (binary, subcommand) pair is allowlisted.
    pub fn is_allowed(&self, binary: &str, subcommand: &str) -> bool {
        self.bins
            .get(binary)
            .map_or(false, |subs| subs.iter().any(|s| s == subcommand))
    }

    /// Run `binary subcommand args...` if allowed. Returns combined stdout; on failure stderr is included in the error.
    /// When `working_dir` is set, the child process runs with that CWD.
    pub fn run(
        &self,
        binary: &str,
        subcommand: &str,
        args: &[String],
        working_dir: Option<&Path>,
    ) -> Result<String, String> {
        let allowed = self
            .bins
            .get(binary)
            .ok_or_else(|| format!("binary not allowlisted: {}", binary))?;
        if !allowed.iter().any(|s| s == subcommand) {
            return Err(format!(
                "subcommand not allowlisted: {} {}",
                binary, subcommand
            ));
        }
        let mut cmd = Command::new(binary);
        cmd.args(subcommand.split_whitespace()).args(args);
        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }
        let output = cmd.output().map_err(|e| format!("exec failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if output.status.success() {
            Ok(stdout)
        } else {
            let mut msg = stdout;
            if !stderr.is_empty() {
                if !msg.is_empty() {
                    msg.push_str("\n");
                }
                msg.push_str(&stderr);
            }
            Err(format!("exit {}: {}", output.status, msg))
        }
    }
}

/// Per-profile write sandbox: validates that filesystem paths fall within writable roots.
///
/// Writable roots are computed from `<profileRoot>/sandbox/`:
/// - The sandbox directory itself is always a writable root
/// - Direct-child symlinks are canonicalized and their targets become additional writable roots
///
/// Users grant write access by creating symlinks in the sandbox directory.
/// The `ln` binary must never appear in any skill's allowlist.
#[derive(Debug, Clone)]
pub struct WriteSandbox {
    /// Canonical writable root paths. A write target is valid if its canonical
    /// path starts with any of these roots.
    writable_roots: Vec<PathBuf>,
}

impl WriteSandbox {
    /// Build a sandbox from a profile's sandbox directory.
    ///
    /// If the directory does not exist, the sandbox has no writable roots
    /// and all write-path validations will fail.
    pub fn new(sandbox_dir: &Path) -> Self {
        let mut writable_roots = Vec::new();

        // Canonicalize the sandbox directory itself as the primary writable root.
        if let Ok(canonical) = std::fs::canonicalize(sandbox_dir) {
            writable_roots.push(canonical);

            // Scan direct children for symlinks; canonicalize their targets.
            if let Ok(entries) = std::fs::read_dir(sandbox_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    // Only process symlinks (not regular files/dirs, which are
                    // already covered by the sandbox root prefix check).
                    if path.symlink_metadata().map_or(false, |m| m.is_symlink()) {
                        if let Ok(target) = std::fs::canonicalize(&path) {
                            // Avoid duplicates (e.g., symlink pointing back into sandbox).
                            if !writable_roots.iter().any(|r| *r == target) {
                                writable_roots.push(target);
                            }
                        }
                        // Broken symlinks are silently skipped — they grant no access.
                    }
                }
            }
        }

        Self { writable_roots }
    }

    /// Validate that a path falls within a writable root.
    ///
    /// Returns the canonical path on success. For paths that don't exist yet
    /// (new file creation), the parent directory is canonicalized and the
    /// filename is appended.
    pub fn validate(&self, path: &str) -> Result<PathBuf, String> {
        if self.writable_roots.is_empty() {
            return Err("no writable roots configured (sandbox directory missing)".to_string());
        }

        let target = Path::new(path);
        let canonical = Self::canonicalize_for_write(target)?;

        for root in &self.writable_roots {
            if canonical.starts_with(root) {
                return Ok(canonical);
            }
        }

        Err(format!(
            "write path outside sandbox: {}",
            canonical.display()
        ))
    }

    /// Returns true if this sandbox has at least one writable root.
    pub fn has_roots(&self) -> bool {
        !self.writable_roots.is_empty()
    }

    /// The writable roots (for diagnostics / status).
    pub fn roots(&self) -> &[PathBuf] {
        &self.writable_roots
    }

    /// Canonicalize a path for write validation. If the path doesn't exist yet,
    /// canonicalize the parent and append the filename.
    fn canonicalize_for_write(path: &Path) -> Result<PathBuf, String> {
        // Try direct canonicalization first (path exists).
        if let Ok(canonical) = std::fs::canonicalize(path) {
            return Ok(canonical);
        }

        // Path doesn't exist — canonicalize parent, append filename.
        let parent = path
            .parent()
            .ok_or_else(|| format!("cannot resolve parent of: {}", path.display()))?;
        let name = path
            .file_name()
            .ok_or_else(|| format!("cannot resolve filename of: {}", path.display()))?;

        let canonical_parent = std::fs::canonicalize(parent).map_err(|e| {
            format!(
                "cannot resolve write path parent {}: {}",
                parent.display(),
                e
            )
        })?;

        Ok(canonical_parent.join(name))
    }
}

#[cfg(test)]
mod sandbox_tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("chai-sandbox-test-{}-{}", name, std::process::id()))
    }

    fn setup_sandbox(name: &str) -> (PathBuf, PathBuf) {
        let base = test_dir(name);
        let _ = fs::remove_dir_all(&base);
        let sandbox = base.join("sandbox");
        fs::create_dir_all(&sandbox).expect("create sandbox dir");
        (base, sandbox)
    }

    fn cleanup(base: &Path) {
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn sandbox_dir_is_writable_root() {
        let (base, sandbox) = setup_sandbox("basic");
        let sb = WriteSandbox::new(&sandbox);
        assert!(sb.has_roots());
        assert_eq!(sb.roots().len(), 1);

        // File inside sandbox should validate.
        let file = sandbox.join("test.txt");
        fs::write(&file, "hello").expect("write");
        assert!(sb.validate(file.to_str().unwrap()).is_ok());

        cleanup(&base);
    }

    #[test]
    fn path_outside_sandbox_rejected() {
        let (base, sandbox) = setup_sandbox("outside");
        let sb = WriteSandbox::new(&sandbox);

        // A file outside the sandbox should be rejected.
        let outside = base.join("outside.txt");
        fs::write(&outside, "nope").expect("write");
        let result = sb.validate(outside.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("outside sandbox"));

        cleanup(&base);
    }

    #[test]
    fn traversal_rejected() {
        let (base, sandbox) = setup_sandbox("traversal");
        let sb = WriteSandbox::new(&sandbox);

        // Path with .. traversal that escapes sandbox should be rejected.
        let sneaky = sandbox.join("..").join("outside.txt");
        let outside = base.join("outside.txt");
        fs::write(&outside, "nope").expect("write");
        let result = sb.validate(sneaky.to_str().unwrap());
        assert!(result.is_err());

        cleanup(&base);
    }

    #[test]
    fn new_file_in_sandbox_validates() {
        let (base, sandbox) = setup_sandbox("newfile");
        let sb = WriteSandbox::new(&sandbox);

        // A file that doesn't exist yet, but whose parent is inside the sandbox.
        let new_file = sandbox.join("new_note.md");
        assert!(!new_file.exists());
        let result = sb.validate(new_file.to_str().unwrap());
        assert!(result.is_ok());

        cleanup(&base);
    }

    #[test]
    fn new_file_outside_sandbox_rejected() {
        let (base, sandbox) = setup_sandbox("newfile-outside");
        let sb = WriteSandbox::new(&sandbox);

        // A new file whose parent is outside the sandbox.
        let new_file = base.join("sneaky.md");
        let result = sb.validate(new_file.to_str().unwrap());
        assert!(result.is_err());

        cleanup(&base);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_grants_access_to_target() {
        let (base, sandbox) = setup_sandbox("symlink");

        // Create an external directory and symlink it into the sandbox.
        let external = base.join("external-repo");
        fs::create_dir_all(&external).expect("create external");
        let link = sandbox.join("repo");
        symlink(&external, &link).expect("create symlink");

        let sb = WriteSandbox::new(&sandbox);
        assert_eq!(sb.roots().len(), 2); // sandbox + symlink target

        // File inside the symlinked target should validate.
        let file = external.join("src.rs");
        fs::write(&file, "fn main() {}").expect("write");
        assert!(sb.validate(file.to_str().unwrap()).is_ok());

        // Accessing via the symlink path should also validate.
        let via_link = link.join("src.rs");
        assert!(sb.validate(via_link.to_str().unwrap()).is_ok());

        cleanup(&base);
    }

    #[cfg(unix)]
    #[test]
    fn broken_symlink_grants_no_access() {
        let (base, sandbox) = setup_sandbox("broken-symlink");

        // Create a symlink to a nonexistent target.
        let link = sandbox.join("broken");
        symlink("/nonexistent/path", &link).expect("create broken symlink");

        let sb = WriteSandbox::new(&sandbox);
        // Only the sandbox root should be a writable root.
        assert_eq!(sb.roots().len(), 1);

        cleanup(&base);
    }

    #[test]
    fn missing_sandbox_dir_has_no_roots() {
        let sb = WriteSandbox::new(Path::new("/nonexistent/chai/sandbox"));
        assert!(!sb.has_roots());
        assert!(sb.writable_roots.is_empty());

        let result = sb.validate("/tmp/anything");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no writable roots"));
    }

    #[test]
    fn subdirectory_in_sandbox_validates() {
        let (base, sandbox) = setup_sandbox("subdir");
        let subdir = sandbox.join("project").join("src");
        fs::create_dir_all(&subdir).expect("create subdirs");
        let file = subdir.join("main.rs");
        fs::write(&file, "fn main() {}").expect("write");

        let sb = WriteSandbox::new(&sandbox);
        assert!(sb.validate(file.to_str().unwrap()).is_ok());

        cleanup(&base);
    }
}
