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

    /// Create ~/.chai with profiles (assistant, developer), active symlink, and shared skills
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
            if let Err(e) = run_chat(profile.as_deref(), session).await {
                log::error!("chat failed: {}", e);
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
    /// Write or overwrite SKILL.md for a skill
    WriteSkillMd {
        /// Skill directory name
        skill_name: String,
        /// Complete SKILL.md content including frontmatter
        #[arg(long)]
        content: String,
    },
    /// Write or overwrite tools.json for a skill (validates JSON before writing)
    WriteToolsJson {
        /// Skill directory name
        skill_name: String,
        /// Complete tools.json content as JSON
        #[arg(long)]
        content: String,
    },
    /// Write a script to a skill's scripts/ directory
    WriteScript {
        /// Skill directory name
        skill_name: String,
        /// Script filename without .sh extension
        script_name: String,
        /// Complete script content
        #[arg(long)]
        content: String,
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
}

#[derive(Subcommand)]
enum FileCmd {
    /// Write content to a file (creates or overwrites). Parent directory must exist.
    Write {
        /// Absolute file path to write to
        #[arg(long)]
        path: String,
        /// Content to write to the file
        #[arg(long)]
        content: String,
    },
    /// Append content to an existing file. Creates the file if it does not exist.
    Append {
        /// Absolute file path to append to
        #[arg(long)]
        path: String,
        /// Content to append to the file
        #[arg(long)]
        content: String,
    },
    /// Delete a file. Refuses to delete directories.
    Delete {
        /// Absolute file path to delete
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
        /// Value to set the key to
        #[arg(long)]
        value: String,
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

fn run_file(cmd: FileCmd) -> anyhow::Result<()> {
    match cmd {
        FileCmd::Write { path, content } => {
            let target = std::path::Path::new(&path);
            if let Some(parent) = target.parent() {
                if !parent.exists() {
                    anyhow::bail!("parent directory does not exist: {}", parent.display());
                }
            }
            std::fs::write(target, &content)
                .map_err(|e| anyhow::anyhow!("failed to write {}: {}", path, e))?;
            println!("wrote {} ({} bytes)", path, content.len());
            Ok(())
        }
        FileCmd::Append { path, content } => {
            use std::io::Write;
            let target = std::path::Path::new(&path);
            if let Some(parent) = target.parent() {
                if !parent.exists() {
                    anyhow::bail!("parent directory does not exist: {}", parent.display());
                }
            }
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(target)
                .map_err(|e| anyhow::anyhow!("failed to open {}: {}", path, e))?;
            file.write_all(content.as_bytes())
                .map_err(|e| anyhow::anyhow!("failed to append to {}: {}", path, e))?;
            println!("appended {} bytes to {}", content.len(), path);
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
        FileCmd::FrontmatterEdit { path, key, value } => {
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
        // No frontmatter — create one.
        return format!("{}---\n{}: {}\n---\n{}", leading, key, value, trimmed);
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
                // Closing delimiter — insert key if not yet found.
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
        assert!(out.starts_with("\n\n---\ntitle: new\n---\nbody\n"));
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
    if let Ok(root) = std::env::var("CHAI_SKILLS_ROOT") {
        return Ok(std::path::PathBuf::from(root));
    }
    let chai_home = lib::profile::chai_home()?;
    Ok(chai_home.join("skills"))
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
                let content_dir = lib::skills::versioning::resolve_active_dir(&dir);
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
            let skill_dir = skill_root()?.join(&skill_name);
            if !skill_dir.exists() {
                anyhow::bail!(
                    "skill '{}' not found at {}",
                    skill_name,
                    skill_dir.display()
                );
            }
            let content_dir = lib::skills::versioning::resolve_active_dir(&skill_dir);
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
                "---\nname: {}\ndescription: {}\nmetadata:\n  requires:\n    bins: []\n---\n",
                skill_name, desc
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
            let skill_dir = skill_root()?.join(&skill_name);
            if !skill_dir.exists() {
                anyhow::bail!("skill '{}' not found", skill_name);
            }
            let content_dir = lib::skills::versioning::resolve_active_dir(&skill_dir);
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
                println!("  {} → {}", name, pin.hash);
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
                println!("  {} → {}", name, pin.hash);
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
    "Session restarted. Next message will start with a clean history.";

async fn run_chat(profile: Option<&str>, session: Option<String>) -> anyhow::Result<()> {
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

        match agent_turn_via_gateway(profile, current_session.clone(), input.to_string()).await {
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
