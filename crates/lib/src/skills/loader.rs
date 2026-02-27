//! Load skills from dirs: each skill is a directory with SKILL.md (YAML frontmatter + markdown).
//! Skills with `metadata.requires.bins` are only loaded when all listed binaries are on PATH.
//! When present, `tools.json` in the skill directory is parsed and attached as the tool descriptor.

use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use super::descriptor::ToolDescriptor;

/// A loaded skill (name, description, source, path, optional tool descriptor).
#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub source: SkillSource,
    pub path: PathBuf,
    /// Raw SKILL.md content (for agent context).
    pub content: String,
    /// When the skill dir contains tools.json, parsed descriptor (tools, allowlist, execution mapping).
    pub tool_descriptor: Option<ToolDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSource {
    /// From the config directory's skills subdirectory (e.g. ~/.chai/skills).
    Skills,
    /// From config.skills.extraDirs.
    Extra,
}

/// Flattened skill for agent use (name + description + content).
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
}

/// Frontmatter parsed from SKILL.md (minimal).
#[derive(Debug, Default, Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    metadata: Option<SkillMetadata>,
}

#[derive(Debug, Default, Deserialize)]
struct SkillMetadata {
    #[serde(default)]
    requires: Option<Requires>,
}

#[derive(Debug, Default, Deserialize)]
struct Requires {
    #[serde(default)]
    bins: Option<Vec<String>>,
}

/// Load all skills from the config directory's skills and any extra dirs from config.
/// Each dir should contain subdirs, each with a SKILL.md file.
/// Precedence: config dir first, then extra (later overwrites earlier by name).
pub fn load_skills(skills_dir: Option<&Path>, extra_dirs: &[PathBuf]) -> Result<Vec<SkillEntry>> {
    let mut merged: std::collections::HashMap<String, SkillEntry> = std::collections::HashMap::new();

    if let Some(d) = skills_dir {
        for e in load_skills_from_dir(d, SkillSource::Skills)? {
            merged.insert(e.name.clone(), e);
        }
    }
    for dir in extra_dirs {
        for e in load_skills_from_dir(dir, SkillSource::Extra)? {
            merged.insert(e.name.clone(), e);
        }
    }

    Ok(merged.into_values().collect())
}

fn load_skills_from_dir(dir: &Path, source: SkillSource) -> Result<Vec<SkillEntry>> {
    let mut out = Vec::new();
    let read_dir = match std::fs::read_dir(dir) {
        Ok(d) => d,
        Err(_) => return Ok(out),
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }
        let content = match std::fs::read_to_string(&skill_md) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let (name, description, required_bins) = parse_skill_frontmatter(&content, &path);
        if let Some(bins) = &required_bins {
            if !bins.is_empty() && !bins.iter().all(|b| bin_on_path(b)) {
                log::debug!(
                    "skipping skill {}: required bins {:?} not all on PATH",
                    name,
                    bins
                );
                continue;
            }
        }
        let tool_descriptor = load_tool_descriptor(&path);
        out.push(SkillEntry {
            name,
            description,
            source,
            path: path.to_path_buf(),
            content,
            tool_descriptor,
        });
    }
    Ok(out)
}

/// If the skill directory contains tools.json, parse and return it. Otherwise None.
fn load_tool_descriptor(skill_dir: &Path) -> Option<ToolDescriptor> {
    let path = skill_dir.join("tools.json");
    let content = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<ToolDescriptor>(&content) {
        Ok(d) => Some(d),
        Err(e) => {
            log::warn!(
                "failed to parse {}: {}",
                path.display(),
                e
            );
            None
        }
    }
}

/// Returns true if the given binary name is found on PATH (or has path separators and exists).
fn bin_on_path(bin: &str) -> bool {
    if bin.contains(std::path::MAIN_SEPARATOR) {
        return Path::new(bin).is_file();
    }
    let path_var = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };
    let path_var = path_var.to_string_lossy();
    let separator = if cfg!(windows) { ';' } else { ':' };
    for dir in path_var.split(separator) {
        let dir = dir.trim();
        if dir.is_empty() {
            continue;
        }
        let candidate = Path::new(dir).join(bin);
        if candidate.is_file() {
            return true;
        }
        #[cfg(windows)]
        {
            let with_ext = Path::new(dir).join(format!("{}.exe", bin));
            if with_ext.is_file() {
                return true;
            }
        }
    }
    false
}

fn parse_skill_frontmatter(
    content: &str,
    fallback_path: &Path,
) -> (String, String, Option<Vec<String>>) {
    let name_from_path = fallback_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let mut name = name_from_path.clone();
    let mut description = String::new();
    let mut required_bins: Option<Vec<String>> = None;

    if content.starts_with("---") {
        if let Some(end) = content[3..].find("---") {
            let yaml = content[3..3 + end].trim();
            if let Ok(fm) = serde_yaml::from_str::<SkillFrontmatter>(yaml) {
                if let Some(n) = fm.name {
                    name = n;
                }
                if let Some(d) = fm.description {
                    description = d;
                }
                if let Some(ref meta) = fm.metadata {
                    if let Some(ref req) = meta.requires {
                        required_bins = req.bins.clone();
                    }
                }
            }
        }
    }

    (name, description, required_bins)
}

impl From<SkillEntry> for Skill {
    fn from(e: SkillEntry) -> Self {
        Skill {
            name: e.name,
            description: e.description,
            content: e.content,
        }
    }
}

impl From<&SkillEntry> for Skill {
    fn from(e: &SkillEntry) -> Self {
        Skill {
            name: e.name.clone(),
            description: e.description.clone(),
            content: e.content.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn load_skills_parses_tools_json_when_present() {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let skills_dir: PathBuf = [&manifest_dir, "config", "skills"].iter().collect();
        if !skills_dir.join("notesmd-cli").join("SKILL.md").exists() {
            return;
        }
        let skills = load_skills(Some(skills_dir.as_path()), &[]).unwrap();
        let notesmd = skills.iter().find(|s| s.name == "notesmd-cli");
        let Some(entry) = notesmd else {
            return;
        };
        let Some(desc) = &entry.tool_descriptor else {
            panic!("notesmd-cli skill dir has tools.json but tool_descriptor is None");
        };
        assert!(desc.tools.len() >= 1);
        assert_eq!(desc.tools[0].name, "notesmd_cli_search");
        assert!(desc.allowlist.contains_key("notesmd-cli"));
        assert!(desc.execution.len() >= 1);
        assert_eq!(desc.execution[0].tool, "notesmd_cli_search");
        assert_eq!(desc.execution[0].binary, "notesmd-cli");
        assert_eq!(desc.execution[0].subcommand, "search");
    }
}
