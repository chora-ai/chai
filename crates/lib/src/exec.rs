//! Safe execution layer: run allowlisted binaries with allowlisted subcommands only.
//! No shell is used; arguments are passed as a list to avoid injection.
//!
//! Also provides `WriteSandbox` for path boundary enforcement: validates that
//! write-target arguments fall within per-profile writable roots before execution.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Environment variable that overrides the `chai` binary path used by the
/// allowlist executor. When set, any tool execution that references the
/// binary name `"chai"` will use this path instead. This allows local
/// development builds (e.g. `target/debug/chai`) to be used for testing
/// without installing to the system PATH.
const CHAI_BIN_ENV: &str = "CHAI_BIN";

/// Resolve the actual binary path for a given binary name.
///
/// When the binary is `"chai"` and the `CHAI_BIN` environment variable is set,
/// returns the value of that variable so that tool calls use the locally built
/// chai binary (e.g. `target/debug/chai`) instead of the system-installed one.
/// When the variable is not set or the binary is not `"chai"`, returns the
/// binary name unchanged (resolved via PATH by the OS).
pub fn resolve_binary(binary: &str) -> String {
    if binary == "chai" {
        if let Ok(custom) = std::env::var(CHAI_BIN_ENV) {
            if !custom.is_empty() {
                log::info!("using CHAI_BIN={}", custom);
                return custom;
            }
        }
    }
    binary.to_string()
}

/// Build a `Command` for the given resolved binary, subcommand, and args.
///
/// When `binary_wrapper` is present, the command is constructed as
/// `wrapper[0] wrapper[1..] resolved_binary subcommand args...` instead of
/// `resolved_binary subcommand args...`. The wrapper is a transparent prefix
/// (e.g. `nix develop --command`) that determines *how* the binary is invoked,
/// not *what* is invoked — the allowlist still validates the declared binary
/// and subcommand.
fn build_command(
    resolved: &str,
    subcommand: &str,
    args: &[String],
    binary_wrapper: Option<&[String]>,
) -> Command {
    match binary_wrapper {
        Some(wrapper) => {
            let mut cmd = Command::new(&wrapper[0]);
            cmd.args(&wrapper[1..]);
            cmd.arg(resolved);
            cmd.args(subcommand.split_whitespace());
            cmd.args(args);
            cmd
        }
        None => {
            let mut cmd = Command::new(resolved);
            cmd.args(subcommand.split_whitespace());
            cmd.args(args);
            cmd
        }
    }
}

/// Allowlist: binary name -> set of allowed subcommands (e.g. "git" -> ["search", "create", ...]).
#[derive(Debug, Clone, Default)]
pub struct Allowlist {
    /// Binary name (e.g. "git") -> allowed subcommands.
    bins: HashMap<String, Vec<String>>,
}

impl Allowlist {
    pub fn new() -> Self {
        Self {
            bins: HashMap::new(),
        }
    }

    /// Allow a binary to run only the given subcommands (e.g. "git" and ["status", "log"]).
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
    /// Only exit code 0 is treated as success.
    pub fn run(
        &self,
        binary: &str,
        subcommand: &str,
        args: &[String],
        working_dir: Option<&Path>,
    ) -> Result<String, String> {
        self.run_with_codes(binary, subcommand, args, working_dir, &[], None)
    }

    /// Run `binary subcommand args...` if allowed, treating the given exit codes
    /// as success in addition to 0. Returns combined stdout; on failure stderr is
    /// included in the error. When `working_dir` is set, the child process runs
    /// with that CWD. When `binary_wrapper` is present, the command is constructed
    /// as `wrapper[0] wrapper[1..] binary subcommand args...` instead of
    /// `binary subcommand args...`.
    pub fn run_with_codes(
        &self,
        binary: &str,
        subcommand: &str,
        args: &[String],
        working_dir: Option<&Path>,
        success_exit_codes: &[i32],
        binary_wrapper: Option<&[String]>,
    ) -> Result<String, String> {
        self.run_with_codes_and_exit(binary, subcommand, args, working_dir, success_exit_codes, binary_wrapper)
            .map(|(_, output)| output)
    }

    /// Like `run_with_codes`, but also returns the exit code on success.
    pub fn run_with_codes_and_exit(
        &self,
        binary: &str,
        subcommand: &str,
        args: &[String],
        working_dir: Option<&Path>,
        success_exit_codes: &[i32],
        binary_wrapper: Option<&[String]>,
    ) -> Result<(i32, String), String> {
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
        let resolved = resolve_binary(binary);
        let mut cmd = build_command(&resolved, subcommand, args, binary_wrapper);
        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }
        let output = cmd.output().map_err(|e| format!("exec failed: {}", e))?;
        Self::collect_output_with_codes(output, success_exit_codes)
    }

    /// Run `binary subcommand args...` if allowed, piping `stdin` bytes to the child's stdin.
    /// Returns combined stdout; on failure stderr is included in the error.
    /// When `working_dir` is set, the child process runs with that CWD.
    /// Only exit code 0 is treated as success.
    pub fn run_with_stdin(
        &self,
        binary: &str,
        subcommand: &str,
        args: &[String],
        working_dir: Option<&Path>,
        stdin: &[u8],
    ) -> Result<String, String> {
        self.run_with_stdin_with_codes(binary, subcommand, args, working_dir, stdin, &[], None)
    }

    /// Run `binary subcommand args...` if allowed, piping `stdin` bytes to the child's stdin,
    /// and treating the given exit codes as success in addition to 0. Returns combined stdout;
    /// on failure stderr is included in the error. When `working_dir` is set, the child process
    /// runs with that CWD. When `binary_wrapper` is present, the command is constructed as
    /// `wrapper[0] wrapper[1..] binary subcommand args...` instead of
    /// `binary subcommand args...`.
    pub fn run_with_stdin_with_codes(
        &self,
        binary: &str,
        subcommand: &str,
        args: &[String],
        working_dir: Option<&Path>,
        stdin: &[u8],
        success_exit_codes: &[i32],
        binary_wrapper: Option<&[String]>,
    ) -> Result<String, String> {
        self.run_with_stdin_with_codes_and_exit(binary, subcommand, args, working_dir, stdin, success_exit_codes, binary_wrapper)
            .map(|(_, output)| output)
    }

    /// Like `run_with_stdin_with_codes`, but also returns the exit code on success.
    pub fn run_with_stdin_with_codes_and_exit(
        &self,
        binary: &str,
        subcommand: &str,
        args: &[String],
        working_dir: Option<&Path>,
        stdin: &[u8],
        success_exit_codes: &[i32],
        binary_wrapper: Option<&[String]>,
    ) -> Result<(i32, String), String> {
        use std::io::Write;
        use std::process::Stdio;

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
        let resolved = resolve_binary(binary);
        let mut cmd = build_command(&resolved, subcommand, args, binary_wrapper);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }
        let mut child = cmd.spawn().map_err(|e| format!("exec failed: {}", e))?;

        // Take the stdin pipe and write content. Use explicit error handling
        // instead of `if let Some` so that a missing pipe (which should never
        // happen since Stdio::piped() is set above) surfaces as an error
        // rather than silently skipping stdin. Drop the pipe before waiting
        // so the child sees EOF on stdin.
        {
            let mut pipe = child.stdin.take().ok_or_else(|| {
                "failed to acquire stdin pipe: Stdio::piped() was set but pipe is unavailable"
                    .to_string()
            })?;
            pipe.write_all(stdin)
                .map_err(|e| format!("failed to write stdin: {}", e))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("exec failed: {}", e))?;
        Self::collect_output_with_codes(output, success_exit_codes)
    }

    /// Collect stdout/stderr from a completed child output into a Result.
    /// Exit codes in `success_exit_codes` are treated as success (in addition
    /// to 0, which is always success). For example, passing `[1]` causes grep's
    /// "no match" exit to return `Ok(stdout)` instead of `Err(...)`.
    ///
    /// When a non-zero exit code is in the success list, stderr is appended to
    /// stdout (separated by a newline if both are non-empty) so that postProcess
    /// scripts can inspect error messages that git and other tools write to stderr.
    /// Exit codes not in this list still surface as tool errors.
    ///
    /// Returns `Ok((exit_code, output))` on success or `Err(...)` on failure.
    fn collect_output_with_codes(
        output: std::process::Output,
        success_exit_codes: &[i32],
    ) -> Result<(i32, String), String> {
        let code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if output.status.success() {
            return Ok((0, stdout));
        }
        // Check if the exit code is in the explicit success list.
        if success_exit_codes.contains(&code) {
            // Treat this exit code as success, but still include stderr
            // so that postProcess scripts can inspect error messages
            // (e.g. git writing diagnostics to stderr).
            let mut result = stdout;
            if !stderr.is_empty() {
                if !result.is_empty() {
                    result.push(10 as char); // newline
                }
                result.push_str(&stderr);
            }
            return Ok((code, result));
        }
        let mut msg = stdout;
        if !stderr.is_empty() {
            if !msg.is_empty() {
                msg.push(10 as char); // newline
            }
            msg.push_str(&stderr);
        }
        Err(format!("exit {}: {}", output.status, msg))
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

    /// Build a sandbox that uses the current working directory as the sole
    /// writable root. Used when `sandbox.mode` is `"current"` and the profile
    /// sandbox directory does not exist.
    ///
    /// If the CWD cannot be canonicalized, the sandbox has no writable roots
    /// and all write-path validations will fail.
    pub fn from_cwd() -> Self {
        let mut writable_roots = Vec::new();
        if let Ok(cwd) = std::env::current_dir().and_then(|p| std::fs::canonicalize(&p)) {
            writable_roots.push(cwd);
        }
        Self { writable_roots }
    }

    /// Validate that a path falls within a writable root and does not
    /// target a `.git/` directory.
    ///
    /// Returns the canonical path on success. For paths that don't exist yet
    /// (new file creation), the parent directory is canonicalized and the
    /// filename is appended.
    ///
    /// Relative paths are resolved against the primary sandbox root (the
    /// sandbox directory itself), not the process working directory. This
    /// ensures that tools providing sandbox-relative paths work correctly
    /// regardless of where the gateway process was launched.
    ///
    /// `.git/` directories are excluded from write access regardless of
    /// whether they fall within a writable root. Git state must only be
    /// modified through the git skill's constrained tools, not through
    /// arbitrary file writes that bypass branch protection and hook safety.
    pub fn validate(&self, path: &str) -> Result<PathBuf, String> {
        if self.writable_roots.is_empty() {
            return Err("no writable roots configured (sandbox directory missing)".to_string());
        }

        let target = Path::new(path);
        // Relative paths must be anchored to the sandbox root, not the process
        // CWD. `std::fs::canonicalize` resolves relative paths against the
        // process CWD, which would cause validation to use the wrong base and
        // either incorrectly reject valid sandbox-relative paths or accept
        // paths that happen to exist relative to the gateway's launch directory.
        let resolved_target = if target.is_relative() {
            self.writable_roots[0].join(target)
        } else {
            target.to_path_buf()
        };
        let canonical = Self::canonicalize_for_write(&resolved_target)?;

        // Reject writes that target a .git/ directory. The .git/ directory
        // is a special filesystem namespace that should only be modified
        // through git's own tools, not through arbitrary file writes. This
        // prevents the files skill from bypassing the git skill's branch
        // protection, hook safety, and other deny-pattern restrictions.
        if path_intersects_git_dir(&canonical) {
            return Err(format!(
                "write path targets a .git directory: {}",
                canonical.display()
            ));
        }

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
    /// walks up the ancestor chain until finding an existing directory, canonicalizes
    /// that, then re-appends the non-existing suffix. This handles cases where
    /// multiple levels of parent directories do not exist yet (e.g. `a/b/c.txt`
    /// where neither `a` nor `a/b` exist).
    fn canonicalize_for_write(path: &Path) -> Result<PathBuf, String> {
        // Try direct canonicalization first (path exists).
        if let Ok(canonical) = std::fs::canonicalize(path) {
            return Ok(canonical);
        }

        // Path doesn't exist — walk up the ancestor chain until we find an
        // existing directory, then re-append the non-existing suffix components.
        let mut suffix: Vec<std::ffi::OsString> = Vec::new();
        let mut current = path.to_path_buf();

        loop {
            let name = current
                .file_name()
                .ok_or_else(|| format!("cannot resolve write path: {}", path.display()))?
                .to_os_string();
            suffix.push(name);

            let parent = current
                .parent()
                .ok_or_else(|| format!("cannot resolve parent of: {}", path.display()))?;

            if let Ok(canonical_parent) = std::fs::canonicalize(parent) {
                // Found an existing ancestor — rebuild the full path.
                let mut result = canonical_parent;
                for component in suffix.into_iter().rev() {
                    result = result.join(component);
                }
                return Ok(result);
            }

            current = parent.to_path_buf();
        }
    }
}

/// Check whether a canonical path intersects a `.git/` directory.
///
/// Returns true if:
/// - The path ends with a `.git` component (e.g., `/project/.git`)
/// - The path contains a `.git` component anywhere (e.g., `/project/.git/refs/heads/main`)
///
/// This is used by `WriteSandbox::validate()` to reject writes that target
/// git's internal state, which must only be modified through the git skill's
/// constrained tools.
fn path_intersects_git_dir(canonical: &Path) -> bool {
    canonical
        .components()
        .any(|c| c.as_os_str() == std::ffi::OsStr::new(".git"))
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
    fn resolve_binary_returns_chai_bin_env_when_set() {
        // Temporarily set CHAI_BIN for this test.
        std::env::set_var("CHAI_BIN", "/custom/path/to/chai");
        assert_eq!(resolve_binary("chai"), "/custom/path/to/chai");
        std::env::remove_var("CHAI_BIN");
    }

    #[test]
    fn resolve_binary_returns_binary_name_when_env_not_set() {
        std::env::remove_var("CHAI_BIN");
        assert_eq!(resolve_binary("chai"), "chai");
    }

    #[test]
    fn resolve_binary_returns_binary_name_when_env_empty() {
        std::env::set_var("CHAI_BIN", "");
        assert_eq!(resolve_binary("chai"), "chai");
        std::env::remove_var("CHAI_BIN");
    }

    #[test]
    fn resolve_binary_does_not_affect_other_binaries() {
        std::env::set_var("CHAI_BIN", "/custom/chai");
        assert_eq!(resolve_binary("git"), "git");
        assert_eq!(resolve_binary("cat"), "cat");
        std::env::remove_var("CHAI_BIN");
    }

    #[test]
    fn relative_path_resolved_against_sandbox_root() {
        let (base, sandbox) = setup_sandbox("relative");
        let sb = WriteSandbox::new(&sandbox);

        // Write a file inside the sandbox so it exists for canonicalization.
        let file = sandbox.join("notes").join("entry.md");
        fs::create_dir_all(file.parent().unwrap()).expect("create subdir");
        fs::write(&file, "hello").expect("write");

        // Provide a relative path — should resolve against the sandbox root.
        let result = sb.validate("notes/entry.md");
        assert!(result.is_ok(), "relative path inside sandbox should be valid: {:?}", result);

        cleanup(&base);
    }

    #[test]
    fn relative_path_new_file_resolved_against_sandbox_root() {
        let (base, sandbox) = setup_sandbox("relative-new");
        let sb = WriteSandbox::new(&sandbox);

        // File does not exist yet — parent is the sandbox root itself.
        let result = sb.validate("new_note.md");
        assert!(result.is_ok(), "relative new file in sandbox root should be valid: {:?}", result);

        cleanup(&base);
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
    fn new_file_in_missing_subdirectory_validates() {
        let (base, sandbox) = setup_sandbox("newfile-subdir");
        let sb = WriteSandbox::new(&sandbox);

        // Neither "test/" nor "test/test.md" exist yet — multiple missing levels.
        let new_file = sandbox.join("test").join("test.md");
        assert!(!new_file.exists());
        assert!(!new_file.parent().unwrap().exists());
        let result = sb.validate(new_file.to_str().unwrap());
        assert!(result.is_ok(), "deeply nested new file should validate: {:?}", result);

        // The returned path should still be anchored inside the sandbox.
        let canonical = result.unwrap();
        let sandbox_canonical = fs::canonicalize(&sandbox).expect("canonicalize sandbox");
        assert!(
            canonical.starts_with(&sandbox_canonical),
            "canonical path {:?} should be inside sandbox {:?}",
            canonical,
            sandbox_canonical
        );

        cleanup(&base);
    }

    #[test]
    fn new_file_in_missing_subdirectory_relative_path_validates() {
        let (base, sandbox) = setup_sandbox("newfile-subdir-rel");
        let sb = WriteSandbox::new(&sandbox);

        // Relative path: "test/test.md" — sandbox root is the base, neither dir exists.
        let result = sb.validate("test/test.md");
        assert!(result.is_ok(), "relative nested new file should validate: {:?}", result);

        cleanup(&base);
    }

    #[test]
    fn new_file_deeply_nested_outside_sandbox_rejected() {
        let (base, sandbox) = setup_sandbox("newfile-deep-outside");
        let sb = WriteSandbox::new(&sandbox);

        // A deeply nested path whose first existing ancestor is outside the sandbox.
        let outside = base.join("outside").join("deep").join("file.txt");
        let result = sb.validate(outside.to_str().unwrap());
        assert!(result.is_err(), "deeply nested path outside sandbox should be rejected");

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

    #[cfg(unix)]
    #[test]
    fn validate_returns_symlink_target_root_for_symlinked_path() {
        let (base, sandbox) = setup_sandbox("symlink-root");

        // Create an external directory and symlink it into the sandbox.
        let external = base.join("external-repo");
        fs::create_dir_all(&external).expect("create external");
        let link = sandbox.join("repo");
        symlink(&external, &link).expect("create symlink");

        let sb = WriteSandbox::new(&sandbox);
        let sandbox_root = std::fs::canonicalize(&sandbox).expect("canonicalize sandbox");
        let external_root = std::fs::canonicalize(&external).expect("canonicalize external");

        // Write a file in the external directory.
        let file = external.join("notes.md");
        fs::write(&file, "content").expect("write");

        // Validate via the relative path through the sandbox symlink.
        // The returned canonical path must start with the EXTERNAL root, not the
        // sandbox root -- this is the value `validate_write_paths` uses to select
        // the correct CWD for the binary (the symlink target, not the sandbox dir).
        let canonical = sb.validate("repo/notes.md").expect("should validate");
        assert!(
            canonical.starts_with(&external_root),
            "canonical path {:?} should start with external root {:?}",
            canonical,
            external_root
        );
        assert!(
            !canonical.starts_with(&sandbox_root) || canonical.starts_with(&external_root),
            "canonical path {:?} should not be anchored only to the sandbox root {:?}",
            canonical,
            sandbox_root
        );

        cleanup(&base);
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

    // --- .git/ directory exclusion tests ---

    #[test]
    fn git_dir_rejects_write_to_git_refs() {
        let (base, sandbox) = setup_sandbox("git-refs");
        let git_dir = sandbox.join("project").join(".git").join("refs").join("heads");
        fs::create_dir_all(&git_dir).expect("create .git/refs/heads");
        let ref_file = git_dir.join("main");
        fs::write(&ref_file, "abc123").expect("write ref");

        let sb = WriteSandbox::new(&sandbox);
        let result = sb.validate(ref_file.to_str().unwrap());
        let err = result.expect_err("write to .git/refs/heads/main should be rejected");
        assert!(err.contains(".git"), "error should mention .git: {}", err);

        cleanup(&base);
    }

    #[test]
    fn git_dir_rejects_write_to_git_config() {
        let (base, sandbox) = setup_sandbox("git-config");
        let git_dir = sandbox.join("project").join(".git");
        fs::create_dir_all(&git_dir).expect("create .git");
        let config = git_dir.join("config");
        fs::write(&config, "[core]").expect("write config");

        let sb = WriteSandbox::new(&sandbox);
        let result = sb.validate(config.to_str().unwrap());
        assert!(result.is_err(), "write to .git/config should be rejected");
        assert!(result.unwrap_err().contains(".git"), "error should mention .git");

        cleanup(&base);
    }

    #[test]
    fn git_dir_rejects_write_to_git_hooks() {
        let (base, sandbox) = setup_sandbox("git-hooks");
        let hooks_dir = sandbox.join("project").join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).expect("create .git/hooks");
        let hook = hooks_dir.join("pre-commit");
        fs::write(&hook, "#!/bin/sh").expect("write hook");

        let sb = WriteSandbox::new(&sandbox);
        let result = sb.validate(hook.to_str().unwrap());
        assert!(result.is_err(), "write to .git/hooks/pre-commit should be rejected");

        cleanup(&base);
    }

    #[test]
    fn git_dir_rejects_write_to_git_dir_itself() {
        let (base, sandbox) = setup_sandbox("git-dir-itself");
        let git_dir = sandbox.join("project").join(".git");
        fs::create_dir_all(&git_dir).expect("create .git");

        let sb = WriteSandbox::new(&sandbox);
        let result = sb.validate(git_dir.to_str().unwrap());
        assert!(result.is_err(), "write to .git directory itself should be rejected");

        cleanup(&base);
    }

    #[test]
    fn git_dir_rejects_new_file_in_git_dir() {
        let (base, sandbox) = setup_sandbox("git-new-file");
        let git_dir = sandbox.join("project").join(".git");
        fs::create_dir_all(&git_dir).expect("create .git");
        // A new file that doesn't exist yet, but is inside .git/
        let new_file = git_dir.join("MERGE_MSG");

        let sb = WriteSandbox::new(&sandbox);
        let result = sb.validate(new_file.to_str().unwrap());
        assert!(result.is_err(), "write to new file in .git/ should be rejected");

        cleanup(&base);
    }

    #[test]
    fn git_dir_allows_write_outside_git_dir() {
        let (base, sandbox) = setup_sandbox("git-outside");
        let project = sandbox.join("project");
        let git_dir = project.join(".git");
        fs::create_dir_all(&git_dir).expect("create .git");
        let src_file = project.join("src").join("main.rs");
        fs::create_dir_all(src_file.parent().unwrap()).expect("create src");
        fs::write(&src_file, "fn main() {}").expect("write");

        let sb = WriteSandbox::new(&sandbox);
        let result = sb.validate(src_file.to_str().unwrap());
        assert!(result.is_ok(), "write to file outside .git/ should be allowed: {:?}", result);

        cleanup(&base);
    }

    #[test]
    fn path_intersects_git_dir_returns_true_for_git_component() {
        assert!(path_intersects_git_dir(Path::new("/home/user/project/.git")));
        assert!(path_intersects_git_dir(Path::new("/home/user/project/.git/refs/heads/main")));
        assert!(path_intersects_git_dir(Path::new("/home/user/project/.git/config")));
    }

    #[test]
    fn path_intersects_git_dir_returns_false_for_no_git_component() {
        assert!(!path_intersects_git_dir(Path::new("/home/user/project/src/main.rs")));
        assert!(!path_intersects_git_dir(Path::new("/home/user/project/README.md")));
        assert!(!path_intersects_git_dir(Path::new("/tmp/sandbox")));
    }

    #[test]
    fn path_intersects_git_dir_does_not_match_gitignore() {
        // ".gitignore" contains ".git" as a prefix but is NOT a .git directory
        assert!(!path_intersects_git_dir(Path::new("/home/user/project/.gitignore")));
        assert!(!path_intersects_git_dir(Path::new("/home/user/project/.gitmodules")));
    }
}

#[cfg(test)]
#[cfg(unix)]
mod collect_output_tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Output;

    fn make_output(stdout: &str, stderr: &str, code: i32) -> Output {
        Output {
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
            status: ExitStatusExt::from_raw(code << 8),
        }
    }

    #[test]
    fn success_exit_0_returns_stdout() {
        let output = make_output("hello", "", 0);
        let result = Allowlist::collect_output_with_codes(output, &[]);
        assert_eq!(result, Ok((0, "hello".to_string())));
    }

    #[test]
    fn error_exit_includes_stderr() {
        let output = make_output("", "fatal: error", 1);
        let result = Allowlist::collect_output_with_codes(output, &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("fatal: error"));
    }

    #[test]
    fn success_exit_code_returns_stdout_and_stderr() {
        // When exit code 1 is in successExitCodes, output should be Ok
        // with both stdout and stderr combined.
        let output = make_output("On branch dev\nnothing to commit", "fatal: error details", 1);
        let result = Allowlist::collect_output_with_codes(output, &[1]);
        assert!(result.is_ok());
        let (code, text) = result.unwrap();
        assert_eq!(code, 1);
        assert!(text.contains("nothing to commit"), "should include stdout");
        assert!(text.contains("fatal: error details"), "should include stderr");
    }

    #[test]
    fn success_exit_code_empty_stderr_returns_stdout_only() {
        let output = make_output("matches found", "", 1);
        let result = Allowlist::collect_output_with_codes(output, &[1]);
        assert_eq!(result, Ok((1, "matches found".to_string())));
    }

    #[test]
    fn success_exit_code_empty_stdout_returns_stderr() {
        // git writes "not a git repository" to stderr, stdout is empty.
        let output = make_output("", "fatal: not a git repository", 128);
        let result = Allowlist::collect_output_with_codes(output, &[128]);
        assert!(result.is_ok());
        let (code, text) = result.unwrap();
        assert_eq!(code, 128);
        assert!(text.contains("not a git repository"), "should include stderr");
    }

    #[test]
    fn exit_code_not_in_success_list_still_errors() {
        let output = make_output("", "fatal: error", 2);
        let result = Allowlist::collect_output_with_codes(output, &[1]);
        assert!(result.is_err());
    }
}
