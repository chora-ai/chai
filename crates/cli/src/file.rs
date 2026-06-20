use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum FileCmd {
    /// Write content to a file (creates or overwrites). Content is read from stdin when --content is omitted.
    Write {
        /// Absolute file path to write to
        #[arg(long)]
        path: String,
        /// Content to write. If omitted, content is read from stdin.
        /// Accepts values that begin with dashes (e.g. YAML frontmatter).
        #[arg(long, allow_hyphen_values = true)]
        content: Option<String>,
    },
    /// Append content to an existing file. Creates the file if it does not exist. Content is read from stdin when --content is omitted.
    Append {
        /// Absolute file path to append to
        #[arg(long)]
        path: String,
        /// Content to append. If omitted, content is read from stdin.
        /// Accepts values that begin with dashes (e.g. YAML frontmatter).
        #[arg(long, allow_hyphen_values = true)]
        content: Option<String>,
    },
    /// Patch a file by replacing a range of lines. Content is read from stdin when --content is omitted.
    Patch {
        /// Absolute file path to patch
        #[arg(long)]
        path: String,
        /// Line number to start replacing at (1-indexed, inclusive)
        #[arg(long)]
        start_line: usize,
        /// Line number to end replacing at (1-indexed, inclusive). Defaults to start_line (single line replacement).
        #[arg(long)]
        end_line: Option<usize>,
        /// The content expected at [start_line, end_line] before the patch. If provided, the tool
        /// verifies the file matches before applying the patch. Rejects the edit if the expected
        /// content does not match what is actually in the file.
        #[arg(long, allow_hyphen_values = true)]
        original_content: Option<String>,
        /// Read original_content from a file instead of passing it as a CLI flag. Takes precedence
        /// over --original-content. This avoids CLI argument encoding issues for content that
        /// must match file content byte-for-byte.
        #[arg(long)]
        original_content_file: Option<String>,
        /// Replacement content. If ommitted, content is read from stdin.
        /// Accepts values that begin with dashes (e.g. YAML frontmatter).
        #[arg(long, allow_hyphen_values = true)]
        content: Option<String>,
    },
    /// Replace all occurrences of a regex pattern in a file. Supports capture groups ($1-$9) in the replacement string.
    Replace {
        /// Absolute file path
        #[arg(long)]
        path: String,
        /// Read search pattern from a file instead of passing it as a CLI flag.
        /// This avoids CLI argument encoding issues for patterns that must
        /// match file content byte-for-byte (e.g., multi-line patterns with
        /// real newlines). Takes precedence over --pattern.
        #[arg(long)]
        pattern_file: Option<String>,
        /// Search pattern (extended regex, whole-file matching with multiline mode so ^ and $ match line boundaries)
        /// Accepts values that begin with dashes (e.g. `- **bold**`, `--flag`).
        /// Prefer --pattern-file for multi-line patterns to avoid encoding issues.
        #[arg(long, allow_hyphen_values = true)]
        pattern: Option<String>,
        /// Replacement string. Use $1-$9 for capture group references. Use $$ for a literal $.
        /// If omitted, replacement content is read from stdin.
        /// Accepts values that begin with dashes (e.g. `- replacement`, `--flag`).
        #[arg(long, allow_hyphen_values = true)]
        replacement: Option<String>,
        /// Maximum number of replacements to apply. 0 (default) means unlimited.
        /// Use 1 to replace only the first match and avoid unintended changes
        /// when the same pattern appears in multiple locations.
        #[arg(long, default_value_t = 0)]
        max_replacements: usize,
        /// Treat the pattern as literal text instead of regex. Use this when the
        /// pattern contains regex metacharacters (e.g. source code, markdown tables,
        /// JSON) that should be matched as-is rather than interpreted as regex.
        /// Capture groups ($1-$9) are not supported in literal mode.
        #[arg(long)]
        literal: bool,
        /// Show line numbers in the diff output
        #[arg(long)]
        line_numbers: bool,
    },
    /// Read a range of lines from a file with line numbers. Outputs lines in the format {line_number}|{content}.
    ReadLines {
        /// Absolute file path to read
        #[arg(long)]
        path: String,
        /// Line number to start reading at (1-indexed, inclusive)
        #[arg(long)]
        start_line: usize,
        /// Line number to end reading at (1-indexed, inclusive). Defaults to start_line (single line read).
        #[arg(long)]
        end_line: Option<usize>,
    },
    /// Delete a file. Refuses to delete directories.
    Delete {
        /// Absolute file path to delete
        #[arg(long)]
        path: String,
    },
    /// Delete an empty directory. Refuses to delete non-empty directories or files.
    DeleteDir {
        /// Absolute directory path to delete
        #[arg(long)]
        path: String,
    },
    /// Read YAML frontmatter from a markdown file. Outputs the YAML block without delimiters.
    FrontmatterRead {
        /// Absolute file path to read frontmatter from
        #[arg(long)]
        path: String,
    },
    /// Set a YAML frontmatter key to a value. Adds the key if missing, updates if present.
    /// Creates a frontmatter block if the file has none.
    FrontmatterEdit {
        /// Absolute file path to edit
        #[arg(long)]
        path: String,
        /// Frontmatter key to set
        #[arg(long)]
        key: String,
        /// Value to set the key to. Takes precedence over --value-file.
        #[arg(long)]
        value: Option<String>,
        /// Read value from a file instead of passing it as a CLI flag. Used when
        /// --value is not provided.
        #[arg(long)]
        value_file: Option<String>,
    },
    /// Remove a YAML frontmatter key. No-op if the key does not exist.
    FrontmatterDelete {
        /// Absolute file path to edit
        #[arg(long)]
        path: String,
        /// Frontmatter key to remove
        #[arg(long)]
        key: String,
    },
    /// Rename a markdown note and update all wikilinks that reference it.
    /// Searches all .md files under --root for [[old-name]] links and replaces
    /// them with [[new-name]].
    RenameNote {
        /// Absolute path to the existing note
        #[arg(long)]
        from: String,
        /// Absolute path to move the note to (parent directory must exist)
        #[arg(long)]
        to: String,
        /// Root directory to search for wikilinks to update (defaults to current directory)
        #[arg(long)]
        root: Option<String>,
    },
}

pub(crate) fn run_file(cmd: FileCmd) -> Result<()> {
    match cmd {
        FileCmd::Write { path, content } => {
            let content = read_content_from_stdin_or(content)?;
            let target = if std::path::Path::new(&path).is_relative() {
                std::env::current_dir()?.join(&path)
            } else {
                std::path::PathBuf::from(&path)
            };
            if let Some(parent) = target.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        anyhow::anyhow!("failed to create parent directory {}: {}", parent.display(), e)
                    })?;
                }
            }
            let existing = if target.exists() {
                let old_content = std::fs::read_to_string(&target)
                    .unwrap_or_default();
                let old_lines = old_content.lines().count();
                Some(old_lines)
            } else {
                None
            };
            std::fs::write(&target, &content)
                .map_err(|e| anyhow::anyhow!("failed to write {}: {}", target.display(), e))?;
            if let Some(old_lines) = existing {
                println!("wrote {} ({} bytes, overwriting existing {} lines)", target.display(), content.len(), old_lines);
            } else {
                println!("wrote {} ({} bytes)", target.display(), content.len());
            }
            Ok(())
        }
        FileCmd::Append { path, content } => {
            use std::io::Write;
            let content = read_content_from_stdin_or(content)?;
            let target = if std::path::Path::new(&path).is_relative() {
                std::env::current_dir()?.join(&path)
            } else {
                std::path::PathBuf::from(&path)
            };
            if let Some(parent) = target.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        anyhow::anyhow!("failed to create parent directory {}: {}", parent.display(), e)
                    })?;
                }
            }
            let existed = target.exists();
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&target)
                .map_err(|e| anyhow::anyhow!("failed to open {}: {}", target.display(), e))?;
            // If the file already has content that doesn't end with a newline,
            // prepend one so the appended content starts on a new line.
            let needs_leading_newline = if target.metadata().map(|m| m.len()).unwrap_or(0) > 0 {
                let existing = std::fs::read(&target)
                    .map_err(|e| anyhow::anyhow!("failed to read {}: {}", target.display(), e))?;
                existing.last() != Some(&b'\n')
            } else {
                false
            };
            if needs_leading_newline {
                file.write_all(b"\n")
                    .map_err(|e| anyhow::anyhow!("failed to append to {}: {}", target.display(), e))?;
            }
            file.write_all(content.as_bytes())
                .map_err(|e| anyhow::anyhow!("failed to append to {}: {}", target.display(), e))?;
            // Ensure the file ends with a newline if the appended content doesn't.
            if !content.ends_with('\n') {
                file.write_all(b"\n")
                    .map_err(|e| anyhow::anyhow!("failed to append to {}: {}", target.display(), e))?;
            }
            if existed {
                println!("appended {} bytes to {}", content.len(), target.display());
            } else {
                println!("appended {} bytes to {} (created new file)", content.len(), target.display());
            }
            Ok(())
        }
        FileCmd::Patch { path, start_line, end_line, original_content, original_content_file, content } => {
            let content = read_content_from_stdin_or(content)?;
            // Resolve original_content: --original-content-file takes precedence,
            // then --original-content. File-based passing avoids CLI argument
            // encoding issues for content that must match file content byte-for-byte.
            let original_content = if let Some(ref file_path) = original_content_file {
                Some(std::fs::read_to_string(file_path)
                    .map_err(|e| anyhow::anyhow!("failed to read original-content-file {}: {}", file_path, e))?)
            } else {
                original_content
            };
            let target = std::path::Path::new(&path);
            if !target.exists() {
                anyhow::bail!("file does not exist: {}", path);
            }
            if !target.is_file() {
                anyhow::bail!("not a file: {}", path);
            }
            if start_line == 0 {
                anyhow::bail!("start_line must be at least 1 (1-indexed)");
            }
            let end_line = end_line.unwrap_or(start_line);
            if end_line < start_line {
                anyhow::bail!("end_line ({}) must be >= start_line ({})", end_line, start_line);
            }

            let original = std::fs::read_to_string(target)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path, e))?;

            if start_line > original.lines().count() {
                anyhow::bail!(
                    "start_line ({}) exceeds file length ({})",
                    start_line,
                    original.lines().count()
                );
            }

            let effective_end = end_line.min(original.lines().count());

            // Verify original_content if provided, and collect trailing whitespace
            // from the original file if the match succeeded via stage 4 (trailing-
            // whitespace tolerance). This allows us to preserve the file's trailing
            // whitespace in the replacement content.
            let trailing_ws = if let Some(ref expected) = original_content {
                verify_original(&original, start_line, effective_end, expected)?
            } else {
                Vec::new()
            };

            // Apply trailing whitespace from the original file to the replacement
            // content. When the LLM drops trailing whitespace, the verification
            // (stage 4) accepts the match but we preserve the original whitespace
            // in the output. For each replacement line, strip its trailing whitespace
            // and re-append the original line's trailing whitespace. This handles
            // both cases: the LLM omitted trailing whitespace entirely, or the LLM
            // provided partial trailing whitespace (we replace, not append, to avoid
            // doubling). Extra replacement lines (expanding the range) have no
            // original trailing whitespace to preserve.
            let content = if !trailing_ws.is_empty() {
                let mut lines: Vec<String> = content.lines().map(String::from).collect();
                for (i, ws) in trailing_ws.iter().enumerate() {
                    if i < lines.len() {
                        // Trim the replacement line's trailing whitespace,
                        // then re-append the original's.
                        let trimmed = lines[i].trim_end();
                        lines[i] = trimmed.to_string();
                        lines[i].push_str(ws);
                    }
                }
                lines.join("\n")
            } else {
                content
            };

            let diff = format_patch_diff(&original, start_line, Some(effective_end), &content);
            let result = patch_string(&original, start_line, Some(end_line), &content);

            std::fs::write(target, &result)
                .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;

            let removed = effective_end - start_line + 1;
            let added = content.lines().count();
            println!(
                "patched {} - removed {} line(s), added {} line(s)\n{}",
                path, removed, added, diff
            );
            Ok(())
        }
        FileCmd::Replace { path, pattern_file, pattern, replacement, max_replacements, literal, line_numbers } => {
            let target = std::path::Path::new(&path);
            if !target.exists() {
                anyhow::bail!("file does not exist: {}", path);
            }
            if !target.is_file() {
                anyhow::bail!("not a file: {}", path);
            }

            // Resolve pattern: --pattern-file takes precedence, then --pattern.
            // File-based passing avoids CLI argument encoding issues for content
            // that must match file content byte-for-byte (e.g., multi-line
            // patterns with real newlines).
            let pattern = if let Some(ref file_path) = pattern_file {
                std::fs::read_to_string(file_path)
                    .map_err(|e| anyhow::anyhow!("failed to read pattern-file {}: {}", file_path, e))?
            } else {
                pattern.ok_or_else(|| anyhow::anyhow!("either --pattern or --pattern-file is required"))?
            };

            // Resolve replacement: --replacement flag, then stdin.
            let replacement = read_content_from_stdin_or(replacement)?;

            let original = std::fs::read_to_string(target)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path, e))?;

            if literal {
                // Literal mode: skip regex entirely and go straight to
                // literal matching with trailing-whitespace tolerance.
                // This handles patterns containing regex metacharacters
                // (source code, markdown tables, JSON, etc.) that would
                // fail or require impractical escaping as regex.
                // Line-boundary enforcement is off: explicit literal mode
                // gives standard find-and-replace semantics where the
                // pattern can match as a substring within a line.
                let literal_match = try_literal_trailing_ws_match(
                    &original, &pattern, &replacement, max_replacements, false,
                );

                match literal_match {
                    LiteralMatchResult::Matched { new_content, match_count, .. } => {
                        std::fs::write(target, new_content.as_bytes())
                            .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;

                        let diff = format_replace_diff(&original, &new_content, line_numbers);
                        println!(
                            "{} replacement(s) in {} (literal match)\n{}",
                            match_count, path, diff
                        );
                        if match_count > 1 && max_replacements == 0 {
                            println!("\nhint: {} match(es) replaced — use max_replacements: 1 to limit to first match", match_count);
                        }
                        Ok(())
                    }
                    LiteralMatchResult::NoMatch { leading_ws_hint } => {
                        if leading_ws_hint {
                            println!("0 replacements in {}", path);
                            println!("\nhint: pattern did not match, but would match with leading-whitespace normalization — check indentation");
                        } else {
                            println!("0 replacements in {}", path);
                        }
                        Ok(())
                    }
                }
            } else {
                // Regex mode: build and apply a regex pattern.
                let re = regex::RegexBuilder::new(&pattern)
                    .multi_line(true)
                .build()
                .map_err(|e| anyhow::anyhow!("invalid pattern: {}. If the pattern should be matched as literal text (not regex), use literal: true.", e))?;

            // Collect all captures, then apply up to max_replacements.
            // max_replacements == 0 means unlimited.
            let all_captures: Vec<_> = re.captures_iter(&original).collect();
            let count = all_captures.len();
            let line_count = original.lines().count();

            // Safety: when regex mode matches far more positions than there are
            // lines, the pattern is likely degenerate — e.g., an alternation
            // like `| foo | bar |` matching at every character boundary. In that
            // case, retry as literal before writing anything to disk.
            let match_cap = degenerate_match_cap(line_count);
            if count > match_cap && max_replacements == 0 {
                // Regex produced an unreasonable number of matches.
                // Try literal mode — if the pattern contains unintentional
                // regex metacharacters, literal matching will find the
                // intended targets. Line-boundary enforcement is on for
                // this auto-retry path to prevent false positives.
                let literal_match = try_literal_trailing_ws_match(
                    &original, &pattern, &replacement, max_replacements, true,
                );
                match literal_match {
                    LiteralMatchResult::Matched { new_content, match_count: literal_count, .. } => {
                        std::fs::write(target, new_content.as_bytes())
                            .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;

                        let diff = format_replace_diff(&original, &new_content, line_numbers);
                        println!(
                            "{} replacement(s) in {} (literal match — regex matched {} positions, likely due to unintentional regex metacharacters)\n{}",
                            literal_count, path, count, diff
                        );
                        if literal_count > 1 && max_replacements == 0 {
                            println!("\nhint: {} match(es) replaced — use max_replacements: 1 to limit to first match", literal_count);
                        }
                        return Ok(());
                    }
                    LiteralMatchResult::NoMatch { .. } => {
                        // Literal mode also didn't find what the agent intended.
                        // Refuse the replacement to prevent file corruption.
                        anyhow::bail!(
                            "regex matched {} positions (exceeds safety cap of {} for a {}-line file) — likely a degenerate pattern. Use literal: true or adjust the pattern.",
                            count, match_cap, line_count
                        );
                    }
                }
            }

            if count > 0 {
                let limit = if max_replacements > 0 {
                    max_replacements.min(count)
                } else {
                    count
                };

                // Only expand capture group references ($1-$9) when the
                // pattern contains explicit capture groups. When there are
                // no capture groups, caps.expand() would silently consume
                // $N references in the replacement as empty strings — e.g.,
                // "($1-$9)" would become "(-)" instead of the intended
                // literal text. Using the replacement as-is avoids this.
                let has_capture_groups = re.captures_len() > 1;

                let new_content = if limit == count {
                    // No limit or limit equals total matches — use replace_all
                    if has_capture_groups {
                        re.replace_all(&original, |caps: &regex::Captures| {
                            let mut expanded = String::new();
                            caps.expand(&replacement, &mut expanded);
                            expanded
                        }).into_owned()
                    } else {
                        re.replace_all(&original, regex::NoExpand(&replacement)).into_owned()
                    }
                } else {
                    // Apply only the first `limit` matches, building the result
                    // from left-to-right captures.
                    let mut result = String::with_capacity(original.len());
                    let mut last_end = 0;
                    for caps in all_captures.iter().take(limit) {
                        let mat = caps.get(0).unwrap();
                        result.push_str(&original[last_end..mat.start()]);
                        if has_capture_groups {
                            let mut expanded = String::new();
                            caps.expand(&replacement, &mut expanded);
                            result.push_str(&expanded);
                        } else {
                            result.push_str(&replacement);
                        }
                        last_end = mat.end();
                    }
                    result.push_str(&original[last_end..]);
                    result
                };

                std::fs::write(target, new_content.as_bytes())
                    .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;

                let diff = format_replace_diff(&original, &new_content, line_numbers);
                if limit < count {
                    println!(
                        "{} of {} match(es) replaced in {}\n{}",
                        limit, count, path, diff
                    );
                } else {
                    println!(
                        "{} replacement(s) in {}\n{}",
                        count, path, diff
                    );
                    if count > 1 && max_replacements == 0 {
                        println!("\nhint: {} match(es) replaced — use max_replacements: 1 to limit to first match", count);
                    }
                }
                return Ok(());
            }

            // Regex matched 0 times. Fall back to a trailing-whitespace-
            // tolerant literal search: treat the pattern as literal text
            // and match with trailing whitespace stripped from both the
            // pattern and the file content. This handles the common case
            // where the LLM drops trailing whitespace when copying content
            // from file reads into the pattern parameter. Line-boundary
            // enforcement is on for this auto-retry path to prevent
            // false positives.
            let literal_match = try_literal_trailing_ws_match(
                &original, &pattern, &replacement, max_replacements, true,
            );

            match literal_match {
                LiteralMatchResult::Matched { new_content, match_count, .. } => {
                    std::fs::write(target, new_content.as_bytes())
                        .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;

                    let diff = format_replace_diff(&original, &new_content, line_numbers);
                    println!(
                        "{} replacement(s) in {} (trailing-whitespace-tolerant match)\n{}",
                        match_count, path, diff
                    );
                    if match_count > 1 && max_replacements == 0 {
                        println!("\nhint: {} match(es) replaced — use max_replacements: 1 to limit to first match", match_count);
                    }
                    Ok(())
                }
                LiteralMatchResult::NoMatch { leading_ws_hint } => {
                    if leading_ws_hint {
                        println!("0 replacements in {}", path);
                        println!("\nhint: pattern did not match, but would match with leading-whitespace normalization — check indentation");
                    } else {
                        println!("0 replacements in {}", path);
                    }
                    Ok(())
                }
            }
            } // else (regex mode)
        }
        FileCmd::ReadLines { path, start_line, end_line } => {
            let target = std::path::Path::new(&path);
            if !target.exists() {
                anyhow::bail!("file does not exist: {}", path);
            }
            if !target.is_file() {
                anyhow::bail!("not a file: {}", path);
            }
            if start_line == 0 {
                anyhow::bail!("start_line must be at least 1 (1-indexed)");
            }
            let end_line = end_line.unwrap_or(start_line);
            if end_line < start_line {
                anyhow::bail!("end line ({}) must be >= start line ({})", end_line, start_line);
            }
            let content = std::fs::read_to_string(target)
                .map_err(|e| anyhow::anyhow!("failed to read {}; {}", path, e))?;
            for (i, line) in content.lines().enumerate() {
                let line_num = i + 1;
                if line_num >= start_line && line_num <= end_line {
                    println!("{}|{}", line_num, line);
                }
                if line_num > end_line {
                    break;
                }
            }
            Ok(())
        }
        FileCmd::Delete { path } => {
            let target = std::path::Path::new(&path);
            if !target.exists() {
                anyhow::bail!("file does not exist: {}", path);
            }
            if !target.is_file() {
                anyhow::bail!("refusing to delete non-file: {}", path);
            }
            std::fs::remove_file(target)
                .map_err(|e| anyhow::anyhow!("failed to delete {}: {}", path, e))?;
            println!("deleted {}", path);
            Ok(())
        }
        FileCmd::DeleteDir { path } => {
            let target = std::path::Path::new(&path);
            if !target.exists() {
                anyhow::bail!("directory does not exist: {}", path);
            }
            if !target.is_dir() {
                anyhow::bail!("refusing to delete non-directory: {}", path);
            }
            // Check if directory is empty
            let mut entries = std::fs::read_dir(target)
                .map_err(|e| anyhow::anyhow!("failed to read directory {}: {}", path, e))?;
            if entries.next().is_some() {
                anyhow::bail!("refusing to delete non-empty directory: {}", path);
            }
            std::fs::remove_dir(target)
                .map_err(|e| anyhow::anyhow!("failed to delete directory {}: {}", path, e))?;
            println!("deleted directory {}", path);
            Ok(())
        }
        FileCmd::FrontmatterRead { path } => {
            let target = std::path::Path::new(&path);
            let content = std::fs::read_to_string(target)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path, e))?;
            match extract_frontmatter(&content) {
                Some(fm) => {
                    print!("{}", fm);
                    Ok(())
                }
                None => {
                    println!("no frontmatter found in {}", path);
                    println!("\nhint: no frontmatter found — use notes_frontmatter_edit to create one");
                    std::process::exit(1);
                }
            }
        }
        FileCmd::FrontmatterEdit { path, key, value, value_file } => {
            // Resolve value: --value takes precedence over --value-file.
            let value = if let Some(ref v) = value {
                v.clone()
            } else if let Some(ref file_path) = value_file {
                std::fs::read_to_string(file_path)
                    .map_err(|e| anyhow::anyhow!("failed to read value-file {}: {}", file_path, e))?
            } else {
                anyhow::bail!("either --value or --value-file is required");
            };
            let target = std::path::Path::new(&path);
            let content = std::fs::read_to_string(target)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path, e))?;
            let new_content = edit_frontmatter(&content, &key, &value);
            std::fs::write(target, &new_content)
                .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;
            println!("set {}={} in {}", key, value, path);
            // Show resulting frontmatter after edit.
            if let Some(fm) = extract_frontmatter(&new_content) {
                println!("\n{}", fm.trim_end());
            }
            Ok(())
        }
        FileCmd::FrontmatterDelete { path, key } => {
            let target = std::path::Path::new(&path);
            let content = std::fs::read_to_string(target)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path, e))?;
            let new_content = delete_frontmatter_key(&content, &key);
            std::fs::write(target, &new_content)
                .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;
            println!("removed {} from {}", key, path);
            Ok(())
        }
        FileCmd::RenameNote { from, to, root } => {
            let from_path = std::path::Path::new(&from);
            let to_path = std::path::Path::new(&to);
            let root_path = match root {
                Some(ref r) => std::path::PathBuf::from(r),
                None => std::env::current_dir()?,
            };

            if !from_path.exists() {
                anyhow::bail!("source does not exist: {}", from);
            }
            if !from_path.is_file() {
                anyhow::bail!("source is not a file: {}", from);
            }
            if to_path.exists() {
                anyhow::bail!("destination already exists: {}", to);
            }
            if let Some(parent) = to_path.parent() {
                if !parent.exists() {
                    anyhow::bail!(
                        "destination parent directory does not exist: {}",
                        parent.display()
                    );
                }
            }
            if !root_path.is_dir() {
                anyhow::bail!("root is not a directory: {}", root_path.display());
            }

            // Extract note names (filename without .md extension) for link updating.
            let old_name = from_path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("cannot extract name from: {}", from))?;
            let new_name = to_path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("cannot extract name from: {}", to))?;

            // Rename the file.
            std::fs::rename(from_path, to_path)
                .map_err(|e| anyhow::anyhow!("failed to rename {} -> {}: {}", from, to, e))?;
            println!("renamed {} -> {}", from, to);

            // Update wikilinks if the note name changed.
            if old_name != new_name {
                let updated = update_wikilinks(&root_path, old_name, new_name)?;
                if updated > 0 {
                    println!("updated wikilinks in {} file(s)", updated);
                }
            }

            Ok(())
        }
    }
}

/// Read content from stdin when --content was not provided.
pub(crate) fn read_content_from_stdin_or(content: Option<String>) -> Result<String> {
    match content {
        Some(c) => Ok(c),
        None => {
            use std::io::Read;
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| anyhow::anyhow!("failed to read stdin: {}", e))?;
            Ok(s)
        }
    }
}

/// Apply a patch to a string: replace lines [start_line, end_line](1-indexed,
/// inclusive) with `replacement`. Extracted for reuse and testing.
fn patch_string(original: &str, start_line: usize, end_line: Option<usize>, replacement: &str) -> String {
    let end_line = end_line.unwrap_or(start_line);
    let lines: Vec<&str> = original.lines().collect();
    let total_lines = lines.len();
    let trailing_newline = original.ends_with('\n');

    if start_line > total_lines {
        return original.to_string();
    }

    let effective_end = end_line.min(total_lines);

    let mut result = String::new();

    for line in &lines[..start_line - 1] {
        result.push_str(line);
        result.push('\n');
    }

    if !replacement.is_empty() {
        result.push_str(replacement);
        if !replacement.ends_with('\n') {
            result.push('\n');
        }
    }

    for line in &lines[effective_end..] {
        result.push_str(line);
        result.push('\n');
    }

    if !trailing_newline && result.ends_with('\n') {
        result.pop();
    }

    result
}

/// Produce a contextual diff showing what changed in a patch operation.
/// Shows the removed lines prefixed with `-` and the added lines prefixed with `+`,
/// with line numbers for each. Also includes a few lines of context before and after
/// the change to help the agent verify boundary alignment.
fn format_patch_diff(
    original: &str,
    start_line: usize,
    end_line: Option<usize>,
    replacement: &str,
) -> String {
    let end_line = end_line.unwrap_or(start_line);
    let lines: Vec<&str> = original.lines().collect();
    let total_lines = lines.len();
    let effective_end = end_line.min(total_lines);

    let context = 3; // lines of context before/after
    let ctx_start = start_line.saturating_sub(context + 1);
    let ctx_end = (effective_end + context).min(total_lines);

    let mut diff = String::new();

    // Context lines before the change
    for i in ctx_start..(start_line - 1) {
        diff.push_str(&format!(" {}|{}\n", i + 1, lines[i]));
    }

    // Removed lines
    for i in (start_line - 1)..effective_end {
        diff.push_str(&format!("-{}|{}\n", i + 1, lines[i]));
    }

    // Added lines
    let replacement_lines: Vec<&str> = replacement.lines().collect();
    for (offset, line) in replacement_lines.iter().enumerate() {
        diff.push_str(&format!("+{}|{}\n", start_line + offset, line));
    }

    // Context lines after the change
    for i in effective_end..ctx_end {
        diff.push_str(&format!(" {}|{}\n", i + 1, lines[i]));
    }

    diff
}

/// Produce a diff between original and new content, showing all changed lines
/// with context. Each hunk shows removed lines prefixed with `-` and added
/// lines prefixed with `+`, with a few lines of surrounding context.
///
/// When the only difference between original and new content is a trailing
/// newline (added or removed), the LCS-based line diff produces no hunks
/// because `.lines()` does not distinguish between files with and without
/// trailing newlines. In that case, the function outputs a trailing-newline
/// indicator so the agent can see that the file's newline status changed.
fn format_replace_diff(original: &str, new: &str, line_numbers: bool) -> String {
    let orig_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let hunks = compute_diff_hunks(&orig_lines, &new_lines);

    let orig_trailing_nl = original.ends_with('\n');
    let new_trailing_nl = new.ends_with('\n');
    let trailing_nl_changed = orig_trailing_nl != new_trailing_nl
        && orig_lines == new_lines;

    if hunks.is_empty() && !trailing_nl_changed {
        return String::new();
    }

    let context = 3;
    let mut diff = String::new();

    for hunk in &hunks {
        // Context before
        let ctx_start = hunk.orig_start.saturating_sub(context);
        for i in ctx_start..hunk.orig_start {
            if line_numbers {
                diff.push_str(&format!(" {}|{}\n", i + 1, orig_lines[i]));
            } else {
                diff.push_str(&format!(" {}\n", orig_lines[i]));
            }
        }

        // Removed lines
        for i in hunk.orig_start..hunk.orig_end {
            if line_numbers {
                diff.push_str(&format!("-{}|{}\n", i + 1, orig_lines[i]));
            } else {
                diff.push_str(&format!("-{}\n", orig_lines[i]));
            }
        }

        // Added lines — use original-file line numbers so the diff is internally
        // consistent (all line numbers refer to the original file). The first
        // added line corresponds to the original start of the hunk.
        for (offset, line_idx) in (hunk.new_start..hunk.new_end).enumerate() {
            if line_numbers {
                diff.push_str(&format!("+{}|{}\n", hunk.orig_start + offset + 1, new_lines[line_idx]));
            } else {
                diff.push_str(&format!("+{}\n", new_lines[line_idx]));
            }
        }

        // Context after (from original, since we show the original context)
        let ctx_end = (hunk.orig_end + context).min(orig_lines.len());
        for i in hunk.orig_end..ctx_end {
            if line_numbers {
                diff.push_str(&format!(" {}|{}\n", i + 1, orig_lines[i]));
            } else {
                diff.push_str(&format!(" {}\n", orig_lines[i]));
            }
        }
    }

    // When the only change is a trailing newline (added or removed),
    // the LCS-based line diff produces no hunks because .lines()
    // treats both files the same. Show an explicit indicator instead.
    if trailing_nl_changed {
        if new_trailing_nl {
            diff.push_str("+\\n (trailing newline added)\n");
        } else {
            diff.push_str("-\\n (trailing newline removed)\n");
        }
    }

    diff
}

/// A contiguous region of change between original and new content.
#[derive(Debug)]
struct DiffHunk {
    /// Start index in orig_lines (inclusive)
    orig_start: usize,
    /// End index in orig_lines (exclusive)
    orig_end: usize,
    /// Start index in new_lines (inclusive)
    new_start: usize,
    /// End index in new_lines (exclusive)
    new_end: usize,
}

/// Compute diff hunks using the LCS algorithm. Delete and Insert ops that are
/// adjacent (not separated by an Equal) are merged into a single hunk. Nearby
/// hunks within 6 lines are also merged.
fn compute_diff_hunks(orig: &[&str], new: &[&str]) -> Vec<DiffHunk> {
    let m = orig.len();
    let n = new.len();

    // Build LCS length table
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if orig[i - 1] == new[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to produce tagged changes, but this time we also track
    // the positions in both sequences so we can detect gaps.
    #[derive(Debug)]
    enum Tag {
        Delete(usize), // orig index
        Insert(usize), // new index
    }

    let mut tags = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && orig[i - 1] == new[j - 1] {
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            tags.push(Tag::Insert(j - 1));
            j -= 1;
        } else {
            tags.push(Tag::Delete(i - 1));
            i -= 1;
        }
    }
    tags.reverse();

    if tags.is_empty() {
        return Vec::new();
    }

    // Group tags into hunks. Tags that are adjacent in both sequences
    // (no gap of equal lines) belong to the same hunk. We detect gaps
    // by checking if consecutive Delete tags have adjacent orig indices,
    // and consecutive Insert tags have adjacent new indices.
    //
    // Strategy: Walk the tags and maintain the current hunk's ranges.
    // When we encounter a tag that creates a gap in both orig and new
    // sequences, flush the current hunk and start a new one.
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut cur_del: Option<(usize, usize)> = None; // (start, end_exclusive)
    let mut cur_ins: Option<(usize, usize)> = None; // (start, end_exclusive)
    let mut last_orig: Option<usize> = None;
    let mut last_new: Option<usize> = None;

    for tag in &tags {
        let (tag_orig, tag_new) = match tag {
            Tag::Delete(idx) => (Some(*idx), None),
            Tag::Insert(idx) => (None, Some(*idx)),
        };

        // Check if this tag is adjacent to the previous one.
        // A tag is adjacent if it doesn't create a gap in either sequence
        // relative to the current hunk's extent.
        let mut is_gap = false;
        if let Some(lo) = last_orig {
            if let Some(to) = tag_orig {
                // Two Delete tags: check orig adjacency
                if to > lo + 1 {
                    is_gap = true;
                }
            }
        }
        if let Some(ln) = last_new {
            if let Some(tn) = tag_new {
                // Two Insert tags: check new adjacency
                if tn > ln + 1 {
                    is_gap = true;
                }
            }
        }

        // If there's a gap and we have an existing hunk, flush it
        if is_gap {
            if let Some((ds, de)) = cur_del.take() {
                let (ns, ne) = cur_ins.take().unwrap_or((ds, ds));
                hunks.push(DiffHunk { orig_start: ds, orig_end: de, new_start: ns, new_end: ne });
            } else if let Some((ns, ne)) = cur_ins.take() {
                hunks.push(DiffHunk { orig_start: ns.min(m), orig_end: ns.min(m), new_start: ns, new_end: ne });
            }
            last_orig = None;
            last_new = None;
        }

        // Extend current hunk
        match tag {
            Tag::Delete(idx) => {
                match &mut cur_del {
                    Some((_, end)) => { *end = idx + 1; }
                    None => { cur_del = Some((*idx, idx + 1)); }
                }
                last_orig = Some(*idx);
            }
            Tag::Insert(idx) => {
                match &mut cur_ins {
                    Some((_, end)) => { *end = idx + 1; }
                    None => { cur_ins = Some((*idx, idx + 1)); }
                }
                last_new = Some(*idx);
            }
        }
    }

    // Flush final hunk
    if let Some((ds, de)) = cur_del {
        let (ns, ne) = cur_ins.unwrap_or((ds, ds));
        hunks.push(DiffHunk { orig_start: ds, orig_end: de, new_start: ns, new_end: ne });
    } else if let Some((ns, ne)) = cur_ins {
        hunks.push(DiffHunk { orig_start: ns.min(m), orig_end: ns.min(m), new_start: ns, new_end: ne });
    }

    // Merge nearby hunks (within 6 lines in orig) for context overlap
    if hunks.len() <= 1 {
        return hunks;
    }
    let gap = 6;
    let mut merged: Vec<DiffHunk> = Vec::new();
    let (mut cos, mut coe) = (hunks[0].orig_start, hunks[0].orig_end);
    let (mut cns, mut cne) = (hunks[0].new_start, hunks[0].new_end);

    for hunk in hunks.iter().skip(1) {
        if hunk.orig_start <= coe + gap {
            coe = hunk.orig_end;
            cne = hunk.new_end;
        } else {
            merged.push(DiffHunk { orig_start: cos, orig_end: coe, new_start: cns, new_end: cne });
            cos = hunk.orig_start;
            coe = hunk.orig_end;
            cns = hunk.new_start;
            cne = hunk.new_end;
        }
    }
    merged.push(DiffHunk { orig_start: cos, orig_end: coe, new_start: cns, new_end: cne });
    merged
}

/// Fold common Unicode characters to their ASCII equivalents for fuzzy comparison.
/// This handles the common case where an LLM substitutes ASCII lookalikes for
/// Unicode characters (e.g., em dash -> "--", smart quotes -> ASCII quotes).
fn fold_unicode_to_ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            // Dashes
            '\u{2014}' => out.push_str("--"),  // em dash -> --
            '\u{2013}' => out.push('-'),       // en dash -> -
            '\u{2015}' => out.push_str("--"),  // horizontal bar -> --
            '\u{2010}' => out.push('-'),       // hyphen -> -
            '\u{2011}' => out.push('-'),       // non-breaking hyphen -> -
            '\u{2012}' => out.push('-'),       // figure dash -> -
            // Quotes
            '\u{2018}' => out.push('\''),      // left single quotation mark -> '
            '\u{2019}' => out.push('\''),      // right single quotation mark -> '
            '\u{201C}' => out.push('"'),       // left double quotation mark -> "
            '\u{201D}' => out.push('"'),       // right double quotation mark -> "
            // Dots
            '\u{00B7}' => out.push('.'),       // middle dot -> .
            '\u{2026}' => out.push_str("..."), // ellipsis -> ...
            // Spaces
            '\u{00A0}' => out.push(' '),       // non-breaking space -> space
            '\u{2003}' => out.push(' '),       // em space -> space
            '\u{2009}' => out.push(' '),       // thin space -> space
            _ => out.push(ch),
        }
    }
    out
}

/// Result of a trailing-whitespace-tolerant literal match attempt.
enum LiteralMatchResult {
    /// A match was found and the replacement was applied.
    Matched {
        new_content: String,
        match_count: usize,
    },
    /// No match found even with whitespace tolerance.
    /// `leading_ws_hint` is true when the pattern would match if leading
    /// whitespace (indentation) were normalized — this helps the agent
    /// diagnose why the pattern didn't match.
    NoMatch {
        leading_ws_hint: bool,
    },
}

/// Attempt a trailing-whitespace-tolerant literal match. This is used in
/// three contexts: (1) explicit literal mode (literal: true), (2) the
/// degenerate-regex→literal fallback, and (3) the zero-regex-match fallback.
///
/// The algorithm:
/// 1. Treat the pattern as literal text (regex metacharacters are not interpreted).
/// 2. Strip trailing whitespace from each line of both the pattern and the file content.
/// 3. Search for the stripped pattern in the stripped file content.
/// 4. If found, map the match back to the original (unstripped) content.
/// 5. Apply the replacement, preserving the original trailing whitespace
///    for lines that are kept from the match (not added by the replacement).
/// 6. If no match is found, check whether the pattern would match with
///    leading-whitespace normalization (strip leading whitespace from each
///    line) and return a hint if so.
///
/// When `enforce_line_boundaries` is true (auto-retry paths), matches must
/// align with line boundaries — the pattern must match one or more complete
/// lines. When false (explicit literal mode), mid-line substring matches
/// are allowed, giving standard find-and-replace semantics.
fn try_literal_trailing_ws_match(
    original: &str,
    pattern: &str,
    replacement: &str,
    max_replacements: usize,
    enforce_line_boundaries: bool,
) -> LiteralMatchResult {
    // Strip trailing whitespace from each line of the pattern and the
    // file content, then search for the stripped pattern as literal text.
    let stripped_pattern = strip_trailing_ws_per_line(pattern);
    let stripped_original = strip_trailing_ws_per_line(original);

    // Don't attempt fallback for empty or single-character patterns
    // (too likely to produce false positives).
    if stripped_pattern.len() <= 1 {
        return LiteralMatchResult::NoMatch { leading_ws_hint: false };
    }

    // Find all matches of the stripped pattern in the stripped content.
    // When `enforce_line_boundaries` is true (the default for auto-retry
    // paths), only accept matches that start and end at line boundaries —
    // i.e., the pattern must match one or more complete lines, not a
    // substring within a line. This prevents an unindented pattern like
    // "let x = 1;" from matching an indented line like "    let x = 1;"
    // as a substring, which would bypass the leading-whitespace hint.
    // When `enforce_line_boundaries` is false (explicit literal mode),
    // mid-line substring matches are allowed, giving standard find-and-
    // replace semantics.
    // Respect max_replacements: 0 means unlimited.
    let mut match_ranges: Vec<(usize, usize)> = Vec::new();
    let mut search_start = 0;
    while let Some(pos) = stripped_original[search_start..].find(&stripped_pattern) {
        let abs_pos = search_start + pos;
        let abs_end = abs_pos + stripped_pattern.len();

        // Check that the match starts at a line boundary.
        let starts_at_line_boundary = abs_pos == 0
            || stripped_original.as_bytes().get(abs_pos - 1) == Some(&b'\n');

        // Check that the match ends at a line boundary.
        let ends_at_line_boundary = abs_end == stripped_original.len()
            || stripped_original.as_bytes().get(abs_end) == Some(&b'\n');

        if !enforce_line_boundaries || (starts_at_line_boundary && ends_at_line_boundary) {
            match_ranges.push((abs_pos, abs_end));
            if max_replacements > 0 && match_ranges.len() >= max_replacements {
                break;
            }
        }

        search_start = abs_pos + 1;
    }

    if match_ranges.is_empty() {
        // Check if the pattern would match with leading-whitespace
        // normalization (strip leading whitespace from each line of
        // both the pattern and the file content). This helps the agent
        // diagnose indentation mismatches.
        let leading_ws_hint = check_leading_ws_hint(&stripped_original, &stripped_pattern);
        return LiteralMatchResult::NoMatch { leading_ws_hint };
    }

    log::warn!(
        "files_replace: regex matched 0 times but trailing-whitespace-tolerant \
         literal search found {} match(es); pattern had different trailing whitespace \
         than the file content",
        match_ranges.len(),
    );

    // Map each match range in the stripped content back to a range in the
    // original content, then apply the replacement with trailing whitespace
    // preservation.
    //
    // To map stripped positions to original positions, we build a mapping
    // from stripped byte offsets to original byte offsets by walking both
    // strings simultaneously, accounting for the trailing whitespace that
    // was stripped.
    let stripped_to_orig = build_stripped_to_original_offset_map(original, &stripped_original);

    let mut result = String::with_capacity(original.len());
    let mut last_orig_end = 0;
    let mut match_count = 0;

    for (stripped_start, stripped_end) in &match_ranges {
        let orig_start = stripped_to_orig[*stripped_start];
        let orig_end = stripped_to_orig[*stripped_end];

        // Copy content before this match
        result.push_str(&original[last_orig_end..orig_start]);

        // Extract the original matched text and its trailing whitespace
        let original_matched = &original[orig_start..orig_end];
        let orig_trailing_ws: Vec<String> = original_matched
            .lines()
            .map(|l| {
                let trimmed = l.trim_end();
                l[trimmed.len()..].to_string()
            })
            .collect();

        // Apply trailing whitespace from the original to the replacement.
        // For each line in the replacement, if there is a corresponding
        // original line with trailing whitespace, strip the replacement
        // line's trailing whitespace and re-append the original's.
        let mut replacement_lines: Vec<String> = replacement.lines().map(String::from).collect();
        for (i, ws) in orig_trailing_ws.iter().enumerate() {
            if !ws.is_empty() && i < replacement_lines.len() {
                let trimmed = replacement_lines[i].trim_end();
                replacement_lines[i] = format!("{}{}", trimmed, ws);
            }
        }

        result.push_str(&replacement_lines.join("\n"));
        last_orig_end = orig_end;
        match_count += 1;
    }

    // Copy remaining content after the last match
    result.push_str(&original[last_orig_end..]);

    LiteralMatchResult::Matched {
        new_content: result,
        match_count,
    }
}

/// Compute the safety cap for regex match counts. When the number of regex
/// matches exceeds this value (and `max_replacements` is unlimited), the
/// pattern is likely degenerate — e.g., an alternation like `| foo | bar |`
/// matching at every character boundary. The cap is the larger of twice the
/// file's line count or 10, which accommodates legitimate multi-match
/// regexes while catching catastrophic single-character matches.
fn degenerate_match_cap(line_count: usize) -> usize {
    std::cmp::max(line_count * 2, 10)
}

/// Check whether a pattern would match if leading whitespace (indentation)
/// were stripped from each line of both the content and the pattern. Returns
/// true if the leading-whitespace-stripped pattern is found in the
/// leading-whitespace-stripped content.
///
/// This is used to provide a diagnostic hint when a pattern fails to match,
/// helping the agent understand that the mismatch is due to indentation
/// differences rather than content differences.
fn check_leading_ws_hint(stripped_original: &str, stripped_pattern: &str) -> bool {
    // strip_trailing_ws_per_line has already been applied; now also strip
    // leading whitespace from each line.
    let strip_both_ws = |s: &str| -> String {
        s.lines().map(|l| l.trim()).collect::<Vec<_>>().join("\n")
    };
    let both_stripped_original = strip_both_ws(stripped_original);
    let both_stripped_pattern = strip_both_ws(stripped_pattern);

    // Don't report hints for very short patterns (likely false positives)
    if both_stripped_pattern.len() <= 1 {
        return false;
    }

    both_stripped_original.contains(&both_stripped_pattern)
}

/// Strip trailing whitespace from each line of a string, preserving
/// the newline structure.
fn strip_trailing_ws_per_line(s: &str) -> String {
    s.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
}

/// Build a mapping from byte offsets in the stripped version of a string
/// back to byte offsets in the original string. The stripped version is
/// produced by `strip_trailing_ws_per_line`, which removes trailing
/// whitespace from each line.
///
/// The returned vector has one entry per byte in the stripped string:
/// `map[i]` is the corresponding byte offset in the original string.
fn build_stripped_to_original_offset_map(original: &str, stripped: &str) -> Vec<usize> {
    let mut map = Vec::with_capacity(stripped.len() + 1);
    let mut orig_pos = 0;
    let mut stripped_pos = 0;

    for orig_line in original.lines() {
        let stripped_line = orig_line.trim_end();
        let trailing_ws_len = orig_line.len() - stripped_line.len();

        // Map each byte of the stripped line
        for _ in 0..stripped_line.len() {
            map.push(orig_pos);
            orig_pos += 1;
            stripped_pos += 1;
        }

        // Skip the trailing whitespace in the original
        orig_pos += trailing_ws_len;

        // Handle the newline between lines
        // In the stripped string, lines are joined with \n
        // In the original string, there may be \n or \r\n between lines
        if stripped_pos < stripped.len() {
            // There's a \n in the stripped string at this position
            // Find the corresponding newline in the original
            if orig_pos < original.len() {
                if original.as_bytes().get(orig_pos) == Some(&b'\r') && original.as_bytes().get(orig_pos + 1) == Some(&b'\n') {
                    // CRLF: the \n in stripped corresponds to the \n in original
                    // Skip the \r
                    orig_pos += 1;
                }
                // Map the \n
                map.push(orig_pos);
                orig_pos += 1;
                stripped_pos += 1;
            }
        }
    }

    // Add one extra entry for end-of-string lookups
    map.push(orig_pos);

    map
}

/// Verify that the content at [start_line, end_line] in `original` matches `expected`.
/// Returns `Ok(Vec<String>)` on match. For stages 1-3, the vector is empty (no
/// whitespace correction needed). For stage 4 (trailing-whitespace-tolerant match),
/// the vector contains the trailing whitespace from each original line (empty strings
/// for lines with no trailing whitespace), so the caller can preserve it in the
/// replacement content. Returns an error describing the mismatch if all stages fail.
///
/// Comparison is done in four stages:
/// 1. Exact byte-for-byte match (fast path)
/// 2. Unicode NFC-normalized match (handles normalization form differences)
/// 3. Unicode-to-ASCII folded match (handles LLM substitution of ASCII lookalikes)
/// 4. Trailing-whitespace-tolerant match (handles LLM dropping trailing whitespace)
///
/// Stages 2-4 matches are accepted with a log warning, since the file content
/// has almost certainly not changed -- the only difference is how the LLM
/// represented the characters or whitespace.
fn verify_original(original: &str, start_line: usize, end_line: usize, expected: &str) -> Result<Vec<String>> {
    use unicode_normalization::UnicodeNormalization;

    let lines: Vec<&str> = original.lines().collect();
    let effective_end = end_line.min(lines.len());
    let actual_lines = &lines[start_line - 1..effective_end];
    let actual: String = actual_lines.join("\n");

    // Stage 1: Exact match
    if actual == expected {
        return Ok(Vec::new());
    }

    // Stage 2: NFC-normalized match
    let actual_nfc: String = actual.nfc().collect();
    let expected_nfc: String = expected.nfc().collect();
    if actual_nfc == expected_nfc {
        log::warn!(
            "original_content: exact match failed but NFC-normalized match succeeded \
             (lines {}-{}); this indicates a Unicode normalization form difference, \
             not a content change",
            start_line,
            effective_end,
        );
        return Ok(Vec::new());
    }

    // Stage 3: Unicode-to-ASCII folded match
    let actual_folded = fold_unicode_to_ascii(&actual_nfc);
    let expected_folded = fold_unicode_to_ascii(&expected_nfc);
    if actual_folded == expected_folded {
        log::warn!(
            "original_content: exact and NFC match failed but Unicode-ASCII folded match succeeded \
             (lines {}-{}); the LLM likely substituted ASCII lookalikes for Unicode characters",
            start_line,
            effective_end,
        );
        return Ok(Vec::new());
    }

    // Stage 4: Trailing-whitespace-tolerant match
    // LLMs frequently drop or alter trailing whitespace when reproducing file
    // content. When the only difference is trailing whitespace per line, we
    // accept the match and return the trailing whitespace from the original
    // lines so the caller can preserve it in the replacement content.
    // Leading whitespace (indentation) is not stripped — it is semantically
    // meaningful. Like stages 2-3, this is an optimistic concurrency check,
    // not a security boundary.
    let strip_trailing_ws = |s: &str| -> String {
        s.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
    };
    let actual_trimmed = strip_trailing_ws(&actual_folded);
    let expected_trimmed = strip_trailing_ws(&expected_folded);
    if actual_trimmed == expected_trimmed {
        log::warn!(
            "original_content: exact, NFC, and folded match all failed but trailing-whitespace-tolerant match succeeded \
             (lines {}-{}); the LLM likely dropped trailing whitespace when reproducing file content",
            start_line,
            effective_end,
        );
        // Extract trailing whitespace from each original line for reapplication
        // to the replacement content.
        let trailing_ws: Vec<String> = actual_lines
            .iter()
            .map(|l| {
                let trimmed = l.trim_end();
                l[trimmed.len()..].to_string()
            })
            .collect();
        return Ok(trailing_ws);
    }

    // All stages failed -- genuine mismatch
    let expected_fmt = expected.lines().map(|l| format!("    {}", l)).collect::<Vec<_>>().join("\n");
    let actual_fmt = actual.lines().map(|l| format!("    {}", l)).collect::<Vec<_>>().join("\n");

    // When the strings are the same length but differ, include byte-level
    // diff info so invisible character mismatches can be diagnosed.
    let byte_hint = if actual.len() == expected.len() {
        let first_diff = actual.bytes().zip(expected.bytes())
            .position(|(a, e)| a != e);
        match first_diff {
            Some(pos) => format!(
                "\n  hint: same length ({} bytes) but differ at byte offset {}; expected byte 0x{:02x}, actual byte 0x{:02x}",
                expected.len(),
                pos,
                expected.as_bytes().get(pos).copied().unwrap_or(0),
                actual.as_bytes().get(pos).copied().unwrap_or(0),
            ),
            None => String::new(),
        }
    } else {
        format!(
            "\n  hint: different lengths - expected {} bytes, actual {} bytes",
            expected.len(),
            actual.len(),
        )
    };

    anyhow::bail!(
        "original_content mismatch at lines {}-{}:\n  expected:\n{}\n  actual:\n{}\n{}",
        start_line,
        effective_end,
        expected_fmt,
        actual_fmt,
        byte_hint,
    )
}

/// Extract YAML frontmatter from markdown content (between first `---` pair).
/// Returns the YAML content without the `---` delimiters, or None if no
/// frontmatter is found.
fn extract_frontmatter(content: &str) -> Option<String> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    // Skip the opening --- line.
    let after_open = &trimmed[3..];
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);
    // Find the closing ---.
    if let Some(close_pos) = after_open.find("\n---") {
        let yaml = &after_open[..close_pos];
        if yaml.trim().is_empty() {
            return None;
        }
        Some(format!("{}\n", yaml))
    } else {
        None
    }
}

/// True if `line` is a YAML mapping entry for exactly `key` (not a longer key
/// that shares a prefix, e.g. `author:` vs `authorized:`).
fn frontmatter_line_matches_key(line: &str, key: &str) -> bool {
    let prefix = format!("{}:", key);
    if !line.starts_with(&prefix) {
        return false;
    }
    match line.get(prefix.len()..) {
        None | Some("") => true,
        Some(rest) => rest.starts_with(char::is_whitespace),
    }
}

/// Set a top-level frontmatter key to a value. If the key exists, its line is
/// replaced. If the key does not exist, it is inserted before the closing `---`.
/// If there is no frontmatter block, one is created at the top.
fn edit_frontmatter(content: &str, key: &str, value: &str) -> String {
    let trimmed = content.trim_start();
    let leading = &content[..content.len() - trimmed.len()];

    if !trimmed.starts_with("---") {
        // No frontmatter -- create one.
        // Add a blank line after the closing --- for standard YAML frontmatter convention.
        let separator = if trimmed.is_empty() { "" } else { "\n" };
        return format!("{}---\n{}: {}\n---\n{}{}", leading, key, value, separator, trimmed);
    }

    let lines: Vec<&str> = trimmed.lines().collect();
    let mut result = Vec::new();
    let mut in_frontmatter = false;
    let mut found_key = false;
    let mut closed = false;

    for (i, line) in lines.iter().enumerate() {
        if i == 0 && line.trim() == "---" {
            in_frontmatter = true;
            result.push(line.to_string());
            continue;
        }
        if in_frontmatter && !closed {
            if line.trim() == "---" {
                // Closing delimiter -- insert key if not yet found.
                if !found_key {
                    result.push(format!("{}: {}", key, value));
                }
                result.push(line.to_string());
                closed = true;
                in_frontmatter = false;
                continue;
            }
            if frontmatter_line_matches_key(line, key) {
                result.push(format!("{}: {}", key, value));
                found_key = true;
                continue;
            }
        }
        result.push(line.to_string());
    }

    let mut out = format!("{}{}", leading, result.join("\n"));
    // Preserve trailing newline if original had one.
    if content.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Remove a top-level frontmatter key. Returns the content unchanged if the key
/// is not found or there is no frontmatter.
fn delete_frontmatter_key(content: &str, key: &str) -> String {
    let trimmed = content.trim_start();
    let leading = &content[..content.len() - trimmed.len()];
    if !trimmed.starts_with("---") {
        return content.to_string();
    }

    let lines: Vec<&str> = trimmed.lines().collect();
    let mut result = Vec::new();
    let mut in_frontmatter = false;
    let mut closed = false;

    for (i, line) in lines.iter().enumerate() {
        if i == 0 && line.trim() == "---" {
            in_frontmatter = true;
            result.push(line.to_string());
            continue;
        }
        if in_frontmatter && !closed {
            if line.trim() == "---" {
                closed = true;
                in_frontmatter = false;
                result.push(line.to_string());
                continue;
            }
            if frontmatter_line_matches_key(line, key) {
                // Skip this line (delete the key).
                continue;
            }
        }
        result.push(line.to_string());
    }

    let mut out = format!("{}{}", leading, result.join("\n"));
    if content.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Walk all .md files under `root` and replace `[[old_name]]` and `[[old_name|`
/// with `[[new_name]]` and `[[new_name|`. Returns the number of files modified.
fn update_wikilinks(
    root: &std::path::Path,
    old_name: &str,
    new_name: &str,
) -> Result<usize> {
    let mut updated = 0;
    let plain_pattern = format!("[[{}]]", old_name);
    let alias_pattern = format!("[[{}|", old_name);
    let plain_replacement = format!("[[{}]]", new_name);
    let alias_replacement = format!("[[{}|", new_name);

    walk_md_files(root, &mut |path| {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };
        if !content.contains(&plain_pattern) && !content.contains(&alias_pattern) {
            return;
        }
        let new_content = content
            .replace(&plain_pattern, &plain_replacement)
            .replace(&alias_pattern, &alias_replacement);
        if new_content != content {
            if std::fs::write(path, &new_content).is_ok() {
                updated += 1;
            }
        }
    })?;

    Ok(updated)
}

/// Recursively walk a directory, calling `f` on each .md file.
fn walk_md_files(
    dir: &std::path::Path,
    f: &mut impl FnMut(&std::path::Path),
) -> Result<()> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("failed to read directory {}: {}", dir.display(), e))?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_md_files(&path, f)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            f(&path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::edit_frontmatter;
    use super::delete_frontmatter_key;
    use super::patch_string;
    use super::format_patch_diff;
    use super::format_replace_diff;
    use super::compute_diff_hunks;
    use super::verify_original;
    use super::fold_unicode_to_ascii;
    use super::try_literal_trailing_ws_match;
    use super::degenerate_match_cap;
    use super::strip_trailing_ws_per_line;
    use super::build_stripped_to_original_offset_map;
    use super::LiteralMatchResult;

    #[test]
    fn edit_frontmatter_updates_existing_with_leading_newlines() {
        let input = "\n\n---\ntitle: old\nkind: note\n---\nbody\n";
        let out = edit_frontmatter(input, "title", "new");
        assert!(out.contains("\n\n---\ntitle: new\nkind: note\n---\nbody\n"));
    }

    #[test]
    fn edit_frontmatter_inserts_missing_key_with_leading_whitespace() {
        let input = "  \n---\ntitle: old\n---\nbody\n";
        let out = edit_frontmatter(input, "author", "ryan");
        assert!(out.contains("author: ryan"));
    }

    #[test]
    fn edit_frontmatter_creates_frontmatter_without_moving_leading_whitespace_into_body_gap() {
        let input = "\n\nbody\n";
        let out = edit_frontmatter(input, "title", "new");
        assert!(out.starts_with("\n\n---\ntitle: new\n---\n\nbody\n"));
    }

    #[test]
    fn delete_frontmatter_key_works_with_leading_whitespace() {
        let input = "  \n---\ntitle: old\nremove: me\nkind: note\n---\nbody\n";
        let out = delete_frontmatter_key(input, "remove");
        assert!(out.contains("  \n---\ntitle: old\nkind: note\n---\nbody\n"));
    }

    #[test]
    fn edit_frontmatter_does_not_match_longer_key_with_same_prefix() {
        let input = "---\nauthor: old\nauthorized: yes\n---\n";
        let out = edit_frontmatter(input, "author", "new");
        assert!(out.contains("author: new"));
        assert!(out.contains("authorized: yes"));
        assert!(!out.contains("authorized: new"));
    }

    #[test]
    fn delete_frontmatter_key_does_not_remove_longer_key_with_same_prefix() {
        let input = "---\nauthor: old\nauthorized: yes\n---\n";
        let out = delete_frontmatter_key(input, "author");
        assert!(!out.contains("author: old"));
        assert!(out.contains("authorized: yes"));
    }

    // --- patch_string tests ---

    #[test]
    fn patch_string_replaces_single_line() {
        let input = "a\nb\nc";
        let out = patch_string(input, 2, None, "replaced");
        assert_eq!(out, "a\nreplaced\nc");
    }

    #[test]
    fn patch_string_replaces_range() {
        let input = "a\nb\nc\nd\ne";
        let out = patch_string(input, 2, Some(4), "x\ny");
        assert_eq!(out, "a\nx\ny\ne");
    }

    #[test]
    fn patch_string_expands_range() {
        let input = "a\nb\nc";
        let out = patch_string(input, 2, Some(2), "x\ny\nz");
        assert_eq!(out, "a\nx\ny\nz\nc");
    }

    #[test]
    fn patch_string_contracts_range() {
        let input = "a\nb\nc\nd\ne";
        let out = patch_string(input, 2, Some(4), "x");
        assert_eq!(out, "a\nx\ne");
    }

    #[test]
    fn patch_string_deletes_range_with_empty_replacement() {
        let input = "a\nb\nc\nd";
        let out = patch_string(input, 2, Some(3), "");
        assert_eq!(out, "a\nd");
    }

    #[test]
    fn patch_string_replaces_first_line() {
        let input = "a\nb\nc";
        let out = patch_string(input, 1, None, "x");
        assert_eq!(out, "x\nb\nc");
    }

    #[test]
    fn patch_string_replaces_last_line() {
        let input = "a\nb\nc";
        let out = patch_string(input, 3, None, "x");
        assert_eq!(out, "a\nb\nx");
    }

    #[test]
    fn patch_string_replaces_all_lines() {
        let input = "a\nb\nc";
        let out = patch_string(input, 1, Some(3), "x\ny");
        assert_eq!(out, "x\ny");
    }

    #[test]
    fn patch_string_start_line_past_end_returns_original() {
        let input = "a\nb\nc";
        let out = patch_string(input, 10, None, "x");
        assert_eq!(out, "a\nb\nc");
    }

    #[test]
    fn patch_string_end_line_past_file_is_clamped() {
        let input = "a\nb\nc";
        // end_line=100 exceeds file length (3), so effective_end=3
        let out = patch_string(input, 2, Some(100), "x");
        assert_eq!(out, "a\nx");
    }

    #[test]
    fn patch_string_preserves_no_trailing_newline() {
        let input = "a\nb\nc";
        let out = patch_string(input, 2, None, "x");
        assert_eq!(out, "a\nx\nc");
    }

    #[test]
    fn patch_string_single_line_file() {
        let input = "only";
        let out = patch_string(input, 1, None, "replaced");
        assert_eq!(out, "replaced");
    }

    #[test]
    fn patch_string_replacement_without_trailing_newline() {
        let input = "a\nb\nc\n";
        let out = patch_string(input, 2, Some(2), "x");
        assert_eq!(out, "a\nx\nc\n");
    }

    #[test]
    fn patch_string_replacement_with_trailing_newline() {
        let input = "a\nb\nc\n";
        let out = patch_string(input, 2, Some(2), "x\n");
        // The replacement "x\n" already ends with \n, so the trailing \n is not doubled
        assert_eq!(out, "a\nx\nc\n");
    }

    // --- format_patch_diff tests ---

    #[test]
    fn format_patch_diff_shows_removed_and_added() {
        let input = "a\nb\nc\nd\ne";
        let diff = format_patch_diff(input, 2, Some(3), "x\ny");
        assert!(diff.contains("-2|b"));
        assert!(diff.contains("-3|c"));
        assert!(diff.contains("+2|x"));
        assert!(diff.contains("+3|y"));
    }

    #[test]
    fn format_patch_diff_shows_context_before() {
        let input = "a\nb\nc\nd\ne";
        let diff = format_patch_diff(input, 5, Some(5), "X");
        assert!(diff.contains(" 4|d"));
    }

    #[test]
    fn format_patch_diff_shows_context_after() {
        let input = "a\nb\nc\nd\ne";
        let diff = format_patch_diff(input, 2, Some(2), "X");
        assert!(diff.contains(" 3|c"));
    }

    #[test]
    fn format_patch_diff_handles_start_of_file() {
        let input = "a\nb\nc\nd\ne";
        let diff = format_patch_diff(input, 1, Some(1), "X");
        assert!(diff.contains("+1|X"));
        assert!(!diff.contains(" 0|"));
    }

    #[test]
    fn format_patch_diff_handles_end_of_file() {
        let input = "a\nb\nc\nd\ne";
        let diff = format_patch_diff(input, 5, Some(5), "X");
        assert!(diff.contains("+5|X"));
    }

    #[test]
    fn format_patch_diff_single_line_replacement() {
        let input = "a\nb\nc";
        let diff = format_patch_diff(input, 2, None, "X");
        assert!(diff.contains("-2|b"));
        assert!(diff.contains("+2|X"));
    }

    #[test]
    fn format_patch_diff_deletion_shows_no_added() {
        let input = "a\nb\nc";
        let diff = format_patch_diff(input, 2, Some(3), "");
        assert!(diff.contains("-2|b"));
        assert!(diff.contains("-3|c"));
        assert!(!diff.contains("+"));
    }

    // --- format_replace_diff tests ---

    #[test]
    fn format_replace_diff_added_lines_use_original_line_numbers() {
        // Replace line 3 ("c") with two lines ("x" and "y").
        // Added lines should use original-file line numbers (3 and 4),
        // not new-file line numbers.
        let original = "a\nb\nc\nd\ne";
        let new = "a\nb\nx\ny\nd\ne";
        let diff = format_replace_diff(original, new, true);
        assert!(diff.contains("-3|c"), "should show removed line with original line number");
        assert!(diff.contains("+3|x"), "added line should use original start line number");
        assert!(diff.contains("+4|y"), "second added line should use original start + 1");
    }

    #[test]
    fn format_replace_diff_context_uses_original_line_numbers() {
        let original = "a\nb\nc\nd\ne";
        let new = "a\nb\nX\nd\ne";
        let diff = format_replace_diff(original, new, true);
        // Context before: lines from original before the change
        assert!(diff.contains(" 2|b"), "context before should use original line number");
        // Context after: lines from original after the change
        assert!(diff.contains(" 4|d"), "context after should use original line number");
    }

    #[test]
    fn format_replace_diff_removal_only_shows_original_line_numbers() {
        let original = "a\nb\nc\nd\ne";
        let new = "a\nb\ne";  // removed lines 3 and 4
        let diff = format_replace_diff(original, new, true);
        assert!(diff.contains("-3|c"), "removed line should use original line number");
        assert!(diff.contains("-4|d"), "removed line should use original line number");
        assert!(!diff.contains("+"), "no added lines expected");
    }

    #[test]
    fn format_replace_diff_all_line_numbers_consistent() {
        // Multi-line replacement where added lines exceed removed lines.
        // All line numbers in the diff should refer to the original file.
        let original = "line1\nline2\nline3\nline4\nline5";
        let new =    "line1\nline2\nnew_a\nnew_b\nnew_c\nline4\nline5";
        let diff = format_replace_diff(original, new, true);
        // Removed: line 3 of original
        assert!(diff.contains("-3|line3"));
        // Added: 3 new lines starting at original position 3
        assert!(diff.contains("+3|new_a"));
        assert!(diff.contains("+4|new_b"));
        assert!(diff.contains("+5|new_c"));
        // Context after: line 4 of original
        assert!(diff.contains(" 4|line4"));
    }

    // --- verify_original tests ---

    #[test]
    fn verify_original_matches_single_line() {
        let input = "a\nb\nc";
        assert!(verify_original(input, 2, 2, "b").is_ok());
    }

    #[test]
    fn verify_original_matches_range() {
        let input = "a\nb\nc\nd";
        assert!(verify_original(input, 2, 4, "b\nc\nd").is_ok());
    }

    #[test]
    fn verify_original_rejects_mismatch() {
        let input = "a\nb\nc";
        let result = verify_original(input, 2, 2, "wrong");
        assert!(result.is_err());
    }

    #[test]
    fn verify_original_rejects_partial_range_mismatch() {
        let input = "a\nb\nc\nd";
        let result = verify_original(input, 2, 3, "b\nwrong");
        assert!(result.is_err());
    }

    #[test]
    fn verify_original_matches_full_file() {
        let input = "a\nb\nc";
        assert!(verify_original(input, 1, 3, "a\nb\nc").is_ok());
    }

    #[test]
    fn verify_original_matches_first_line() {
        let input = "a\nb\nc";
        assert!(verify_original(input, 1, 1, "a").is_ok());
    }

    #[test]
    fn verify_original_matches_last_line() {
        let input = "a\nb\nc";
        assert!(verify_original(input, 3, 3, "c").is_ok());
    }

    #[test]
    fn verify_original_clamps_end_line_past_file() {
        let input = "a\nb\nc";
        assert!(verify_original(input, 2, 100, "b\nc").is_ok());
    }

    // --- verify_original Unicode tests ---

    #[test]
    fn verify_original_accepts_nfc_normalized_match() {
        let nfc = "caf\u{0065}\u{0301}"; // e + combining acute
        let nfd = "caf\u{00e9}"; // precomposed é
        let file_content = format!("a\n{}\nb", nfd);
        assert!(verify_original(&file_content, 2, 2, nfc).is_ok());
    }

    #[test]
    fn verify_original_accepts_smart_quote_as_ascii() {
        let file_content = "a\nOllama\u{2019}s feature\nb";
        assert!(verify_original(file_content, 2, 2, "Ollama's feature").is_ok());
    }

    #[test]
    fn verify_original_accepts_em_dash_as_double_hyphen() {
        let file_content = "a\nsomething \u{2014} other\nb";
        assert!(verify_original(file_content, 2, 2, "something -- other").is_ok());
    }

    #[test]
    fn verify_original_accepts_en_dash_as_hyphen() {
        let file_content = "a\n2024\u{2013}2025\nb";
        assert!(verify_original(file_content, 2, 2, "2024-2025").is_ok());
    }

    #[test]
    fn verify_original_accepts_smart_double_quotes_as_ascii() {
        let file_content = "a\n\u{201C}hello\u{201D} world\nb";
        assert!(verify_original(file_content, 2, 2, "\"hello\" world").is_ok());
    }

    #[test]
    fn verify_original_accepts_middle_dot_as_period() {
        let file_content = "a\ntest \u{00B7} point\nb";
        assert!(verify_original(file_content, 2, 2, "test . point").is_ok());
    }

    #[test]
    fn verify_original_accepts_non_breaking_space_as_space() {
        let file_content = "a\nhello\u{00A0}world\nb";
        assert!(verify_original(file_content, 2, 2, "hello world").is_ok());
    }

    #[test]
    fn verify_original_rejects_genuine_content_mismatch() {
        let file_content = "a\nhello world\nb";
        let result = verify_original(file_content, 2, 2, "goodbye world");
        assert!(result.is_err());
    }

    #[test]
    fn verify_original_rejects_genuine_mismatch_with_unicode() {
        let file_content = "a\nOllama\u{2019}s different\nb";
        let result = verify_original(file_content, 2, 2, "Ollama's different");
        // This should actually succeed because ' and \u{2019} fold to the same ASCII
        // Let's check: fold_unicode_to_ascii converts \u{2019} to ', so "Ollama's different" matches
        assert!(result.is_ok());
    }

    #[test]
    fn verify_original_accepts_trailing_whitespace_difference() {
        let file_content = "a\nhello world   \nb";
        let result = verify_original(file_content, 2, 2, "hello world");
        assert!(result.is_ok());
        let ws = result.unwrap();
        assert_eq!(ws.len(), 1);
        assert_eq!(ws[0], "   ");
    }

    #[test]
    fn verify_original_accepts_trailing_whitespace_difference_multiline() {
        let file_content = "a\nhello   \nworld  \nb";
        let result = verify_original(file_content, 2, 3, "hello\nworld");
        assert!(result.is_ok());
        let ws = result.unwrap();
        assert_eq!(ws.len(), 2);
        assert_eq!(ws[0], "   ");
        assert_eq!(ws[1], "  ");
    }

    #[test]
    fn verify_original_returns_empty_vec_for_exact_match() {
        let file_content = "a\nhello world\nb";
        let result = verify_original(file_content, 2, 2, "hello world");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn verify_original_rejects_leading_whitespace_difference() {
        let file_content = "a\n  hello world\nb";
        let result = verify_original(file_content, 2, 2, "hello world");
        assert!(result.is_err());
    }

    #[test]
    fn verify_original_rejects_content_mismatch_with_trailing_whitespace() {
        let file_content = "a\nhello world   \nb";
        let result = verify_original(file_content, 2, 2, "goodbye world");
        assert!(result.is_err());
    }

    #[test]
    fn verify_original_captures_trailing_tab() {
        let file_content = "a\nhello world\t\nb";
        let result = verify_original(file_content, 2, 2, "hello world");
        assert!(result.is_ok());
        let ws = result.unwrap();
        assert_eq!(ws[0], "\t");
    }

    #[test]
    fn verify_original_captures_mixed_trailing_whitespace() {
        let file_content = "a\nhello world \t \n";
        let result = verify_original(file_content, 2, 2, "hello world");
        assert!(result.is_ok());
        let ws = result.unwrap();
        assert_eq!(ws[0], " \t ");
    }

    #[test]
    fn verify_original_captures_trailing_unicode_whitespace() {
        let file_content = "a\nhello world\u{00A0}\n";
        let result = verify_original(&file_content, 2, 2, "hello world");
        // \u{00A0} folds to ' ' in the Unicode-to-ASCII stage, then the
        // trailing whitespace check strips it. But the original trailing
        // whitespace capture uses the raw original line.
        assert!(result.is_ok());
        let ws = result.unwrap();
        assert_eq!(ws[0], "\u{00A0}");
    }

    #[test]
    fn verify_original_partial_trailing_whitespace() {
        // Agent provides "hello world " but original has "hello world   "
        // The match succeeds (both fold to "hello world" after stripping)
        // and the original's full trailing whitespace is captured.
        let file_content = "a\nhello world   \n";
        let result = verify_original(file_content, 2, 2, "hello world ");
        assert!(result.is_ok());
        let ws = result.unwrap();
        assert_eq!(ws[0], "   ");
    }

    // --- fold_unicode_to_ascii tests ---

    #[test]
    fn fold_em_dash() {
        assert_eq!(fold_unicode_to_ascii("\u{2014}"), "--");
    }

    #[test]
    fn fold_en_dash() {
        assert_eq!(fold_unicode_to_ascii("\u{2013}"), "-");
    }

    #[test]
    fn fold_right_single_quote() {
        assert_eq!(fold_unicode_to_ascii("\u{2019}"), "'");
    }

    #[test]
    fn fold_left_double_quote() {
        assert_eq!(fold_unicode_to_ascii("\u{201C}"), "\"");
    }

    #[test]
    fn fold_non_breaking_space() {
        assert_eq!(fold_unicode_to_ascii("\u{00A0}"), " ");
    }

    #[test]
    fn fold_passthrough_ascii() {
        assert_eq!(fold_unicode_to_ascii("hello world"), "hello world");
    }

    #[test]
    fn fold_mixed_unicode() {
        assert_eq!(
            fold_unicode_to_ascii("Ollama\u{2019}s \u{201C}best\u{201D} \u{2014} ever"),
            "Ollama's \"best\" -- ever"
        );
    }

    // --- strip_trailing_ws_per_line tests ---

    #[test]
    fn strip_trailing_spaces() {
        assert_eq!(strip_trailing_ws_per_line("hello   "), "hello");
    }

    #[test]
    fn strip_trailing_spaces_multiline() {
        assert_eq!(strip_trailing_ws_per_line("hello   \nworld  "), "hello\nworld");
    }

    #[test]
    fn strip_no_trailing_ws() {
        assert_eq!(strip_trailing_ws_per_line("hello\nworld"), "hello\nworld");
    }

    #[test]
    fn strip_trailing_tabs() {
        assert_eq!(strip_trailing_ws_per_line("hello\t\t\nworld\t"), "hello\nworld");
    }

    // --- build_stripped_to_original_offset_map tests ---

    #[test]
    fn offset_map_basic() {
        let original = "hello   \nworld\n";
        let stripped = strip_trailing_ws_per_line(original);
        let map = build_stripped_to_original_offset_map(original, &stripped);
        // "hello" (5 bytes) maps to original 0..5, then \n maps to original 8
        // (after skipping "   " trailing whitespace)
        // "world" (5 bytes) maps to original 9..14, then \n maps to original 14
        assert_eq!(map[0], 0);  // 'h'
        assert_eq!(map[5], 8);  // '\n' after "hello   "
        assert_eq!(map[6], 9);  // 'w'
        assert_eq!(map[11], 14); // '\n' after "world"
    }

    #[test]
    fn offset_map_no_trailing_ws() {
        let original = "hello\nworld\n";
        let stripped = strip_trailing_ws_per_line(original);
        let map = build_stripped_to_original_offset_map(original, &stripped);
        assert_eq!(map[0], 0);
        assert_eq!(map[5], 5);  // '\n'
        assert_eq!(map[6], 6);  // 'w'
    }

    // --- try_literal_trailing_ws_match tests ---

    #[test]
    fn literal_match_single_line() {
        let original = "foo   \nbar\nbaz\n";
        let result = try_literal_trailing_ws_match(original, "foo", "FOO", 0, true);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 1);
                assert_eq!(new_content, "FOO   \nbar\nbaz\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_match_multiline_pattern() {
        let original = "foo   \nbar   \nbaz\n";
        let result = try_literal_trailing_ws_match(original, "foo\nbar", "FOO\nBAR", 0, true);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 1);
                assert_eq!(new_content, "FOO   \nBAR   \nbaz\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_match_no_match() {
        let original = "foo\nbar\nbaz\n";
        let result = try_literal_trailing_ws_match(original, "qux", "QUX", 0, true);
        match result {
            LiteralMatchResult::Matched { .. } => panic!("expected no match"),
            LiteralMatchResult::NoMatch { .. } => {}
        }
    }

    #[test]
    fn literal_match_multiple_occurrences() {
        let original = "foo   \nbar   \nfoo\nbaz\n";
        let result = try_literal_trailing_ws_match(original, "foo", "FOO", 0, true);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 2);
                assert_eq!(new_content, "FOO   \nbar   \nFOO\nbaz\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_match_max_replacements() {
        let original = "foo   \nbar   \nfoo\nbaz\n";
        let result = try_literal_trailing_ws_match(original, "foo", "FOO", 1, true);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 1);
                // Only the first "foo" is replaced; trailing ws preserved
                assert!(new_content.starts_with("FOO   \nbar   \nfoo\nbaz\n"));
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_match_short_pattern_rejected() {
        let original = "a b c\n";
        let result = try_literal_trailing_ws_match(original, "a", "X", 0, true);
        match result {
            LiteralMatchResult::Matched { .. } => panic!("expected no match for short pattern"),
            LiteralMatchResult::NoMatch { .. } => {}
        }
    }

    #[test]
    fn literal_match_preserves_trailing_ws_in_kept_lines() {
        // Pattern "foo\nbar" matches "foo   \nbar   " with trailing ws stripped.
        // Replacement "FOO\nBAR" should have original trailing ws reapplied.
        let original = "foo   \nbar   \nbaz\n";
        let result = try_literal_trailing_ws_match(original, "foo\nbar", "FOO\nBAR", 0, true);
        match result {
            LiteralMatchResult::Matched { new_content, .. } => {
                assert_eq!(new_content, "FOO   \nBAR   \nbaz\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_match_with_max_replacements_replaces_first() {
        let original = "foo   \nbar\nfoo  \nbaz\n";
        let result = try_literal_trailing_ws_match(original, "foo", "FOO", 1, true);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 1);
                assert_eq!(new_content, "FOO   \nbar\nfoo  \nbaz\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_match_no_trailing_ws_in_original() {
        let original = "foo\nbar\nbaz\n";
        let result = try_literal_trailing_ws_match(original, "foo", "FOO", 0, true);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 1);
                assert_eq!(new_content, "FOO\nbar\nbaz\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match"),
        }
    }

    // --- check_leading_ws_hint tests ---

    #[test]
    fn no_match_with_leading_ws_hint() {
        // Pattern lacks indentation that the file has — should get a hint
        let original = "fn main() {\n    let x = 1;\n}\n";
        let result = try_literal_trailing_ws_match(original, "let x = 1;", "let y = 2;", 0, true);
        match result {
            LiteralMatchResult::NoMatch { leading_ws_hint } => {
                assert!(leading_ws_hint, "expected leading_ws_hint to be true");
            }
            LiteralMatchResult::Matched { .. } => panic!("expected no match"),
        }
    }

    #[test]
    fn no_match_without_leading_ws_hint() {
        // Pattern doesn't match at all, even with indentation stripped
        let original = "fn main() {\n    let x = 1;\n}\n";
        let result = try_literal_trailing_ws_match(original, "nonexistent content", "replacement", 0, true);
        match result {
            LiteralMatchResult::NoMatch { leading_ws_hint } => {
                assert!(!leading_ws_hint, "expected leading_ws_hint to be false");
            }
            LiteralMatchResult::Matched { .. } => panic!("expected no match"),
        }
    }

    #[test]
    fn literal_match_line_boundary_rejects_mid_line_substring() {
        // Pattern "x = 1" is a substring of "    let x = 1;" but does not
        // start at a line boundary — should be rejected, with a hint.
        let original = "fn main() {\n    let x = 1;\n}\n";
        let result = try_literal_trailing_ws_match(original, "x = 1;", "x = 2;", 0, true);
        match result {
            LiteralMatchResult::NoMatch { leading_ws_hint } => {
                assert!(leading_ws_hint, "expected leading_ws_hint to be true");
            }
            LiteralMatchResult::Matched { .. } => panic!("expected no match for mid-line substring"),
        }
    }

    #[test]
    fn literal_match_line_boundary_accepts_full_line() {
        // Pattern "    let x = 1;" matches the complete line including indent
        let original = "fn main() {\n    let x = 1;\n}\n";
        let result = try_literal_trailing_ws_match(original, "    let x = 1;", "    let y = 2;", 0, true);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 1);
                assert_eq!(new_content, "fn main() {\n    let y = 2;\n}\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match for full line"),
        }
    }

    // --- literal mode without line-boundary enforcement (Finding 1 fix) ---

    #[test]
    fn literal_explicit_mode_allows_mid_line_substring() {
        // When enforce_line_boundaries is false (explicit literal mode),
        // a pattern that is a substring of a line should match.
        let original = "fn main() {\n    let x = 1;\n}\n";
        let result = try_literal_trailing_ws_match(original, "x = 1;", "x = 2;", 0, false);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 1);
                assert_eq!(new_content, "fn main() {\n    let x = 2;\n}\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match for mid-line substring in explicit literal mode"),
        }
    }

    #[test]
    fn literal_explicit_mode_mid_line_multiple_matches() {
        // Multiple substring matches within different lines
        let original = "foo = 1;\nbar = foo + foo;\nbaz = 2;\n";
        let result = try_literal_trailing_ws_match(original, "foo", "FOO", 0, false);
        match result {
            LiteralMatchResult::Matched { match_count, .. } => {
                assert_eq!(match_count, 3, "should match all 3 occurrences of 'foo'");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected matches for substring"),
        }
    }

    #[test]
    fn literal_explicit_mode_preserves_trailing_ws_on_matched_line() {
        // When a mid-line substring match is replaced, trailing WS on the
        // matched line should still be preserved from the original.
        let original = "    let x = 1;   \n}\n";
        let result = try_literal_trailing_ws_match(original, "x = 1;", "x = 2;", 0, false);
        match result {
            LiteralMatchResult::Matched { new_content, .. } => {
                assert_eq!(new_content, "    let x = 2;   \n}\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_explicit_mode_short_pattern_still_rejected() {
        // Short patterns (<=1 char) are still rejected even in explicit
        // literal mode to prevent false positives.
        let original = "a b c\n";
        let result = try_literal_trailing_ws_match(original, "a", "X", 0, false);
        match result {
            LiteralMatchResult::NoMatch { .. } => {}
            LiteralMatchResult::Matched { .. } => panic!("expected no match for short pattern"),
        }
    }

    #[test]
    fn literal_auto_retry_still_rejects_mid_line_substring() {
        // When enforce_line_boundaries is true (auto-retry paths), the
        // line-boundary constraint still applies.
        let original = "fn main() {\n    let x = 1;\n}\n";
        let result = try_literal_trailing_ws_match(original, "x = 1;", "x = 2;", 0, true);
        match result {
            LiteralMatchResult::NoMatch { leading_ws_hint } => {
                assert!(leading_ws_hint, "expected leading_ws_hint to be true");
            }
            LiteralMatchResult::Matched { .. } => panic!("expected no match for mid-line substring with line-boundary enforcement"),
        }
    }

    // --- degenerate_match_cap tests ---

    #[test]
    fn degenerate_match_cap_small_file() {
        // A 10-line file: cap should be at least 20 (2x line count)
        assert_eq!(degenerate_match_cap(10), 20);
    }

    #[test]
    fn degenerate_match_cap_large_file() {
        // A 200-line file: cap should be 400 (2x line count)
        assert_eq!(degenerate_match_cap(200), 400);
    }

    #[test]
    fn degenerate_match_cap_zero_lines() {
        // An empty file: cap should be 10 (the minimum floor)
        assert_eq!(degenerate_match_cap(0), 10);
    }

    #[test]
    fn degenerate_match_cap_exactly_50_lines() {
        // A 50-line file: 2 * 50 = 100
        assert_eq!(degenerate_match_cap(50), 100);
    }

    // --- degenerate regex → literal promotion tests ---

    #[test]
    fn degenerate_regex_alternation_promotes_to_literal() {
        // A markdown table row with `|` acts as regex alternation,
        // matching at every character boundary. The literal fallback
        // should find the single intended match.
        let original = "| Status | Description |\n| --- | --- |\n| 🟠 P1 | Compute thing |\n";
        let pattern = "| 🟠 P1 | Compute thing |";

        // As regex, this alternation matches every position
        let re = regex::RegexBuilder::new(pattern)
            .multi_line(true)
            .build()
            .unwrap();
        let regex_count = re.captures_iter(original).count();
        let line_count = original.lines().count();
        assert!(regex_count > degenerate_match_cap(line_count),
            "alternation pattern should match far more positions than the cap (matched {})", regex_count);

        // But as literal, it finds exactly 1 match
        let result = try_literal_trailing_ws_match(original, pattern, "| 🟢 P2 | Compute thing |", 0, true);
        match result {
            LiteralMatchResult::Matched { match_count, new_content, .. } => {
                assert_eq!(match_count, 1);
                assert_eq!(new_content, "| Status | Description |\n| --- | --- |\n| 🟢 P2 | Compute thing |\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected literal match for table row"),
        }
    }

    #[test]
    fn legitimate_regex_not_degenerate() {
        // A legitimate regex that matches a reasonable number of times
        // should not exceed the cap.
        let original = "foo bar\nbaz bar\nqux bar\n";
        let re = regex::RegexBuilder::new(r"\bbar\b")
            .multi_line(true)
            .build()
            .unwrap();
        let count = re.captures_iter(original).count();
        let line_count = original.lines().count();
        assert!(count <= degenerate_match_cap(line_count),
            "legitimate regex should not exceed cap (matched {}, cap {})", count, degenerate_match_cap(line_count));
    }

    #[test]
    fn degenerate_regex_empty_alternation_matches_everywhere() {
        // `|` alone is alternation with two empty branches, matching
        // at every position between characters.
        let original = "hello world\n";
        let re = regex::RegexBuilder::new("|")
            .multi_line(true)
            .build()
            .unwrap();
        let count = re.captures_iter(original).count();
        let line_count = original.lines().count();
        assert!(count > degenerate_match_cap(line_count),
            "bare alternation should exceed cap (matched {})", count);
    }

    // --- capture group expansion guard tests ---

    #[test]
    fn no_capture_groups_dollar_sign_preserved_in_replacement() {
        // When the regex has no capture groups, $1-$9 in the replacement
        // should be treated as literal text, not expanded as empty capture
        // group references. Our code path uses regex::NoExpand to prevent
        // the regex crate's default $N expansion when has_capture_groups
        // is false.
        let re = regex::RegexBuilder::new("foo")
            .multi_line(true)
            .build()
            .unwrap();
        assert_eq!(re.captures_len(), 1, "no explicit capture groups");
        let original = "foo bar\n";
        // Using NoExpand (as our code does when has_capture_groups is false)
        // preserves $1-$9 as literal text.
        let result = re.replace_all(original, regex::NoExpand("($1-$9)")).into_owned();
        assert_eq!(result, "($1-$9) bar\n",
            "dollar-sign references should be literal when no capture groups exist");
    }

    #[test]
    fn capture_groups_expanded_in_replacement() {
        // When the regex has capture groups, $1-$9 should be expanded.
        // Our code path uses caps.expand() when has_capture_groups is true.
        let re = regex::RegexBuilder::new(r"(f)(o+)")
            .multi_line(true)
            .build()
            .unwrap();
        assert_eq!(re.captures_len(), 3, "two explicit capture groups + group 0");
        let original = "foo bar\n";
        let result = re.replace_all(original, "$1-$2").into_owned();
        assert_eq!(result, "f-oo bar\n",
            "capture group references should be expanded");
    }

    #[test]
    fn no_capture_groups_literal_dollar_preserved() {
        // A replacement like "$HOME/bin" should not have $HOME expanded
        // when the pattern has no capture groups. Using NoExpand (as our
        // code does) preserves the dollar signs as literal text.
        let re = regex::RegexBuilder::new(r"/usr/bin")
            .multi_line(true)
            .build()
            .unwrap();
        assert_eq!(re.captures_len(), 1);
        let original = "export PATH=/usr/bin\n";
        let result = re.replace_all(original, regex::NoExpand("$HOME/bin")).into_owned();
        assert_eq!(result, "export PATH=$HOME/bin\n",
            "$HOME should be literal when no capture groups");
    }

    // --- Bug 1: verify_original multi-line CRLF and trailing newline tests ---

    #[test]
    fn verify_original_crlf_file_matches_lf_expected() {
        // CRLF in file, LF in expected: .lines() normalizes both
        let file_content = "line1\r\nline2\r\nline3\r\n";
        assert!(verify_original(file_content, 1, 3, "line1\nline2\nline3").is_ok());
    }

    #[test]
    fn verify_original_crlf_multi_line_range() {
        let file_content = "a\r\nb\r\nc\r\nd\r\n";
        assert!(verify_original(file_content, 2, 3, "b\nc").is_ok());
    }

    #[test]
    fn verify_original_crlf_single_line() {
        let file_content = "a\r\nb\r\nc";
        assert!(verify_original(file_content, 2, 2, "b").is_ok());
    }

    #[test]
    fn verify_original_trailing_newline_in_expected() {
        // File has no trailing newline, but expected includes one.
        // .lines() strips trailing newline from both, so the join matches
        // at stage 1.
        let file_content = "line1\nline2";
        assert!(verify_original(file_content, 1, 2, "line1\nline2\n").is_ok());
    }

    #[test]
    fn verify_original_no_trailing_newline_in_expected() {
        // File has trailing newline, expected doesn't.
        // .lines() on the file gives ["line1", "line2"], join → "line1\nline2"
        // .lines() on expected "line1\nline2" gives ["line1", "line2"]
        // But verify_original joins file lines then compares against expected as raw string.
        // actual = "line1\nline2", expected = "line1\nline2" → stage 1 match
        let file_content = "line1\nline2\n";
        assert!(verify_original(file_content, 1, 2, "line1\nline2").is_ok());
    }

    #[test]
    fn verify_original_crlf_trailing_newline_in_expected() {
        // CRLF file with trailing newline in expected
        let file_content = "a\r\nb\r\n";
        assert!(verify_original(file_content, 1, 2, "a\nb\n").is_ok());
    }

    #[test]
    fn verify_original_multi_line_with_trailing_ws_and_crlf() {
        // CRLF file where lines also have trailing whitespace.
        // Stage 4 (trailing WS tolerance) should match.
        let file_content = "hello   \r\nworld  \r\n";
        let result = verify_original(file_content, 1, 2, "hello\nworld");
        assert!(result.is_ok());
        let ws = result.unwrap();
        assert_eq!(ws.len(), 2);
        assert_eq!(ws[0], "   ");
        assert_eq!(ws[1], "  ");
    }

    #[test]
    fn verify_original_rejects_content_mismatch_with_crlf() {
        let file_content = "a\r\nb\r\nc";
        let result = verify_original(file_content, 2, 2, "wrong");
        assert!(result.is_err());
    }

    // --- Bug 2: format_replace_diff trailing newline tests ---

    #[test]
    fn format_replace_diff_shows_trailing_newline_added() {
        let original = "a\nb\nc";
        let new = "a\nb\nc\n";
        let diff = format_replace_diff(original, new, true);
        assert!(diff.contains("+\\n (trailing newline added)"),
            "should indicate trailing newline was added, got: {:?}", diff);
    }

    #[test]
    fn format_replace_diff_shows_trailing_newline_removed() {
        let original = "a\nb\nc\n";
        let new = "a\nb\nc";
        let diff = format_replace_diff(original, new, true);
        assert!(diff.contains("-\\n (trailing newline removed)"),
            "should indicate trailing newline was removed, got: {:?}", diff);
    }

    #[test]
    fn format_replace_diff_no_trailing_newline_change_when_content_differs() {
        // When both content and trailing newline differ, the LCS hunks
        // capture the content changes; the trailing-newline indicator
        // should NOT appear (it only fires when lines are identical).
        let original = "a\nb\nc";
        let new = "a\nX\nc\n";
        let diff = format_replace_diff(original, new, true);
        assert!(!diff.contains("trailing newline"),
            "should not show trailing newline indicator when content differs, got: {:?}", diff);
        assert!(diff.contains("-2|b"), "should show content change");
        assert!(diff.contains("+2|X"), "should show content change");
    }

    #[test]
    fn format_replace_diff_empty_when_identical() {
        let original = "a\nb\nc\n";
        let new = "a\nb\nc\n";
        let diff = format_replace_diff(original, new, true);
        assert!(diff.is_empty(), "identical content should produce empty diff");
    }

    #[test]
    fn format_replace_diff_trailing_newline_added_no_line_numbers() {
        let original = "a\nb";
        let new = "a\nb\n";
        let diff = format_replace_diff(original, new, false);
        assert!(diff.contains("+\\n (trailing newline added)"),
            "should show trailing newline indicator without line numbers, got: {:?}", diff);
    }

    #[test]
    fn format_replace_diff_trailing_newline_single_line_file() {
        let original = "hello";
        let new = "hello\n";
        let diff = format_replace_diff(original, new, true);
        assert!(diff.contains("+\\n (trailing newline added)"),
            "should show trailing newline added for single-line file, got: {:?}", diff);
    }

    #[test]
    fn format_replace_diff_trailing_newline_removed_single_line() {
        let original = "hello\n";
        let new = "hello";
        let diff = format_replace_diff(original, new, true);
        assert!(diff.contains("-\\n (trailing newline removed)"),
            "should show trailing newline removed for single-line file, got: {:?}", diff);
    }

    // --- compute_diff_hunks correctness tests ---

    #[test]
    fn compute_diff_hunks_single_line_change() {
        let orig = vec!["a", "b", "c"];
        let new = vec!["a", "X", "c"];
        let hunks = compute_diff_hunks(&orig, &new);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].orig_start, 1);
        assert_eq!(hunks[0].orig_end, 2);
        assert_eq!(hunks[0].new_start, 1);
        assert_eq!(hunks[0].new_end, 2);
    }

    #[test]
    fn compute_diff_hunks_two_separate_changes() {
        // Two changes separated by >6 unchanged lines in orig so they
        // remain as separate hunks (merge gap is 6).
        let orig = vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"];
        let new = vec!["a", "B", "c", "d", "e", "f", "g", "h", "i", "j", "K"];
        let hunks = compute_diff_hunks(&orig, &new);
        // b→B at index 1 (orig_end=2), K→K at index 10 (orig_start=10)
        // Gap in orig: 10 - 2 = 8 > 6, so separate hunks.
        assert_eq!(hunks.len(), 2, "should have two hunks for two separate changes, got: {:?}", hunks);
        assert_eq!(hunks[0].orig_start, 1);
        assert_eq!(hunks[0].orig_end, 2);
        assert_eq!(hunks[1].orig_start, 10);
        assert_eq!(hunks[1].orig_end, 11);
    }

    #[test]
    fn compute_diff_hunks_nearby_changes_merged() {
        let orig = vec!["a", "b", "c", "d", "e"];
        let new = vec!["a", "B", "c", "D", "e"];
        let hunks = compute_diff_hunks(&orig, &new);
        // Two changes: b→B (line 2) and d→D (line 4), separated by
        // only 1 unchanged line. The merge gap is 6, so they merge.
        assert_eq!(hunks.len(), 1, "nearby changes should be merged into one hunk");
    }

    #[test]
    fn compute_diff_hunks_insertion_only() {
        let orig = vec!["a", "c"];
        let new = vec!["a", "b", "c"];
        let hunks = compute_diff_hunks(&orig, &new);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].orig_start, 1);
        assert_eq!(hunks[0].orig_end, 1); // no lines removed
        assert_eq!(hunks[0].new_start, 1);
        assert_eq!(hunks[0].new_end, 2);
    }

    #[test]
    fn compute_diff_hunks_deletion_only() {
        let orig = vec!["a", "b", "c"];
        let new = vec!["a", "c"];
        let hunks = compute_diff_hunks(&orig, &new);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].orig_start, 1);
        assert_eq!(hunks[0].orig_end, 2);
        assert_eq!(hunks[0].new_start, 1);
        assert_eq!(hunks[0].new_end, 1); // no lines added
    }

    #[test]
    fn compute_diff_hunks_identical() {
        let orig = vec!["a", "b", "c"];
        let new = vec!["a", "b", "c"];
        let hunks = compute_diff_hunks(&orig, &new);
        assert!(hunks.is_empty());
    }

    #[test]
    fn compute_diff_hunks_multi_line_replacement() {
        // Replace one line with three lines
        let orig = vec!["a", "b", "c", "d"];
        let new = vec!["a", "x", "y", "z", "c", "d"];
        let hunks = compute_diff_hunks(&orig, &new);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].orig_start, 1);
        assert_eq!(hunks[0].orig_end, 2); // removed "b"
        assert_eq!(hunks[0].new_start, 1);
        assert_eq!(hunks[0].new_end, 4); // added "x", "y", "z"
    }
}
