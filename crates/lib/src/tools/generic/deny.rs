//! Deny pattern enforcement for the generic tool executor.
//!
//! Checks tool call parameters against regex deny patterns before execution.
//! Supports `denyResolveCommand` to resolve the effective value when the
//! parameter is absent, and `denyAlwaysResolve` to always use the resolve
//! command regardless of whether the parameter has a raw value.

use std::path::Path;

use crate::exec::Allowlist;
use crate::skills::{ArgKind, ExecutionSpec};

use super::argv::{json_value_to_string, run_script};

/// Check deny patterns on all args that have them. For each arg with a
/// `denyPattern`, resolve the effective value (using the explicit param value
/// if present, or `denyResolveCommand` if the param is absent/empty), then
/// check it against the regex pattern. If the value matches, the operation is
/// rejected with an error.
pub(crate) fn enforce_deny_patterns(
    spec: &ExecutionSpec,
    args: &serde_json::Value,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
    working_dir: Option<&Path>,
) -> Result<(), String> {
    let obj = match args.as_object() {
        Some(o) => o,
        None => return Ok(()),
    };

    for arg in &spec.args {
        let Some(ref deny_pattern) = arg.deny_pattern else {
            continue;
        };

        let always_resolve = arg.deny_always_resolve == Some(true);

        // Get the raw param value from the tool call JSON.
        let raw_value = match arg.kind {
            ArgKind::Positional | ArgKind::Flag | ArgKind::WorkingDir => {
                match obj.get(&arg.param) {
                    Some(v) if !v.is_null() => json_value_to_string(v),
                    _ => None,
                }
            }
            ArgKind::FlagIfBoolean | ArgKind::Stdin | ArgKind::TempFile => None,
        };

        // Determine the effective value to check against the deny pattern.
        //
        // Two modes:
        // 1. denyAlwaysResolve=true: The denyResolveCommand always provides
        //    the value to check. The raw param value is NOT the thing being
        //    denied — it's context (e.g., a working directory path, but the
        //    deny pattern checks the current branch name within that dir).
        // 2. denyAlwaysResolve=false (default): The raw param value is
        //    checked directly when present. The denyResolveCommand is only
        //    invoked when the parameter is absent or empty (e.g., branch
        //    is omitted from git push, so resolve the current branch).
        let effective_value = if always_resolve {
            if let Some(ref deny_cmd) = arg.deny_resolve_command {
                resolve_deny_value(deny_cmd, allowlist, skill_dir, working_dir)?
            } else {
                return Err(format!(
                    "denyAlwaysResolve is set on param '{}' but no denyResolveCommand is configured",
                    arg.param
                ));
            }
        } else if let Some(ref val) = raw_value {
            if !val.is_empty() {
                val.clone()
            } else if let Some(ref deny_cmd) = arg.deny_resolve_command {
                resolve_deny_value(deny_cmd, allowlist, skill_dir, working_dir)?
            } else {
                continue // Empty value, no denyResolveCommand — skip.
            }
        } else if let Some(ref deny_cmd) = arg.deny_resolve_command {
            resolve_deny_value(deny_cmd, allowlist, skill_dir, working_dir)?
        } else {
            continue // Param absent, no denyResolveCommand — skip.
        };

        // Check the effective value against the deny pattern.
        let re = regex::Regex::new(deny_pattern).map_err(|e| {
            format!("invalid denyPattern '{}' on param '{}': {}", deny_pattern, arg.param, e)
        })?;

        if re.is_match(&effective_value) {
            return Err(format!(
                "protected value on param '{}': '{}' matches deny pattern '{}'",
                arg.param, effective_value, deny_pattern
            ));
        }
    }

    Ok(())
}

/// Resolve the effective value for deny pattern checking using a
/// `denyResolveCommand`. Runs the script or allowlisted command and returns
/// the trimmed stdout. The working directory is passed as an argument to the
/// script so it can resolve context-dependent values (e.g., current git
/// branch).
fn resolve_deny_value(
    cmd: &crate::skills::ResolveCommandSpec,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
    working_dir: Option<&Path>,
) -> Result<String, String> {
    // Substitute $workingDir in args with the resolved working directory.
    let argv: Vec<String> = cmd
        .args
        .iter()
        .map(|a| {
            if a == "$workingDir" {
                working_dir
                    .map(|d| d.to_string_lossy().into_owned())
                    .unwrap_or_default()
            } else {
                a.clone()
            }
        })
        .collect();

    if let (Some(dir), Some(ref script_name)) = (skill_dir, &cmd.script) {
        if let Ok(out) = run_script(dir, script_name, &argv) {
            let s = out.trim().to_string();
            return if s.is_empty() {
                Err("denyResolveCommand returned empty value".to_string())
            } else {
                Ok(s)
            };
        }
        return Err("denyResolveCommand script failed".to_string());
    }

    if let (Some(ref binary), Some(ref subcommand)) = (&cmd.binary, &cmd.subcommand) {
        match allowlist.run(binary, subcommand, &argv, working_dir) {
            Ok(out) => {
                let s = out.trim().to_string();
                return if s.is_empty() {
                    Err("denyResolveCommand returned empty value".to_string())
                } else {
                    Ok(s)
                };
            }
            Err(e) => return Err(format!("denyResolveCommand failed: {}", e)),
        }
    }

    Err("denyResolveCommand has no script or binary/subcommand".to_string())
}
