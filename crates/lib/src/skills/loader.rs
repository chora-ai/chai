//! Load skills from dirs: each skill is a directory with SKILL.md (YAML frontmatter + markdown).
//! Skills with `metadata.requires.bins` are only loaded when the required binaries are available.
//! The `bins` field supports two forms:
//! - Flat list `["git", "curl"]` — all binaries must be on PATH (AND semantics, backward compatible).
//! - OR-groups `[["cargo"], ["nix"]]` — any group must have all its binaries on PATH (OR of ANDs).
//!
//! When OR-groups are present, the loader records which group matched so that
//! `condition.binGroup` in execution specs can select the appropriate spec.
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
    /// Execution specs with unsatisfied `condition` fields are filtered out during loading.
    pub tool_descriptor: Option<ToolDescriptor>,
    /// Capability tier from SKILL.md frontmatter (`minimal`, `moderate`, `full`). Now parsed from top-level field (was previously nested inside `generated_from`).
    pub capability_tier: Option<String>,
    /// Parent skill this is a variant of (e.g. `git-read` is a variant of `git`).
    pub variant_of: Option<String>,
    /// When `bins` uses OR-groups, the index of the group that matched during loading.
    /// `None` when bins is absent, empty, or uses the flat (All) form.
    pub matched_bin_group: Option<usize>,
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
    bins: Option<BinsRequirement>,
}

/// Binary requirement specification: either all binaries must be present (AND),
/// or any one group must have all its binaries present (OR of ANDs).
///
/// Backward-compatible: a flat list `["git", "curl"]` deserializes as `All`,
/// while a list of lists `[["cargo"], ["nix"]]` deserializes as `AnyOf`.
#[derive(Debug, Clone)]
enum BinsRequirement {
    /// All binaries must be on PATH (flat list: `["git", "curl"]`).
    All(Vec<String>),
    /// Any group must have all its binaries on PATH (OR-groups: `[["cargo"], ["nix"]]`).
    AnyOf(Vec<Vec<String>>),
}

impl BinsRequirement {
    /// Check whether the requirement is satisfied and return the matched group
    /// index (for `AnyOf`) or `None` (for `All`).
    ///
    /// Returns `Some(matched_index)` when satisfied, `None` when not.
    fn check(&self) -> Option<Option<usize>> {
        match self {
            BinsRequirement::All(bins) => {
                if bins.is_empty() {
                    return Some(None);
                }
                if bins.iter().all(|b| bin_on_path(b)) {
                    Some(None)
                } else {
                    None
                }
            }
            BinsRequirement::AnyOf(groups) => {
                if groups.is_empty() {
                    return Some(None);
                }
                // Find the first group where all binaries are on PATH.
                for (i, group) in groups.iter().enumerate() {
                    if group.iter().all(|b| bin_on_path(b)) {
                        return Some(Some(i));
                    }
                }
                None
            }
        }
    }
}

impl<'de> Deserialize<'de> for BinsRequirement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, SeqAccess, Visitor};

        struct BinsRequirementVisitor;

        impl<'de> Visitor<'de> for BinsRequirementVisitor {
            type Value = BinsRequirement;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "a list of strings (e.g. [\"git\", \"curl\"]) or a list of lists (e.g. [[\"cargo\"], [\"nix\"]])",
                )
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                // Peek at the first element to determine the form.
                // We collect everything into a serde_json::Value array first,
                // then discriminate based on the first element's type.
                let mut elements: Vec<serde_json::Value> = Vec::new();
                while let Some(elem) = seq.next_element::<serde_json::Value>()? {
                    elements.push(elem);
                }

                if elements.is_empty() {
                    return Ok(BinsRequirement::All(Vec::new()));
                }

                // Check if the first element is a string or an array.
                match &elements[0] {
                    serde_json::Value::String(_) => {
                        // Flat list of strings → All
                        let bins: Vec<String> = elements
                            .into_iter()
                            .map(|v| {
                                v.as_str()
                                    .map(String::from)
                                    .ok_or_else(|| de::Error::custom("expected string in bins list"))
                            })
                            .collect::<Result<_, _>>()?;
                        Ok(BinsRequirement::All(bins))
                    }
                    serde_json::Value::Array(_) => {
                        // List of lists → AnyOf
                        let groups: Vec<Vec<String>> = elements
                            .into_iter()
                            .map(|v| {
                                v.as_array()
                                    .ok_or_else(|| de::Error::custom("expected array in bins OR-group"))
                                    .and_then(|arr| {
                                        arr.iter()
                                            .map(|item| {
                                                item.as_str()
                                                    .map(String::from)
                                                    .ok_or_else(|| {
                                                        de::Error::custom(
                                                            "expected string in bins group",
                                                        )
                                                    })
                                            })
                                            .collect::<Result<_, _>>()
                                    })
                            })
                            .collect::<Result<_, _>>()?;
                        Ok(BinsRequirement::AnyOf(groups))
                    }
                    other => Err(de::Error::custom(format!(
                        "expected string or array in bins list, got {}",
                        json_value_kind(other)
                    ))),
                }
            }
        }

        deserializer.deserialize_seq(BinsRequirementVisitor)
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
        let matched_bin_group = match &parsed.required_bins {
            None => None,
            Some(bins) => match bins.check() {
                Some(group) => group,
                None => {
                    log::debug!(
                        "skipping skill {}: required bins not satisfied",
                        parsed.name,
                    );
                    continue;
                }
            },
        };
        let mut tool_descriptor = load_tool_descriptor(&content_dir);
        // Filter execution specs based on condition.binGroup when OR-groups are present.
        if let Some(ref desc) = tool_descriptor {
            if matched_bin_group.is_some() {
                let group_idx = matched_bin_group.unwrap();
                let filtered: Vec<_> = desc
                    .execution
                    .iter()
                    .filter(|spec| match spec.condition {
                        Some(ref cond) => cond.bin_group == group_idx,
                        None => true,
                    })
                    .cloned()
                    .collect();
                // Only rebuild if some specs were filtered out.
                if filtered.len() != desc.execution.len() {
                    let mut new_desc = desc.clone();
                    new_desc.execution = filtered;
                    tool_descriptor = Some(new_desc);
                }
            }
        }
        out.push(SkillEntry {
            name: parsed.name,
            description: parsed.description,
            path: content_dir.clone(),
            content,
            tool_descriptor,
            capability_tier: parsed.capability_tier,
            variant_of: parsed.variant_of,
            matched_bin_group,
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
pub(crate) fn bin_on_path(bin: &str) -> bool {
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
    required_bins: Option<BinsRequirement>,
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
        assert_eq!(desc.tools[0].name, "git_status");
        assert!(desc.allowlist.contains_key("git"));
        assert!(desc.execution.len() >= 1);
        assert_eq!(desc.execution[0].tool, "git_status");
        assert_eq!(desc.execution[0].binary, "git");
        assert_eq!(desc.execution[0].subcommand, "status");
    }

    #[test]
    fn bins_requirement_all_satisfied() {
        let req = BinsRequirement::All(vec!["git".to_string()]);
        // git is almost certainly on PATH in any dev environment.
        assert!(req.check().is_some());
        assert_eq!(req.check().unwrap(), None); // All form returns None for group index
    }

    #[test]
    fn bins_requirement_all_unsatisfied() {
        let req = BinsRequirement::All(vec!["no_such_binary_xyz_12345".to_string()]);
        assert!(req.check().is_none());
    }

    #[test]
    fn bins_requirement_all_empty() {
        let req = BinsRequirement::All(Vec::new());
        assert!(req.check().is_some());
    }

    #[test]
    fn bins_requirement_anyof_first_group_matches() {
        let req = BinsRequirement::AnyOf(vec![
            vec!["git".to_string()],
            vec!["no_such_binary_xyz_12345".to_string()],
        ]);
        let result = req.check();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Some(0)); // first group matched
    }

    #[test]
    fn bins_requirement_anyof_second_group_matches() {
        let req = BinsRequirement::AnyOf(vec![
            vec!["no_such_binary_xyz_12345".to_string()],
            vec!["git".to_string()],
        ]);
        let result = req.check();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Some(1)); // second group matched
    }

    #[test]
    fn bins_requirement_anyof_no_group_matches() {
        let req = BinsRequirement::AnyOf(vec![
            vec!["no_such_binary_xyz_12345".to_string()],
            vec!["also_no_such_binary_abc_67890".to_string()],
        ]);
        assert!(req.check().is_none());
    }

    #[test]
    fn bins_requirement_anyof_empty() {
        let req = BinsRequirement::AnyOf(Vec::new());
        assert!(req.check().is_some());
    }

    #[test]
    fn bins_requirement_deserialize_flat_strings() {
        let yaml = r#"["git", "curl"]"#;
        let req: BinsRequirement = serde_json::from_str(yaml).unwrap();
        match req {
            BinsRequirement::All(bins) => {
                assert_eq!(bins, vec!["git", "curl"]);
            }
            BinsRequirement::AnyOf(_) => panic!("expected All, got AnyOf"),
        }
    }

    #[test]
    fn bins_requirement_deserialize_nested_lists() {
        let yaml = r#"[["cargo"], ["nix"]]"#;
        let req: BinsRequirement = serde_json::from_str(yaml).unwrap();
        match req {
            BinsRequirement::AnyOf(groups) => {
                assert_eq!(groups, vec![vec!["cargo"], vec!["nix"]]);
            }
            BinsRequirement::All(_) => panic!("expected AnyOf, got All"),
        }
    }

    #[test]
    fn bins_requirement_deserialize_empty_list() {
        let yaml = r#"[]"#;
        let req: BinsRequirement = serde_json::from_str(yaml).unwrap();
        match req {
            BinsRequirement::All(bins) => {
                assert!(bins.is_empty());
            }
            BinsRequirement::AnyOf(_) => panic!("expected All, got AnyOf"),
        }
    }

    #[test]
    fn bins_requirement_deserialize_from_yaml_frontmatter() {
        let yaml = r#"
metadata:
  requires:
    bins: [["cargo"], ["nix"]]
"#;
        let fm: SkillFrontmatter = serde_yaml::from_str(yaml).unwrap();
        let req = fm.metadata.unwrap().requires.unwrap().bins.unwrap();
        match req {
            BinsRequirement::AnyOf(groups) => {
                assert_eq!(groups, vec![vec!["cargo"], vec!["nix"]]);
            }
            BinsRequirement::All(_) => panic!("expected AnyOf, got All"),
        }
    }

    #[test]
    fn bins_requirement_deserialize_flat_from_yaml_frontmatter() {
        let yaml = r#"
metadata:
  requires:
    bins: ["git", "curl"]
"#;
        let fm: SkillFrontmatter = serde_yaml::from_str(yaml).unwrap();
        let req = fm.metadata.unwrap().requires.unwrap().bins.unwrap();
        match req {
            BinsRequirement::All(bins) => {
                assert_eq!(bins, vec!["git", "curl"]);
            }
            BinsRequirement::AnyOf(_) => panic!("expected All, got AnyOf"),
        }
    }
}
