//! Compact worker roster for orchestrator system context (see gateway `build_system_context_static`).

use std::collections::HashMap;

use crate::config::{
    canonical_provider, provider_discovery_enabled, worker_skills_enabled_list, AgentsConfig,
    WorkerConfig,
};
use crate::skills::SkillEntry;

use super::choice::provider_choice_from_canonical;
use super::model::resolve_model;

/// Effective `(provider, model)` when `delegate_task` omits `provider` and `model` for this worker
/// (mirrors runtime resolution in `orchestration/delegate.rs`).
pub fn effective_worker_defaults(agents: &AgentsConfig, w: &WorkerConfig) -> (String, String) {
    let global_default_provider = agents
        .default_provider
        .as_deref()
        .and_then(canonical_provider)
        .unwrap_or("ollama");

    let provider_canonical = w
        .default_provider
        .as_deref()
        .and_then(canonical_provider)
        .unwrap_or(global_default_provider);

    let provider_choice = provider_choice_from_canonical(provider_canonical);
    let config_model = w
        .default_model
        .as_deref()
        .or(agents.default_model.as_deref());
    let model = resolve_model(config_model, None, provider_choice);
    (provider_canonical.to_string(), model)
}

/// Renders worker ids, default provider/model, enabled skills (with descriptions from `skill_catalog`), and
/// allowed `(provider, model)` pairs for `delegate_task` (same rules as runtime delegation). Empty when there are no workers.
pub fn build_workers_context(agents: &AgentsConfig, skill_catalog: &[SkillEntry]) -> String {
    let Some(workers) = agents.workers.as_ref() else {
        return String::new();
    };
    if workers.is_empty() {
        return String::new();
    }

    let skill_by_name: HashMap<&str, &SkillEntry> =
        skill_catalog.iter().map(|e| (e.name.as_str(), e)).collect();

    let mut out = String::new();
    out.push_str("## Agents\n\n");
    out.push_str("You are the orchestrator agent.\n");
    out.push_str("You have one or more worker agents.\n\n");
    out.push_str("### Orchestration\n\n");
    out.push_str("You have a unique skill: orchestration.\n");
    out.push_str("This skill has one tool: `delegate_task`.\n");
    out.push_str("You can use this tool to delegate a task to a worker.\n");
    out.push_str(
        "When you delegate a task to a worker, the worker attempts to complete the task.\n",
    );
    out.push_str(
        "When a worker completes a task, the worker responds to you and you respond to the user.\n",
    );
    out.push_str(
        "If a worker fails to complete a task, you can try again before responding to the user.\n",
    );
    out.push_str("Example tool call: {\"\":\"\"}.\n\n");
    out.push_str("### Workers\n\n");
    out.push_str("Each worker may have one or more skills.\n");
    out.push_str("Each worker may have one or more provider/model pairs.\n");
    out.push_str(
        "Available workers, their skills, and their provider/model pairs are listed below.\n\n",
    );
    for w in workers {
        line_for_worker(&mut out, agents, w, &skill_by_name);
    }
    out
}

fn provider_delegation_blocked(agents: &AgentsConfig, provider_canonical: &str) -> bool {
    let Some(ref list) = agents.delegate_blocked_providers else {
        return false;
    };
    list.iter()
        .filter_map(|p| canonical_provider(p.as_str()))
        .any(|p| p == provider_canonical)
}

/// Worker may target this provider in `delegate_task` (mirrors `resolve_delegate_target` gates).
fn worker_may_use_provider(
    agents: &AgentsConfig,
    w: &WorkerConfig,
    provider_canonical: &str,
) -> bool {
    if !provider_discovery_enabled(agents, provider_canonical) {
        return false;
    }
    if provider_delegation_blocked(agents, provider_canonical) {
        return false;
    }
    let global_default_provider = agents
        .default_provider
        .as_deref()
        .and_then(canonical_provider)
        .unwrap_or("ollama");
    let default_provider = w
        .default_provider
        .as_deref()
        .and_then(canonical_provider)
        .unwrap_or(global_default_provider);
    match &w.enabled_providers {
        None => true,
        Some(list) if list.is_empty() => provider_canonical == default_provider,
        Some(list) => list
            .iter()
            .filter_map(|p| canonical_provider(p))
            .any(|p| p == provider_canonical),
    }
}

/// `(provider, model)` pairs the orchestrator may use for this worker via `delegate_task` (mirrors `assert_delegation_pair_allowed` for a worker id).
fn usable_delegate_pairs_for_worker(
    agents: &AgentsConfig,
    w: &WorkerConfig,
) -> Vec<(String, String)> {
    if let Some(ref list) = w.delegate_allowed_models {
        if !list.is_empty() {
            let mut pairs: Vec<(String, String)> = list
                .iter()
                .filter_map(|e| {
                    let p = canonical_provider(e.provider.as_str())?;
                    let m = e.model.trim();
                    if m.is_empty() || !worker_may_use_provider(agents, w, p) {
                        return None;
                    }
                    Some((p.to_string(), m.to_string()))
                })
                .collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            pairs.dedup();
            return pairs;
        }
    }
    let (def_p, def_m) = effective_worker_defaults(agents, w);
    if worker_may_use_provider(agents, w, def_p.as_str()) {
        vec![(def_p, def_m)]
    } else {
        Vec::new()
    }
}

fn single_line_skill_description(description: &str) -> String {
    let s = description.trim();
    if s.is_empty() {
        return "(this skill is missing a description)".to_string();
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn line_for_worker(
    out: &mut String,
    agents: &AgentsConfig,
    w: &WorkerConfig,
    skill_by_name: &HashMap<&str, &SkillEntry>,
) {
    let id = w.id.trim();
    if id.is_empty() {
        return;
    }
    out.push_str("#### ");
    out.push_str(id);
    out.push_str("\n\n");
    out.push_str("id: `");
    out.push_str(id);
    out.push_str("`\n");

    let pairs = usable_delegate_pairs_for_worker(agents, w);
    for (i, (p, m)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push_str("\n");
        }
        out.push_str("provider/model: `");
        out.push_str(p);
        out.push_str("`/`");
        out.push_str(m);
        out.push_str("`");
    }
    out.push_str("\n");

    let names = worker_skills_enabled_list(w);
    if !names.is_empty() {
        for (i, name) in names.iter().enumerate() {
            if i > 0 {
                out.push_str("; ");
            }
            out.push_str("skill: `");
            out.push_str(name.trim());
            out.push_str("` — ");
            match skill_by_name.get(name.as_str()) {
                Some(entry) => {
                    out.push_str(&single_line_skill_description(&entry.description));
                }
                None => {
                    out.push_str("(this skill is missing a description)");
                }
            }
        }
        out.push_str("\n");
    }
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
            default_model: Some("llama3.2:latest".to_string()),
            enabled_providers: None,
            skills_enabled: None,
            context_mode: None,
            delegate_allowed_models: None,
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
        assert!(s.contains("bob"));
        assert!(s.contains("ollama"));
        assert!(s.contains("provider/model:"));
        assert!(s.contains("`ollama`/`llama3.2:latest`"));
    }

    #[test]
    fn worker_without_defaults_inherits_orchestrator() {
        let mut a = AgentsConfig::default();
        a.orchestrator_id = Some("alice".to_string());
        a.default_provider = Some("ollama".to_string());
        a.default_model = Some("llama3.2:latest".to_string());
        a.workers = Some(vec![WorkerConfig {
            id: "bob".to_string(),
            default_provider: None,
            default_model: None,
            enabled_providers: None,
            skills_enabled: None,
            context_mode: None,
            delegate_allowed_models: None,
        }]);
        let s = build_workers_context(&a, &[]);
        assert!(s.contains("bob"));
        assert!(s.contains("ollama"));
        assert!(s.contains("llama3.2:latest"));
    }

    #[test]
    fn worker_skill_lists_description_from_catalog() {
        use crate::config::AllowedModelEntry;
        use std::path::PathBuf;

        let mut a = AgentsConfig::default();
        a.orchestrator_id = Some("orch".to_string());
        a.default_provider = Some("ollama".to_string());
        a.default_model = Some("llama3.2:latest".to_string());
        a.workers = Some(vec![WorkerConfig {
            id: "w1".to_string(),
            default_provider: None,
            default_model: None,
            enabled_providers: None,
            skills_enabled: Some(vec!["my-skill".to_string()]),
            context_mode: None,
            delegate_allowed_models: Some(vec![
                AllowedModelEntry {
                    provider: "ollama".to_string(),
                    model: "m-a".to_string(),
                    local: false,
                    tool_capable: None,
                },
                AllowedModelEntry {
                    provider: "lms".to_string(),
                    model: "m-b".to_string(),
                    local: false,
                    tool_capable: None,
                },
            ]),
        }]);
        a.enabled_providers = Some(vec!["ollama".to_string(), "lms".to_string()]);

        let catalog = vec![SkillEntry {
            name: "my-skill".to_string(),
            description: "does a thing".to_string(),
            path: PathBuf::from("/tmp/x"),
            content: String::new(),
            tool_descriptor: None,
            capability_tier: None,
            model_variant_of: None,
        }];
        let s = build_workers_context(&a, &catalog);
        assert!(s.contains("`my-skill` — does a thing"));
        assert!(s.contains("provider/model: `ollama`/`m-a`"));
        assert!(s.contains("provider/model: `lms`/`m-b`"));
    }
}
