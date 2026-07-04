//! Output post-processing for the generic tool executor: hint conditions,
//! side-read augmentation, and output truncation.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::skills::{HintCondition, SideReadSpec};

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

/// Evaluate hint conditions against the post-processed output and exit code.
/// Returns the output with any matching hints appended (each preceded by a
/// blank line separator, each starting with `hint:`).
///
/// This function is called after `postProcess` and before `truncate_output`
/// so that inline hints are present in the output for truncation to preserve.
/// Truncation moves `hint:` lines to the end of the output (after the
/// truncation notice), so they are never lost and always appear last.
pub(crate) fn apply_hint_conditions(
    conditions: &[HintCondition],
    exit_code: i32,
    output: &str,
    tool_args: &serde_json::Value,
) -> String {
    let mut hints = Vec::new();
    for condition in conditions {
        if condition.matches(exit_code, output, tool_args) {
            let hint_text = condition.expand_hint(tool_args);
            hints.push(hint_text);
        }
    }
    if hints.is_empty() {
        return output.to_string();
    }
    let mut result = output.to_string();
    // Ensure output ends with a newline before appending hints.
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    for hint in hints {
        result.push('\n'); // blank line separator
        result.push_str("hint: ");
        result.push_str(&hint);
        result.push('\n');
    }
    result
}

/// Extract a line number from a line prefixed with `{number}\t{content}`
/// (the format used by `files_read`, `git_diff_lines`, `git_show_lines`, etc.).
/// Returns `None` when the line does not start with a number and tab.
fn extract_line_number(line: &str) -> Option<usize> {
    let tab_pos = line.find('\t')?;
    let prefix = &line[..tab_pos];
    prefix.parse().ok()
}

/// Truncate tool output to `max_lines` lines, appending a notice when
/// the output exceeds the limit. The notice includes the total line count
/// and a hint for narrowing the query. Lines starting with `hint:` are
/// preserved (moved after the truncation notice) so that diagnostic hints
/// appended by postProcess scripts or hintConditions are never lost to
/// truncation and always appear at the end of the output.
///
/// When `truncation_hint` is provided, it replaces the generic "Narrow your
/// query path, pattern, or range to reduce results." notice with a
/// tool-specific message. Template variables:
/// - `{kept}` = non-hint lines shown
/// - `{total}` = total lines (including hints)
/// - `{omitted}` = non-hint lines omitted
/// - `{next_start}` = first omitted line. When output lines are prefixed
///   with line numbers in the format `{number}\t{content}`, `{next_start}`
///   is derived from the last kept line number + 1 (so pagination hints
///   reference the correct file line). Otherwise, `{next_start}` = `kept + 1`.
pub(crate) fn truncate_output(output: &str, max_lines: usize, truncation_hint: Option<&str>) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let total = lines.len();
    if total <= max_lines {
        return output.to_string();
    }

    // Separate hint lines from non-hint lines so hints survive truncation.
    let mut hint_lines: Vec<&str> = Vec::new();
    let mut non_hint_lines: Vec<&str> = Vec::new();
    for line in &lines {
        if line.starts_with("hint:") {
            hint_lines.push(line);
        } else {
            non_hint_lines.push(line);
        }
    }

    let non_hint_total = non_hint_lines.len();
    let kept = std::cmp::min(max_lines, non_hint_total);
    let omitted = non_hint_total - kept;

    let mut result = non_hint_lines[..kept].join("\n");

    let notice = if let Some(template) = truncation_hint {
        // Derive {next_start} from the last kept line when it has a line-number
        // prefix (e.g. "501\tcontent" → next_start = 502). Fall back to
        // output-line numbering (kept + 1) when no prefix is found.
        let next_start = extract_line_number(non_hint_lines[kept - 1])
            .map(|n| n + 1)
            .unwrap_or(kept + 1);
        template
            .replace("{kept}", &kept.to_string())
            .replace("{total}", &total.to_string())
            .replace("{omitted}", &omitted.to_string())
            .replace("{next_start}", &next_start.to_string())
    } else {
        format!(
            "output truncated: {} of {} lines shown; {} lines omitted. \
             Narrow your query path, pattern, or range to reduce results.",
            kept, total, omitted
        )
    };

    result.push_str(&format!("\n\n[{}]", notice));

    // Append hint lines after the truncation notice so hints are always at
    // the end of the output, even when truncation fires.
    if !hint_lines.is_empty() {
        for hint in &hint_lines {
            result.push('\n');
            result.push('\n');
            result.push_str(hint);
        }
    }

    result
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

    // --- apply_hint_conditions tests ---

    fn make_hint_condition(
        match_: Option<&str>,
        exit_code: Option<crate::skills::HintExitCode>,
        not_empty: Option<bool>,
        when_arg: Option<std::collections::HashMap<String, serde_json::Value>>,
        hint: &str,
    ) -> crate::skills::HintCondition {
        crate::skills::HintCondition {
            match_: match_.map(|s| s.to_string()),
            exit_code,
            not_empty,
            when_arg,
            hint: hint.to_string(),
        }
    }

    #[test]
    fn apply_hint_conditions_no_matches_returns_original() {
        let conditions = vec![make_hint_condition(
            Some("error"),
            None,
            None,
            None,
            "an error occurred",
        )];
        let args = serde_json::json!({});
        let result = apply_hint_conditions(&conditions, 0, "success output", &args);
        assert_eq!(result, "success output");
    }

    #[test]
    fn apply_hint_conditions_single_match() {
        let conditions = vec![make_hint_condition(
            Some("not found"),
            None,
            None,
            None,
            "file not found — use files_list",
        )];
        let args = serde_json::json!({});
        let result = apply_hint_conditions(&conditions, 0, "error: not found", &args);
        assert!(result.contains("error: not found"), "original output preserved");
        assert!(result.contains("hint: file not found — use files_list"), "hint appended");
    }

    #[test]
    fn apply_hint_conditions_multiple_matches() {
        let conditions = vec![
            make_hint_condition(Some("CONFLICT"), None, None, None, "resolve conflicts"),
            make_hint_condition(Some("is up to date"), None, None, None, "already up to date"),
        ];
        let args = serde_json::json!({});
        let result = apply_hint_conditions(&conditions, 0, "CONFLICT: file.txt", &args);
        assert!(result.contains("hint: resolve conflicts"), "first hint appended");
        assert!(!result.contains("hint: already up to date"), "second condition not met");
    }

    #[test]
    fn apply_hint_conditions_all_match() {
        let conditions = vec![
            make_hint_condition(Some("nothing to commit"), None, None, None, "working tree clean"),
            make_hint_condition(Some("untracked files"), None, None, None, "untracked files present"),
        ];
        let args = serde_json::json!({});
        // Git can output both "nothing to commit" and "untracked files" in the same message
        let result = apply_hint_conditions(
            &conditions,
            0,
            "nothing to commit, untracked files present",
            &args,
        );
        assert!(result.contains("hint: working tree clean"), "first hint");
        assert!(result.contains("hint: untracked files present"), "second hint");
    }

    #[test]
    fn apply_hint_conditions_empty_conditions_returns_original() {
        let conditions: Vec<crate::skills::HintCondition> = vec![];
        let args = serde_json::json!({});
        let result = apply_hint_conditions(&conditions, 0, "output", &args);
        assert_eq!(result, "output");
    }

    #[test]
    fn apply_hint_conditions_exit_code_nonzero() {
        let conditions = vec![make_hint_condition(
            None,
            Some(crate::skills::HintExitCode::Nonzero("nonzero".to_string())),
            None,
            None,
            "command failed",
        )];
        let args = serde_json::json!({});
        let result = apply_hint_conditions(&conditions, 1, "error output", &args);
        assert!(result.contains("hint: command failed"));
    }

    #[test]
    fn apply_hint_conditions_exit_code_nonzero_with_zero_code() {
        let conditions = vec![make_hint_condition(
            None,
            Some(crate::skills::HintExitCode::Nonzero("nonzero".to_string())),
            None,
            None,
            "command failed",
        )];
        let args = serde_json::json!({});
        let result = apply_hint_conditions(&conditions, 0, "success", &args);
        assert!(!result.contains("hint:"));
    }

    #[test]
    fn apply_hint_conditions_not_empty_true() {
        let conditions = vec![make_hint_condition(
            None,
            None,
            Some(true),
            None,
            "use files_read_lines",
        )];
        let args = serde_json::json!({});
        let result = apply_hint_conditions(&conditions, 0, "search results", &args);
        assert!(result.contains("hint: use files_read_lines"));
    }

    #[test]
    fn apply_hint_conditions_not_empty_true_with_empty_output() {
        let conditions = vec![make_hint_condition(
            None,
            None,
            Some(true),
            None,
            "use files_read_lines",
        )];
        let args = serde_json::json!({});
        let result = apply_hint_conditions(&conditions, 0, "", &args);
        assert!(!result.contains("hint:"));
    }

    #[test]
    fn apply_hint_conditions_template_variable() {
        let conditions = vec![make_hint_condition(
            None,
            Some(crate::skills::HintExitCode::Specific(0)),
            None,
            None,
            "reset to {ref} — use git_status",
        )];
        let args = serde_json::json!({ "ref": "HEAD~1" });
        let result = apply_hint_conditions(&conditions, 0, "output", &args);
        assert!(result.contains("hint: reset to HEAD~1 — use git_status"));
    }

    #[test]
    fn apply_hint_conditions_hint_format_has_blank_line_separator() {
        let conditions = vec![make_hint_condition(
            Some("error"),
            None,
            None,
            None,
            "check input",
        )];
        let args = serde_json::json!({});
        let result = apply_hint_conditions(&conditions, 0, "error occurred", &args);
        // Output should be: "error occurred\n\nhint: check input\n"
        assert!(result.contains("\n\nhint: check input\n"), "blank line separator before hint");
    }

    #[test]
    fn apply_hint_conditions_preserves_trailing_newline() {
        let conditions = vec![make_hint_condition(
            Some("error"),
            None,
            None,
            None,
            "check input",
        )];
        let args = serde_json::json!({});
        let result = apply_hint_conditions(&conditions, 0, "error occurred\n", &args);
        // Output already ends with newline, so just one \n before hint
        assert!(result.contains("error occurred\n\nhint: check input\n"));
    }

    // --- truncate_output tests ---

    #[test]
    fn truncate_output_returns_original_when_within_limit() {
        let output = "line1\nline2\nline3";
        assert_eq!(truncate_output(output, 5, None), output);
    }

    #[test]
    fn truncate_output_returns_original_when_at_limit() {
        let output = "line1\nline2\nline3";
        assert_eq!(truncate_output(output, 3, None), output);
    }

    #[test]
    fn truncate_output_truncates_when_exceeding_limit() {
        let output = "line1\nline2\nline3\nline4\nline5";
        let result = truncate_output(output, 3, None);
        assert!(result.starts_with("line1\nline2\nline3"));
        assert!(result.contains("output truncated"));
        assert!(result.contains("3 of 5 lines shown"));
        assert!(result.contains("2 lines omitted"));
    }

    #[test]
    fn truncate_output_handles_single_line() {
        let output = "only line";
        assert_eq!(truncate_output(output, 1, None), output);
    }

    #[test]
    fn truncate_output_empty_string_is_within_limit() {
        let output = "";
        assert_eq!(truncate_output(output, 10, None), output);
    }

    #[test]
    fn truncate_output_notice_suggests_narrowing() {
        let output = "a\nb\nc\nd\ne";
        let result = truncate_output(output, 2, None);
        assert!(result.contains("Narrow your query"));
    }

    #[test]
    fn truncate_output_preserves_hint_lines() {
        let output = "line1\nline2\nline3\nline4\nline5\nhint: check indentation";
        let result = truncate_output(output, 3, None);
        assert!(result.starts_with("line1\nline2\nline3"), "first 3 non-hint lines kept");
        assert!(result.contains("hint: check indentation"), "hint preserved after truncation");
        assert!(result.contains("output truncated"), "truncation notice present");
        assert!(result.contains("3 of 6 lines shown"), "total includes hint line");
        assert!(result.contains("2 lines omitted"), "omits non-hint lines beyond limit");
    }

    #[test]
    fn truncate_output_preserves_multiple_hints() {
        let output = "line1\nhint: first\nline2\nhint: second\nline3\nline4\nline5";
        let result = truncate_output(output, 2, None);
        assert!(result.contains("hint: first"), "first hint preserved");
        assert!(result.contains("hint: second"), "second hint preserved");
        assert!(result.contains("output truncated"));
    }

    #[test]
    fn truncate_output_hint_not_counted_against_limit() {
        // 4 non-hint lines + 1 hint line, limit 3
        // Should keep 3 non-hint lines + hint + notice
        let output = "a\nb\nc\nd\nhint: useful tip";
        let result = truncate_output(output, 3, None);
        assert!(result.contains("a"), "line a kept");
        assert!(result.contains("b"), "line b kept");
        assert!(result.contains("c"), "line c kept");
        assert!(!result.contains("\nd\n"), "line d truncated");
        assert!(result.contains("hint: useful tip"), "hint preserved");
        assert!(result.contains("3 of 5 lines shown"), "total counts all lines");
        assert!(result.contains("1 lines omitted"), "only 1 non-hint line omitted");
    }

    #[test]
    fn truncate_output_all_hints_no_content() {
        let output = "hint: a\nhint: b\nhint: c";
        let result = truncate_output(output, 2, None);
        // 0 non-hint lines, 3 hint lines. total=3, kept=min(2,0)=0, omitted=0
        assert!(result.contains("hint: a"), "first hint preserved");
        assert!(result.contains("hint: b"), "second hint preserved");
        assert!(result.contains("hint: c"), "third hint preserved");
        assert!(result.contains("0 of 3 lines shown"), "kept is 0 non-hint lines");
    }

    #[test]
    fn truncate_output_hint_appears_after_notice() {
        let output = "line1\nline2\nline3\nline4\nhint: check this";
        let result = truncate_output(output, 2, None);
        let hint_pos = result.find("hint: check this").expect("hint present");
        let notice_pos = result.find("[output truncated").expect("notice present");
        assert!(hint_pos > notice_pos, "hint appears after truncation notice");
    }

    #[test]
    fn truncate_output_with_custom_hint_template() {
        let output = "line1\nline2\nline3\nline4\nline5";
        let template = "output truncated: {kept} of {total} lines shown; {omitted} lines omitted. Use git_diff_lines with start_line: {next_start} to read the remaining lines.";
        let result = truncate_output(output, 3, Some(template));
        assert!(result.starts_with("line1\nline2\nline3"));
        assert!(result.contains("3 of 5 lines shown"));
        assert!(result.contains("2 lines omitted"));
        assert!(result.contains("start_line: 4"), "next_start is kept+1");
        assert!(result.contains("git_diff_lines"), "custom hint content present");
        assert!(!result.contains("Narrow your query"), "generic notice replaced");
    }

    #[test]
    fn truncate_output_with_custom_hint_template_and_hints() {
        let output = "line1\nline2\nline3\nline4\nhint: useful\nline5\nline6";
        let template = "truncated: {kept}/{total} shown, {omitted} omitted. Use files_read_lines with start_line: {next_start}.";
        let result = truncate_output(output, 3, Some(template));
        assert!(result.contains("hint: useful"), "hint preserved with custom template");
        assert!(result.contains("start_line: 4"), "next_start computed from non-hint lines");
        assert!(result.contains("files_read_lines"), "custom hint content");
    }

    #[test]
    fn truncate_output_custom_hint_no_truncation() {
        let output = "line1\nline2";
        let template = "should not appear: {kept}";
        let result = truncate_output(output, 5, Some(template));
        assert_eq!(result, output, "template not applied when no truncation");
    }

    // --- extract_line_number + truncate_output with line-number prefixes ---

    #[test]
    fn extract_line_number_parses_tab_prefixed_line() {
        assert_eq!(extract_line_number("501\tuse std::path::PathBuf;"), Some(501));
        assert_eq!(extract_line_number("1\thello"), Some(1));
        assert_eq!(extract_line_number("9999\tlast line"), Some(9999));
    }

    #[test]
    fn extract_line_number_returns_none_for_plain_lines() {
        assert_eq!(extract_line_number("line1"), None);
        assert_eq!(extract_line_number("no tab here"), None);
        assert_eq!(extract_line_number(""), None);
    }

    #[test]
    fn extract_line_number_returns_none_for_non_numeric_prefix() {
        assert_eq!(extract_line_number("abc\tcontent"), None);
        assert_eq!(extract_line_number("12abc\tcontent"), None);
    }

    #[test]
    fn truncate_output_next_start_from_line_number_prefix() {
        // Simulates files_read with start_line: 501 — output lines are prefixed
        // with file-level line numbers in the format "{number}\t{content}".
        let mut output = String::new();
        for i in 501..=630 {
            if i > 501 { output.push('\n'); }
            output.push_str(&format!("{}\tcontent line {}", i, i));
        }
        // 130 lines total, limit 100 → kept=100, last kept line is "600\t..."
        let template = "output truncated: {kept} of {total} lines shown; {omitted} more lines available. To continue reading, use files_read with start_line: {next_start}.";
        let result = truncate_output(&output, 100, Some(template));
        assert!(result.contains("start_line: 601"), "next_start should be 601 (last kept line number + 1), not 101 (kept + 1)");
    }

    #[test]
    fn truncate_output_next_start_from_line_one() {
        // Simulates files_read from line 1 — truncation should still work correctly.
        let mut output = String::new();
        for i in 1..=600 {
            if i > 1 { output.push('\n'); }
            output.push_str(&format!("{}\tcontent line {}", i, i));
        }
        // 600 lines total, limit 500 → kept=500, last kept line is "500\t..."
        let template = "output truncated: {kept} of {total} lines shown; {omitted} more lines available. To continue reading, use files_read with start_line: {next_start}.";
        let result = truncate_output(&output, 500, Some(template));
        assert!(result.contains("start_line: 501"), "next_start should be 501 when reading from line 1");
    }

    #[test]
    fn truncate_output_next_start_falls_back_without_line_numbers() {
        // Plain lines (no number\t prefix) should fall back to kept + 1.
        let output = "line1\nline2\nline3\nline4\nline5";
        let template = "output truncated: {kept} of {total} lines shown; {omitted} lines omitted. Use start_line: {next_start}.";
        let result = truncate_output(output, 3, Some(template));
        assert!(result.contains("start_line: 4"), "next_start falls back to kept+1 when no line-number prefix");
    }

    // --- apply_hint_conditions + truncate_output interaction ---

    #[test]
    fn hint_conditions_then_truncation_preserves_hints() {
        // Simulate the full pipeline: apply_hint_conditions then truncate_output
        let conditions = vec![make_hint_condition(
            Some("error"),
            None,
            None,
            None,
            "check input",
        )];
        let args = serde_json::json!({});

        // Build output that exceeds the truncation limit
        let mut lines = String::new();
        for i in 1..=10 {
            if i > 1 { lines.push('\n'); }
            lines.push_str(&format!("line{} with error content", i));
        }

        let with_hints = apply_hint_conditions(&conditions, 0, &lines, &args);
        assert!(with_hints.contains("hint: check input"), "hint appended");

        let truncated = truncate_output(&with_hints, 5, None);
        assert!(truncated.contains("hint: check input"), "hint preserved through truncation");
        assert!(truncated.contains("output truncated"), "truncation notice present");
    }
}
