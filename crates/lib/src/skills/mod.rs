//! Skills: load AgentSkills-compatible SKILL.md from directories.
//!
//! Precedence: workspace > managed > bundled. One bundled skill (Obsidian) is included as a working example.

mod loader;

pub use loader::{load_skills, Skill, SkillEntry, SkillSource};
