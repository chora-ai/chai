//! Generic tool executor driven by a skill's tools.json descriptor.
//! Builds argv from the execution spec's arg mapping and runs via the allowlist.
//! Supports resolve-by-command or resolve-by-script for param resolution,
//! sandbox-validated path enforcement (readPath/writePath), runtime path-like
//! value checking (absolute paths, home-relative paths, `file://` URLs,
//! directory traversal) for unannotated parameters, default CWD confinement
//! to the sandbox root, and side-read augmentation (append a nearby file's
//! contents to the tool result).

mod argv;
mod deny;
mod output;
mod sandbox;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::agent::ToolExecutor;
use crate::exec::{Allowlist, WriteSandbox};
use crate::skills::ToolDescriptor;
use crate::tools::post_process::run_post_process;

/// Executes tools using a descriptor's allowlist and execution mapping.
/// Holds per-tool (allowlist, spec, skill_dir) for param resolution and argv building.
/// When a `WriteSandbox` is present, validates `readPath`- and `writePath`-annotated
/// arguments against writable roots before executing. When a tool has a `sideRead`
/// spec, the named file is appended to the tool result when found; `oncePerSession`
/// prevents re-appending the same file within the same session.
#[derive(Debug, Clone)]
pub struct GenericToolExecutor {
    /// tool_name -> (allowlist, execution spec, skill_dir for script resolution)
    map: HashMap<String, (Allowlist, crate::skills::ExecutionSpec, Option<std::path::PathBuf>)>,
    /// Optional per-profile write sandbox for path boundary enforcement.
    sandbox: Option<WriteSandbox>,
    /// session_id -> set of "path/filename" keys already surfaced by sideRead
    /// this session. Shared via Arc so clones of the executor share state.
    /// Grows monotonically; no eviction (sessions are few relative to memory).
    side_read_seen: Arc<Mutex<HashMap<String, HashSet<String>>>>,
}

impl GenericToolExecutor {
    /// Build an executor from skill descriptors and optional skill dirs.
    /// `resolveCommand.script` in tools.json runs the named script from the skill's `scripts/` directory.
    /// When `sandbox` is provided, `writePath`-annotated arguments are validated before execution.
    pub fn from_descriptors(
        descriptors: &[(String, ToolDescriptor)],
        skill_dirs: &[(String, std::path::PathBuf)],
        sandbox: Option<WriteSandbox>,
    ) -> Self {
        let dir_map: HashMap<&String, &std::path::PathBuf> =
            skill_dirs.iter().map(|(n, p)| (n, p)).collect();
        let mut map = HashMap::new();
        for (skill_name, desc) in descriptors {
            let allowlist = desc.to_allowlist();
            let skill_dir = dir_map.get(skill_name).cloned().cloned();
            for spec in &desc.execution {
                map.insert(
                    spec.tool.clone(),
                    (allowlist.clone(), spec.clone(), skill_dir.clone()),
                );
            }
        }
        Self {
            map,
            sandbox,
            side_read_seen: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Return true if this executor handles the given tool name.
    pub fn has_tool(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    /// Tool names this executor can run.
    pub fn tool_names(&self) -> impl Iterator<Item = &String> {
        self.map.keys()
    }
}

impl ToolExecutor for GenericToolExecutor {
    fn execute(&self, name: &str, args: &serde_json::Value, session_id: Option<&str>) -> Result<String, String> {
        let (allowlist, spec, skill_dir) = self
            .map
            .get(name)
            .ok_or_else(|| format!("unknown tool: {}", name))?;

        let (working_dir, canonical_paths) =
            sandbox::validate_write_paths(spec, args, allowlist, skill_dir.as_deref(), &self.sandbox)?;

        // Default CWD to sandbox root when no working directory was determined
        // from path-annotated parameters. This ensures that relative paths in
        // unannotated parameters resolve within the sandbox boundary rather than
        // relative to the gateway's launch directory.
        let working_dir = match (working_dir, &self.sandbox) {
            (Some(dir), _) => Some(dir),
            (None, Some(sb)) => sb.roots().first().cloned(),
            (None, None) => None,
        };

        let resolved_args;
        let effective_args = if canonical_paths.is_empty() {
            args
        } else {
            resolved_args = sandbox::substitute_canonical_paths(args, &canonical_paths);
            &resolved_args
        };

        sandbox::ensure_write_path_parents(spec, &canonical_paths)?;

        deny::enforce_deny_patterns(spec, effective_args, allowlist, skill_dir.as_deref(), working_dir.as_deref())?;

        let mut argv = argv::build_argv(spec, effective_args, allowlist, skill_dir.as_deref())?;

        let stdin_content = argv::extract_stdin_content(spec, effective_args, allowlist, skill_dir.as_deref())?;

        let (temp_argv, temp_paths) = argv::write_temp_files(spec, effective_args, allowlist, skill_dir.as_deref())?;
        argv.extend(temp_argv);

        let success_codes = spec.success_exit_codes.as_deref().unwrap_or(&[]);
        let result = if let Some(ref content) = stdin_content {
            allowlist.run_with_stdin_with_codes(
                &spec.binary,
                &spec.subcommand,
                &argv,
                working_dir.as_deref(),
                content.as_bytes(),
                success_codes,
            )
        } else {
            allowlist.run_with_codes(
                &spec.binary,
                &spec.subcommand,
                &argv,
                working_dir.as_deref(),
                success_codes,
            )
        };

        // Clean up temp files regardless of execution success or failure.
        for path in &temp_paths {
            let _ = std::fs::remove_file(path);
        }

        let result = result?;

        let result = if let Some(ref pp) = spec.post_process {
            run_post_process(pp, &result, allowlist, skill_dir.as_deref(), effective_args)
        } else {
            result
        };

        let result = if let Some(max_lines) = spec.max_output_lines {
            output::truncate_output(&result, max_lines)
        } else {
            result
        };

        if let Some(ref sr) = spec.side_read {
            Ok(output::apply_side_read(sr, effective_args, &result, session_id, &self.side_read_seen))
        } else {
            Ok(result)
        }
    }
}
