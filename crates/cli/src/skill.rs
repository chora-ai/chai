use anyhow::Result;
use clap::Subcommand;

use crate::file::read_content_from_stdin_or;

#[derive(Subcommand)]
pub(crate) enum SkillCmd {
    /// Discover a CLI binary's interface by running its help output
    Discover {
        /// CLI binary name (e.g. 'chai', 'git', 'rg')
        binary: String,
        /// Specific subcommand to get detailed help for
        #[arg(long)]
        subcommand: Option<String>,
    },
    /// List installed skills with population status
    List,
    /// Read a skill's SKILL.md, tools.json, allowlist.json, or execution.json file
    Read {
        /// Skill directory name (e.g. 'files')
        skill_name: String,
        /// File to read: 'skill_md', 'tools_json', 'allowlist_json', or 'execution_json'
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
        /// Complete tools.json content as JSON (tool definitions array). If omitted, content is read from stdin.
        #[arg(long, allow_hyphen_values = true)]
        content: Option<String>,
    },
    /// Write or overwrite allowlist.json for a skill (validates JSON before writing). Content is read from stdin when --content is omitted.
    WriteAllowlistJson {
        /// Skill directory name
        skill_name: String,
        /// Complete allowlist.json content as JSON. If omitted, content is read from stdin.
        #[arg(long, allow_hyphen_values = true)]
        content: Option<String>,
    },
    /// Write or overwrite execution.json for a skill (validates JSON before writing). Content is read from stdin when --content is omitted.
    WriteExecutionJson {
        /// Skill directory name
        skill_name: String,
        /// Complete execution.json content as JSON. If omitted, content is read from stdin.
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
    /// Validate a skill's tools.json, allowlist.json, and execution.json for schema conformance
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
    /// Preview what a tool call would execute without running the command
    DryRun {
        /// Tool name to preview (e.g. 'git_commit', 'files_write')
        tool: String,
        /// Tool call arguments as JSON (e.g. '{"message": "feat: add feature"}')
        #[arg(long)]
        args: String,
        /// Simulated command output for post-execution pipeline preview (postProcess, hintConditions, truncation)
        #[arg(long)]
        simulated_output: Option<String>,
        /// Profile name for sandbox resolution (uses default profile if omitted)
        #[arg(long)]
        profile: Option<String>,
    },
}

pub(crate) fn run_skill(cmd: SkillCmd) -> Result<()> {
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
                let has_allowlist_json = content_dir.join("allowlist.json").exists();
                let has_execution_json = content_dir.join("execution.json").exists();
                let tool_count = if has_tools_json {
                    count_tools(&content_dir.join("tools.json")).unwrap_or(0)
                } else {
                    0
                };
                println!(
                    "{:<20}  SKILL.md: {:<3}  tools.json: {:<3}  allowlist.json: {:<3}  execution.json: {:<3}  tools: {}",
                    name,
                    if has_skill_md { "yes" } else { "no" },
                    if has_tools_json { "yes" } else { "no" },
                    if has_allowlist_json { "yes" } else { "no" },
                    if has_execution_json { "yes" } else { "no" },
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
                "allowlist_json" => content_dir.join("allowlist.json"),
                "execution_json" => content_dir.join("execution.json"),
                other => anyhow::bail!(
                    "file type must be 'skill_md', 'tools_json', 'allowlist_json', or 'execution_json', got '{}'",
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
            let tools_json = "[]\n";
            let allowlist_json = "{}\n";
            let execution_json = "[]\n";
            // Compute hash from initial content and create versioned layout
            let entries: Vec<(&str, &[u8])> = vec![
                ("SKILL.md", skill_md.as_bytes()),
                ("tools.json", tools_json.as_bytes()),
                ("allowlist.json", allowlist_json.as_bytes()),
                ("execution.json", execution_json.as_bytes()),
            ];
            let hash = lib::skills::versioning::compute_hash_from_entries(&entries);
            let snapshot_dir = skill_dir.join("versions").join(&hash);
            std::fs::create_dir_all(&snapshot_dir)?;
            std::fs::write(snapshot_dir.join("SKILL.md"), &skill_md)?;
            std::fs::write(snapshot_dir.join("tools.json"), tools_json)?;
            std::fs::write(snapshot_dir.join("allowlist.json"), allowlist_json)?;
            std::fs::write(snapshot_dir.join("execution.json"), execution_json)?;
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
        SkillCmd::WriteAllowlistJson {
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
                "allowlist.json",
                content.as_bytes(),
            )?;
            println!(
                "wrote allowlist.json for '{}' ({} bytes, version {})",
                skill_name,
                content.len(),
                hash,
            );
            Ok(())
        }
        SkillCmd::WriteExecutionJson {
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
                "execution.json",
                content.as_bytes(),
            )?;
            println!(
                "wrote execution.json for '{}' ({} bytes, version {})",
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
            let tools_content = std::fs::read_to_string(&tools_path)?;
            let tools_root: serde_json::Value = serde_json::from_str(&tools_content)
                .map_err(|e| anyhow::anyhow!("invalid JSON in tools.json: {}", e))?;
            match tools_root {
                serde_json::Value::Object(_) => {
                    // Legacy single-file format.
                    println!("WARNING: tools.json uses the legacy single-file format — migrate to the three-file format (tools.json as array, allowlist.json, execution.json)");
                    validate_tools_json_legacy(&tools_content)
                }
                serde_json::Value::Array(_) => {
                    // New three-file format.
                    let allowlist_path = content_dir.join("allowlist.json");
                    let execution_path = content_dir.join("execution.json");
                    let allowlist_content = match std::fs::read_to_string(&allowlist_path) {
                        Ok(c) => c,
                        Err(_) => {
                            anyhow::bail!(
                                "tools.json is present (new format) but allowlist.json is missing"
                            );
                        }
                    };
                    let execution_content = match std::fs::read_to_string(&execution_path) {
                        Ok(c) => c,
                        Err(_) => {
                            anyhow::bail!(
                                "tools.json is present (new format) but execution.json is missing"
                            );
                        }
                    };
                    validate_tools_three_file(&tools_content, &allowlist_content, &execution_content)
                }
                other => {
                    anyhow::bail!(
                        "unexpected root type in tools.json: expected object or array, got {}",
                        json_value_kind(&other)
                    );
                }
            }
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
        SkillCmd::DryRun {
            tool,
            args,
            simulated_output,
            profile,
        } => {
            let args_value: serde_json::Value = serde_json::from_str(&args)
                .map_err(|e| anyhow::anyhow!("invalid JSON in --args: {}", e))?;

            let skills_dir = skill_root()?;
            if !skills_dir.exists() {
                anyhow::bail!("skill root not found: {}", skills_dir.display());
            }

            let all_entries = lib::skills::load_skills(&skills_dir)?;
            if all_entries.is_empty() {
                anyhow::bail!("no skills found at {}", skills_dir.display());
            }

            // Build descriptors and skill dirs for the executor.
            let mut descriptors: Vec<(String, lib::skills::ToolDescriptor)> = Vec::new();
            let mut skill_dirs: Vec<(String, std::path::PathBuf)> = Vec::new();
            for entry in &all_entries {
                if let Some(ref desc) = entry.tool_descriptor {
                    descriptors.push((entry.name.clone(), desc.clone()));
                    skill_dirs.push((entry.name.clone(), entry.path.clone()));
                }
            }

            // Resolve the write sandbox from the profile.
            let sandbox = {
                let paths = lib::profile::resolve_profile_dir(profile.as_deref())?;
                let sandbox_dir = paths.sandbox_dir();
                if sandbox_dir.exists() {
                    Some(lib::exec::WriteSandbox::new(&sandbox_dir))
                } else {
                    None
                }
            };

            let executor = lib::tools::GenericToolExecutor::from_descriptors(
                &descriptors,
                &skill_dirs,
                sandbox,
            );

            if !executor.has_tool(&tool) {
                // List available tools to help the user.
                let available: Vec<&String> = executor.tool_names().collect();
                anyhow::bail!(
                    "tool '{}' not found in any loaded skill. available tools: {}",
                    tool,
                    available.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                );
            }

            let result = executor
                .dry_run(&tool, &args_value, simulated_output.as_deref())
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            let json = serde_json::to_string_pretty(&result)
                .map_err(|e| anyhow::anyhow!("failed to serialize dry-run result: {}", e))?;
            println!("{}", json);
            Ok(())
        }
    }
}

fn skill_root() -> Result<std::path::PathBuf> {
    let chai_home = lib::profile::chai_home()?;
    Ok(lib::config::default_skills_dir(&chai_home))
}

fn validate_skill_name(name: &str) -> Result<()> {
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        anyhow::bail!("skill_name must not contain '..', '/', or '\\'");
    }
    Ok(())
}

fn count_tools(path: &std::path::Path) -> Option<usize> {
    let content = std::fs::read_to_string(path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;
    match data {
        serde_json::Value::Array(arr) => Some(arr.len()),
        serde_json::Value::Object(_) => data.get("tools")?.as_array().map(|a| a.len()),
        _ => None,
    }
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

/// Validate the legacy single-file tools.json format (root object with tools/allowlist/execution keys).
fn validate_tools_json_legacy(content: &str) -> Result<()> {
    let data: serde_json::Value =
        serde_json::from_str(content).map_err(|e| anyhow::anyhow!("fail: invalid JSON: {}", e))?;

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

    validate_cross_file_consistency(tools, allowlist, execution, &mut errors, &mut warnings);

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

/// Validate the new three-file format (tools.json as array, allowlist.json, execution.json).
fn validate_tools_three_file(
    tools_content: &str,
    allowlist_content: &str,
    execution_content: &str,
) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Validate each file independently.
    let tools_data: serde_json::Value = serde_json::from_str(tools_content)
        .map_err(|e| anyhow::anyhow!("tools.json: invalid JSON: {}", e))?;
    let tools = match tools_data.as_array() {
        Some(arr) => arr,
        None => {
            anyhow::bail!(
                "tools.json: root must be a JSON array, got {}",
                json_value_kind(&tools_data)
            );
        }
    };

    let allowlist_data: serde_json::Value = serde_json::from_str(allowlist_content)
        .map_err(|e| anyhow::anyhow!("allowlist.json: invalid JSON: {}", e))?;
    let allowlist = match allowlist_data.as_object() {
        Some(obj) => obj,
        None => {
            anyhow::bail!(
                "allowlist.json: root must be a JSON object, got {}",
                json_value_kind(&allowlist_data)
            );
        }
    };

    let execution_data: serde_json::Value = serde_json::from_str(execution_content)
        .map_err(|e| anyhow::anyhow!("execution.json: invalid JSON: {}", e))?;
    let execution = match execution_data.as_array() {
        Some(arr) => arr,
        None => {
            anyhow::bail!(
                "execution.json: root must be a JSON array, got {}",
                json_value_kind(&execution_data)
            );
        }
    };

    // Validate cross-file consistency.
    validate_cross_file_consistency(tools, allowlist, execution, &mut errors, &mut warnings);

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

/// Validate cross-file consistency: tool names in execution must match tools,
/// binary/subcommand pairs in execution must be in allowlist, and every tool
/// must have an execution spec.
fn validate_cross_file_consistency(
    tools: &[serde_json::Value],
    allowlist: &serde_json::Map<String, serde_json::Value>,
    execution: &[serde_json::Value],
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
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

        // Validate binaryWrapper: when present, must be a non-empty array of strings.
        if let Some(bw) = ex.get("binaryWrapper") {
            if let Some(arr) = bw.as_array() {
                if arr.is_empty() {
                    errors.push(format!(
                        "execution[{}]: binaryWrapper must be a non-empty array",
                        i
                    ));
                } else if !arr.iter().all(|v| v.is_string()) {
                    errors.push(format!(
                        "execution[{}]: binaryWrapper must contain only strings",
                        i
                    ));
                }
            } else {
                errors.push(format!(
                    "execution[{}]: binaryWrapper must be an array, got {}",
                    i,
                    json_value_kind(bw)
                ));
            }
        }

        // Validate condition: when present, must have a valid binGroup index.
        if let Some(cond) = ex.get("condition") {
            if let Some(cond_obj) = cond.as_object() {
                if let Some(bg) = cond_obj.get("binGroup") {
                    if let Some(idx) = bg.as_u64() {
                        // binGroup validation against the bins OR-groups is
                        // deferred to load-time (the validator doesn't have
                        // access to the SKILL.md frontmatter). Here we just
                        // check it's a non-negative integer.
                        if idx > usize::MAX as u64 {
                            errors.push(format!(
                                "execution[{}]: condition.binGroup index too large",
                                i
                            ));
                        }
                    } else {
                        errors.push(format!(
                            "execution[{}]: condition.binGroup must be an integer, got {}",
                            i,
                            json_value_kind(bg)
                        ));
                    }
                } else {
                    errors.push(format!(
                        "execution[{}]: condition missing required field \"binGroup\"",
                        i
                    ));
                }
            } else {
                errors.push(format!(
                    "execution[{}]: condition must be an object, got {}",
                    i,
                    json_value_kind(cond)
                ));
            }
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
}
