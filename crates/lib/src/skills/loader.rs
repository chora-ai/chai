//! Load skills from dirs: each skill is a directory with SKILL.md (YAML frontmatter + markdown).
//! Skills with `metadata.requires.bins` are only loaded when all listed binaries are on PATH.
//! When present, `tools.json` in the skill directory is parsed and attached as the tool descriptor.

use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use super::descriptor::ToolDescriptor;

/// A loaded skill (name, description, path, optional tool descriptor).
#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    /// Raw SKILL.md content (for agent context).
    pub content: String,
    /// When the skill dir contains tools.json, parsed descriptor (tools, allowlist, execution mapping).
    pub tool_descriptor: Option<ToolDescriptor>,
    /// Capability tier from SKILL.md frontmatter (`minimal`, `moderate`, `full`). Now parsed from top-level field (was previously nested inside `generated_from`).
    pub capability_tier: Option<String>,
    /// Parent skill this is a variant of (e.g. `git-read` is a variant of `git`).
    pub variant_of: Option<String>,
}

/// Flattened skill for agent use (name + description + content).
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
}

/// Frontmatter parsed from SKILL.md — only runtime-consumed fields.
/// Directory name is the authoritative skill name; `name` is not parsed from frontmatter.
/// Derivation metadata (`generated_from`) has been removed; `capability_tier` is now top-level.
#[derive(Debug, Default, Deserialize)]
struct SkillFrontmatter {
    description: Option<String>,
    /// Minimum model capability: `minimal`, `moderate`, or `full`.
    #[serde(default)]
    capability_tier: Option<String>,
    /// Parent skill this is a variant of (e.g. `git-read` → `git`).
    #[serde(default, rename = "variant_of")]
    variant_of: Option<String>,
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

/// Load all skill packages under `skills_root` (e.g. `~/.chai/skills`): each immediate subdirectory
/// that uses the versioned layout (`active` symlink → `versions/<hash>/` with `SKILL.md`).
pub fn load_skills(skills_root: &Path) -> Result<Vec<SkillEntry>> {
    load_skills_from_root(skills_root)
}

fn load_skills_from_root(dir: &Path) -> Result<Vec<SkillEntry>> {
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
        // Require versioned layout: `active` -> `versions/<hash>/` with SKILL.md
        let Some(content_dir) = super::versioning::resolve_active_dir(&path) else {
            continue;
        };
        let skill_md = content_dir.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }
        let content = match std::fs::read_to_string(&skill_md) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let parsed = parse_skill_frontmatter(&content, &path);
        if let Some(bins) = &parsed.required_bins {
            if !bins.is_empty() && !bins.iter().all(|b| bin_on_path(b)) {
                log::debug!(
                    "skipping skill {}: required bins {:?} not all on PATH",
                    parsed.name,
                    bins
                );
                continue;
            }
        }
        let tool_descriptor = load_tool_descriptor(&content_dir);
        out.push(SkillEntry {
            name: parsed.name,
            description: parsed.description,
            path: content_dir.clone(),
            content,
            tool_descriptor,
            capability_tier: parsed.capability_tier,
            variant_of: parsed.variant_of,
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
            log::warn!("failed to parse {}: {}", path.display(), e);
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

/// Parsed skill frontmatter fields.
struct ParsedFrontmatter {
    name: String,
    description: String,
    required_bins: Option<Vec<String>>,
    capability_tier: Option<String>,
    variant_of: Option<String>,
}

fn parse_skill_frontmatter(content: &str, fallback_path: &Path) -> ParsedFrontmatter {
    // Directory name is the authoritative skill name; frontmatter `name` is not parsed.
    let name_from_path = fallback_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let mut parsed = ParsedFrontmatter {
        name: name_from_path,
        description: String::new(),
        required_bins: None,
        capability_tier: None,
        variant_of: None,
    };

    if content.starts_with("---") {
        if let Some(end) = content[3..].find("---") {
            let yaml = content[3..3 + end].trim();
            if let Ok(fm) = serde_yaml::from_str::<SkillFrontmatter>(yaml) {
                if let Some(d) = fm.description {
                    parsed.description = d;
                }
                parsed.capability_tier = fm.capability_tier;
                parsed.variant_of = fm.variant_of;
                if let Some(ref meta) = fm.metadata {
                    if let Some(ref req) = meta.requires {
                        parsed.required_bins = req.bins.clone();
                    }
                }
            }
        }
    }

    parsed
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
        let skills_dir: PathBuf = [&manifest_dir, "tests", "fixtures", "loader_tool_test"]
            .iter()
            .collect();
        if !skills_dir.join("active").exists() {
            panic!(
                "missing test fixture (versioned layout) at {}",
                skills_dir.display()
            );
        }
        let skills = load_skills(skills_dir.parent().unwrap()).unwrap();
        let entry = skills
            .iter()
            .find(|s| s.name == "loader_tool_test")
            .expect("fixture skill loader_tool_test");
        let Some(desc) = &entry.tool_descriptor else {
            panic!("fixture has tools.json but tool_descriptor is None");
        };
        assert!(desc.tools.len() >= 1);
        assert_eq!(desc.tools[0].name, "notesmd_search");
        assert!(desc.allowlist.contains_key("notesmd"));
        assert!(desc.execution.len() >= 1);
        assert_eq!(desc.execution[0].tool, "notesmd_search");
        assert_eq!(desc.execution[0].binary, "notesmd-cli");
        assert_eq!(desc.execution[0].subcommand, "search");
    }
}
