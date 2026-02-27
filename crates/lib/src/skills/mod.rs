//! Skills: load AgentSkills-compatible SKILL.md from directories.
//!
//! Skills load from the config directory's skills (~/.chai/skills) and any config.skills.extraDirs. Precedence: extra overwrites config dir by name.
//! When a skill directory contains `tools.json`, it is parsed as a tool descriptor (see descriptor module).

mod descriptor;
mod loader;

pub use descriptor::{ArgKind, ArgMapping, ExecutionSpec, ToolDescriptor};
pub use loader::{load_skills, Skill, SkillEntry, SkillSource};
