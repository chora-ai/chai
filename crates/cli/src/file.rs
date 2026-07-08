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
        /// Overwrite an existing file. Without this flag, writing to an existing file is rejected.
        #[arg(long)]
        overwrite: bool,
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
    /// Edit a file by replacing specific content. Content is read from stdin when --new-content is omitted.
    Edit {
        /// Absolute file path to edit
        #[arg(long)]
        path: String,
        /// Line number of the first line in old_content (1-indexed, inclusive).
        /// Optional — when omitted, the tool searches for old_content and
        /// requires exactly one match.
        #[arg(long)]
        start_line: Option<usize>,
        /// Line number to end replacing at (1-indexed, inclusive). When omitted,
        /// the end line is inferred from the number of lines in old_content
        /// (start_line + old_content lines - 1).
        #[arg(long)]
        end_line: Option<usize>,
        /// The content currently in the file that will be replaced. The number of lines
        /// determines the range when --end-line is omitted. Must match the file
        /// content or the edit is rejected.
        #[arg(long, allow_hyphen_values = true)]
        old_content: Option<String>,
        /// Read old_content from a file instead of passing it as a CLI flag. Takes precedence
        /// over --old-content. This avoids CLI argument encoding issues for content that
        /// must match file content byte-for-byte.
        #[arg(long = "old-content-file")]
        old_content_file: Option<String>,
        /// Replacement content. If ommitted, content is read from stdin.
        /// Accepts values that begin with dashes (e.g. YAML frontmatter).
        #[arg(long = "new-content", allow_hyphen_values = true)]
        new_content: Option<String>,
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
        /// Treat the pattern as literal text instead of regex. Use this when the
        /// pattern contains regex metacharacters (e.g. source code, markdown tables,
        /// JSON) that should be matched as-is rather than interpreted as regex.
        /// Capture groups ($1-$9) are not supported in literal mode.
        #[arg(long)]
        literal: bool,
        /// Check the pattern against a file without making changes. Use this before
        /// making replacements in large files to prevent unintended replacements.
        #[arg(long)]
        dry_run: bool,
        /// Expected number of replacements. When provided, the tool rejects before
        /// writing if the actual match count differs. When omitted and the actual
        /// match count exceeds a safety threshold (5), the tool shows the diff
        /// without writing.
        #[arg(long)]
        count: Option<usize>,
        /// Show line numbers in the diff output
        #[arg(long)]
        line_number: bool,
    },
    /// Read a range of lines from a file with line numbers. Outputs lines in the format {line_number}\t{content}.
    ReadLines {
        /// Absolute file path to read
        #[arg(long)]
        path: String,
        /// Line number to start reading at (1-indexed, inclusive)
        #[arg(long)]
        start_line: usize,
        /// Line number to end reading at (1-indexed, inclusive).
        #[arg(long)]
        end_line: Option<usize>,
    },
    /// Delete a file. Refuses to delete directories.
    Delete {
        /// Absolute file path to delete
        #[arg(long)]
        path: String,
    },
    /// Delete an empty directory. Refuses to delete non-empty directories.
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
    /// Searches all .md files under --scope for [[old-name]] links and replaces
    /// them with [[new-name]].
    Rename {
        /// Absolute path to the existing note
        #[arg(long)]
        from: String,
        /// Absolute path to move the note to (parent directory must exist)
        #[arg(long)]
        to: String,
        /// Directory to search for wikilinks to update (defaults to current directory)
        #[arg(long)]
        scope: Option<String>,
    },
}

pub(crate) fn run_file(cmd: FileCmd) -> Result<()> {
    match cmd {
        FileCmd::Write { path, content, overwrite } => {
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
            if target.exists() && !overwrite {
                anyhow::bail!("overwrite must be set to true to overwrite an existing file");
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
            } else if overwrite {
                println!("wrote {} ({} bytes, new file created)", target.display(), content.len());
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
        FileCmd::Edit { path, start_line, end_line, old_content, old_content_file, new_content } => {
            let new_content = read_content_from_stdin_or(new_content)?;
            // Resolve old_content: --old-content-file takes precedence,
            // then --old-content. File-based passing avoids CLI argument
            // encoding issues for content that must match file content byte-for-byte.
            let old_content = if let Some(ref file_path) = old_content_file {
                Some(std::fs::read_to_string(file_path)
                    .map_err(|e| anyhow::anyhow!("failed to read old-content-file {}: {}", file_path, e))?)
            } else {
                old_content
            };
            let target = std::path::Path::new(&path);
            if !target.exists() {
                anyhow::bail!("file does not exist: {}", path);
            }
            if !target.is_file() {
                anyhow::bail!("not a file: {}", path);
            }

            let original = std::fs::read_to_string(target)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path, e))?;

            // old_content is required — without it there is nothing to match.
            let old_content = old_content.ok_or_else(|| {
                anyhow::anyhow!("old_content is required — provide the content to be replaced")
            })?;

            // When start_line is provided, use it as an anchor (current behavior).
            // When omitted, search for old_content in the file and require exactly one match.
            let start_line = match start_line {
                Some(n) => {
                    if n == 0 {
                        anyhow::bail!("start_line must be at least 1 (1-indexed)");
                    }
                    n
                }
                None => {
                    // Search mode: find all matches of old_content using the
                    // five-stage verification cascade adapted for search.
                    let matches = find_old_content_matches(&original, &old_content);
                    if matches.is_empty() {
                        anyhow::bail!(
                            "old_content not found in {}\n\nhint: use files_read to read the file content, or verify the file path is correct",
                            path,
                        );
                    }
                    if matches.len() > 1 {
                        anyhow::bail!(
                            "old_content matches at {} locations — provide start_line to target a specific occurrence",
                            matches.len(),
                        );
                    }
                    // Exactly one match — use its start line.
                    matches[0].0
                }
            };

            if start_line > original.lines().count() {
                anyhow::bail!(
                    "start_line ({}) exceeds file length ({})",
                    start_line,
                    original.lines().count()
                );
            }

            // Resolve end_line: when not explicitly provided, infer from the
            // number of lines in old_content. This makes the replacement
            // range always match what the agent described, eliminating the trap
            // where end_line defaulted to start_line (single-line) even when
            // old_content spanned multiple lines.
            let end_line = resolve_end_line(start_line, end_line, Some(&old_content))?;
            if end_line < start_line {
                anyhow::bail!("end_line ({}) must be >= start_line ({})", end_line, start_line);
            }

            let effective_end = end_line.min(original.lines().count());

            // Verify old_content against the file content at the resolved
            // location, and collect trailing whitespace from the original file
            // if the match succeeded via stage 4 (trailing-whitespace
            // tolerance). This allows us to preserve the file's trailing
            // whitespace in the replacement content.
            let trailing_ws = verify_original(&original, start_line, effective_end, &old_content)?;

            // Apply trailing whitespace from the original file to the replacement
            // content. When the LLM drops trailing whitespace, the verification
            // (stage 4) accepts the match but we preserve the original whitespace
            // in the output. For each replacement line, strip its trailing whitespace
            // and re-append the original line's trailing whitespace. This handles
            // both cases: the LLM omitted trailing whitespace entirely, or the LLM
            // provided partial trailing whitespace (we replace, not append, to avoid
            // doubling). Extra replacement lines (expanding the range) have no
            // original trailing whitespace to preserve.
            let new_content = if !trailing_ws.is_empty() {
                let mut lines: Vec<String> = new_content.lines().map(String::from).collect();
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
                new_content
            };

            let diff = format_patch_diff(&original, start_line, Some(effective_end), &new_content);
            let result = patch_string(&original, start_line, Some(end_line), &new_content);

            std::fs::write(target, &result)
                .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;

            let removed = effective_end - start_line + 1;
            let added = new_content.lines().count();
            println!(
                "edited {} - removed {} line(s), added {} line(s)\n{}",
                path, removed, added, diff
            );
            Ok(())
        }
        FileCmd::Replace { path, pattern_file, pattern, replacement, dry_run, literal, count, line_number } => {
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
                    &original, &pattern, &replacement, false,
                );

                match literal_match {
                    LiteralMatchResult::Matched { new_content, match_count, match_ranges } => {
                        let new_content = collapse_consecutive_blank_lines(&new_content);

                        let edits = compute_replace_edits(&original, &match_ranges, &replacement);
                        let diff = format_replace_diff_from_edits(&original, &new_content, &edits, line_number);

                        if dry_run {
                            println!(
                                "dry run: {} replacement(s) in {} (literal match)\n{}",
                                match_count, path, diff
                            );
                        } else if check_replace_count(match_count, count, &path, &diff)? {
                            std::fs::write(target, new_content.as_bytes())
                                .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;
                            println!(
                                "{} replacement(s) in {} (literal match)\n{}",
                                match_count, path, diff
                            );
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

            // Collect all captures.
            let all_captures: Vec<_> = re.captures_iter(&original).collect();
            let regex_match_count = all_captures.len();
            let line_count = original.lines().count();

            // Safety: when regex mode matches far more positions than there are
            // lines, the pattern is likely degenerate — e.g., an alternation
            // like `| foo | bar |` matching at every character boundary. In that
            // case, retry as literal before writing anything to disk.
            let match_cap = degenerate_match_cap(line_count);
            if regex_match_count > match_cap {
                // Regex produced an unreasonable number of matches.
                // Try literal mode — if the pattern contains unintentional
                // regex metacharacters, literal matching will find the
                // intended targets. Line-boundary enforcement is on for
                // this auto-retry path to prevent false positives.
                let literal_match = try_literal_trailing_ws_match(
                    &original, &pattern, &replacement, true,
                );
                match literal_match {
                    LiteralMatchResult::Matched { new_content, match_count: literal_count, match_ranges } => {
                        let new_content = collapse_consecutive_blank_lines(&new_content);

                        let edits = compute_replace_edits(&original, &match_ranges, &replacement);
                        let diff = format_replace_diff_from_edits(&original, &new_content, &edits, line_number);

                        if dry_run {
                            println!(
                                "dry run: {} replacement(s) in {} (literal match — regex matched {} positions, likely due to unintentional regex metacharacters)\n{}",
                                literal_count, path, regex_match_count, diff
                            );
                        } else if check_replace_count(literal_count, count, &path, &diff)? {
                            std::fs::write(target, new_content.as_bytes())
                                .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;
                            println!(
                                "{} replacement(s) in {} (literal match — regex matched {} positions, likely due to unintentional regex metacharacters)\n{}",
                                literal_count, path, regex_match_count, diff
                            );
                        }
                        return Ok(());
                    }
                    LiteralMatchResult::NoMatch { .. } => {
                        // Literal mode also didn't find what the agent intended.
                        // Refuse the replacement to prevent file corruption.
                        anyhow::bail!(
                            "regex matched {} positions (exceeds safety cap of {} for a {}-line file) — likely a degenerate pattern. Use literal: true or adjust the pattern.",
                            regex_match_count, match_cap, line_count
                        );
                    }
                }
            }

            if regex_match_count > 0 {
                // Only expand capture group references ($1-$9) when the
                // pattern contains explicit capture groups. When there are
                // no capture groups, caps.expand() would silently consume
                // $N references in the replacement as empty strings — e.g.,
                // "($1-$9)" would become "(-)" instead of the intended
                // literal text. Using the replacement as-is avoids this.
                let has_capture_groups = re.captures_len() > 1;

                // Collect match byte-offset ranges for diff generation.
                let mut match_ranges: Vec<(usize, usize)> = Vec::with_capacity(regex_match_count);
                let mut replacement_texts: Vec<String> = Vec::with_capacity(regex_match_count);
                let mut result = String::with_capacity(original.len());
                let mut last_end = 0;

                for caps in all_captures.iter() {
                    let mat = caps.get(0).unwrap();
                    result.push_str(&original[last_end..mat.start()]);
                    let expanded = if has_capture_groups {
                        let mut expanded = String::new();
                        caps.expand(&replacement, &mut expanded);
                        expanded
                    } else {
                        replacement.clone()
                    };
                    result.push_str(&expanded);
                    match_ranges.push((mat.start(), mat.end()));
                    replacement_texts.push(expanded);
                    last_end = mat.end();
                }
                result.push_str(&original[last_end..]);

                let new_content = collapse_consecutive_blank_lines(&result);

                let edits = if has_capture_groups {
                    compute_replace_edits_with_replacements(&original, &match_ranges, &replacement_texts)
                } else {
                    compute_replace_edits(&original, &match_ranges, &replacement)
                };
                let diff = format_replace_diff_from_edits(&original, &new_content, &edits, line_number);

                if dry_run {
                    println!(
                        "dry run: {} replacement(s) in {}\n{}",
                        regex_match_count, path, diff
                    );
                } else if check_replace_count(regex_match_count, count, &path, &diff)? {
                    std::fs::write(target, new_content.as_bytes())
                        .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;
                    println!(
                        "{} replacement(s) in {}\n{}",
                        regex_match_count, path, diff
                    );
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
                &original, &pattern, &replacement, true,
            );

            match literal_match {
                LiteralMatchResult::Matched { new_content, match_count, match_ranges } => {
                    let new_content = collapse_consecutive_blank_lines(&new_content);

                    let edits = compute_replace_edits(&original, &match_ranges, &replacement);
                    let diff = format_replace_diff_from_edits(&original, &new_content, &edits, line_number);

                    if dry_run {
                        println!(
                            "dry run: {} replacement(s) in {} (trailing-whitespace-tolerant match)\n{}",
                            match_count, path, diff
                        );
                    } else if check_replace_count(match_count, count, &path, &diff)? {
                        std::fs::write(target, new_content.as_bytes())
                            .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;
                        println!(
                            "{} replacement(s) in {} (trailing-whitespace-tolerant match)\n{}",
                            match_count, path, diff
                        );
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
            if let Some(end) = end_line {
                if end < start_line {
                    anyhow::bail!("end_line ({}) must be >= start_line ({})", end, start_line);
                }
            }
            let content = std::fs::read_to_string(target)
                .map_err(|e| anyhow::anyhow!("failed to read {}; {}", path, e))?;
            for (i, line) in content.lines().enumerate() {
                let line_num = i + 1;
                if line_num >= start_line {
                    println!("{}\t{}", line_num, line);
                }
                if let Some(end) = end_line {
                    if line_num >= end {
                        break;
                    }
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
        FileCmd::Rename { from, to, scope } => {
            let from_path = std::path::Path::new(&from);
            let to_path = std::path::Path::new(&to);
            let scope_path = match scope {
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
            if !scope_path.is_dir() {
                anyhow::bail!("scope is not a directory: {}", scope_path.display());
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
                let updated = update_wikilinks(&scope_path, old_name, new_name)?;
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

/// Resolve the end line for an edit operation. When `end_line` is explicitly
/// provided, use it. When omitted, infer from the number of lines in
/// `old_content` so the replacement range matches what the agent
/// described. Returns the resolved end line (1-indexed, inclusive).
fn resolve_end_line(start_line: usize, end_line: Option<usize>, old_content: Option<&str>) -> Result<usize> {
    match end_line {
        Some(explicit) => Ok(explicit),
        None => {
            match old_content {
                Some(expected) => {
                    let expected_lines = expected.lines().count();
                    if expected_lines == 0 {
                        anyhow::bail!("old_content is empty — cannot infer end_line");
                    }
                    Ok(start_line + expected_lines - 1)
                }
                None => {
                    anyhow::bail!("either --end-line or --old-content is required to determine the replacement range");
                }
            }
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
///
/// Line numbers follow the post-edit convention:
/// - **Removed lines** use original-file line numbers.
/// - **Added lines** use new-file line numbers.
/// - **Context-after lines** use new-file line numbers (shifted by the net
///   line change).
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

    // Context lines before the change — these lines are unchanged so their
    // line numbers are the same in both the original and new file.
    for i in ctx_start..(start_line - 1) {
        diff.push_str(&format!(" {}\t{}\n", i + 1, lines[i]));
    }

    // Removed lines — use original-file line numbers since these lines
    // only exist in the original.
    for i in (start_line - 1)..effective_end {
        diff.push_str(&format!("-{}\t{}\n", i + 1, lines[i]));
    }

    // Added lines — use new-file line numbers so the diff reflects the
    // post-edit state.
    let replacement_lines: Vec<&str> = replacement.lines().collect();
    for (offset, line) in replacement_lines.iter().enumerate() {
        diff.push_str(&format!("+{}\t{}\n", start_line + offset, line));
    }

    // Context lines after the change — use new-file line numbers. The shift
    // from original to new line numbers is the net line change:
    //   (replacement lines) - (removed lines).
    let lines_removed = effective_end - (start_line - 1);
    let lines_added = replacement_lines.len();
    let net_change: isize = lines_added as isize - lines_removed as isize;
    for i in effective_end..ctx_end {
        let new_lineno = (i as isize + 1 + net_change) as usize;
        diff.push_str(&format!(" {}\t{}\n", new_lineno, lines[i]));
    }

    diff
}

/// A single replacement edit: lines `orig_start..orig_end` (1-indexed, inclusive
/// on both ends) in the original file were replaced by `replacement_line_count`
/// new lines in the new file. This captures the edit structure directly from the
/// match positions, bypassing the LCS algorithm and avoiding the ambiguity
/// problem when lines repeat in the file.
struct ReplaceEdit {
    /// 1-indexed start line in the original file (inclusive).
    orig_start: usize,
    /// 1-indexed end line in the original file (inclusive).
    orig_end: usize,
    /// Number of lines in the replacement text.
    replacement_line_count: usize,
}

/// Convert a byte offset within `content` to a 1-indexed line number.
///
/// This counts newlines rather than using `.lines().count()`, which avoids
/// an off-by-one error when the offset falls partway through a line:
/// `.lines()` treats a trailing partial line as a complete line, inflating
/// the count by one.
fn byte_offset_to_line(content: &str, offset: usize) -> usize {
    if offset == 0 {
        return 1;
    }
    content[..offset].matches('\n').count() + 1
}

/// Compute `ReplaceEdit` list from byte-offset match ranges in the original
/// content and the replacement text. Each `(byte_start, byte_end)` pair
/// identifies a match in the original; the replacement text is the same for
/// all matches.
fn compute_replace_edits(original: &str, match_ranges: &[(usize, usize)], replacement: &str) -> Vec<ReplaceEdit> {
    let replacement_line_count = if replacement.is_empty() { 0 } else { replacement.lines().count() };
    let mut edits = Vec::with_capacity(match_ranges.len());

    for &(byte_start, byte_end) in match_ranges {
        let start_line = byte_offset_to_line(original, byte_start);
        let end_line = if byte_end == 0 {
            start_line
        } else {
            // The last byte of the match determines the line.
            byte_offset_to_line(original, byte_end - 1).max(start_line)
        };
        edits.push(ReplaceEdit {
            orig_start: start_line,
            orig_end: end_line,
            replacement_line_count,
        });
    }

    edits
}

/// Compute `ReplaceEdit` list from byte-offset match ranges in the original
/// content, where each match may have a different replacement text (e.g.,
/// regex with capture groups). The `replacements` slice must be the same
/// length as `match_ranges`.
fn compute_replace_edits_with_replacements(
    original: &str,
    match_ranges: &[(usize, usize)],
    replacements: &[String],
) -> Vec<ReplaceEdit> {
    assert_eq!(match_ranges.len(), replacements.len());
    let mut edits = Vec::with_capacity(match_ranges.len());

    for (i, &(byte_start, byte_end)) in match_ranges.iter().enumerate() {
        let start_line = byte_offset_to_line(original, byte_start);
        let end_line = if byte_end == 0 {
            start_line
        } else {
            byte_offset_to_line(original, byte_end - 1).max(start_line)
        };
        let replacement_line_count = if replacements[i].is_empty() { 0 } else { replacements[i].lines().count() };
        edits.push(ReplaceEdit {
            orig_start: start_line,
            orig_end: end_line,
            replacement_line_count,
        });
    }

    edits
}

/// Produce a diff from a list of structured edit regions, showing all changed
/// lines with context. Takes explicit edit positions rather than using the LCS
/// algorithm to infer them. This avoids the LCS ambiguity problem when lines
/// repeat in the file (e.g., two struct instances with identical surrounding
/// lines).
///
/// Line numbers follow the post-edit convention: removed lines use
/// original-file line numbers, added and context lines use new-file line
/// numbers.
fn format_replace_diff_from_edits(
    original: &str,
    new: &str,
    edits: &[ReplaceEdit],
    line_number: bool,
) -> String {
    if edits.is_empty() {
        return String::new();
    }

    let orig_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let context = 3;
    let mut diff = String::new();
    let mut cumulative_shift: isize = 0;

    for edit in edits {
        let orig_start = edit.orig_start; // 1-indexed
        let orig_end = edit.orig_end;     // 1-indexed
        let removed_count = orig_end - orig_start + 1;
        let added_count = edit.replacement_line_count;
        let net_change: isize = added_count as isize - removed_count as isize;

        // Compute the new-file line range for this edit.
        let new_start = (orig_start as isize + cumulative_shift) as usize;
        let new_end = new_start + added_count; // exclusive

        // Compute context range (0-indexed in orig_lines).
        let ctx_before_start = (orig_start - 1).saturating_sub(context);
        let ctx_after_end = (orig_end + context).min(orig_lines.len());
        let ctx_before_count = (orig_start - 1) - ctx_before_start;
        let ctx_after_count = ctx_after_end - orig_end;

        // Hunk header.
        let orig_count = removed_count + ctx_before_count + ctx_after_count;
        let new_count = added_count + ctx_before_count + ctx_after_count;
        let new_start_1idx = (ctx_before_start as isize + 1 + cumulative_shift) as usize;
        let orig_start_1idx = ctx_before_start + 1;
        diff.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            orig_start_1idx, orig_count, new_start_1idx, new_count
        ));

        // Context before — unchanged lines. Their new-file line numbers are
        // offset by the cumulative shift from earlier edits.
        for i in ctx_before_start..(orig_start - 1) {
            if line_number {
                let new_lineno = (i as isize + 1 + cumulative_shift) as usize;
                diff.push_str(&format!(" {}\t{}\n", new_lineno, orig_lines[i]));
            } else {
                diff.push_str(&format!(" {}\n", orig_lines[i]));
            }
        }

        // Removed lines — use original-file line numbers.
        for i in (orig_start - 1)..orig_end {
            if line_number {
                diff.push_str(&format!("-{}\t{}\n", i + 1, orig_lines[i]));
            } else {
                diff.push_str(&format!("-{}\n", orig_lines[i]));
            }
        }

        // Added lines — use new-file line numbers.
        for line_idx in (new_start - 1)..(new_end - 1).min(new_lines.len()) {
            if line_number {
                diff.push_str(&format!("+{}\t{}\n", line_idx + 1, new_lines[line_idx]));
            } else {
                diff.push_str(&format!("+{}\n", new_lines[line_idx]));
            }
        }

        // Context after — use new-file line numbers and new-file content.
        let shift_after = cumulative_shift + net_change;
        for i in orig_end..ctx_after_end {
            if line_number {
                let new_lineno = (i as isize + 1 + shift_after) as usize;
                diff.push_str(&format!(
                    " {}\t{}\n",
                    new_lineno,
                    new_lines
                        .get((i as isize + shift_after) as usize)
                        .unwrap_or(&orig_lines[i])
                ));
            } else {
                diff.push_str(&format!(
                    " {}\n",
                    new_lines
                        .get((i as isize + shift_after) as usize)
                        .unwrap_or(&orig_lines[i])
                ));
            }
        }

        cumulative_shift += net_change;
    }

    diff
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
        /// Byte-offset ranges of matches in the original content, for
        /// producing structured diff output.
        match_ranges: Vec<(usize, usize)>,
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
    let mut orig_match_ranges: Vec<(usize, usize)> = Vec::with_capacity(match_ranges.len());

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
        orig_match_ranges.push((orig_start, orig_end));
    }

    // Copy remaining content after the last match
    result.push_str(&original[last_orig_end..]);

    LiteralMatchResult::Matched {
        new_content: result,
        match_count,
        match_ranges: orig_match_ranges,
    }
}

/// Compute the safety cap for regex match counts. When the number of regex
/// matches exceeds this value, the pattern is likely degenerate — e.g., an
/// alternation like `| foo | bar |` matching at every character boundary. The
/// cap is the larger of twice the file's line count or 10, which accommodates
/// legitimate multi-match regexes while catching catastrophic single-character
/// matches.
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
/// Collapse runs of 2+ consecutive blank lines down to a single blank line.
/// This is applied after deletion-only replacements to avoid leaving
/// double blank lines where a deleted block's separator lines become
/// adjacent. The collapse is applied to the entire file — blank-line
/// runs are rare in well-formatted code and collapsing them is safe.
fn collapse_consecutive_blank_lines(content: &str) -> String {
    let trailing_newline = content.ends_with('\n');
    let lines: Vec<&str> = content.lines().collect();
    let mut result: Vec<&str> = Vec::with_capacity(lines.len());
    let mut blank_run = 0;

    for line in &lines {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                result.push(line);
            }
            // Skip lines 2+ in a blank run
        } else {
            blank_run = 0;
            result.push(line);
        }
    }

    let mut out = result.join("\n");
    if trailing_newline && !out.is_empty() {
        out.push('\n');
    }
    out
}

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
        return Ok(Vec::new());
    }

    // Stage 3: Unicode-to-ASCII folded match
    let actual_folded = fold_unicode_to_ascii(&actual_nfc);
    let expected_folded = fold_unicode_to_ascii(&expected_nfc);
    if actual_folded == expected_folded {
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

    // Stage 5: Blank-line-boundary-tolerant match
    // When the agent reads a line range and constructs old_content from
    // what it saw, blank lines at the top or bottom of the range may be
    // included or excluded differently from the actual file. Strip leading
    // and trailing blank lines from both actual and expected before comparing
    // (with trailing whitespace already stripped per Stage 4). Blank lines
    // within the content body must still match — only boundary blank lines
    // are tolerated.
    let strip_boundary_blank_lines = |s: &str| -> String {
        let lines: Vec<&str> = s.lines().collect();
        let first_nonblank = lines.iter().position(|l| !l.trim().is_empty());
        let last_nonblank = lines.iter().rposition(|l| !l.trim().is_empty());
        match (first_nonblank, last_nonblank) {
            (Some(first), Some(last)) => lines[first..=last].join("\n"),
            _ => String::new(), // all blank or empty
        }
    };
    let actual_boundary_stripped = strip_boundary_blank_lines(&actual_trimmed);
    let expected_boundary_stripped = strip_boundary_blank_lines(&expected_trimmed);
    if actual_boundary_stripped == expected_boundary_stripped {
        // Extract trailing whitespace from each original line for reapplication
        // to the replacement content, same as Stage 4.
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

    // Find the first line that differs between expected and actual, to give
    // an actionable hint about where the mismatch is (rather than just byte
    // offsets or lengths, which are hard to map to line boundaries).
    // Line numbers are expressed as file line numbers (1-indexed from
    // start_line) so the agent can cross-reference with files_read output.
    let line_hint = {
        let expected_lines: Vec<&str> = expected.lines().collect();
        let actual_lines: Vec<&str> = actual.lines().collect();
        let first_diff = expected_lines.iter().zip(actual_lines.iter())
            .enumerate()
            .find(|(_, (e, a))| e != a);
        match first_diff {
            Some((i, (exp_line, act_line))) => format!(
                "\nhint: first difference at line {} of the content (file line {}) — expected: {:?}, actual: {:?}",
                i + 1,
                start_line + i,
                exp_line,
                act_line,
            ),
            None if expected_lines.len() != actual_lines.len() => format!(
                "\nhint: old_content has {} lines, file range lines {}-{} has {} lines",
                expected_lines.len(),
                start_line,
                effective_end,
                actual_lines.len(),
            ),
            None => String::new(),
        }
    };

    // When the strings are the same length but differ, include byte-level
    // diff info so invisible character mismatches can be diagnosed.
    let byte_hint = if actual.len() == expected.len() {
        let first_diff = actual.bytes().zip(expected.bytes())
            .position(|(a, e)| a != e);
        match first_diff {
            Some(pos) => format!(
                "\nhint: same length ({} bytes) but differ at byte offset {}; expected byte 0x{:02x}, actual byte 0x{:02x}",
                expected.len(),
                pos,
                expected.as_bytes().get(pos).copied().unwrap_or(0),
                actual.as_bytes().get(pos).copied().unwrap_or(0),
            ),
            None => String::new(),
        }
    } else {
        format!(
            "\nhint: different lengths - expected {} bytes, actual {} bytes",
            expected.len(),
            actual.len(),
        )
    };

    anyhow::bail!(
        "old_content mismatch at lines {}-{}:\n  expected:\n{}\n  actual:\n{}\n{}\n{}",
        start_line,
        effective_end,
        expected_fmt,
        actual_fmt,
        line_hint,
        byte_hint,
    )
}

/// Safety threshold for auto-dry-run in replace mode. When `count` is omitted
/// and the actual match count exceeds this value, the tool shows the diff
/// without writing, prompting the agent to confirm with an explicit `count`.
const REPLACE_AUTO_DRY_RUN_THRESHOLD: usize = 5;

/// Check the replacement count against the `count` parameter and the
/// auto-dry-run threshold. Returns `Ok(true)` if the write should proceed,
/// `Ok(false)` if the write should be skipped (auto-dry-run), or an error
/// if the count was specified but doesn't match.
///
/// This helper is inserted in all four replacement code paths (literal mode,
/// regex mode, degenerate-regex fallback, zero-regex fallback) to enforce
/// count verification and auto-dry-run uniformly.
fn check_replace_count(
    actual_count: usize,
    expected_count: Option<usize>,
    path: &str,
    diff: &str,
) -> Result<bool> {
    if let Some(expected) = expected_count {
        if actual_count != expected {
            anyhow::bail!(
                "expected {} replacement(s) but found {} — adjust the pattern or count\n{}",
                expected, actual_count, diff
            );
        }
        // Count matched — proceed with write.
        Ok(true)
    } else if actual_count > REPLACE_AUTO_DRY_RUN_THRESHOLD {
        // Auto-dry-run: show the diff but don't write.
        println!(
            "{} replacement(s) in {} (preview — not written)\n{}\nhint: {} replacements exceeded safety threshold — set count: {} to apply, or use files_edit for a single targeted edit",
            actual_count, path, diff, actual_count, actual_count
        );
        Ok(false)
    } else {
        // No count specified and within threshold — proceed with write.
        Ok(true)
    }
}

/// Find all matches of `old_content` in `original` using the five-stage
/// verification cascade adapted for search. Returns a vector of
/// `(start_line, end_line)` tuples (1-indexed, inclusive) for all matches
/// found through the first stage that produces any matches.
///
/// The stages mirror `verify_original`:
/// 1. Exact match
/// 2. NFC-normalized match
/// 3. Unicode-to-ASCII folded match
/// 4. Trailing-whitespace-tolerant match
/// 5. Blank-line-boundary-tolerant match
///
/// For each stage, we slide a window of `old_content.lines().count()` lines
/// across `original.lines()` and compare. The first stage that produces any
/// matches wins — we don't combine matches across stages.
fn find_old_content_matches(original: &str, old_content: &str) -> Vec<(usize, usize)> {
    use unicode_normalization::UnicodeNormalization;

    let orig_lines: Vec<&str> = original.lines().collect();
    let old_lines: Vec<&str> = old_content.lines().collect();
    let window_size = old_lines.len();

    if window_size == 0 || window_size > orig_lines.len() {
        return Vec::new();
    }

    // Helper: check all windows for a given normalization function.
    // Returns match positions as (start_line, end_line) 1-indexed inclusive.
    let find_with_normalizer = |normalize: &dyn Fn(&str) -> String| -> Vec<(usize, usize)> {
        let normalized_old: String = normalize(&old_lines.join("\n"));
        let mut matches = Vec::new();

        for start in 0..=(orig_lines.len() - window_size) {
            let window: String = orig_lines[start..start + window_size].join("\n");
            if normalize(&window) == normalized_old {
                matches.push((start + 1, start + window_size));
            }
        }
        matches
    };

    // Stage 1: Exact match
    let exact_matches = find_with_normalizer(&|s| s.to_string());
    if !exact_matches.is_empty() {
        return exact_matches;
    }

    // Stage 2: NFC-normalized match
    let nfc_matches = find_with_normalizer(&|s| s.nfc().collect::<String>());
    if !nfc_matches.is_empty() {
        return nfc_matches;
    }

    // Stage 3: Unicode-to-ASCII folded match (applied after NFC)
    let folded_matches = find_with_normalizer(&|s| {
        fold_unicode_to_ascii(&s.nfc().collect::<String>())
    });
    if !folded_matches.is_empty() {
        return folded_matches;
    }

    // Stage 4: Trailing-whitespace-tolerant match
    let trailing_ws_matches = find_with_normalizer(&|s| {
        s.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
    });
    if !trailing_ws_matches.is_empty() {
        return trailing_ws_matches;
    }

    // Stage 5: Blank-line-boundary-tolerant match
    // Strip leading/trailing blank lines from both the window and old_content
    // (with trailing whitespace already stripped per Stage 4).
    let strip_boundary_blank = |s: &str| -> String {
        let lines: Vec<&str> = s.lines().collect();
        let first_nonblank = lines.iter().position(|l| !l.trim().is_empty());
        let last_nonblank = lines.iter().rposition(|l| !l.trim().is_empty());
        match (first_nonblank, last_nonblank) {
            (Some(first), Some(last)) => lines[first..=last].join("\n"),
            _ => String::new(),
        }
    };
    let boundary_matches = find_with_normalizer(&|s| {
        let trimmed: String = s.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n");
        strip_boundary_blank(&trimmed)
    });
    if !boundary_matches.is_empty() {
        return boundary_matches;
    }

    Vec::new()
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
    use super::resolve_end_line;
    use super::format_patch_diff;
    use super::format_replace_diff_from_edits;
    use super::compute_replace_edits;
    use super::compute_replace_edits_with_replacements;
    use super::ReplaceEdit;
    use super::verify_original;
    use super::fold_unicode_to_ascii;
    use super::try_literal_trailing_ws_match;
    use super::degenerate_match_cap;
    use super::strip_trailing_ws_per_line;
    use super::build_stripped_to_original_offset_map;
    use super::LiteralMatchResult;
    use super::byte_offset_to_line;
    use super::collapse_consecutive_blank_lines;
    use super::find_old_content_matches;
    use super::check_replace_count;
    use super::REPLACE_AUTO_DRY_RUN_THRESHOLD;

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

    // --- resolve_end_line tests ---

    #[test]
    fn resolve_end_line_explicit_value_used() {
        // When end_line is explicitly provided, use it as-is.
        assert_eq!(resolve_end_line(3, Some(5), Some("a\nb")).unwrap(), 5);
    }

    #[test]
    fn resolve_end_line_inferred_from_single_line_expected() {
        // Single line old_content: end_line = start_line.
        assert_eq!(resolve_end_line(3, None, Some("hello")).unwrap(), 3);
    }

    #[test]
    fn resolve_end_line_inferred_from_multi_line_expected() {
        // 3-line old_content starting at line 5: end_line = 7.
        assert_eq!(resolve_end_line(5, None, Some("a\nb\nc")).unwrap(), 7);
    }

    #[test]
    fn resolve_end_line_inferred_from_expected_starting_at_line_1() {
        assert_eq!(resolve_end_line(1, None, Some("a\nb")).unwrap(), 2);
    }

    #[test]
    fn resolve_end_line_rejects_empty_expected_without_explicit_end() {
        // Empty old_content cannot infer end_line.
        assert!(resolve_end_line(1, None, Some("")).is_err());
    }

    #[test]
    fn resolve_end_line_rejects_no_expected_no_end() {
        // Neither end_line nor old_content provided.
        assert!(resolve_end_line(1, None, None).is_err());
    }

    #[test]
    fn resolve_end_line_explicit_overrides_inference() {
        // Even if old_content has 3 lines, explicit end_line wins.
        assert_eq!(resolve_end_line(1, Some(10), Some("a\nb\nc")).unwrap(), 10);
    }

    #[test]
    fn resolve_end_line_trailing_newline_in_expected() {
        // Rust's .lines() does not include a trailing empty element for a
        // trailing newline, so "a\nb\n" has 2 lines → end_line = start + 1.
        assert_eq!(resolve_end_line(3, None, Some("a\nb\n")).unwrap(), 4);
    }

    #[test]
    fn patch_string_with_inferred_end_line_multi_line_expected() {
        // The original bug: agent provides multi-line old_content
        // without specifying end_line. With inference, the range is
        // correct and the right lines are replaced.
        let input = "line1\nline2\nline3\nline4\nline5";
        // Agent wants to replace lines 2-4 (3 lines of expected content)
        let old_content = "line2\nline3\nline4";
        let end_line = resolve_end_line(2, None, Some(old_content)).unwrap();
        assert_eq!(end_line, 4);
        let out = patch_string(input, 2, Some(end_line), "new2\nnew3\nnew4");
        assert_eq!(out, "line1\nnew2\nnew3\nnew4\nline5");
    }

    #[test]
    fn patch_string_with_inferred_end_line_single_line() {
        // Single-line replacement: inferred end_line == start_line.
        let input = "a\nb\nc";
        let old_content = "b";
        let end_line = resolve_end_line(2, None, Some(old_content)).unwrap();
        assert_eq!(end_line, 2);
        let out = patch_string(input, 2, Some(end_line), "replaced");
        assert_eq!(out, "a\nreplaced\nc");
    }

    #[test]
    fn patch_string_inferred_end_line_preserves_boundary_blank_lines() {
        // When the agent points start_line at the content (not the blank
        // line), boundary blank lines are preserved because they're
        // outside the replacement range.
        let input = "a\n\nheading\nbody\n\nc";
        // Agent targets lines 3-4 (heading + body), blank lines at 2 and 5
        // are outside the range and preserved.
        let old_content = "heading\nbody";
        let end_line = resolve_end_line(3, None, Some(old_content)).unwrap();
        assert_eq!(end_line, 4);
        let out = patch_string(input, 3, Some(end_line), "new heading\nnew body");
        assert_eq!(out, "a\n\nnew heading\nnew body\n\nc");
    }

    // --- format_patch_diff tests ---

    #[test]
    fn format_patch_diff_shows_removed_and_added() {
        let input = "a\nb\nc\nd\ne";
        let diff = format_patch_diff(input, 2, Some(3), "x\ny");
        assert!(diff.contains("-2\tb"));
        assert!(diff.contains("-3\tc"));
        assert!(diff.contains("+2\tx"));
        assert!(diff.contains("+3\ty"));
    }

    #[test]
    fn format_patch_diff_shows_context_before() {
        let input = "a\nb\nc\nd\ne";
        let diff = format_patch_diff(input, 5, Some(5), "X");
        assert!(diff.contains(" 4\td"));
    }

    #[test]
    fn format_patch_diff_shows_context_after() {
        let input = "a\nb\nc\nd\ne";
        let diff = format_patch_diff(input, 2, Some(2), "X");
        assert!(diff.contains(" 3\tc"));
    }

    #[test]
    fn format_patch_diff_handles_start_of_file() {
        let input = "a\nb\nc\nd\ne";
        let diff = format_patch_diff(input, 1, Some(1), "X");
        assert!(diff.contains("+1\tX"));
        assert!(!diff.contains(" 0\t"));
    }

    #[test]
    fn format_patch_diff_handles_end_of_file() {
        let input = "a\nb\nc\nd\ne";
        let diff = format_patch_diff(input, 5, Some(5), "X");
        assert!(diff.contains("+5\tX"));
    }

    #[test]
    fn format_patch_diff_single_line_replacement() {
        let input = "a\nb\nc";
        let diff = format_patch_diff(input, 2, None, "X");
        assert!(diff.contains("-2\tb"));
        assert!(diff.contains("+2\tX"));
    }

    #[test]
    fn format_patch_diff_deletion_shows_no_added() {
        let input = "a\nb\nc";
        let diff = format_patch_diff(input, 2, Some(3), "");
        assert!(diff.contains("-2\tb"));
        assert!(diff.contains("-3\tc"));
        assert!(!diff.contains("+"));
    }

    #[test]
    fn format_patch_diff_context_after_uses_new_line_number_on_insertion() {
        // Replace 1 line with 3 lines: net change is +2.
        // Context-after lines should show shifted line numbers.
        let input = "a\nb\nc\nd\ne";
        let diff = format_patch_diff(input, 2, Some(2), "x\ny\nz");
        assert!(diff.contains("-2\tb"));
        assert!(diff.contains("+2\tx"));
        assert!(diff.contains("+3\ty"));
        assert!(diff.contains("+4\tz"));
        // "c" was at line 3 in original, now at line 5 in new file
        assert!(diff.contains(" 5\tc"), "context after should use new-file line number");
        assert!(!diff.contains(" 3\tc"), "should not show stale original line number");
    }

    #[test]
    fn format_patch_diff_context_after_uses_new_line_number_on_deletion() {
        // Delete 2 lines: net change is -2.
        // Context-after lines should show shifted line numbers.
        let input = "a\nb\nc\nd\ne\nf\ng";
        let diff = format_patch_diff(input, 3, Some(4), "");
        assert!(diff.contains("-3\tc"));
        assert!(diff.contains("-4\td"));
        // "e" was at line 5 in original, now at line 3 in new file
        assert!(diff.contains(" 3\te"), "context after should use new-file line number");
        assert!(!diff.contains(" 5\te"), "should not show stale original line number");
    }

    // --- format_replace_diff_from_edits tests ---

    #[test]
    fn format_replace_diff_from_edits_replacement_adds_lines() {
        // Replace line 3 ("c") with 2 lines ("X" and "Y") — net +1.
        let original = "a\nb\nc\nd\ne";
        let new = "a\nb\nX\nY\nd\ne";
        let edits = vec![ReplaceEdit { orig_start: 3, orig_end: 3, replacement_line_count: 2 }];
        let diff = format_replace_diff_from_edits(original, new, &edits, true);
        assert!(diff.contains("-3\tc"), "removed line uses original line number");
        assert!(diff.contains("+3\tX"), "first added line at new-file line 3");
        assert!(diff.contains("+4\tY"), "second added line at new-file line 4");
        // Context after: "d" was at line 4 in original, now at line 5.
        assert!(diff.contains(" 5\td"), "context after uses new-file line number");
        assert!(!diff.contains(" 4\td"), "no stale line number for context after");
    }

    #[test]
    fn format_replace_diff_from_edits_multi_edit_no_ambiguous_lcs() {
        // This is the core bug scenario: two identical blocks with the same
        // replacement. The LCS algorithm can match the wrong instances, but
        // format_replace_diff_from_edits uses explicit edit positions.
        let original = "line1\nvariant_of: None,\n},\nline4\nvariant_of: None,\n},\nline7";
        let new =      "line1\nvariant_of: None,\nmatched_bin_group: None,\n},\nline4\nvariant_of: None,\nmatched_bin_group: None,\n},\nline7";
        // Two edits: replace "}," (line 3) with "matched_bin_group: None,\n},"
        // and replace "}," (line 6) with "matched_bin_group: None,\n},"
        let edits = vec![
            ReplaceEdit { orig_start: 3, orig_end: 3, replacement_line_count: 2 },
            ReplaceEdit { orig_start: 6, orig_end: 6, replacement_line_count: 2 },
        ];
        let diff = format_replace_diff_from_edits(original, new, &edits, true);
        // First edit: line 3 replaced by lines 3-4.
        assert!(diff.contains("-3\t},"), "first removed line at original line 3");
        assert!(diff.contains("+3\tmatched_bin_group: None,"), "first inserted line at new-file line 3");
        assert!(diff.contains("+4\t},"), "first replacement closing brace at new-file line 4");
        // Second edit: line 6 replaced by lines 7-8 (shifted by +1 from first edit).
        assert!(diff.contains("-6\t},"), "second removed line at original line 6");
        assert!(diff.contains("+7\tmatched_bin_group: None,"), "second inserted line at new-file line 7");
        assert!(diff.contains("+8\t},"), "second replacement closing brace at new-file line 8");
        // Context after second edit: "line7" was at line 7, now at line 9.
        assert!(diff.contains(" 9\tline7"), "context after second edit uses shifted new-file line number");
    }

    #[test]
    fn byte_offset_to_line_at_start() {
        assert_eq!(byte_offset_to_line("hello\nworld", 0), 1);
    }

    #[test]
    fn byte_offset_to_line_mid_first_line() {
        assert_eq!(byte_offset_to_line("hello\nworld", 3), 1);
    }

    #[test]
    fn byte_offset_to_line_start_of_second_line() {
        assert_eq!(byte_offset_to_line("hello\nworld", 6), 2);
    }

    #[test]
    fn byte_offset_to_line_mid_second_line() {
        assert_eq!(byte_offset_to_line("hello\nworld", 8), 2);
    }

    #[test]
    fn byte_offset_to_line_indented_line() {
        // Pattern like "let y = 2;" matching after indentation
        let content = "fn main() {\n    let y = 2;\n    println!();\n}";
        // "    let y = 2;" starts at byte 13 (after "fn main() {\n")
        assert_eq!(byte_offset_to_line(content, 13), 2);
        // The "l" of "let" is at byte 17 (after "fn main() {\n    ")
        assert_eq!(byte_offset_to_line(content, 17), 2);
    }

    #[test]
    fn compute_replace_edits_from_byte_ranges() {
        let original = "a\nb\nc\nd\ne";
        // Match "b\nc" which spans bytes 2..5 (after "a\n")
        let match_ranges = vec![(2, 5)];
        let edits = compute_replace_edits(original, &match_ranges, "x\ny");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].orig_start, 2); // line 2: "b"
        assert_eq!(edits[0].orig_end, 3);   // line 3: "c"
        assert_eq!(edits[0].replacement_line_count, 2);
    }

    #[test]
    fn compute_replace_edits_multi_match() {
        let original = "a\nb\nc\nb\nc\nd";
        // Match "b\nc" at positions (2, 5) and (6, 9)
        let match_ranges = vec![(2, 5), (6, 9)];
        let edits = compute_replace_edits(original, &match_ranges, "x");
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].orig_start, 2);
        assert_eq!(edits[0].orig_end, 3);
        assert_eq!(edits[0].replacement_line_count, 1);
        assert_eq!(edits[1].orig_start, 4);
        assert_eq!(edits[1].orig_end, 5);
        assert_eq!(edits[1].replacement_line_count, 1);
    }

    #[test]
    fn compute_replace_edits_with_replacements_different_lengths() {
        let original = "a\nb\nc\nd\ne";
        let match_ranges = vec![(2, 3), (6, 7)];
        let replacements = vec!["x\ny".to_string(), "z".to_string()];
        let edits = compute_replace_edits_with_replacements(original, &match_ranges, &replacements);
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].replacement_line_count, 2);
        assert_eq!(edits[1].replacement_line_count, 1);
    }

    #[test]
    fn format_replace_diff_from_edits_pure_deletion() {
        // Delete line 3.
        let original = "a\nb\nc\nd\ne";
        let new = "a\nb\nd\ne";
        let edits = vec![ReplaceEdit { orig_start: 3, orig_end: 3, replacement_line_count: 0 }];
        let diff = format_replace_diff_from_edits(original, new, &edits, true);
        assert!(diff.contains("-3\tc"), "removed line uses original line number");
        // Context after: "d" was at line 4, now at line 3.
        assert!(diff.contains(" 3\td"), "context after uses new-file line number after deletion");
        assert!(!diff.contains(" 4\td"), "no stale line number for context after");
    }

    #[test]
    fn format_replace_diff_from_edits_empty_edits() {
        let original = "a\nb\nc";
        let new = "a\nb\nc";
        let edits: Vec<ReplaceEdit> = vec![];
        let diff = format_replace_diff_from_edits(original, new, &edits, true);
        assert!(diff.is_empty(), "no edits should produce empty diff");
    }

    #[test]
    fn format_replace_diff_from_edits_no_line_number() {
        let original = "a\nb\nc\nd\ne";
        let new = "a\nb\nX\nY\nd\ne";
        let edits = vec![ReplaceEdit { orig_start: 3, orig_end: 3, replacement_line_count: 2 }];
        let diff = format_replace_diff_from_edits(original, new, &edits, false);
        assert!(diff.contains("-c"), "removed line without line number");
        assert!(diff.contains("+X"), "added line without line number");
        assert!(!diff.contains("\t"), "no line numbers when line_number=false");
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
        let result = try_literal_trailing_ws_match(original, "foo", "FOO", true);
        match result {
            LiteralMatchResult::Matched { new_content, match_count, .. } => {
                assert_eq!(match_count, 1);
                assert_eq!(new_content, "FOO   \nbar\nbaz\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_match_multiline_pattern() {
        let original = "foo   \nbar   \nbaz\n";
        let result = try_literal_trailing_ws_match(original, "foo\nbar", "FOO\nBAR", true);
        match result {
            LiteralMatchResult::Matched { new_content, match_count, .. } => {
                assert_eq!(match_count, 1);
                assert_eq!(new_content, "FOO   \nBAR   \nbaz\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_match_no_match() {
        let original = "foo\nbar\nbaz\n";
        let result = try_literal_trailing_ws_match(original, "qux", "QUX", true);
        match result {
            LiteralMatchResult::Matched { .. } => panic!("expected no match"),
            LiteralMatchResult::NoMatch { .. } => {}
        }
    }

    #[test]
    fn literal_match_multiple_occurrences() {
        let original = "foo   \nbar   \nfoo\nbaz\n";
        let result = try_literal_trailing_ws_match(original, "foo", "FOO", true);
        match result {
            LiteralMatchResult::Matched { new_content, match_count, .. } => {
                assert_eq!(match_count, 2);
                assert_eq!(new_content, "FOO   \nbar   \nFOO\nbaz\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_match_short_pattern_rejected() {
        let original = "a b c\n";
        let result = try_literal_trailing_ws_match(original, "a", "X", true);
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
        let result = try_literal_trailing_ws_match(original, "foo\nbar", "FOO\nBAR", true);
        match result {
            LiteralMatchResult::Matched { new_content, .. } => {
                assert_eq!(new_content, "FOO   \nBAR   \nbaz\n");
            }
            LiteralMatchResult::NoMatch { .. } => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_match_no_trailing_ws_in_original() {
        let original = "foo\nbar\nbaz\n";
        let result = try_literal_trailing_ws_match(original, "foo", "FOO", true);
        match result {
            LiteralMatchResult::Matched { new_content, match_count, .. } => {
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
        let result = try_literal_trailing_ws_match(original, "let x = 1;", "let y = 2;", true);
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
        let result = try_literal_trailing_ws_match(original, "nonexistent content", "replacement", true);
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
        let result = try_literal_trailing_ws_match(original, "x = 1;", "x = 2;", true);
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
        let result = try_literal_trailing_ws_match(original, "    let x = 1;", "    let y = 2;", true);
        match result {
            LiteralMatchResult::Matched { new_content, match_count, .. } => {
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
        let result = try_literal_trailing_ws_match(original, "x = 1;", "x = 2;", false);
        match result {
            LiteralMatchResult::Matched { new_content, match_count, .. } => {
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
        let result = try_literal_trailing_ws_match(original, "foo", "FOO", false);
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
        let result = try_literal_trailing_ws_match(original, "x = 1;", "x = 2;", false);
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
        let result = try_literal_trailing_ws_match(original, "a", "X", false);
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
        let result = try_literal_trailing_ws_match(original, "x = 1;", "x = 2;", true);
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
        let result = try_literal_trailing_ws_match(original, pattern, "| 🟢 P2 | Compute thing |", true);
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

    // --- verify_original Stage 5: blank-line-boundary-tolerant match tests ---

    #[test]
    fn verify_original_accepts_leading_blank_line_difference() {
        // File has a leading blank line in the range, agent's expected doesn't.
        let file_content = "a\n\nb\nc\nd";
        assert!(verify_original(file_content, 2, 4, "b\nc").is_ok());
    }

    #[test]
    fn verify_original_accepts_trailing_blank_line_difference() {
        // File has a trailing blank line in the range, agent's expected doesn't.
        let file_content = "a\nb\nc\n\nd";
        assert!(verify_original(file_content, 2, 4, "b\nc").is_ok());
    }

    #[test]
    fn verify_original_accepts_both_boundary_blank_line_differences() {
        // File has both leading and trailing blank lines, agent's expected has neither.
        let file_content = "a\n\nb\nc\n\nd";
        assert!(verify_original(file_content, 2, 5, "b\nc").is_ok());
    }

    #[test]
    fn verify_original_accepts_extra_leading_blank_line_in_expected() {
        // Agent's expected has a leading blank line, file doesn't.
        let file_content = "a\nb\nc\nd";
        assert!(verify_original(file_content, 2, 3, "\nb\nc").is_ok());
    }

    #[test]
    fn verify_original_accepts_extra_trailing_blank_line_in_expected() {
        // Agent's expected has a trailing blank line, file doesn't.
        let file_content = "a\nb\nc\nd";
        assert!(verify_original(file_content, 2, 3, "b\nc\n").is_ok());
    }

    #[test]
    fn verify_original_rejects_interior_blank_line_difference() {
        // Blank lines within the body (not at boundaries) must still match.
        // File has a blank line between b and c, expected doesn't.
        let file_content = "a\nb\n\nc\nd";
        let result = verify_original(file_content, 2, 4, "b\nc");
        assert!(result.is_err());
    }

    #[test]
    fn verify_original_boundary_tolerance_with_trailing_whitespace() {
        // Both boundary blank-line difference and trailing whitespace difference.
        let file_content = "a\n\nb   \nc  \n\nd";
        let result = verify_original(file_content, 2, 5, "b\nc");
        assert!(result.is_ok());
        // Stage 5 should capture trailing whitespace same as Stage 4.
        // Range lines 2-5: ["", "b   ", "c  ", ""]
        let ws = result.unwrap();
        assert_eq!(ws.len(), 4); // 4 lines in the file range
        assert_eq!(ws[1], "   "); // trailing ws on "b" line (line 3)
        assert_eq!(ws[2], "  ");  // trailing ws on "c" line (line 4)
    }

    // --- collapse_consecutive_blank_lines tests ---

    #[test]
    fn collapse_blank_lines_no_consecutive() {
        assert_eq!(collapse_consecutive_blank_lines("a\n\nb\nc"), "a\n\nb\nc");
    }

    #[test]
    fn collapse_blank_lines_double_blank() {
        assert_eq!(collapse_consecutive_blank_lines("a\n\n\nb"), "a\n\nb");
    }

    #[test]
    fn collapse_blank_lines_triple_blank() {
        assert_eq!(collapse_consecutive_blank_lines("a\n\n\n\nb"), "a\n\nb");
    }

    #[test]
    fn collapse_blank_lines_multiple_runs() {
        assert_eq!(
            collapse_consecutive_blank_lines("a\n\n\nb\n\n\nc"),
            "a\n\nb\n\nc"
        );
    }

    #[test]
    fn collapse_blank_lines_preserves_trailing_newline() {
        assert_eq!(collapse_consecutive_blank_lines("a\n\n\nb\n"), "a\n\nb\n");
    }

    #[test]
    fn collapse_blank_lines_no_trailing_newline() {
        assert_eq!(collapse_consecutive_blank_lines("a\n\n\nb"), "a\n\nb");
    }

    #[test]
    fn collapse_blank_lines_whitespace_only_lines() {
        // Lines with only whitespace are treated as blank.
        assert_eq!(collapse_consecutive_blank_lines("a\n  \n\nb"), "a\n  \nb");
    }

    #[test]
    fn collapse_blank_lines_empty_input() {
        assert_eq!(collapse_consecutive_blank_lines(""), "");
    }

    #[test]
    fn collapse_blank_lines_all_blank() {
        assert_eq!(collapse_consecutive_blank_lines("\n\n\n"), "");
    }

    #[test]
    fn collapse_blank_lines_single_line() {
        assert_eq!(collapse_consecutive_blank_lines("hello"), "hello");
    }

    // --- find_old_content_matches tests ---

    #[test]
    fn find_old_content_unique_match() {
        let original = "line one\nline two\nline three";
        let matches = find_old_content_matches(original, "line two");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], (2, 2)); // line 2, 1-indexed
    }

    #[test]
    fn find_old_content_multiple_matches() {
        let original = "value = 1\nvalue = 1\nvalue = 1";
        let matches = find_old_content_matches(original, "value = 1");
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0], (1, 1));
        assert_eq!(matches[1], (2, 2));
        assert_eq!(matches[2], (3, 3));
    }

    #[test]
    fn find_old_content_no_match() {
        let original = "line one\nline two\nline three";
        let matches = find_old_content_matches(original, "this content does not exist");
        assert!(matches.is_empty());
    }

    #[test]
    fn find_old_content_multi_line_match() {
        let original = "a\nb\nc\nd\ne";
        let matches = find_old_content_matches(original, "b\nc\nd");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], (2, 4));
    }

    #[test]
    fn find_old_content_trailing_ws_tolerance() {
        // File has trailing whitespace, old_content doesn't.
        // Stage 4 should find the match.
        let original = "function foo() {\n    return 42;   \n}";
        let matches = find_old_content_matches(original, "    return 42;");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], (2, 2));
    }

    #[test]
    fn find_old_content_blank_line_boundary_tolerance() {
        // old_content has a leading blank line, file window has a trailing
        // blank line instead. After stripping boundary blanks, both reduce
        // to "b\nc" — stage 5 match. Stages 1-4 don't match because the
        // blank line position differs (leading vs trailing).
        let original = "b\nc\n\nd";
        let old_content = "\nb\nc";
        let matches = find_old_content_matches(original, old_content);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], (1, 3)); // lines 1-3 (1-indexed)
    }

    #[test]
    fn find_old_content_nfc_tolerance() {
        // NFC normalization: file has precomposed é, old_content has decomposed.
        let nfd = "caf\u{00e9}"; // precomposed é
        let nfc = "caf\u{0065}\u{0301}"; // e + combining acute
        let original = format!("a\n{}\nb", nfd);
        let matches = find_old_content_matches(&original, nfc);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], (2, 2));
    }

    #[test]
    fn find_old_content_unicode_ascii_tolerance() {
        // Unicode-to-ASCII folding: file has smart quote, old_content has ASCII.
        let original = "a\nOllama\u{2019}s feature\nb";
        let matches = find_old_content_matches(original, "Ollama's feature");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], (2, 2));
    }

    #[test]
    fn find_old_content_window_larger_than_file() {
        let original = "short";
        let matches = find_old_content_matches(original, "short\nfile");
        assert!(matches.is_empty());
    }

    // --- check_replace_count tests ---

    #[test]
    fn check_replace_count_matches() {
        // count=3, actual=3 → proceed
        assert_eq!(check_replace_count(3, Some(3), "test.txt", "").unwrap(), true);
    }

    #[test]
    fn check_replace_count_mismatch_rejects() {
        // count=1, actual=3 → reject
        assert!(check_replace_count(3, Some(1), "test.txt", "diff").is_err());
    }

    #[test]
    fn check_replace_count_auto_dry_run_threshold() {
        // No count, actual > threshold → skip write
        assert_eq!(check_replace_count(REPLACE_AUTO_DRY_RUN_THRESHOLD + 1, None, "test.txt", "diff").unwrap(), false);
    }

    #[test]
    fn check_replace_count_below_threshold_no_count() {
        // No count, actual <= threshold → proceed
        assert_eq!(check_replace_count(REPLACE_AUTO_DRY_RUN_THRESHOLD, None, "test.txt", "").unwrap(), true);
    }

    #[test]
    fn check_replace_count_zero_matches_with_count_zero() {
        // count=0, actual=0 → proceed (0 replacements is valid)
        assert_eq!(check_replace_count(0, Some(0), "test.txt", "").unwrap(), true);
    }

    #[test]
    fn check_replace_count_zero_matches_without_count() {
        // No count, actual=0 → proceed (0 is <= threshold)
        assert_eq!(check_replace_count(0, None, "test.txt", "").unwrap(), true);
    }

    #[test]
    fn check_replace_count_exactly_at_threshold() {
        // No count, actual == threshold → proceed (not exceeding)
        assert_eq!(check_replace_count(REPLACE_AUTO_DRY_RUN_THRESHOLD, None, "test.txt", "").unwrap(), true);
    }

    #[test]
    fn check_replace_count_count_overrides_threshold() {
        // count specified and matches, even if > threshold → proceed
        let large = REPLACE_AUTO_DRY_RUN_THRESHOLD + 10;
        assert_eq!(check_replace_count(large, Some(large), "test.txt", "").unwrap(), true);
    }

}
