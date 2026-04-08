//! Skills: load AgentSkills-compatible SKILL.md from directories.
//!
//! Skills load from the shared `~/.chai/skills` root only (one package per immediate subdirectory with `SKILL.md`).
//! When a skill directory contains `tools.json`, it is parsed as a tool descriptor (see descriptor module).

mod descriptor;
mod loader;
pub mod lockfile;
mod validation;
pub mod versioning;

pub use descriptor::{ArgKind, ArgMapping, ExecutionSpec, PostProcessSpec, ToolDescriptor};
pub use loader::{load_skills, Skill, SkillEntry};
pub use validation::validate_skill_composition;
