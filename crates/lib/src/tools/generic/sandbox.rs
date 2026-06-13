//! Sandbox path validation for the generic tool executor.
//!
//! Validates `readPath`- and `writePath`-annotated arguments against the
//! per-profile write sandbox, resolves canonical paths through symlinks,
//! substitutes canonical paths into the tool call JSON, creates parent
//! directories for write targets, and applies a runtime path-like value
//! check (absolute paths, home-relative paths, `file://` URLs, and
//! directory traversal) to unannotated `positional` and `flag` parameters.

use std::collections::HashMap;
use std::path::Path;

use crate::exec::{Allowlist, WriteSandbox};
use crate::skills::{ArgKind, ExecutionSpec};

use super::argv::{json_value_to_string, transform_param_value};

/// Check whether a value looks like a filesystem path that could escape
/// the sandbox. Returns an error if the value matches a path-like pattern
/// and the parameter is not annotated as a path parameter.
///
/// The check rejects:
/// - Absolute paths starting with `/`
/// - Home-relative paths starting with `~`
/// - Paths containing `..` as a path component (directory traversal)
/// - `file://` URLs (local filesystem access via URL scheme)
///
/// Values that don't match these patterns pass through: simple names,
/// URLs (http/https/ssh), patterns, numbers, relative subpaths without
/// traversal, and comment-like values starting with `//` or `///`.
fn check_path_like_value(param: &str, value: &str) -> Result<(), String> {
    // Absolute path — reject single-slash paths (e.g. /etc/passwd) but allow
    // double-slash or triple-slash prefixes which are line comments (//, ///)
    // or doc comments, not filesystem paths. No mainstream OS uses // or ///
    // as a distinct absolute path.
    if value.starts_with('/') && !value.starts_with("//") {
        return Err(format!(
            "parameter '{}' received an absolute path '{}' but is not annotated as a path parameter; add readPath, writePath, or unsafePath",
            param, value
        ));
    }
    // Home-relative path
    if value.starts_with('~') {
        return Err(format!(
            "parameter '{}' received a home-relative path '{}' but is not annotated as a path parameter; add readPath, writePath, or unsafePath",
            param, value
        ));
    }
    // file:// URL — local filesystem access via URL scheme
    if value.starts_with("file://") {
        return Err(format!(
            "parameter '{}' received a file:// URL '{}' but is not annotated as a path parameter; add readPath, writePath, or unsafePath",
            param, value
        ));
    }
    // Traversal — `..` as a path component
    for component in value.split(|c: char| c == '/' || c == '\\') {
        if component == ".." {
            return Err(format!(
                "parameter '{}' received a path with '..' traversal '{}' but is not annotated as a path parameter; add readPath, writePath, or unsafePath",
                param, value
            ));
        }
    }
    Ok(())
}

/// Validate `readPath`- and `writePath`-annotated arguments against the
/// write sandbox, and apply the runtime path-like value check to unannotated
/// `positional` and `flag` parameters. Returns the resolved working directory
/// (if any sandboxed path was found) and a map of parameter names to their
/// canonical paths.
pub(crate) fn validate_write_paths(
    spec: &ExecutionSpec,
    args: &serde_json::Value,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
    sandbox: &Option<WriteSandbox>,
) -> Result<(Option<std::path::PathBuf>, HashMap<String, String>), String> {
    let obj = match args.as_object() {
        Some(o) => o,
        None => return Ok((None, HashMap::new())),
    };

    let mut has_sandboxed_path = false;
    let mut matched_root: Option<std::path::PathBuf> = None;
    let mut canonical_paths: HashMap<String, String> = HashMap::new();
    // When a workingDir arg is validated, its canonical path becomes the
    // working directory directly (not the sandbox root), because the path
    // is NOT passed to argv — the process CWD is the only way git knows
    // which repository to operate on.
    let mut working_dir_arg: Option<std::path::PathBuf> = None;

    for arg in &spec.args {
        let is_write = arg.write_path == Some(true);
        // workingDir args implicitly act as readPath for sandbox validation
        // and working directory resolution.
        let is_read = arg.read_path == Some(true) || arg.kind == ArgKind::WorkingDir;
        let is_unsafe = arg.unsafe_path == Some(true);

        // --- Runtime path-like value check for unannotated parameters ---
        // Apply to positional and flag args that have no path annotation
        // and no unsafePath.
        if !is_write && !is_read && !is_unsafe {
            if arg.kind == ArgKind::Positional || arg.kind == ArgKind::Flag {
                if let Some(value) = obj.get(&arg.param).and_then(|v| v.as_str()) {
                    if !value.is_empty() {
                        check_path_like_value(&arg.param, value)?;
                    }
                }
            }
            continue;
        }

        // unsafePath skips sandbox validation entirely
        if is_unsafe {
            continue;
        }

        if !is_write && !is_read {
            continue;
        }
        let kind_label = if is_write { "writePath" } else { "readPath" };

        let sandbox = sandbox.as_ref().ok_or_else(|| {
            format!(
                "tool {} has {} parameter '{}' but no write sandbox is configured",
                spec.tool, kind_label, arg.param
            )
        })?;

        let raw_value = match arg.kind {
            ArgKind::Positional => match obj.get(&arg.param) {
                Some(v) if !v.is_null() => json_value_to_string(v)
                    .ok_or_else(|| format!("{} parameter {} must be a string", kind_label, arg.param))?,
                _ => {
                    if arg.optional == Some(true) && arg.resolve_command.is_some() {
                        String::new()
                    } else if arg.optional == Some(true) {
                        continue
                    } else {
                        return Err(format!("missing {} parameter: {}", kind_label, arg.param));
                    }
                }
            },
            ArgKind::Flag => {
                match obj.get(&arg.param) {
                    Some(v) if !v.is_null() => json_value_to_string(v).ok_or_else(|| {
                        format!("{} parameter {} must be a string", kind_label, arg.param)
                    })?,
                    _ => {
                        if arg.optional == Some(true) && arg.resolve_command.is_some() {
                            String::new()
                        } else {
                            continue;
                        }
                    }
                }
            }
            ArgKind::FlagIfBoolean => continue,
            ArgKind::Stdin => continue,
            ArgKind::TempFile => continue,
            ArgKind::WorkingDir => {
                match obj.get(&arg.param) {
                    Some(v) if !v.is_null() => json_value_to_string(v).ok_or_else(|| {
                        format!("{} parameter {} must be a string", kind_label, arg.param)
                    })?,
                    _ => {
                        if arg.optional == Some(true) && arg.resolve_command.is_some() {
                            String::new()
                        } else {
                            continue;
                        }
                    }
                }
            }
        };

        has_sandboxed_path = true;

        let resolved = transform_param_value(raw_value, arg, allowlist, skill_dir, args);

        if resolved.is_empty() {
            continue;
        }

        let canonical = sandbox.validate(&resolved).map_err(|e| {
            if is_read {
                e.replace("write path outside sandbox", "read path outside sandbox")
            } else {
                e
            }
        })?;

        // For workingDir args, the canonical path IS the working directory.
        if arg.kind == ArgKind::WorkingDir {
            working_dir_arg = Some(canonical.clone());
        }

        canonical_paths.insert(
            arg.param.clone(),
            canonical.to_string_lossy().into_owned(),
        );

        if matched_root.is_none() {
            if let Some(root) = sandbox.roots().iter().find(|r| canonical.starts_with(*r)) {
                matched_root = Some(root.clone());
            }
        }
    }

    if has_sandboxed_path {
        // workingDir takes precedence — the process CWD should be the
        // resolved directory itself, not the sandbox root.
        if let Some(ref dir) = working_dir_arg {
            return Ok((Some(dir.clone()), canonical_paths));
        }
        if let Some(root) = matched_root {
            return Ok((Some(root), canonical_paths));
        }
        if let Some(ref sb) = sandbox {
            if let Some(root) = sb.roots().first() {
                return Ok((Some(root.clone()), canonical_paths));
            }
        }
    }

    Ok((None, canonical_paths))
}

/// Create parent directories for all `writePath`-annotated arguments whose
/// canonical paths have been resolved.
pub(crate) fn ensure_write_path_parents(
    spec: &ExecutionSpec,
    canonical_paths: &HashMap<String, String>,
) -> Result<(), String> {
    for arg in &spec.args {
        if arg.write_path != Some(true) {
            continue;
        }
        let canonical = match canonical_paths.get(&arg.param) {
            Some(p) => std::path::Path::new(p),
            None => continue,
        };
        if let Some(parent) = canonical.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "failed to create parent directory {}: {}",
                        parent.display(),
                        e
                    )
                })?;
            }
        }
    }
    Ok(())
}

/// Replace parameter values in the tool call JSON with their canonical paths.
pub(crate) fn substitute_canonical_paths(
    args: &serde_json::Value,
    canonical_paths: &HashMap<String, String>,
) -> serde_json::Value {
    let mut patched = args.clone();
    if let Some(obj) = patched.as_object_mut() {
        for (param, canonical) in canonical_paths {
            obj.insert(param.clone(), serde_json::Value::String(canonical.clone()));
        }
    }
    patched
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::WriteSandbox;
    use crate::skills::{ArgMapping, ExecutionSpec};
    use std::fs;
    use std::path::PathBuf;

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("chai-generic-test-{}-{}", name, std::process::id()))
    }

    // --- check_path_like_value tests ---

    #[test]
    fn check_path_like_rejects_absolute_path() {
        assert!(check_path_like_value("p", "/etc/passwd").is_err());
    }

    #[test]
    fn check_path_like_rejects_home_relative_path() {
        assert!(check_path_like_value("p", "~/.ssh/id_rsa").is_err());
    }

    #[test]
    fn check_path_like_rejects_traversal() {
        assert!(check_path_like_value("p", "../../etc/passwd").is_err());
    }

    #[test]
    fn check_path_like_rejects_traversal_with_prefix() {
        assert!(check_path_like_value("p", "./../../etc").is_err());
    }

    #[test]
    fn check_path_like_rejects_mid_traversal() {
        assert!(check_path_like_value("p", "foo/../bar").is_err());
    }

    #[test]
    fn check_path_like_allows_simple_name() {
        assert!(check_path_like_value("p", "my-skill").is_ok());
    }

    #[test]
    fn check_path_like_allows_git_ref() {
        assert!(check_path_like_value("p", "HEAD").is_ok());
    }

    #[test]
    fn check_path_like_allows_branch_name() {
        assert!(check_path_like_value("p", "main").is_ok());
    }

    #[test]
    fn check_path_like_allows_url() {
        assert!(check_path_like_value("p", "https://example.com").is_ok());
    }

    #[test]
    fn check_path_like_rejects_file_url() {
        assert!(check_path_like_value("p", "file:///etc/passwd").is_err());
    }

    #[test]
    fn check_path_like_file_url_error_message() {
        let err = check_path_like_value("target", "file:///home/user/.ssh/id_rsa").unwrap_err();
        assert!(err.contains("file://"), "error should mention file://: {}", err);
        assert!(err.contains("target"), "error should mention param name: {}", err);
    }

    #[test]
    fn check_path_like_allows_ssh_url() {
        assert!(check_path_like_value("p", "ssh://git@github.com/user/repo.git").is_ok());
    }

    #[test]
    fn check_path_like_allows_pattern() {
        assert!(check_path_like_value("p", "TODO|FIXME").is_ok());
    }

    #[test]
    fn check_path_like_allows_number() {
        assert!(check_path_like_value("p", "10").is_ok());
    }

    #[test]
    fn check_path_like_allows_relative_subpath() {
        assert!(check_path_like_value("p", "src/main.rs").is_ok());
    }

    #[test]
    fn check_path_like_allows_nested_relative_subpath() {
        assert!(check_path_like_value("p", "chai/crates/lib/src/exec.rs").is_ok());
    }

    #[test]
    fn check_path_like_allows_empty_string() {
        // Empty strings are not checked (handled separately)
        assert!(check_path_like_value("p", "").is_ok());
    }

    // --- double-slash and triple-slash comment patterns ---

    #[test]
    fn check_path_like_allows_double_slash_comment() {
        // `//` is a line comment in C/C++/Java/JS/Rust, not an absolute path
        assert!(check_path_like_value("pattern", "// TODO: fix this").is_ok());
    }

    #[test]
    fn check_path_like_allows_triple_slash_doc_comment() {
        // `///` is a Rust doc comment, not an absolute path
        assert!(check_path_like_value("pattern", "/// WebSocket event name").is_ok());
    }

    #[test]
    fn check_path_like_allows_double_slash_at_line_start() {
        // Multi-line pattern starting with // (common in source code)
        assert!(check_path_like_value("pattern", "// comment\nfn main()").is_ok());
    }

    #[test]
    fn check_path_like_rejects_single_slash_absolute_path() {
        // Single-slash absolute paths should still be rejected
        assert!(check_path_like_value("p", "/etc/passwd").is_err());
    }

    #[test]
    fn check_path_like_rejects_single_slash_root() {
        // Bare `/` should still be rejected
        assert!(check_path_like_value("p", "/").is_err());
    }

    // --- validate_write_paths: runtime path-like check integration ---

    #[test]
    fn validate_write_paths_rejects_path_like_value_in_unannotated_positional() {
        let base = test_dir("vwp-pathlike-pos");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");

        let sb = WriteSandbox::new(&sandbox_dir);
        let sandbox = Some(sb);

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "cat".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "target".to_string(),
                kind: ArgKind::Positional,
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "target": "/etc/passwd" });
        let allowlist = Allowlist::new();

        let result = validate_write_paths(&spec, &args, &allowlist, None, &sandbox);
        assert!(result.is_err(), "absolute path in unannotated positional should be rejected");
        let err = result.unwrap_err();
        assert!(err.contains("absolute path"), "error should mention absolute path: {}", err);
        assert!(err.contains("readPath"), "error should suggest readPath: {}", err);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn validate_write_paths_rejects_traversal_in_unannotated_flag() {
        let base = test_dir("vwp-pathlike-flag");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");

        let sb = WriteSandbox::new(&sandbox_dir);
        let sandbox = Some(sb);

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "cat".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "name".to_string(),
                kind: ArgKind::Flag,
                flag: Some("name".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "name": "../../etc/passwd" });
        let allowlist = Allowlist::new();

        let result = validate_write_paths(&spec, &args, &allowlist, None, &sandbox);
        assert!(result.is_err(), "traversal in unannotated flag should be rejected");
        let err = result.unwrap_err();
        assert!(err.contains(".."), "error should mention traversal: {}", err);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn validate_write_paths_allows_non_path_value_in_unannotated_positional() {
        let base = test_dir("vwp-nonpath-pos");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");

        let sb = WriteSandbox::new(&sandbox_dir);
        let sandbox = Some(sb);

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "chai".to_string(),
            subcommand: "skill list".to_string(),
            args: vec![ArgMapping {
                param: "skill_name".to_string(),
                kind: ArgKind::Positional,
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "skill_name": "my-skill" });
        let allowlist = Allowlist::new();

        let result = validate_write_paths(&spec, &args, &allowlist, None, &sandbox);
        assert!(result.is_ok(), "simple name in unannotated positional should pass: {:?}", result);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn validate_write_paths_unsafe_path_skips_heuristic() {
        let base = test_dir("vwp-unsafepath");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");

        let sb = WriteSandbox::new(&sandbox_dir);
        let sandbox = Some(sb);

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "cat".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "target".to_string(),
                kind: ArgKind::Positional,
                unsafe_path: Some(true),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "target": "/etc/passwd" });
        let allowlist = Allowlist::new();

        let result = validate_write_paths(&spec, &args, &allowlist, None, &sandbox);
        assert!(result.is_ok(), "unsafePath should skip the heuristic check: {:?}", result);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn validate_write_paths_read_path_allows_absolute_value() {
        let base = test_dir("vwp-readpath-absolute");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        let external = base.join("external");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");
        fs::create_dir_all(&external).expect("create external");

        let sb = WriteSandbox::new(&sandbox_dir);
        let sandbox = Some(sb);

        let external_canonical = fs::canonicalize(&external).expect("canonicalize external");

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "cat".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "path".to_string(),
                kind: ArgKind::Positional,
                read_path: Some(true),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "path": external_canonical.to_string_lossy().as_ref() });
        let allowlist = Allowlist::new();

        let result = validate_write_paths(&spec, &args, &allowlist, None, &sandbox);
        // This should fail because the external dir is not a sandbox writable root
        // (no symlink from sandbox). The point is that readPath doesn't trigger
        // the heuristic — it triggers sandbox validation instead.
        assert!(result.is_err(), "readPath should trigger sandbox validation, not heuristic");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn validate_write_paths_flag_if_boolean_not_checked() {
        let base = test_dir("vwp-flagifbool");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");

        let sb = WriteSandbox::new(&sandbox_dir);
        let sandbox = Some(sb);

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "ls".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "all".to_string(),
                kind: ArgKind::FlagIfBoolean,
                flag_if_true: Some("--all".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "all": true });
        let allowlist = Allowlist::new();

        let result = validate_write_paths(&spec, &args, &allowlist, None, &sandbox);
        assert!(result.is_ok(), "flagIfBoolean should not be checked");

        let _ = fs::remove_dir_all(&base);
    }

    // --- substitute_canonical_paths tests ---

    #[test]
    fn substitute_canonical_paths_replaces_named_params() {
        let args = serde_json::json!({ "path": "./chai", "other": "unchanged" });
        let mut overrides = HashMap::new();
        overrides.insert("path".to_string(), "/resolved/chai".to_string());

        let patched = substitute_canonical_paths(&args, &overrides);
        assert_eq!(patched["path"], "/resolved/chai");
        assert_eq!(patched["other"], "unchanged");
    }

    #[test]
    fn substitute_canonical_paths_leaves_non_object_untouched() {
        let args = serde_json::json!("not an object");
        let mut overrides = HashMap::new();
        overrides.insert("path".to_string(), "/resolved".to_string());

        let patched = substitute_canonical_paths(&args, &overrides);
        assert_eq!(patched, serde_json::json!("not an object"));
    }

    #[test]
    fn substitute_canonical_paths_no_overrides_returns_clone() {
        let args = serde_json::json!({ "path": "./chai" });
        let patched = substitute_canonical_paths(&args, &HashMap::new());
        assert_eq!(patched["path"], "./chai");
    }

    // --- validate_write_paths canonical path return tests ---

    #[cfg(unix)]
    #[test]
    fn validate_write_paths_returns_canonical_for_symlinked_read_path() {
        use std::os::unix::fs::symlink;

        let base = test_dir("vwp-symlink");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        let external = base.join("external");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");
        fs::create_dir_all(&external).expect("create external");

        let link = sandbox_dir.join("myrepo");
        symlink(&external, &link).expect("create symlink");

        let sb = WriteSandbox::new(&sandbox_dir);
        let sandbox = Some(sb);

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "ls".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "path".to_string(),
                kind: ArgKind::Positional,
                read_path: Some(true),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "path": "myrepo" });
        let allowlist = Allowlist::new();

        let (working_dir, canonical_paths) =
            validate_write_paths(&spec, &args, &allowlist, None, &sandbox)
                .expect("validation should succeed");

        let external_canonical = fs::canonicalize(&external).expect("canonicalize external");
        assert_eq!(working_dir.as_deref(), Some(external_canonical.as_path()));

        let resolved = canonical_paths.get("path").expect("path should be in canonical map");
        assert_eq!(
            std::path::Path::new(resolved),
            external_canonical.as_path(),
            "canonical path should resolve through the symlink"
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn validate_write_paths_canonical_path_is_substituted_into_argv() {
        use super::super::argv::build_argv;
        use std::os::unix::fs::symlink;

        let base = test_dir("vwp-argv-sub");
        let _ = fs::remove_dir_all(&base);
        let sandbox_dir = base.join("sandbox");
        let external = base.join("external");
        fs::create_dir_all(&sandbox_dir).expect("create sandbox");
        fs::create_dir_all(&external).expect("create external");

        let link = sandbox_dir.join("myrepo");
        symlink(&external, &link).expect("create symlink");

        let sb = WriteSandbox::new(&sandbox_dir);

        let spec = ExecutionSpec {
            tool: "test_tool".to_string(),
            binary: "ls".to_string(),
            subcommand: "".to_string(),
            args: vec![ArgMapping {
                param: "path".to_string(),
                kind: ArgKind::Positional,
                read_path: Some(true),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = serde_json::json!({ "path": "myrepo" });
        let allowlist = Allowlist::new();

        let (_, canonical_paths) =
            validate_write_paths(&spec, &args, &allowlist, None, &Some(sb))
                .expect("validation should succeed");

        let resolved_args = substitute_canonical_paths(&args, &canonical_paths);
        let argv = build_argv(&spec, &resolved_args, &allowlist, None)
            .expect("build_argv should succeed");

        assert_eq!(argv.len(), 1);
        let external_canonical = fs::canonicalize(&external).expect("canonicalize external");
        assert_eq!(
            argv[0],
            external_canonical.to_string_lossy().as_ref(),
            "argv should contain the resolved absolute path"
        );

        let _ = fs::remove_dir_all(&base);
    }
}
