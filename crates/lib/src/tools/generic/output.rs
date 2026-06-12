//! Output post-processing for the generic tool executor: side-read
//! augmentation and output truncation.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::skills::SideReadSpec;

/// Append a nearby file's contents to the tool result when the file exists.
///
/// When `once_per_session` is true and a session ID is provided, the append
/// is skipped for any (session, path) pair that was already surfaced in the
/// current session.
pub(crate) fn apply_side_read(
    sr: &SideReadSpec,
    args: &serde_json::Value,
    current_output: &str,
    session_id: Option<&str>,
    seen: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> String {
    if sr.filename.contains("..") || sr.filename.contains('/') || sr.filename.contains('\\') {
        log::warn!("sideRead: invalid filename in spec: {}", sr.filename);
        return current_output.to_string();
    }

    let path_str = match args
        .as_object()
        .and_then(|o| o.get(&sr.path_param))
        .and_then(|v| v.as_str())
    {
        Some(s) => s,
        None => return current_output.to_string(),
    };

    let candidate = std::path::Path::new(path_str).join(&sr.filename);

    if sr.once_per_session == Some(true) {
        if let Some(sid) = session_id {
            let seen_key = format!("{}/{}", path_str, sr.filename);
            let already_seen = {
                let mut map = seen.lock().unwrap_or_else(|e| e.into_inner());
                let session_seen = map.entry(sid.to_string()).or_default();
                if session_seen.contains(&seen_key) {
                    true
                } else {
                    session_seen.insert(seen_key);
                    false
                }
            };
            if already_seen {
                return current_output.to_string();
            }
        }
    }

    let content = match std::fs::read_to_string(&candidate) {
        Ok(s) => s,
        Err(_) => return current_output.to_string(),
    };

    if content.trim().is_empty() {
        return current_output.to_string();
    }

    let label = sr.label.as_deref().unwrap_or(&sr.filename);
    format!(
        "{}

--- {} (BOF) ---

{}

--- {} (EOF) ---",
        current_output.trim_end(),
        label,
        content.trim_end(),
        label
    )
}

/// Truncate tool output to `max_lines` lines, appending a notice when
/// the output exceeds the limit. The notice includes the total line count
/// and a hint for narrowing the query.
pub(crate) fn truncate_output(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let total = lines.len();
    if total <= max_lines {
        return output.to_string();
    }

    let truncated: String = lines[..max_lines].join("\n");
    let omitted = total - max_lines;
    format!(
        "{}\n\n[output truncated: {} of {} lines shown; {} lines omitted. \
         Narrow your query path, pattern, or range to reduce results.]",
        truncated, max_lines, total, omitted
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("chai-generic-test-{}-{}", name, std::process::id()))
    }

    fn make_seen() -> Arc<Mutex<HashMap<String, HashSet<String>>>> {
        Arc::new(Mutex::new(HashMap::new()))
    }

    fn make_sr(path_param: &str, filename: &str, once_per_session: bool) -> SideReadSpec {
        SideReadSpec {
            path_param: path_param.to_string(),
            filename: filename.to_string(),
            label: None,
            once_per_session: if once_per_session { Some(true) } else { None },
        }
    }

    fn args_with_path(path: &str) -> serde_json::Value {
        serde_json::json!({ "path": path })
    }

    // --- apply_side_read tests ---

    #[test]
    fn side_read_appends_file_content() {
        let dir = test_dir("sr-basic");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("AGENTS.md"), "# Rules\nBe helpful.").expect("write");

        let sr = make_sr("path", "AGENTS.md", false);
        let args = args_with_path(dir.to_str().unwrap());
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "file1.txt\nfile2.rs", None, &seen);
        assert!(result.contains("file1.txt"), "original output preserved");
        assert!(result.contains("--- AGENTS.md (BOF) ---"), "separator present");
        assert!(result.contains("--- AGENTS.md (EOF) ---"), "separator present");
        assert!(result.contains("Be helpful."), "file content appended");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn side_read_absent_file_returns_original() {
        let dir = test_dir("sr-absent");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");

        let sr = make_sr("path", "AGENTS.md", false);
        let args = args_with_path(dir.to_str().unwrap());
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "listing output", None, &seen);
        assert_eq!(result, "listing output");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn side_read_rejects_traversal_in_filename() {
        let sr = SideReadSpec {
            path_param: "path".to_string(),
            filename: "../../../etc/passwd".to_string(),
            label: None,
            once_per_session: None,
        };
        let args = args_with_path("/tmp");
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "safe output", None, &seen);
        assert_eq!(result, "safe output");
    }

    #[test]
    fn side_read_rejects_slash_in_filename() {
        let sr = SideReadSpec {
            path_param: "path".to_string(),
            filename: "sub/AGENTS.md".to_string(),
            label: None,
            once_per_session: None,
        };
        let args = args_with_path("/tmp");
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "safe output", None, &seen);
        assert_eq!(result, "safe output");
    }

    #[test]
    fn side_read_once_per_session_deduplicates() {
        let dir = test_dir("sr-once");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("AGENTS.md"), "# Project rules").expect("write");

        let sr = make_sr("path", "AGENTS.md", true);
        let args = args_with_path(dir.to_str().unwrap());
        let seen = make_seen();

        let first = apply_side_read(&sr, &args, "ls output", Some("session-1"), &seen);
        assert!(first.contains("Project rules"), "first call appends");

        let second = apply_side_read(&sr, &args, "ls output", Some("session-1"), &seen);
        assert_eq!(second, "ls output", "second call in same session is skipped");

        let other_session = apply_side_read(&sr, &args, "ls output", Some("session-2"), &seen);
        assert!(other_session.contains("Project rules"), "different session appends");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn side_read_no_session_always_appends_when_once_per_session() {
        let dir = test_dir("sr-no-session");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("AGENTS.md"), "# Always").expect("write");

        let sr = make_sr("path", "AGENTS.md", true);
        let args = args_with_path(dir.to_str().unwrap());
        let seen = make_seen();

        let first = apply_side_read(&sr, &args, "output", None, &seen);
        assert!(first.contains("Always"), "appends without session");

        let second = apply_side_read(&sr, &args, "output", None, &seen);
        assert!(second.contains("Always"), "appends again without session");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn side_read_uses_custom_label() {
        let dir = test_dir("sr-label");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("AGENTS.md"), "content").expect("write");

        let sr = SideReadSpec {
            path_param: "path".to_string(),
            filename: "AGENTS.md".to_string(),
            label: Some("Instructions".to_string()),
            once_per_session: None,
        };
        let args = args_with_path(dir.to_str().unwrap());
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "listing", None, &seen);
        assert!(result.contains("--- Instructions (BOF) ---"), "custom label used");
        assert!(result.contains("--- Instructions (EOF) ---"), "custom label used");
        assert!(!result.contains("--- AGENTS.md (BOF) ---"), "default label not used");
        assert!(!result.contains("--- AGENTS.md (EOF) ---"), "default label not used");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn side_read_empty_file_returns_original() {
        let dir = test_dir("sr-empty");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("AGENTS.md"), "\n  ").expect("write whitespace-only file");

        let sr = make_sr("path", "AGENTS.md", false);
        let args = args_with_path(dir.to_str().unwrap());
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "listing", None, &seen);
        assert_eq!(result, "listing", "whitespace-only file treated as empty");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn side_read_missing_path_param_returns_original() {
        let sr = make_sr("path", "AGENTS.md", false);
        let args = serde_json::json!({ "other": "/tmp" });
        let seen = make_seen();

        let result = apply_side_read(&sr, &args, "output", None, &seen);
        assert_eq!(result, "output");
    }

    // --- truncate_output tests ---

    #[test]
    fn truncate_output_returns_original_when_within_limit() {
        let output = "line1\nline2\nline3";
        assert_eq!(truncate_output(output, 5), output);
    }

    #[test]
    fn truncate_output_returns_original_when_at_limit() {
        let output = "line1\nline2\nline3";
        assert_eq!(truncate_output(output, 3), output);
    }

    #[test]
    fn truncate_output_truncates_when_exceeding_limit() {
        let output = "line1\nline2\nline3\nline4\nline5";
        let result = truncate_output(output, 3);
        assert!(result.starts_with("line1\nline2\nline3"));
        assert!(result.contains("output truncated"));
        assert!(result.contains("3 of 5 lines shown"));
        assert!(result.contains("2 lines omitted"));
    }

    #[test]
    fn truncate_output_handles_single_line() {
        let output = "only line";
        assert_eq!(truncate_output(output, 1), output);
    }

    #[test]
    fn truncate_output_empty_string_is_within_limit() {
        let output = "";
        assert_eq!(truncate_output(output, 10), output);
    }

    #[test]
    fn truncate_output_notice_suggests_narrowing() {
        let output = "a\nb\nc\nd\ne";
        let result = truncate_output(output, 2);
        assert!(result.contains("Narrow your query"));
    }
}
