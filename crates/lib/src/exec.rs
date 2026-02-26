//! Safe execution layer: run allowlisted binaries with allowlisted subcommands only.
//! No shell is used; arguments are passed as a list to avoid injection.

use std::collections::HashMap;
use std::process::Command;

/// Allowlist: binary name -> set of allowed subcommands (e.g. "obsidian" -> ["search", "create", ...]).
#[derive(Debug, Clone, Default)]
pub struct Allowlist {
    /// Binary name (e.g. "obsidian") -> allowed subcommands.
    bins: HashMap<String, Vec<String>>,
}

impl Allowlist {
    pub fn new() -> Self {
        Self {
            bins: HashMap::new(),
        }
    }

    /// Allow a binary to run only the given subcommands (e.g. "obsidian" and ["search", "create", "move", "delete"]).
    pub fn allow(&mut self, binary: impl Into<String>, subcommands: Vec<&'static str>) {
        self.bins.insert(
            binary.into(),
            subcommands.into_iter().map(String::from).collect(),
        );
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

/// Build the allowlist for the official Obsidian CLI (early access; binary `obsidian`): search, search:context, create only.
pub fn obsidian_allowlist() -> Allowlist {
    let mut a = Allowlist::new();
    a.allow("obsidian", vec!["search", "search:context", "create"]);
    a
}

/// Build the allowlist for `notesmd-cli`: search, search-content, create, daily, print, print-default only.
pub fn notesmd_cli_allowlist() -> Allowlist {
    let mut a = Allowlist::new();
    a.allow(
        "notesmd-cli",
        vec![
            "search",
            "search-content",
            "create",
            "daily",
            "print",
            "print-default",
        ],
    );
    a
}
