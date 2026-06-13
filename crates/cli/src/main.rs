use clap::{Parser, Subcommand};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

#[derive(Parser)]
#[command(name = "chai")]
#[command(about = "Chai CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show version
    Version,

    /// Create ~/.chai with default profiles, active symlink, and bundled skills
    Init,

    /// Run the gateway (HTTP + WebSocket control plane). Uses CHAI_PROFILE or ~/.chai/active unless --profile is set.
    Gateway {
        /// Profile name (overrides CHAI_PROFILE and ~/.chai/active for this process)
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,

        /// WebSocket and HTTP port (default from config or 15151)
        #[arg(long, short)]
        port: Option<u16>,
    },

    /// Chat with the default agent via the gateway (interactive)
    Chat {
        /// Profile name for config resolution (must match the running gateway's profile)
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,

        /// Optional existing session id to continue.
        #[arg(long, value_name = "ID")]
        session: Option<String>,
    },

    /// List profiles, switch the active symlink, or show current profile
    Profile {
        #[command(subcommand)]
        sub: ProfileCmd,
    },

    /// Manage skill packages (discover CLIs, generate, validate, inspect)
    Skill {
        #[command(subcommand)]
        sub: SkillCmd,
    },

    /// File operations for skill tool backends
    File {
        #[command(subcommand)]
        sub: FileCmd,
    },
}

#[derive(Subcommand)]
enum ProfileCmd {
    /// List profile names under ~/.chai/profiles
    List,
    /// Show persistent profile (~/.chai/active) and effective profile if CHAI_PROFILE differs
    Current,
    /// Set ~/.chai/active to profiles/<name> (gateway must not be running)
    Switch {
        /// Profile name
        name: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let default_log_filter = match &cli.command {
        Some(Commands::Gateway { .. }) => "info",
        _ => "warn",
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_log_filter))
        .init();

    match cli.command {
        Some(Commands::Version) => {
            println!("chai {}", env!("CARGO_PKG_VERSION"));
        }
        Some(Commands::Init) => {
            if let Err(e) = run_init() {
                log::error!("init failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Gateway { profile, port }) => {
            if let Err(e) = run_gateway(profile.as_deref(), port).await {
                log::error!("gateway failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Chat { profile, session }) => {
            if let Err(e) = run_chat(profile, session).await {
                log::error!("chat error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Profile { sub }) => {
            if let Err(e) = run_profile(sub) {
                log::error!("profile: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Skill { sub }) => {
            if let Err(e) = run_skill(sub) {
                log::error!("skill: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::File { sub }) => {
            if let Err(e) = run_file(sub) {
                log::error!("file: {}", e);
                std::process::exit(1);
            }
        }
        None => {
            println!("Run with --help for usage");
        }
    }
}

fn run_init() -> anyhow::Result<()> {
    let chai_home = lib::init::init_chai_home()?;
    println!("initialized ~/.chai at {}", chai_home.display());
    Ok(())
}

fn run_profile(cmd: ProfileCmd) -> anyhow::Result<()> {
    let chai_home = lib::profile::chai_home()?;
    match cmd {
        ProfileCmd::List => {
            let names = lib::profile::list_profile_names(&chai_home)?;
            for n in names {
                println!("{}", n);
            }
        }
        ProfileCmd::Current => {
            let persistent = lib::profile::read_persistent_profile_name(&chai_home)?;
            if let Ok(env_name) = std::env::var("CHAI_PROFILE") {
                let env_trim = env_name.trim();
                if !env_trim.is_empty() && env_trim != persistent {
                    println!("persistent: {}", persistent);
                    println!("effective: {} (CHAI_PROFILE)", env_trim);
                } else {
                    println!("{}", persistent);
                }
            } else {
                println!("{}", persistent);
            }
        }
        ProfileCmd::Switch { name } => {
            if lib::profile::gateway_is_running(&chai_home) {
                anyhow::bail!("gateway is running; stop it before switching profile");
            }
            lib::profile::switch_active_profile(&chai_home, name.trim())?;
            println!("active profile is now {}", name.trim());
        }
    }
    Ok(())
}

#[derive(Subcommand)]
enum SkillCmd {
    /// Discover a CLI binary's interface by running its help output
    Discover {
        /// CLI binary name (e.g. 'notesmd-cli', 'git', 'rg')
        binary: String,
        /// Specific subcommand to get detailed help for
        #[arg(long)]
        subcommand: Option<String>,
    },
    /// List installed skills with population status
    List,
    /// Read a skill's SKILL.md or tools.json file
    Read {
        /// Skill directory name (e.g. 'notesmd-daily')
        skill_name: String,
        /// File to read: 'skill_md' or 'tools_json'
        #[arg(long)]
        file: String,
    },
    /// Initialize a new skill directory with template files
    Init {
        /// Name for the new skill directory
        skill_name: String,
        /// Short description for SKILL.md frontmatter
        #[arg(long)]
        description: Option<String>,
    },
    /// Write or overwrite SKILL.md for a skill. Content is read from stdin when --content is omitted.
    WriteSkillMd {
        /// Skill directory name
        skill_name: String,
        /// Complete SKILL.md content including frontmatter. If omitted, content is read from stdin.
        /// Accepts values that begin with dashes (e.g. YAML frontmatter).
        #[arg(long, allow_hyphen_values = true)]
        content: Option<String>,
    },
    /// Write or overwrite tools.json for a skill (validates JSON before writing). Content is read from stdin when --content is omitted.
    WriteToolsJson {
        /// Skill directory name
        skill_name: String,
        /// Complete tools.json content as JSON. If omitted, content is read from stdin.
        #[arg(long, allow_hyphen_values = true)]
        content: Option<String>,
    },
    /// Write a script to a skill's scripts/ directory. Content is read from stdin when --content is omitted.
    WriteScript {
        /// Skill directory name
        skill_name: String,
        /// Script filename without .sh extension
        script_name: String,
        /// Complete script content. If omitted, content is read from stdin.
        #[arg(long, allow_hyphen_values = true)]
        content: Option<String>,
    },
    /// Validate a skill's tools.json for schema conformance
    Validate {
        /// Skill directory name
        skill_name: String,
    },
    /// Pin current active skill versions to skills.lock for the active profile
    Lock,
    /// Restore a previous lockfile generation and update active skill versions
    Rollback {
        /// Generation number to restore (use 'chai skill generations' to list)
        generation: u64,
    },
    /// List available lockfile generations for the active profile
    Generations,
    /// Delete an initialized skill package (removes the skill directory and all version snapshots)
    Delete {
        /// Skill directory name to delete (e.g. 'test-skill', 'myfeed')
        skill_name: String,
    },
}

#[derive(Subcommand)]
enum FileCmd {
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
        /// Search pattern (extended regex, whole-file matching with multiline mode so ^ and $ match line boundaries)
        /// Accepts values that begin with dashes (e.g. `- **bold**`, `--flag`).
        #[arg(long, allow_hyphen_values = true)]
        pattern: String,
        /// Replacement string. Use $1-$9 for capture group references. Use $$ for a literal $.
        /// Accepts values that begin with dashes (e.g. `- replacement`, `--flag`).
        #[arg(long, allow_hyphen_values = true)]
        replacement: String,
        /// Maximum number of replacements to apply. 0 (default) means unlimited.
        /// Use 1 to replace only the first match and avoid unintended changes
        /// when the same pattern appears in multiple locations.
        #[arg(long, default_value_t = 0)]
        max_replacements: usize,
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
        /// Root directory to search for wikilinks to update
        #[arg(long)]
        root: String,
    },
}

/// Read content from stdin when --content was not provided.
fn read_content_from_stdin_or(content: Option<String>) -> anyhow::Result<String> {
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

fn run_file(cmd: FileCmd) -> anyhow::Result<()> {
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
            std::fs::write(&target, &content)
                .map_err(|e| anyhow::anyhow!("failed to write {}: {}", target.display(), e))?;
            println!("wrote {} ({} bytes)", target.display(), content.len());
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
            println!("appended {} bytes to {}", content.len(), target.display());
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
        FileCmd::Replace { path, pattern, replacement, max_replacements, line_numbers } => {
            let target = std::path::Path::new(&path);
            if !target.exists() {
                anyhow::bail!("file does not exist: {}", path);
            }
            if !target.is_file() {
                anyhow::bail!("not a file: {}", path);
            }

            let original = std::fs::read_to_string(target)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path, e))?;

            // Process escape sequences in the replacement string so that
            // \n becomes a newline, \t becomes a tab, and \\ becomes a
            // literal backslash. The regex engine interprets \n in the
            // *pattern* as a newline, but regex::Captures::expand() treats
            // the replacement as literal text — so \n in the replacement
            // would produce a literal backslash-n instead of a newline.
            // Processing escapes here makes the replacement consistent with
            // the pattern's interpretation of \n.
            let replacement = process_replacement_escapes(&replacement);

            let re = regex::RegexBuilder::new(&pattern)
                .multi_line(true)
                .build()
                .map_err(|e| anyhow::anyhow!("invalid pattern: {}", e))?;

            // Collect all captures, then apply up to max_replacements.
            // max_replacements == 0 means unlimited.
            let all_captures: Vec<_> = re.captures_iter(&original).collect();
            let count = all_captures.len();

            if count > 0 {
                let limit = if max_replacements > 0 {
                    max_replacements.min(count)
                } else {
                    count
                };

                let new_content = if limit == count {
                    // No limit or limit equals total matches — use replace_all
                    re.replace_all(&original, |caps: &regex::Captures| {
                        let mut expanded = String::new();
                        caps.expand(&replacement, &mut expanded);
                        expanded
                    }).into_owned()
                } else {
                    // Apply only the first `limit` matches, building the result
                    // from left-to-right captures.
                    let mut result = String::with_capacity(original.len());
                    let mut last_end = 0;
                    for caps in all_captures.iter().take(limit) {
                        let mat = caps.get(0).unwrap();
                        result.push_str(&original[last_end..mat.start()]);
                        let mut expanded = String::new();
                        caps.expand(&replacement, &mut expanded);
                        result.push_str(&expanded);
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
                }
                return Ok(());
            }

            // Regex matched 0 times. Fall back to a trailing-whitespace-
            // tolerant literal search: treat the pattern as literal text
            // (escaping all regex metacharacters) and match per-line with
            // trailing whitespace stripped from both the pattern and the
            // file content. This handles the common case where the LLM
            // drops trailing whitespace when copying content from file
            // reads into the pattern parameter.
            let literal_match = try_literal_trailing_ws_match(
                &original, &pattern, &replacement, max_replacements,
            );

            match literal_match {
                LiteralMatchResult::Matched { new_content, match_count } => {
                    std::fs::write(target, new_content.as_bytes())
                        .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;

                    let diff = format_replace_diff(&original, &new_content, line_numbers);
                    println!(
                        "{} replacement(s) in {} (trailing-whitespace-tolerant match)\n{}",
                        match_count, path, diff
                    );
                    Ok(())
                }
                LiteralMatchResult::NoMatch => {
                    println!("0 replacements in {}", path);
                    Ok(())
                }
            }
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
                    anyhow::bail!("no frontmatter found in {}", path);
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
            let root_path = std::path::Path::new(&root);

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
                anyhow::bail!("root is not a directory: {}", root);
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
                let updated = update_wikilinks(root_path, old_name, new_name)?;
                println!("updated wikilinks in {} file(s)", updated);
            }

            Ok(())
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
fn format_replace_diff(original: &str, new: &str, line_numbers: bool) -> String {
    let orig_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let hunks = compute_diff_hunks(&orig_lines, &new_lines);

    if hunks.is_empty() {
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

        // Added lines
        for (offset, line_idx) in (hunk.new_start..hunk.new_end).enumerate() {
            if line_numbers {
                diff.push_str(&format!("+{}|{}\n", hunk.new_start + offset + 1, new_lines[line_idx]));
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

    diff
}

/// A contiguous region of change between original and new content.
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

/// Process escape sequences in a replacement string. The regex engine
/// interprets `\n` in the *pattern* as a newline, but
/// `regex::Captures::expand()` treats the replacement as literal text —
/// so `\n` in the replacement produces a literal backslash-n instead of
/// a newline. This function processes common escape sequences so the
/// replacement is consistent with the pattern:
///
/// - `\n` → newline
/// - `\t` → tab
/// - `\\` → literal backslash
///
/// Backslashes that don't precede a recognized escape are left as-is
/// (e.g., `\$` for a literal dollar sign in replacement text, or `\1`
/// for a backreference — though the correct backreference syntax in
/// expand() is `$1`, not `\1`).
fn process_replacement_escapes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.peek() {
                Some('n') => { chars.next(); result.push('\n'); }
                Some('t') => { chars.next(); result.push('\t'); }
                Some('\\') => { chars.next(); result.push('\\'); }
                _ => result.push(ch), // unknown escape, keep as-is
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Process escape sequences in a pattern string for the trailing-whitespace-
/// tolerant literal fallback. The regex engine interprets `\n` as a newline,
/// `\t` as a tab, and `\\` as a literal backslash in the pattern. When the
/// fallback treats the pattern as literal text, it must apply the same
/// interpretations so that multi-line patterns are matched correctly.
///
/// This uses the same logic as `process_replacement_escapes` but is named
/// separately for clarity — the pattern and replacement have the same set
/// of recognized escape sequences.
fn process_pattern_escapes_for_literal(s: &str) -> String {
    process_replacement_escapes(s)
}

/// Result of a trailing-whitespace-tolerant literal match attempt.
enum LiteralMatchResult {
    /// A match was found and the replacement was applied.
    Matched {
        new_content: String,
        match_count: usize,
    },
    /// No match found even with whitespace tolerance.
    NoMatch,
}

/// Attempt a trailing-whitespace-tolerant literal match when the regex
/// produced 0 replacements. This handles the common case where the LLM
/// drops trailing whitespace when copying content from file reads into
/// the pattern parameter.
///
/// The algorithm:
/// 1. Process escape sequences in the pattern (\n → newline, \t → tab, \\ → backslash)
///    so that the literal search matches the same content the regex engine would match.
/// 2. Treat the pattern as literal text (regex metacharacters are not interpreted).
/// 3. Strip trailing whitespace from each line of both the pattern and the file content.
/// 4. Search for the stripped pattern in the stripped file content.
/// 5. If found, map the match back to the original (unstripped) content.
/// 6. Apply the replacement, preserving the original trailing whitespace
///    for lines that are kept from the match (not added by the replacement).
fn try_literal_trailing_ws_match(
    original: &str,
    pattern: &str,
    replacement: &str,
    max_replacements: usize,
) -> LiteralMatchResult {
    // Process escape sequences in the pattern so that \n is treated as a
    // real newline, matching the regex engine's interpretation. Without
    // this step, multi-line patterns using \n would never match in the
    // literal fallback because strip_trailing_ws_per_line splits on
    // actual newlines, not on the two-character sequence backslash-n.
    let pattern = process_pattern_escapes_for_literal(pattern);

    // Strip trailing whitespace from each line of the pattern and the
    // file content, then search for the stripped pattern as literal text.
    let stripped_pattern = strip_trailing_ws_per_line(&pattern);
    let stripped_original = strip_trailing_ws_per_line(original);

    // Don't attempt fallback for empty or single-character patterns
    // (too likely to produce false positives).
    if stripped_pattern.len() <= 1 {
        return LiteralMatchResult::NoMatch;
    }

    // Find all matches of the stripped pattern in the stripped content.
    // Respect max_replacements: 0 means unlimited.
    let mut match_ranges: Vec<(usize, usize)> = Vec::new();
    let mut search_start = 0;
    while let Some(pos) = stripped_original[search_start..].find(&stripped_pattern) {
        let abs_pos = search_start + pos;
        match_ranges.push((abs_pos, abs_pos + stripped_pattern.len()));
        if max_replacements > 0 && match_ranges.len() >= max_replacements {
            break;
        }
        search_start = abs_pos + 1;
    }

    if match_ranges.is_empty() {
        return LiteralMatchResult::NoMatch;
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
fn verify_original(original: &str, start_line: usize, end_line: usize, expected: &str) -> anyhow::Result<Vec<String>> {
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

#[cfg(test)]
mod tests {
    use super::edit_frontmatter;
    use super::delete_frontmatter_key;
    use super::patch_string;
    use super::format_patch_diff;
    use super::verify_original;
    use super::fold_unicode_to_ascii;
    use super::process_replacement_escapes;
    use super::try_literal_trailing_ws_match;
    use super::strip_trailing_ws_per_line;
    use super::build_stripped_to_original_offset_map;
    use super::LiteralMatchResult;

    #[test]
    fn edit_frontmatter_updates_existing_with_leading_newlines() {
        let input = "\n\n---\ntitle: old\nkind: note\n---\nbody\n";
        let out = edit_frontmatter(input, "title", "new");
        assert!(out.contains("\n\n---\ntitle: new\nkind: note\n---\nbody\n"));
        assert!(!out.contains("title: old"));
    }

    #[test]
    fn edit_frontmatter_inserts_missing_key_with_leading_whitespace() {
        let input = "   \n---\ntitle: old\n---\nbody\n";
        let out = edit_frontmatter(input, "author", "ryan");
        assert!(out.contains("title: old\nauthor: ryan\n---"));
    }

    #[test]
    fn edit_frontmatter_creates_frontmatter_without_moving_leading_whitespace_into_body_gap() {
        let input = "\n\nbody\n";
        let out = edit_frontmatter(input, "title", "new");
        assert!(out.starts_with("\n\n---\ntitle: new\n---\n\nbody\n"));
    }

    #[test]
    fn delete_frontmatter_key_works_with_leading_whitespace() {
        let input = " \n\n---\ntitle: keep\nremove: yes\nkind: note\n---\nbody\n";
        let out = delete_frontmatter_key(input, "remove");
        assert!(out.starts_with(" \n\n---\n"));
        assert!(out.contains("title: keep\nkind: note\n---\nbody\n"));
        assert!(!out.contains("remove: yes"));
    }

    #[test]
    fn edit_frontmatter_does_not_match_longer_key_with_same_prefix() {
        let input = "---\nauthorized: token\nauthor: old\n---\n";
        let out = edit_frontmatter(input, "author", "new");
        assert!(out.contains("authorized: token"));
        assert!(out.contains("author: new"));
        assert!(!out.contains("author: old"));
    }

    #[test]
    fn delete_frontmatter_key_does_not_remove_longer_key_with_same_prefix() {
        let input = "---\nauthorized: token\nauthor: remove-me\n---\n";
        let out = delete_frontmatter_key(input, "author");
        assert!(out.contains("authorized: token"));
        assert!(!out.contains("author: remove-me"));
    }

    // --- patch_string tests ---

    #[test]
    fn patch_string_replaces_single_line() {
        let input = "line1\nline2\nline3\n";
        let out = patch_string(input, 2, None, "replaced");
        assert_eq!(out, "line1\nreplaced\nline3\n");
    }

    #[test]
    fn patch_string_replaces_range() {
        let input = "a\nb\nc\nd\ne\n";
        let out = patch_string(input, 2, Some(4), "x\ny");
        assert_eq!(out, "a\nx\ny\ne\n");
    }

    #[test]
    fn patch_string_expands_range() {
        let input = "a\nb\nc\n";
        let out = patch_string(input, 2, Some(2), "x\ny\nz");
        assert_eq!(out, "a\nx\ny\nz\nc\n");
    }

    #[test]
    fn patch_string_contracts_range() {
        let input = "a\nb\nc\nd\ne\n";
        let out = patch_string(input, 2, Some(4), "x");
        assert_eq!(out, "a\nx\ne\n");
    }

    #[test]
    fn patch_string_deletes_range_with_empty_replacement() {
        let input = "a\nb\nc\nd\ne\n";
        let out = patch_string(input, 2, Some(3), "");
        assert_eq!(out, "a\nd\ne\n");
    }

    #[test]
    fn patch_string_replaces_first_line() {
        let input = "a\nb\nc\n";
        let out = patch_string(input, 1, None, "x");
        assert_eq!(out, "x\nb\nc\n");
    }

    #[test]
    fn patch_string_replaces_last_line() {
        let input = "a\nb\nc\n";
        let out = patch_string(input, 3, None, "x");
        assert_eq!(out, "a\nb\nx\n");
    }

    #[test]
    fn patch_string_replaces_all_lines() {
        let input = "a\nb\nc\n";
        let out = patch_string(input, 1, Some(3), "x\ny");
        assert_eq!(out, "x\ny\n");
    }

    #[test]
    fn patch_string_start_line_past_end_returns_original() {
        let input = "a\nb\nc\n";
        let out = patch_string(input, 10, None, "x");
        assert_eq!(out, input);
    }

    #[test]
    fn patch_string_end_line_past_file_is_clamped() {
        let input = "a\nb\nc\n";
        // end_line=100 should be clamped to 3, replacing b and c
        let out = patch_string(input, 2, Some(100), "x");
        assert_eq!(out, "a\nx\n");
    }

    #[test]
    fn patch_string_preserves_no_trailing_newline() {
        let input = "a\nb\nc";
        let out = patch_string(input, 2, None, "x");
        assert_eq!(out, "a\nx\nc");
    }

    #[test]
    fn patch_string_single_line_file() {
        let input = "only\n";
        let out = patch_string(input, 1, None, "replaced");
        assert_eq!(out, "replaced\n");
    }

    #[test]
    fn patch_string_replacement_without_trailing_newline() {
        let input = "a\nb\nc\n";
        // Replacement without trailing newline should still produce valid output
        let out = patch_string(input, 2, Some(2), "x");
        assert_eq!(out, "a\nx\nc\n");
    }

    #[test]
    fn patch_string_replacement_with_trailing_newline() {
        let input = "a\nb\nc\n";
        let out = patch_string(input, 2, Some(2), "x\n");
        assert_eq!(out, "a\nx\nc\n");
    }

    // --- format_patch_diff tests ---

    #[test]
    fn format_patch_diff_shows_removed_and_added() {
        let input = "a\nb\nc\nd\ne\n";
        let diff = format_patch_diff(input, 2, Some(3), "x\ny");
        assert!(diff.contains("-2|b"), "should show removed line 2");
        assert!(diff.contains("-3|c"), "should show removed line 3");
        assert!(diff.contains("+2|x"), "should show added line");
        assert!(diff.contains("+3|y"), "should show added line");
    }

    #[test]
    fn format_patch_diff_shows_context_before() {
        let input = "a\nb\nc\nd\ne\nf\ng\n";
        let diff = format_patch_diff(input, 5, Some(5), "X");
        // 3 lines of context before line 5: lines 1, 2, 3 (but only 3 lines back from 5 = lines 2,3,4)
        assert!(diff.contains(" 4|d"), "should show context line 4");
        assert!(!diff.contains(" 1|a"), "should not show context line 1 (too far)");
    }

    #[test]
    fn format_patch_diff_shows_context_after() {
        let input = "a\nb\nc\nd\ne\nf\ng\n";
        let diff = format_patch_diff(input, 2, Some(2), "X");
        // 3 lines of context after line 2: lines 3, 4, 5
        assert!(diff.contains(" 3|c"), "should show context line 3");
        assert!(diff.contains(" 4|d"), "should show context line 4");
        assert!(diff.contains(" 5|e"), "should show context line 5");
        assert!(!diff.contains(" 6|f"), "should not show context line 6 (too far)");
    }

    #[test]
    fn format_patch_diff_handles_start_of_file() {
        let input = "a\nb\nc\nd\ne\n";
        let diff = format_patch_diff(input, 1, Some(1), "X");
        // No context before, but should show context after
        assert!(!diff.contains("-0"), "no line 0");
        assert!(diff.contains(" 2|b"), "should show context line 2");
    }

    #[test]
    fn format_patch_diff_handles_end_of_file() {
        let input = "a\nb\nc\nd\ne\n";
        let diff = format_patch_diff(input, 5, Some(5), "X");
        // Context before but no context after
        assert!(diff.contains(" 4|d"), "should show context line 4");
        assert!(!diff.contains(" 6|"), "should not show line 6 (out of bounds)");
    }

    #[test]
    fn format_patch_diff_single_line_replacement() {
        let input = "a\nb\nc\n";
        let diff = format_patch_diff(input, 2, None, "X");
        assert!(diff.contains("-2|b"), "should show removed line 2");
        assert!(diff.contains("+2|X"), "should show added line");
    }

    #[test]
    fn format_patch_diff_deletion_shows_no_added() {
        let input = "a\nb\nc\nd\n";
        let diff = format_patch_diff(input, 2, Some(3), "");
        assert!(diff.contains("-2|b"), "should show removed line 2");
        assert!(diff.contains("-3|c"), "should show removed line 3");
        assert!(!diff.contains("+"), "should not show any added lines");
    }

    // --- verify_original tests ---

    #[test]
    fn verify_original_matches_single_line() {
        let input = "a\nb\nc\n";
        assert!(verify_original(input, 2, 2, "b").is_ok());
    }

    #[test]
    fn verify_original_matches_range() {
        let input = "a\nb\nc\nd\ne\n";
        assert!(verify_original(input, 2, 4, "b\nc\nd").is_ok());
    }

    #[test]
    fn verify_original_rejects_mismatch() {
        let input = "a\nb\nc\n";
        let result = verify_original(input, 2, 2, "wrong");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("original_content mismatch"), "error should mention mismatch: {}", err);
        assert!(err.contains("expected"), "error should show expected: {}", err);
        assert!(err.contains("actual"), "error should show actual: {}", err);
    }

    #[test]
    fn verify_original_rejects_partial_range_mismatch() {
        let input = "a\nb\nc\nd\n";
        // First line matches but second doesn't
        let result = verify_original(input, 2, 3, "b\nwrong");
        assert!(result.is_err());
    }

    #[test]
    fn verify_original_matches_full_file() {
        let input = "a\nb\nc\n";
        assert!(verify_original(input, 1, 3, "a\nb\nc").is_ok());
    }

    #[test]
    fn verify_original_matches_first_line() {
        let input = "a\nb\nc\n";
        assert!(verify_original(input, 1, 1, "a").is_ok());
    }

    #[test]
    fn verify_original_matches_last_line() {
        let input = "a\nb\nc\n";
        assert!(verify_original(input, 3, 3, "c").is_ok());
    }

    #[test]
    fn verify_original_clamps_end_line_past_file() {
        let input = "a\nb\nc\n";
        // end_line=100 should be clamped to 3, so "b\nc" is the actual range
        assert!(verify_original(input, 2, 100, "b\nc").is_ok());
    }

    // --- verify_original Unicode tests ---

    #[test]
    fn verify_original_accepts_nfc_normalized_match() {
        // e with combining acute (NFD) vs precomposed e-acute (NFC)
        let nfd = "caf\u{0065}\u{0301}"; // "cafe" + combining acute
        let nfc = "caf\u{00E9}";          // precomposed e-acute
        let file_content = format!("a\n{}\nc\n", nfd);
        // If the file is NFD and the LLM sends NFC, it should still match
        assert!(verify_original(&file_content, 2, 2, nfc).is_ok());
    }

    #[test]
    fn verify_original_accepts_smart_quote_as_ascii() {
        // File has right single quotation mark, LLM sends ASCII apostrophe
        let file_content = "a\nOllama\u{2019}s feature\nc\n";
        assert!(verify_original(file_content, 2, 2, "Ollama's feature").is_ok());
    }

    #[test]
    fn verify_original_accepts_em_dash_as_double_hyphen() {
        // File has em dash, LLM sends "--"
        let file_content = "a\nsomething \u{2014} other\nc\n";
        assert!(verify_original(file_content, 2, 2, "something -- other").is_ok());
    }

    #[test]
    fn verify_original_accepts_en_dash_as_hyphen() {
        // File has en dash, LLM sends "-"
        let file_content = "a\n2024\u{2013}2025\nc\n";
        assert!(verify_original(file_content, 2, 2, "2024-2025").is_ok());
    }

    #[test]
    fn verify_original_accepts_smart_double_quotes_as_ascii() {
        // File has left/right double quotation marks, LLM sends ASCII quotes
        let file_content = "a\n\u{201C}hello\u{201D} world\nc\n";
        assert!(verify_original(file_content, 2, 2, "\"hello\" world").is_ok());
    }

    #[test]
    fn verify_original_accepts_middle_dot_as_period() {
        // File has middle dot, LLM sends "."
        let file_content = "a\ntest \u{00B7} point\nc\n";
        assert!(verify_original(file_content, 2, 2, "test . point").is_ok());
    }

    #[test]
    fn verify_original_accepts_non_breaking_space_as_space() {
        // File has non-breaking space, LLM sends regular space
        let file_content = "a\nhello\u{00A0}world\nc\n";
        assert!(verify_original(file_content, 2, 2, "hello world").is_ok());
    }

    #[test]
    fn verify_original_rejects_genuine_content_mismatch() {
        // Even with Unicode folding, genuinely different content should be rejected
        let file_content = "a\nhello world\nc\n";
        let result = verify_original(file_content, 2, 2, "goodbye world");
        assert!(result.is_err());
    }

    #[test]
    fn verify_original_rejects_genuine_mismatch_with_unicode() {
        // File has one Unicode string, LLM sends a completely different one
        let file_content = "a\nOllama\u{2019}s feature\nc\n";
        let result = verify_original(file_content, 2, 2, "Ollama's different");
        assert!(result.is_err());
    }

    #[test]
    fn verify_original_accepts_trailing_whitespace_difference() {
        // File has trailing spaces, LLM drops them
        let file_content = "a\nhello world   \nc\n";
        let result = verify_original(file_content, 2, 2, "hello world");
        assert!(result.is_ok(), "should accept match with trailing whitespace dropped");
        // Stage 4 matched — should return trailing whitespace for reapplication
        let ws = result.unwrap();
        assert_eq!(ws.len(), 1);
        assert_eq!(ws[0], "   ", "should capture trailing spaces from original line");
    }

    #[test]
    fn verify_original_accepts_trailing_whitespace_difference_multiline() {
        // File has trailing spaces on multiple lines, LLM drops them
        let file_content = "a\nhello   \nworld   \nc\n";
        let result = verify_original(file_content, 2, 3, "hello\nworld");
        assert!(result.is_ok(), "should accept match with trailing whitespace dropped on multiple lines");
        let ws = result.unwrap();
        assert_eq!(ws.len(), 2);
        assert_eq!(ws[0], "   ", "should capture trailing spaces from first line");
        assert_eq!(ws[1], "   ", "should capture trailing spaces from second line");
    }

    #[test]
    fn verify_original_returns_empty_vec_for_exact_match() {
        // Stages 1-3 return empty vectors — no whitespace correction needed
        let file_content = "a\nhello world\nc\n";
        let result = verify_original(file_content, 2, 2, "hello world");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty(), "exact match should return empty trailing ws vec");
    }

    #[test]
    fn verify_original_rejects_leading_whitespace_difference() {
        // Leading whitespace (indentation) is semantically meaningful and should not be stripped
        let file_content = "a\n  hello world\nc\n";
        let result = verify_original(file_content, 2, 2, "hello world");
        assert!(result.is_err(), "should reject match with leading whitespace dropped");
    }

    #[test]
    fn verify_original_rejects_content_mismatch_with_trailing_whitespace() {
        // Trailing whitespace tolerance should not mask genuine content differences
        let file_content = "a\nhello world   \nc\n";
        let result = verify_original(file_content, 2, 2, "goodbye world");
        assert!(result.is_err(), "should reject genuine content mismatch even with trailing whitespace");
    }

    #[test]
    fn verify_original_captures_trailing_tab() {
        // File has trailing tab, LLM drops it
        let file_content = "a\nhello world\t\nc\n";
        let result = verify_original(file_content, 2, 2, "hello world");
        assert!(result.is_ok());
        let ws = result.unwrap();
        assert_eq!(ws[0], "\t", "should capture trailing tab");
    }

    #[test]
    fn verify_original_captures_mixed_trailing_whitespace() {
        // File has trailing spaces + tab, LLM drops them
        let file_content = "a\nhello world  \t\nc\n";
        let result = verify_original(file_content, 2, 2, "hello world");
        assert!(result.is_ok());
        let ws = result.unwrap();
        assert_eq!(ws[0], "  \t", "should capture mixed trailing whitespace");
    }

    #[test]
    fn verify_original_captures_trailing_unicode_whitespace() {
        // File has trailing non-breaking space, LLM drops it
        let file_content = format!("a\nhello world\u{00A0}\nc\n");
        let result = verify_original(&file_content, 2, 2, "hello world");
        assert!(result.is_ok());
        let ws = result.unwrap();
        assert_eq!(ws[0], "\u{00A0}", "should capture trailing non-breaking space");
    }

    #[test]
    fn verify_original_partial_trailing_whitespace() {
        // File has "hello world   " (3 trailing spaces), LLM provides "hello world " (1 trailing space)
        // Stage 4 should match (trimmed content is identical), and trailing_ws
        // should capture the full original trailing whitespace for reapplication.
        let file_content = "a\nhello world   \nc\n";
        let result = verify_original(file_content, 2, 2, "hello world ");
        assert!(result.is_ok(), "should accept match when LLM provides partial trailing whitespace");
        let ws = result.unwrap();
        assert_eq!(ws[0], "   ", "should capture full original trailing whitespace for reapplication");
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
    fn fold_preserves_ascii() {
        assert_eq!(fold_unicode_to_ascii("hello world"), "hello world");
    }

    #[test]
    fn fold_mixed_content() {
        assert_eq!(
            fold_unicode_to_ascii("Ollama\u{2019}s \u{201C}best\u{201D} \u{2014} ever"),
            "Ollama's \"best\" -- ever"
        );
    }

    // --- process_replacement_escapes tests ---

    #[test]
    fn replacement_escapes_newline() {
        assert_eq!(process_replacement_escapes("line1\\nline2"), "line1\nline2");
    }

    #[test]
    fn replacement_escapes_tab() {
        assert_eq!(process_replacement_escapes("col1\\tcol2"), "col1\tcol2");
    }

    #[test]
    fn replacement_escapes_backslash() {
        assert_eq!(process_replacement_escapes("path\\\\file"), "path\\file");
    }

    #[test]
    fn replacement_preserves_unknown_escapes() {
        // \$ is not a recognized escape — keep as-is
        assert_eq!(process_replacement_escapes("\\$5"), "\\$5");
    }

    #[test]
    fn replacement_no_escapes() {
        assert_eq!(process_replacement_escapes("hello world"), "hello world");
    }

    #[test]
    fn replacement_multiple_escapes() {
        assert_eq!(
            process_replacement_escapes("line1\\nline2\\nline3"),
            "line1\nline2\nline3"
        );
    }

    #[test]
    fn replacement_trailing_backslash() {
        // Trailing backslash with no following character — keep as-is
        assert_eq!(process_replacement_escapes("end\\"), "end\\");
    }

    // --- strip_trailing_ws_per_line tests ---

    #[test]
    fn strip_ws_single_line() {
        assert_eq!(strip_trailing_ws_per_line("hello   "), "hello");
    }

    #[test]
    fn strip_ws_multi_line() {
        assert_eq!(strip_trailing_ws_per_line("hello   \nworld  "), "hello\nworld");
    }

    #[test]
    fn strip_ws_no_trailing() {
        assert_eq!(strip_trailing_ws_per_line("hello\nworld"), "hello\nworld");
    }

    #[test]
    fn strip_ws_tabs() {
        assert_eq!(strip_trailing_ws_per_line("hello\t\t\nworld\t"), "hello\nworld");
    }

    // --- build_stripped_to_original_offset_map tests ---

    #[test]
    fn offset_map_no_trailing_ws() {
        let original = "hello\nworld\n";
        let stripped = strip_trailing_ws_per_line(original);
        let map = build_stripped_to_original_offset_map(original, &stripped);
        // "hello\nworld" — each byte maps 1:1
        assert_eq!(map.len(), stripped.len() + 1);
        assert_eq!(map[0], 0); // 'h'
        assert_eq!(map[5], 5); // '\n'
        assert_eq!(map[6], 6); // 'w'
        assert_eq!(map[11], 11); // end-of-string
    }

    #[test]
    fn offset_map_with_trailing_ws() {
        let original = "hello   \nworld  \n";
        let stripped = strip_trailing_ws_per_line(original);
        assert_eq!(stripped, "hello\nworld");
        let map = build_stripped_to_original_offset_map(original, &stripped);
        // "hello" in stripped starts at byte 0, maps to byte 0 in original
        // "hello" is 5 bytes, then \n in stripped is byte 5, maps to byte 8 in original (after "   ")
        assert_eq!(map[0], 0); // 'h'
        assert_eq!(map[5], 8); // '\n' after skipping "   "
        assert_eq!(map[6], 9); // 'w'
        // "world" is 5 bytes, end-of-string in stripped is byte 11
        assert_eq!(map[11], 16); // end position (after "  " in original, before trailing \n)
    }

    // --- try_literal_trailing_ws_match tests ---

    #[test]
    fn literal_ws_match_finds_match_with_trailing_ws_dropped() {
        let original = "line one   \nline two   \nline three\n";
        let pattern = "line one\nline two"; // LLM dropped trailing ws
        let replacement = "LINE ONE\nLINE TWO";
        let result = try_literal_trailing_ws_match(original, pattern, replacement, 0);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 1);
                assert_eq!(new_content, "LINE ONE   \nLINE TWO   \nline three\n");
            }
            LiteralMatchResult::NoMatch => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_ws_match_no_match_for_different_content() {
        let original = "line one\nline two\n";
        let pattern = "line three\nline four"; // completely different
        let replacement = "replaced";
        let result = try_literal_trailing_ws_match(original, pattern, replacement, 0);
        assert!(matches!(result, LiteralMatchResult::NoMatch));
    }

    #[test]
    fn literal_ws_match_single_char_pattern_returns_no_match() {
        let original = "abc\n";
        let pattern = "a";
        let replacement = "X";
        let result = try_literal_trailing_ws_match(original, pattern, replacement, 0);
        assert!(matches!(result, LiteralMatchResult::NoMatch), "single-char patterns should not trigger fallback");
    }

    #[test]
    fn literal_ws_match_preserves_trailing_ws_in_replacement() {
        let original = "hello   \nworld\t\n";
        let pattern = "hello\nworld"; // LLM dropped trailing ws
        let replacement = "HELLO\nWORLD";
        let result = try_literal_trailing_ws_match(original, pattern, replacement, 0);
        match result {
            LiteralMatchResult::Matched { new_content, .. } => {
                assert_eq!(new_content, "HELLO   \nWORLD\t\n");
            }
            LiteralMatchResult::NoMatch => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_ws_match_exact_match_still_found() {
        // Pattern matches exactly (no trailing ws difference) — the regex
        // should have matched in this case, but verify the fallback also
        // finds it correctly.
        let original = "line one\nline two\n";
        let pattern = "line one\nline two";
        let replacement = "LINE ONE\nLINE TWO";
        let result = try_literal_trailing_ws_match(original, pattern, replacement, 0);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 1);
                assert_eq!(new_content, "LINE ONE\nLINE TWO\n");
            }
            LiteralMatchResult::NoMatch => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_ws_match_multiple_matches() {
        let original = "foo   \nbar\nfoo   \nbaz\n";
        let pattern = "foo"; // matches twice (after stripping trailing ws)
        let replacement = "FOO";
        let result = try_literal_trailing_ws_match(original, pattern, replacement, 0);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 2);
                assert_eq!(new_content, "FOO   \nbar\nFOO   \nbaz\n");
            }
            LiteralMatchResult::NoMatch => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_ws_match_processes_pattern_escapes() {
        // Pattern uses \n (two-char escape) which should be processed to a
        // real newline before the literal search. Without this processing,
        // the fallback would never match multi-line patterns.
        let original = "line one   \nline two   \nline three\n";
        let pattern = "line one\\nline two"; // \n as escape, not real newline
        let replacement = "LINE ONE\nLINE TWO";
        let result = try_literal_trailing_ws_match(original, pattern, replacement, 0);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 1);
                assert_eq!(new_content, "LINE ONE   \nLINE TWO   \nline three\n");
            }
            LiteralMatchResult::NoMatch => panic!("expected a match with escaped \\n in pattern"),
        }
    }

    #[test]
    fn literal_ws_match_max_replacements_limits_matches() {
        let original = "foo   \nbar\nfoo   \nbaz\n";
        let pattern = "foo";
        let replacement = "FOO";
        let result = try_literal_trailing_ws_match(original, pattern, replacement, 1);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 1);
                assert_eq!(new_content, "FOO   \nbar\nfoo   \nbaz\n");
            }
            LiteralMatchResult::NoMatch => panic!("expected a match"),
        }
    }

    #[test]
    fn literal_ws_match_max_replacements_zero_means_unlimited() {
        let original = "foo   \nbar\nfoo   \nbaz\n";
        let pattern = "foo";
        let replacement = "FOO";
        let result = try_literal_trailing_ws_match(original, pattern, replacement, 0);
        match result {
            LiteralMatchResult::Matched { new_content, match_count } => {
                assert_eq!(match_count, 2);
                assert_eq!(new_content, "FOO   \nbar\nFOO   \nbaz\n");
            }
            LiteralMatchResult::NoMatch => panic!("expected a match"),
        }
    }
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
) -> anyhow::Result<usize> {
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
) -> anyhow::Result<()> {
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

fn skill_root() -> anyhow::Result<std::path::PathBuf> {
    let chai_home = lib::profile::chai_home()?;
    Ok(lib::config::default_skills_dir(&chai_home))
}

fn validate_skill_name(name: &str) -> anyhow::Result<()> {
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        anyhow::bail!("skill_name must not contain '..', '/', or '\\'");
    }
    Ok(())
}

fn run_skill(cmd: SkillCmd) -> anyhow::Result<()> {
    match cmd {
        SkillCmd::Discover { binary, subcommand } => {
            let which = std::process::Command::new("which").arg(&binary).output();
            match which {
                Ok(out) if out.status.success() => {}
                _ => anyhow::bail!("'{}' not found on PATH", binary),
            }
            let mut cmd = std::process::Command::new(&binary);
            if let Some(ref sub) = subcommand {
                cmd.arg(sub);
            }
            cmd.arg("--help");
            let output = cmd
                .output()
                .map_err(|e| anyhow::anyhow!("failed to run '{}': {}", binary, e))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stdout.is_empty() {
                print!("{}", stdout);
            }
            if !stderr.is_empty() {
                eprint!("{}", stderr);
            }
            Ok(())
        }
        SkillCmd::List => {
            let root = skill_root()?;
            if !root.exists() {
                anyhow::bail!("skill root not found: {}", root.display());
            }
            let mut entries: Vec<_> = std::fs::read_dir(&root)?
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                let dir = entry.path();
                let Some(content_dir) = lib::skills::versioning::resolve_active_dir(&dir) else {
                    println!(
                        "{:<20}  SKILL.md: no   tools.json: no   tools: 0  (missing versioned layout)",
                        name
                    );
                    continue;
                };
                let has_skill_md = content_dir.join("SKILL.md").exists();
                let has_tools_json = content_dir.join("tools.json").exists();
                let tool_count = if has_tools_json {
                    count_tools(&content_dir.join("tools.json")).unwrap_or(0)
                } else {
                    0
                };
                println!(
                    "{:<20}  SKILL.md: {:<3}  tools.json: {:<3}  tools: {}",
                    name,
                    if has_skill_md { "yes" } else { "no" },
                    if has_tools_json { "yes" } else { "no" },
                    tool_count
                );
            }
            Ok(())
        }
        SkillCmd::Read { skill_name, file } => {
            validate_skill_name(&skill_name)?;
            let skill_dir = skill_root()?.join(&skill_name);
            if !skill_dir.exists() {
                anyhow::bail!(
                    "skill '{}' not found at {}",
                    skill_name,
                    skill_dir.display()
                );
            }
            let content_dir = lib::skills::versioning::resolve_active_dir(&skill_dir).ok_or_else(
                || {
                    anyhow::anyhow!(
                        "skill '{}' has no valid `active` symlink to versions/<hash>/ (expected versioned layout)",
                        skill_name
                    )
                },
            )?;
            let path = match file.as_str() {
                "skill_md" => content_dir.join("SKILL.md"),
                "tools_json" => content_dir.join("tools.json"),
                other => anyhow::bail!(
                    "file type must be 'skill_md' or 'tools_json', got '{}'",
                    other
                ),
            };
            let content = std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path.display(), e))?;
            print!("{}", content);
            Ok(())
        }
        SkillCmd::Init {
            skill_name,
            description,
        } => {
            validate_skill_name(&skill_name)?;
            let skill_dir = skill_root()?.join(&skill_name);
            if skill_dir.exists() {
                anyhow::bail!(
                    "skill '{}' already exists at {}",
                    skill_name,
                    skill_dir.display()
                );
            }
            std::fs::create_dir_all(&skill_dir)?;
            let desc = description.as_deref().unwrap_or("TODO");
            let skill_md = format!(
                "---\ndescription: {}\ncapability_tier: moderate\nmetadata:\n  requires:\n    bins: []\n---\n",
                desc
            );
            let tools_json = "{\n  \"tools\": [],\n  \"allowlist\": {},\n  \"execution\": []\n}\n";
            // Compute hash from initial content and create versioned layout
            let entries: Vec<(&str, &[u8])> = vec![
                ("SKILL.md", skill_md.as_bytes()),
                ("tools.json", tools_json.as_bytes()),
            ];
            let hash = lib::skills::versioning::compute_hash_from_entries(&entries);
            let snapshot_dir = skill_dir.join("versions").join(&hash);
            std::fs::create_dir_all(&snapshot_dir)?;
            std::fs::write(snapshot_dir.join("SKILL.md"), &skill_md)?;
            std::fs::write(snapshot_dir.join("tools.json"), tools_json)?;
            lib::skills::versioning::set_active_version(&skill_dir, &hash)?;
            println!(
                "initialized skill '{}' at {} (version {})",
                skill_name,
                skill_dir.display(),
                hash,
            );
            Ok(())
        }
        SkillCmd::WriteSkillMd {
            skill_name,
            content,
        } => {
            validate_skill_name(&skill_name)?;
            let content = read_content_from_stdin_or(content)?;
            let skill_dir = skill_root()?.join(&skill_name);
            if !skill_dir.exists() {
                anyhow::bail!("skill '{}' not found", skill_name);
            }
            let hash = lib::skills::versioning::write_and_snapshot(
                &skill_dir,
                "SKILL.md",
                content.as_bytes(),
            )?;
            println!(
                "wrote SKILL.md for '{}' ({} bytes, version {})",
                skill_name,
                content.len(),
                hash,
            );
            Ok(())
        }
        SkillCmd::WriteToolsJson {
            skill_name,
            content,
        } => {
            validate_skill_name(&skill_name)?;
            let content = read_content_from_stdin_or(content)?;
            let skill_dir = skill_root()?.join(&skill_name);
            if !skill_dir.exists() {
                anyhow::bail!("skill '{}' not found", skill_name);
            }
            let _: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| anyhow::anyhow!("invalid JSON: {}", e))?;
            let hash = lib::skills::versioning::write_and_snapshot(
                &skill_dir,
                "tools.json",
                content.as_bytes(),
            )?;
            println!(
                "wrote tools.json for '{}' ({} bytes, version {})",
                skill_name,
                content.len(),
                hash,
            );
            Ok(())
        }
        SkillCmd::WriteScript {
            skill_name,
            script_name,
            content,
        } => {
            validate_skill_name(&skill_name)?;
            let content = read_content_from_stdin_or(content)?;
            if script_name.contains("..") || script_name.contains('/') || script_name.contains('\\')
            {
                anyhow::bail!("script_name must not contain '..', '/', or '\\'");
            }
            let skill_dir = skill_root()?.join(&skill_name);
            if !skill_dir.exists() {
                anyhow::bail!("skill '{}' not found", skill_name);
            }
            let rel_path = format!("scripts/{}.sh", script_name);
            let hash = lib::skills::versioning::write_and_snapshot(
                &skill_dir,
                &rel_path,
                content.as_bytes(),
            )?;
            println!(
                "wrote script '{}.sh' for '{}' ({} bytes, version {})",
                script_name,
                skill_name,
                content.len(),
                hash,
            );
            Ok(())
        }
        SkillCmd::Validate { skill_name } => {
            validate_skill_name(&skill_name)?;
            let skill_dir = skill_root()?.join(&skill_name);
            if !skill_dir.exists() {
                anyhow::bail!("skill '{}' not found", skill_name);
            }
            let content_dir = lib::skills::versioning::resolve_active_dir(&skill_dir).ok_or_else(
                || {
                    anyhow::anyhow!(
                        "skill '{}' has no valid `active` symlink to versions/<hash>/ (expected versioned layout)",
                        skill_name
                    )
                },
            )?;
            let tools_path = content_dir.join("tools.json");
            if !tools_path.exists() {
                anyhow::bail!("no tools.json for '{}'", skill_name);
            }
            let content = std::fs::read_to_string(&tools_path)?;
            validate_tools_json(&content)
        }
        SkillCmd::Lock => {
            let paths = lib::profile::resolve_profile_dir(None)?;
            let skills_dir = skill_root()?;
            let all_entries = lib::skills::load_skills(&skills_dir)?;
            if all_entries.is_empty() {
                anyhow::bail!("no skills found at {}", skills_dir.display());
            }
            let lock = match lib::skills::lockfile::read_lock(&paths.profile_dir)? {
                Some(mut existing) => {
                    existing.update(&all_entries)?;
                    existing
                }
                None => lib::skills::lockfile::SkillsLock::from_entries(&all_entries)?,
            };
            lib::skills::lockfile::write_lock(&paths.profile_dir, &lock)?;
            println!(
                "locked {} skill(s) at generation {} (profile: {})",
                lock.skills.len(),
                lock.generation,
                paths.profile_name,
            );
            for (name, pin) in &lock.skills {
                println!("  {} -> {}", name, pin.hash);
            }
            Ok(())
        }
        SkillCmd::Rollback { generation } => {
            let paths = lib::profile::resolve_profile_dir(None)?;
            let skills_dir = skill_root()?;
            lib::skills::lockfile::rollback(&paths.profile_dir, generation, &skills_dir)?;
            let restored = lib::skills::lockfile::read_lock(&paths.profile_dir)?
                .expect("lockfile should exist after rollback");
            println!(
                "rolled back to generation {} ({} skill(s), profile: {})",
                restored.generation,
                restored.skills.len(),
                paths.profile_name,
            );
            for (name, pin) in &restored.skills {
                println!("  {} -> {}", name, pin.hash);
            }
            Ok(())
        }
        SkillCmd::Generations => {
            let paths = lib::profile::resolve_profile_dir(None)?;
            let generations = lib::skills::lockfile::list_generations(&paths.profile_dir)?;
            if generations.is_empty() {
                println!(
                    "no lockfile generations found (profile: {})",
                    paths.profile_name
                );
            } else {
                let current =
                    lib::skills::lockfile::read_lock(&paths.profile_dir)?.map(|l| l.generation);
                println!("lockfile generations (profile: {}):", paths.profile_name);
                for gen in &generations {
                    let marker = if current == Some(*gen) {
                        " (current)"
                    } else {
                        ""
                    };
                    println!("  generation {}{}", gen, marker);
                }
            }
            Ok(())
        }
        SkillCmd::Delete { skill_name } => {
            validate_skill_name(&skill_name)?;
            let skill_dir = skill_root()?.join(&skill_name);
            if !skill_dir.exists() {
                anyhow::bail!(
                    "skill '{}' not found at {}",
                    skill_name,
                    skill_dir.display()
                );
            }
            if !skill_dir.is_dir() {
                anyhow::bail!(
                    "'{}' is not a directory at {}",
                    skill_name,
                    skill_dir.display()
                );
            }
            std::fs::remove_dir_all(&skill_dir).map_err(|e| {
                anyhow::anyhow!(
                    "failed to delete skill '{}': {}",
                    skill_name,
                    e
                )
            })?;
            println!(
                "deleted skill '{}' ({})",
                skill_name,
                skill_dir.display()
            );
            Ok(())
        }
    }
}

fn count_tools(path: &std::path::Path) -> Option<usize> {
    let content = std::fs::read_to_string(path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;
    data.get("tools")?.as_array().map(|a| a.len())
}

fn json_value_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn validate_tools_json(content: &str) -> anyhow::Result<()> {
    let data: serde_json::Value =
        serde_json::from_str(content).map_err(|e| anyhow::anyhow!("FAIL: invalid JSON: {}", e))?;

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for key in &["tools", "allowlist", "execution"] {
        if data.get(key).is_none() {
            errors.push(format!("missing required key: {}", key));
        }
    }
    if !errors.is_empty() {
        for e in &errors {
            println!("ERROR: {}", e);
        }
        anyhow::bail!("validation failed");
    }

    if !data["tools"].is_array() {
        errors.push(format!(
            "tools must be a JSON array, got {}",
            json_value_kind(&data["tools"])
        ));
    }
    if !data["allowlist"].is_object() {
        errors.push(format!(
            "allowlist must be a JSON object, got {}",
            json_value_kind(&data["allowlist"])
        ));
    }
    if !data["execution"].is_array() {
        errors.push(format!(
            "execution must be a JSON array, got {}",
            json_value_kind(&data["execution"])
        ));
    }
    if !errors.is_empty() {
        for e in &errors {
            println!("ERROR: {}", e);
        }
        anyhow::bail!("validation failed");
    }

    let tools = data["tools"].as_array().expect("tools validated as array");
    let allowlist = data["allowlist"]
        .as_object()
        .expect("allowlist validated as object");
    let execution = data["execution"]
        .as_array()
        .expect("execution validated as array");

    let mut tool_names = std::collections::HashSet::new();
    let mut exec_tool_names = std::collections::HashSet::new();

    for (i, tool) in tools.iter().enumerate() {
        if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
            if !tool_names.insert(name.to_string()) {
                errors.push(format!("tools[{}]: duplicate name \"{}\"", i, name));
            }
        } else {
            errors.push(format!("tools[{}]: missing name", i));
        }
        if let Some(params) = tool.get("parameters") {
            if params.get("type").and_then(|t| t.as_str()) != Some("object") {
                let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                warnings.push(format!(
                    "tools[{}] ({}): parameters.type should be \"object\"",
                    i, name
                ));
            }
        }
    }

    for (i, ex) in execution.iter().enumerate() {
        if let Some(tool_name) = ex.get("tool").and_then(|t| t.as_str()) {
            exec_tool_names.insert(tool_name.to_string());
            if !tool_names.contains(tool_name) {
                errors.push(format!(
                    "execution[{}]: tool \"{}\" not in tools",
                    i, tool_name
                ));
            }
        } else {
            errors.push(format!("execution[{}]: missing tool", i));
        }

        let binary = ex.get("binary").and_then(|b| b.as_str());
        let subcommand = ex.get("subcommand").and_then(|s| s.as_str());

        if let Some(bin) = binary {
            if !allowlist.contains_key(bin) {
                errors.push(format!(
                    "execution[{}]: binary \"{}\" not in allowlist",
                    i, bin
                ));
            } else if let Some(sub) = subcommand {
                let allowed = allowlist[bin]
                    .as_array()
                    .map(|a| a.iter().any(|s| s.as_str() == Some(sub)))
                    .unwrap_or(false);
                if !allowed {
                    errors.push(format!(
                        "execution[{}]: subcommand \"{}\" not in allowlist[\"{}\"]",
                        i, sub, bin
                    ));
                }
            }
        } else {
            errors.push(format!("execution[{}]: missing binary", i));
        }
        if subcommand.is_none() {
            errors.push(format!("execution[{}]: missing subcommand", i));
        }
    }

    for name in &tool_names {
        if !exec_tool_names.contains(name) {
            errors.push(format!("tool \"{}\" has no execution spec", name));
        }
    }
    for name in &exec_tool_names {
        if !tool_names.contains(name) {
            errors.push(format!("execution references undefined tool \"{}\"", name));
        }
    }

    for e in &errors {
        println!("ERROR: {}", e);
    }
    for w in &warnings {
        println!("WARNING: {}", w);
    }

    if errors.is_empty() {
        let allowlist_count: usize = allowlist
            .values()
            .filter_map(|v| v.as_array())
            .map(|a| a.len())
            .sum();
        let status = if warnings.is_empty() {
            "PASS"
        } else {
            "PASS (with warnings)"
        };
        println!(
            "{}: {} tools, {} execution specs, {} allowlisted subcommands",
            status,
            tools.len(),
            execution.len(),
            allowlist_count
        );
        Ok(())
    } else {
        anyhow::bail!("validation failed with {} error(s)", errors.len())
    }
}

async fn run_gateway(profile: Option<&str>, port: Option<u16>) -> anyhow::Result<()> {
    let (mut config, paths) = lib::config::load_config(profile)?;
    if let Some(p) = port {
        config.gateway.port = p;
    }
    log::info!(
        "starting gateway profile={} on {}:{}",
        paths.profile_name,
        config.gateway.bind,
        config.gateway.port
    );
    lib::gateway::run_gateway(config, paths).await
}

#[derive(Debug)]
struct AgentReply {
    session_id: String,
    reply: String,
}

/// Same copy as desktop chat **`/help`** (see `crates/desktop/src/app.rs`).
const CHAT_HELP_TEXT: &str = "available commands:\n\n/new - start a new session (clear conversation history)\n/help - show this help message";

/// Same acknowledgment as desktop **`/new`** (see `crates/desktop/src/app.rs`).
const CHAT_NEW_SESSION_ACK: &str =
    "New session. Next message will start with a clean history.";

async fn run_chat(profile: Option<String>, session: Option<String>) -> anyhow::Result<()> {
    use std::io::{self, Write};

    let mut current_session = session;
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        write!(stdout, "> ")?;
        stdout.flush()?;
        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        if input.eq_ignore_ascii_case("/exit") || input.eq_ignore_ascii_case("/quit") {
            break;
        }
        if input.eq_ignore_ascii_case("/new") {
            current_session = None;
            println!("< {}", CHAT_NEW_SESSION_ACK);
            continue;
        }
        if input.eq_ignore_ascii_case("/help") {
            println!("{}", CHAT_HELP_TEXT);
            continue;
        }

        match agent_turn_via_gateway(profile.as_deref(), current_session.clone(), input.to_string()).await {
            Ok(reply) => {
                current_session = Some(reply.session_id);
                println!("< {}", reply.reply.trim());
            }
            Err(e) => {
                eprintln!("chat error: {}", e);
            }
        }
    }

    Ok(())
}

async fn agent_turn_via_gateway(
    profile: Option<&str>,
    session_id: Option<String>,
    message: String,
) -> Result<AgentReply, String> {
    let (config, paths) = lib::config::load_config(profile).map_err(|e| e.to_string())?;
    let bind = config.gateway.bind.trim();
    let port = config.gateway.port;
    let token = lib::config::resolve_gateway_token(&config);
    let ws_url = format!("ws://{}:{}/ws", bind, port);

    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .map_err(|e| e.to_string())?;

    let first = ws
        .next()
        .await
        .ok_or("no first frame")?
        .map_err(|e| e.to_string())?;
    let Message::Text(challenge_text) = first else {
        return Err("expected text challenge frame".to_string());
    };
    let challenge: serde_json::Value =
        serde_json::from_str(&challenge_text).map_err(|e| e.to_string())?;
    let nonce = challenge
        .get("payload")
        .and_then(|p| p.get("nonce").and_then(|n| n.as_str()))
        .ok_or("expected connect.challenge event with nonce")?
        .to_string();

    let device_token_path = paths.device_token_path();
    let device_json_path = paths.device_json();

    let connect_params =
        if let Some(device_token) = lib::device::load_device_token_from(&device_token_path) {
            serde_json::json!({ "auth": { "deviceToken": device_token } })
        } else {
            let identity = lib::device::DeviceIdentity::load(device_json_path.as_path())
                .or_else(|| {
                    let id = lib::device::DeviceIdentity::generate().ok()?;
                    let _ = id.save(&device_json_path);
                    Some(id)
                })
                .ok_or("failed to load or create device identity")?;
            let signed_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let token_str = token.as_deref().unwrap_or("");
            let scopes: Vec<String> = vec!["operator.read".into(), "operator.write".into()];
            let payload_str = lib::device::build_connect_payload(
                &identity.device_id,
                "chai-cli",
                "operator",
                "operator",
                &scopes,
                signed_at,
                token_str,
                &nonce,
            );
            let signature = identity.sign(&payload_str).map_err(|e| e.to_string())?;
            let mut params = serde_json::json!({
                "client": { "id": "chai-cli", "mode": "operator" },
                "role": "operator",
                "scopes": scopes,
                "device": {
                    "id": identity.device_id,
                    "publicKey": identity.public_key,
                    "signature": signature,
                    "signedAt": signed_at,
                    "nonce": nonce
                }
            });
            if let Some(ref t) = token {
                params["auth"] = serde_json::json!({ "token": t });
            } else {
                params["auth"] = serde_json::json!({});
            }
            params
        };

    let connect_req = serde_json::json!({
        "type": "req",
        "id": "1",
        "method": "connect",
        "params": connect_params
    });
    ws.send(Message::Text(connect_req.to_string()))
        .await
        .map_err(|e| e.to_string())?;

    while let Some(msg) = ws.next().await {
        let msg = msg.map_err(|e| e.to_string())?;
        let Message::Text(text) = msg else { continue };
        let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if res.get("type").and_then(|v| v.as_str()) != Some("res") {
            continue;
        }
        if res.get("id").and_then(|v| v.as_str()) == Some("1") {
            if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                let err = res
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("connect failed");
                return Err(err.to_string());
            }
            if let Some(auth) = res.get("payload").and_then(|p| p.get("auth")) {
                if let Some(dt) = auth.get("deviceToken").and_then(|v| v.as_str()) {
                    let _ = lib::device::save_device_token_to(&device_token_path, dt);
                }
            }
            break;
        }
    }

    let mut agent_params = serde_json::json!({
        "message": message,
    });
    if let Some(id) = session_id {
        agent_params["sessionId"] = serde_json::Value::String(id);
    }

    let agent_req = serde_json::json!({
        "type": "req",
        "id": "2",
        "method": "agent",
        "params": agent_params
    });
    ws.send(Message::Text(agent_req.to_string()))
        .await
        .map_err(|e| e.to_string())?;

    while let Some(msg) = ws.next().await {
        let msg = msg.map_err(|e| e.to_string())?;
        let Message::Text(text) = msg else { continue };
        let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if res.get("type").and_then(|v| v.as_str()) != Some("res") {
            continue;
        }
        if res.get("id").and_then(|v| v.as_str()) == Some("2") {
            if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                let err = res
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("agent failed");
                return Err(err.to_string());
            }
            let payload = res.get("payload").ok_or("missing payload")?;
            let session_id = payload
                .get("sessionId")
                .and_then(|v| v.as_str())
                .ok_or("missing sessionId in agent response")?
                .to_string();
            let reply = payload
                .get("reply")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            return Ok(AgentReply { session_id, reply });
        }
    }

    Err("no agent response".to_string())
}
