//! Agent context loader: load AGENTS.md (and similar files) from the workspace.
//!
//! This is separate from skills. Skills describe tools; agent-ctx describes
//! overall behavior and when to use tools vs normal chat.

use std::fs;
use std::path::Path;

/// Load agent context from the workspace (e.g. AGENTS.md).
///
/// Returns the file contents when AGENTS.md exists and is non-empty; otherwise None.
pub fn load_agent_ctx(workspace_dir: Option<&Path>) -> Option<String> {
    let dir = workspace_dir?;
    let path = dir.join("AGENTS.md");
    match fs::read_to_string(&path) {
        Ok(s) if !s.trim().is_empty() => Some(s),
        _ => None,
    }
}
