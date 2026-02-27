//! Safe execution layer: run allowlisted binaries with allowlisted subcommands only.
//! No shell is used; arguments are passed as a list to avoid injection.

use std::collections::HashMap;
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

    /// Run `binary subcommand args...` if allowed. Returns combined stdout; on failure stderr is included in the error.
    pub fn run(
        &self,
        binary: &str,
        subcommand: &str,
        args: &[String],
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
        let output = Command::new(binary)
            .arg(subcommand)
            .args(args)
            .output()
            .map_err(|e| format!("exec failed: {}", e))?;
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

