//! Post-process step for tool output: run a script or allowlisted command,
//! piping the tool's stdout to the child's stdin and returning the
//! transformed output.

use std::path::Path;

use crate::exec::{resolve_binary, Allowlist};
use crate::skills::PostProcessSpec;

/// Write stdin bytes to the child's pipe, dropping the pipe before returning
/// so the child sees EOF. Returns an error if the pipe cannot be acquired
/// (should not happen when `Stdio::piped()` is set).
fn pipe_stdin(
    child: &mut std::process::Child,
    input: &[u8],
) -> Result<(), String> {
    use std::io::Write;
    {
        let mut pipe = child.stdin.take().ok_or_else(|| {
            "failed to acquire stdin pipe: Stdio::piped() was set but pipe is unavailable"
                .to_string()
        })?;
        pipe.write_all(input)
            .map_err(|e| format!("failed to write stdin: {}", e))?;
    }
    // Pipe is dropped here — child sees EOF on stdin.
    Ok(())
}

/// Substitute `$param_name` placeholders in post-process args with values from
/// the tool call JSON. Placeholders use the format `$param_name` (e.g.
/// `$root`). If the parameter is absent or null in the tool call args,
/// the placeholder is replaced with an empty string.
fn substitute_pp_args(pp_args: &[String], tool_args: &serde_json::Value) -> Vec<String> {
    let obj = match tool_args.as_object() {
        Some(o) => o,
        None => return pp_args.to_vec(),
    };
    pp_args
        .iter()
        .map(|a| {
            if a.starts_with('$') {
                let key = &a[1..];
                match obj.get(key) {
                    Some(v) if !v.is_null() => {
                        // Extract string value; for non-string types, use the
                        // JSON representation.
                        match v.as_str() {
                            Some(s) => s.to_string(),
                            None => v.to_string(),
                        }
                    }
                    _ => String::new(),
                }
            } else {
                a.clone()
            }
        })
        .collect()
}

/// Run a post-process script or command, piping `input` to its stdin.
/// Returns the script's stdout on success, or the original input on failure.
/// `exit_code` is the exit code of the main command, passed to the
/// post-process script as the `CHAI_EXIT_CODE` environment variable so that
/// scripts can make decisions based on whether the command succeeded (0)
/// or returned a non-zero code that was in `successExitCodes`.
/// `tool_args` provides parameter values for `$param_name` substitution in
/// `pp.args` (e.g. `$root` is replaced with the `root` parameter value).
pub fn run_post_process(
    pp: &PostProcessSpec,
    exit_code: i32,
    input: &str,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
    tool_args: &serde_json::Value,
) -> String {
    use std::process::Stdio;

    let resolved_args = substitute_pp_args(&pp.args, tool_args);

    // Script path: run via sh with stdin piped.
    if let (Some(dir), Some(ref script_name)) = (skill_dir, &pp.script) {
        if script_name.contains("..") || script_name.contains('/') || script_name.contains('\\') {
            return input.to_string();
        }
        let scripts_dir = Path::new(dir).join("scripts");
        let mut script_path = scripts_dir.join(script_name);
        if !script_path.starts_with(&scripts_dir) {
            return input.to_string();
        }
        if !script_path.is_file() {
            script_path = script_path.with_extension("sh");
            if !script_path.starts_with(&scripts_dir) || !script_path.is_file() {
                return input.to_string();
            }
        }

        let child = std::process::Command::new("sh")
            .env("CHAI_EXIT_CODE", exit_code.to_string())
            .arg(&script_path)
            .args(&resolved_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();

        match child {
            Ok(mut child) => {
                if let Err(e) = pipe_stdin(&mut child, input.as_bytes()) {
                    log::warn!("run_post_process: {}", e);
                }
                match child.wait_with_output() {
                    Ok(output) if output.status.success() => {
                        let s = String::from_utf8_lossy(&output.stdout).into_owned();
                        if s.is_empty() && !pp.empty_is_result.unwrap_or(false) {
                            input.to_string()
                        } else {
                            s
                        }
                    }
                    _ => input.to_string(),
                }
            }
            Err(_) => input.to_string(),
        }
    }
    // Allowlisted command path: run via allowlist with stdin piped.
    else if let (Some(ref binary), Some(ref subcommand)) = (&pp.binary, &pp.subcommand) {
        if !allowlist.is_allowed(binary, subcommand) {
            return input.to_string();
        }

        let resolved = resolve_binary(binary);
        let mut cmd = std::process::Command::new(&resolved);
        cmd.env("CHAI_EXIT_CODE", exit_code.to_string())
            .args(subcommand.split_whitespace())
            .args(&resolved_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match cmd.spawn() {
            Ok(mut child) => {
                if let Err(e) = pipe_stdin(&mut child, input.as_bytes()) {
                    log::warn!("run_post_process: {}", e);
                }
                match child.wait_with_output() {
                    Ok(output) if output.status.success() => {
                        let s = String::from_utf8_lossy(&output.stdout).into_owned();
                        if s.is_empty() && !pp.empty_is_result.unwrap_or(false) {
                            input.to_string()
                        } else {
                            s
                        }
                    }
                    _ => input.to_string(),
                }
            }
            Err(_) => input.to_string(),
        }
    } else {
        input.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("chai-pp-test-{}-{}", name, std::process::id()))
    }

    fn setup_skill_with_script(name: &str, script_name: &str, script_content: &str) -> PathBuf {
        let dir = test_dir(name);
        let _ = fs::remove_dir_all(&dir);
        let scripts_dir = dir.join("scripts");
        fs::create_dir_all(&scripts_dir).expect("create scripts dir");
        let script_path = scripts_dir.join(format!("{}.sh", script_name));
        fs::write(&script_path, script_content).expect("write script");
        dir
    }

    fn cleanup(dir: &PathBuf) {
        let _ = fs::remove_dir_all(dir);
    }

    fn empty_args() -> serde_json::Value {
        serde_json::json!({})
    }

    #[test]
    fn post_process_script_transforms_stdout() {
        let dir = setup_skill_with_script(
            "pp-basic",
            "uppercase",
            "#!/bin/sh\ntr '[:lower:]' '[:upper:]'",
        );

        let pp = PostProcessSpec {
            script: Some("uppercase".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
            empty_is_result: None,
        };

        let result = run_post_process(&pp, 0, "hello world", &Allowlist::new(), Some(dir.as_path()), &empty_args());
        assert_eq!(result, "HELLO WORLD");
        cleanup(&dir);
    }

    #[test]
    fn post_process_returns_original_on_script_failure() {
        let dir = setup_skill_with_script("pp-fail", "fail", "#!/bin/sh\nexit 1");

        let pp = PostProcessSpec {
            script: Some("fail".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
            empty_is_result: None,
        };

        let result = run_post_process(
            &pp,
            0,
            "original output",
            &Allowlist::new(),
            Some(dir.as_path()),
            &empty_args(),
        );
        assert_eq!(result, "original output");
        cleanup(&dir);
    }

    #[test]
    fn post_process_returns_original_on_empty_output() {
        let dir = setup_skill_with_script("pp-empty", "empty", "#!/bin/sh\ncat > /dev/null");

        let pp = PostProcessSpec {
            script: Some("empty".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
            empty_is_result: None,
        };

        let result = run_post_process(
            &pp,
            0,
            "original output",
            &Allowlist::new(),
            Some(dir.as_path()),
            &empty_args(),
        );
        assert_eq!(result, "original output");
        cleanup(&dir);
    }

    #[test]
    fn post_process_empty_is_result_returns_empty_on_empty_output() {
        let dir = setup_skill_with_script("pp-eir", "empty", "#!/bin/sh\ncat > /dev/null");

        let pp = PostProcessSpec {
            script: Some("empty".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
            empty_is_result: Some(true),
        };

        let result = run_post_process(
            &pp,
            0,
            "original output",
            &Allowlist::new(),
            Some(dir.as_path()),
            &empty_args(),
        );
        assert_eq!(result, "");
        cleanup(&dir);
    }

    #[test]
    fn post_process_passes_args_to_script() {
        let dir = setup_skill_with_script("pp-args", "head-lines", "#!/bin/sh\nhead -n \"$1\"");

        let pp = PostProcessSpec {
            script: Some("head-lines".to_string()),
            binary: None,
            subcommand: None,
            args: vec!["2".to_string()],
            empty_is_result: None,
        };

        let input = "line1\nline2\nline3\nline4\n";
        let result = run_post_process(&pp, 0, input, &Allowlist::new(), Some(dir.as_path()), &empty_args());
        assert_eq!(result, "line1\nline2\n");
        cleanup(&dir);
    }

    #[test]
    fn post_process_substitutes_param_placeholders_in_args() {
        let dir = setup_skill_with_script(
            "pp-subst",
            "echo-arg",
            "#!/bin/sh\necho \"arg=$1\"",
        );

        let pp = PostProcessSpec {
            script: Some("echo-arg".to_string()),
            binary: None,
            subcommand: None,
            args: vec!["$root".to_string()],
            empty_is_result: None,
        };

        let tool_args = serde_json::json!({ "root": "my-notes" });
        let result = run_post_process(&pp, 0, "input", &Allowlist::new(), Some(dir.as_path()), &tool_args);
        assert_eq!(result.trim(), "arg=my-notes");
        cleanup(&dir);
    }

    #[test]
    fn post_process_substitutes_empty_string_for_missing_param() {
        let dir = setup_skill_with_script(
            "pp-subst-missing",
            "echo-arg",
            "#!/bin/sh\nif [ -z \"$1\" ]; then echo \"empty\"; else echo \"got=$1\"; fi",
        );

        let pp = PostProcessSpec {
            script: Some("echo-arg".to_string()),
            binary: None,
            subcommand: None,
            args: vec!["$root".to_string()],
            empty_is_result: None,
        };

        let tool_args = serde_json::json!({});
        let result = run_post_process(&pp, 0, "input", &Allowlist::new(), Some(dir.as_path()), &tool_args);
        assert_eq!(result.trim(), "empty");
        cleanup(&dir);
    }

    #[test]
    fn post_process_rejects_traversal_in_script_name() {
        let dir = setup_skill_with_script("pp-traversal", "legit", "#!/bin/sh\necho pwned");

        let pp = PostProcessSpec {
            script: Some("../../../etc/passwd".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
            empty_is_result: None,
        };

        let result = run_post_process(&pp, 0, "safe", &Allowlist::new(), Some(dir.as_path()), &empty_args());
        assert_eq!(result, "safe");
        cleanup(&dir);
    }

    #[test]
    fn post_process_missing_script_returns_original() {
        let dir = test_dir("pp-missing");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("scripts")).expect("create scripts dir");

        let pp = PostProcessSpec {
            script: Some("nonexistent".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
            empty_is_result: None,
        };

        let result = run_post_process(&pp, 0, "original", &Allowlist::new(), Some(dir.as_path()), &empty_args());
        assert_eq!(result, "original");
        cleanup(&dir);
    }

    #[test]
    fn post_process_no_skill_dir_returns_original() {
        let pp = PostProcessSpec {
            script: Some("anything".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
            empty_is_result: None,
        };

        let result = run_post_process(&pp, 0, "original", &Allowlist::new(), None, &empty_args());
        assert_eq!(result, "original");
    }

    #[test]
    fn post_process_passes_exit_code_env_to_script() {
        let dir = setup_skill_with_script(
            "pp-exit-code",
            "check-exit",
            "#!/bin/sh\necho \"exit=$CHAI_EXIT_CODE\"",
        );

        let pp = PostProcessSpec {
            script: Some("check-exit".to_string()),
            binary: None,
            subcommand: None,
            args: vec![],
            empty_is_result: None,
        };

        let result = run_post_process(&pp, 1, "input", &Allowlist::new(), Some(dir.as_path()), &empty_args());
        assert_eq!(result.trim(), "exit=1");
        cleanup(&dir);
    }

    #[test]
    fn post_process_substitutes_absent_default_for_missing_param() {
        // This test verifies that when $param_name references a parameter that
        // is absent from tool_args, the caller should augment the args with
        // absentDefault values before calling run_post_process. The
        // augment_with_absent_defaults function in mod.rs handles this.
        // Here we test that substitute_pp_args correctly picks up values that
        // have been injected into the tool_args JSON.
        let dir = setup_skill_with_script(
            "pp-absent-default",
            "echo-arg",
            "#!/bin/sh\necho \"ref=$1\"",
        );

        let pp = PostProcessSpec {
            script: Some("echo-arg".to_string()),
            binary: None,
            subcommand: None,
            args: vec!["$ref".to_string()],
            empty_is_result: None,
        };

        // Simulate what augment_with_absent_defaults would produce: "ref" is
        // injected with its absentDefault value.
        let tool_args = serde_json::json!({ "ref": "HEAD~1" });
        let result = run_post_process(&pp, 0, "input", &Allowlist::new(), Some(dir.as_path()), &tool_args);
        assert_eq!(result.trim(), "ref=HEAD~1");
        cleanup(&dir);
    }
}
