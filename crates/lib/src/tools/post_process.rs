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

/// Run a post-process script or command, piping `input` to its stdin.
/// Returns the script's stdout on success, or the original input on failure.
pub fn run_post_process(
    pp: &PostProcessSpec,
    input: &str,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
) -> String {
    use std::process::Stdio;

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
            .arg(&script_path)
            .args(&pp.args)
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
                        if s.is_empty() {
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
        cmd.args(subcommand.split_whitespace())
            .args(&pp.args)
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
                        if s.is_empty() {
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
        };

        let result = run_post_process(&pp, "hello world", &Allowlist::new(), Some(dir.as_path()));
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
        };

        let result = run_post_process(
            &pp,
            "original output",
            &Allowlist::new(),
            Some(dir.as_path()),
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
        };

        let result = run_post_process(
            &pp,
            "original output",
            &Allowlist::new(),
            Some(dir.as_path()),
        );
        assert_eq!(result, "original output");
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
        };

        let input = "line1\nline2\nline3\nline4\n";
        let result = run_post_process(&pp, input, &Allowlist::new(), Some(dir.as_path()));
        assert_eq!(result, "line1\nline2\n");
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
        };

        let result = run_post_process(&pp, "safe", &Allowlist::new(), Some(dir.as_path()));
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
        };

        let result = run_post_process(&pp, "original", &Allowlist::new(), Some(dir.as_path()));
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
        };

        let result = run_post_process(&pp, "original", &Allowlist::new(), None);
        assert_eq!(result, "original");
    }
}
