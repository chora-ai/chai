//! Compact worker roster for orchestrator system context (see gateway `build_system_context`).

use std::collections::HashMap;

use crate::config::{
    canonical_provider_id, worker_enabled_skills_list, AgentsConfig, ProvidersConfig, WorkerConfig,
};
use crate::skills::SkillEntry;

use super::choice::ProviderChoice;
use super::model::resolve_model;

/// Effective `(provider, model)` for this worker — resolves `defaultProvider` / `defaultModel`,
/// falling back to the orchestrator's defaults when omitted (mirrors runtime resolution in
/// `orchestration/delegate.rs`).
pub fn effective_worker_defaults(
    providers: &ProvidersConfig,
    agents: &AgentsConfig,
    w: &WorkerConfig,
) -> (String, String) {
    let global_default_provider = agents
        .default_provider
        .as_deref()
        .and_then(|s| canonical_provider_id(providers, s))
        .or_else(|| providers.entries.first().map(|p| p.id.trim().to_string()))
        .unwrap_or_else(|| "ollama".to_string());

    let provider_id = w
        .default_provider
        .as_deref()
        .and_then(|s| canonical_provider_id(providers, s))
        .unwrap_or(global_default_provider);

    let provider_choice = ProviderChoice::new(&provider_id);
    let config_model = w
        .default_model
        .as_deref()
        .or(agents.default_model.as_deref());
    let model = resolve_model(providers, config_model, None, &provider_choice);
    (provider_id, model)
}

/// Renders worker ids and enabled skills (with descriptions from `skill_catalog`)
/// for `delegate_task`. Empty when there are no workers. Provider/model pairs are
/// not included — the orchestrator cannot act on that information (there is no
/// override mechanism on `delegate_task`), and omitting it keeps worker context
/// minimal for smaller model support.
pub fn build_workers_context(
    agents: &AgentsConfig,
    skill_catalog: &[SkillEntry],
) -> String {
    let Some(workers) = agents.workers.as_ref() else {
        return String::new();
    };
    if workers.is_empty() {
        return String::new();
    }

    let skill_by_name: HashMap<&str, &SkillEntry> =
        skill_catalog.iter().map(|e| (e.name.as_str(), e)).collect();

    let mut out = String::new();
    out.push_str("## Workers\n\n");
    out.push_str("You are the orchestrator agent. You have worker agents.\n\n");
    out.push_str("You can call `delegate_task` to delegate a task to a worker agent.\n\n");
    out.push_str("The worker does not share session history — each worker turn begins with no history.\n\n");
    out.push_str("`delegate_task` calls execute sequentially — each worker turn completes before the next begins.\n\n");
    out.push_str("Only delegate a task to a worker if the worker has the relevant skills.\n\n");
    for w in workers {
        lines_for_worker(&mut out, w, &skill_by_name);
    }
    out
}

fn single_line_skill_description(description: &str) -> String {
    let s = description.trim();
    if s.is_empty() {
        return "(this skill is missing a description)".to_string();
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn lines_for_worker(
    out: &mut String,
    w: &WorkerConfig,
    skill_by_name: &HashMap<&str, &SkillEntry>,
) {
    let id = w.id.trim();
    if id.is_empty() {
        return;
    }
    out.push_str("### ");
    out.push_str(id);
    out.push_str("\n\n");

    let names = worker_enabled_skills_list(w);
    if !names.is_empty() {
        out.push_str("This worker has the following skills:\n\n");
        for name in names.iter() {
            out.push_str("- ");
            match skill_by_name.get(name.as_str()) {
                Some(entry) => {
                    out.push_str(&single_line_skill_description(&entry.description));
                }
                None => {
                    out.push_str("(this skill is missing a description)");
                }
            }
            out.push_str("\n");
        }
        out.push_str("\n");
    }

    out.push_str("Start your instruction with `[");
    out.push_str(id);
    out.push_str("]` to delegate to this worker.\n\n");
    out.push_str("Example:\n\n");
    out.push_str("{ \"instruction\": \"[");
    out.push_str(id);
    out.push_str("] Do X\" }\n\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorkerConfig;

    fn sample_agents() -> AgentsConfig {
        let mut a = AgentsConfig::default();
        a.orchestrator_id = Some("alice".to_string());
        a.workers = Some(vec![WorkerConfig {
            id: "bob".to_string(),
            default_provider: Some("ollama".to_string()),
            default_model: Some("llama3.2:3b".to_string()),
            enabled_skills: None,
            context_mode: None,
        }]);
        a
    }

    #[test]
    fn no_workers_yields_empty() {
        let a = AgentsConfig::default();
        assert!(build_workers_context(&a, &[]).is_empty());
    }

    #[test]
    fn empty_worker_list_yields_empty() {
        let mut a = AgentsConfig::default();
        a.workers = Some(vec![]);
        assert!(build_workers_context(&a, &[]).is_empty());
    }

    #[test]
    fn includes_orchestrator_and_worker() {
        let s = build_workers_context(&sample_agents(), &[]);
        assert!(s.contains("You are the orchestrator agent"));
        assert!(s.contains("sequentially"));
        assert!(s.contains("bob"));
        assert!(s.contains("### bob"));
        assert!(!s.contains("provider"));
    }

    #[test]
    fn worker_without_defaults_still_rendered() {
        let mut a = AgentsConfig::default();
        a.orchestrator_id = Some("alice".to_string());
        a.default_provider = Some("ollama".to_string());
        a.default_model = Some("llama3.2:3b".to_string());
        a.workers = Some(vec![WorkerConfig {
            id: "bob".to_string(),
            default_provider: None,
            default_model: None,
            enabled_skills: None,
            context_mode: None,
        }]);
        let s = build_workers_context(&a, &[]);
        assert!(s.contains("bob"));
        assert!(s.contains("### bob"));
        assert!(s.contains("Start your instruction with `[bob]`"));
    }

    #[test]
    fn worker_skill_lists_description_from_catalog() {
        use std::path::PathBuf;

        let mut a = AgentsConfig::default();
        a.orchestrator_id = Some("orch".to_string());
        a.default_provider = Some("ollama".to_string());
        a.default_model = Some("llama3.2:3b".to_string());
        a.workers = Some(vec![WorkerConfig {
            id: "w1".to_string(),
            default_provider: None,
            default_model: None,
            enabled_skills: Some(vec!["my-skill".to_string()]),
            context_mode: None,
        }]);
        a.enabled_providers = Some(vec!["ollama".to_string()]);

        let catalog = vec![SkillEntry {
            name: "my-skill".to_string(),
            description: "does a thing".to_string(),
            path: PathBuf::from("/tmp/x"),
            content: String::new(),
            tool_descriptor: None,
            capability_tier: None,
            variant_of: None,
        }];
        let s = build_workers_context(&a, &catalog);
        assert!(!s.contains("provider"));
        assert!(s.contains("- does a thing"));
    }

    #[test]
    fn bracket_prefix_rendered_per_worker() {
        let s = build_workers_context(&sample_agents(), &[]);
        assert!(s.contains("Start your instruction with `[bob]`"));
    }

    #[test]
    fn no_provider_model_in_context() {
        let s = build_workers_context(&sample_agents(), &[]);
        assert!(!s.contains("provider"));
        assert!(!s.contains("model"));
    }
}
