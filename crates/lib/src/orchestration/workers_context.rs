//! Compact worker roster for orchestrator system context (see gateway `build_system_context_static`).

use crate::config::{canonical_provider, AgentsConfig, WorkerConfig};

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
    let config_model = w.default_model.as_deref().or(agents.default_model.as_deref());
    let model = resolve_model(config_model, None, provider_choice);
    (provider_canonical.to_string(), model)
}

/// Renders worker ids and **effective** provider/model for `delegate_task`. Empty when there are no workers.
pub fn build_workers_context(agents: &AgentsConfig) -> String {
    let Some(workers) = agents.workers.as_ref() else {
        return String::new();
    };
    if workers.is_empty() {
        return String::new();
    }

    let oid = agents
        .orchestrator_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("orchestrator");

    let mut out = String::new();
    out.push_str("## Workers\n\n");
    out.push_str("You are `");
    out.push_str(oid);
    out.push_str("` — the orchestrator agent. You can:\n\n");
    out.push_str("- delegate a task to a worker agent (`delegate_task`)\n");
    out.push_str("- complete a task without a worker agent (use your skills)\n");
    out.push_str("- share available worker agents and ask the user to choose\n\n");
    out.push_str("Available worker agents (id — providers — models):\n\n");
    for w in workers {
        line_for_worker(&mut out, agents, w);
    }
    out
}

fn line_for_worker(out: &mut String, agents: &AgentsConfig, w: &WorkerConfig) {
    let id = w.id.trim();
    if id.is_empty() {
        return;
    }
    let (prov, model) = effective_worker_defaults(agents, w);
    out.push_str("- `");
    out.push_str(id);
    out.push_str("` — `");
    out.push_str(&prov);
    out.push_str("` — `");
    out.push_str(&model);
    out.push_str("`\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorkerConfig;

    fn sample_agents() -> AgentsConfig {
        let mut a = AgentsConfig::default();
        a.orchestrator_id = Some("hermes".to_string());
        a.workers = Some(vec![WorkerConfig {
            id: "apollo".to_string(),
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
        assert!(build_workers_context(&a).is_empty());
    }

    #[test]
    fn empty_worker_list_yields_empty() {
        let mut a = AgentsConfig::default();
        a.workers = Some(vec![]);
        assert!(build_workers_context(&a).is_empty());
    }

    #[test]
    fn includes_orchestrator_and_worker() {
        let s = build_workers_context(&sample_agents());
        assert!(s.contains("hermes"));
        assert!(s.contains("apollo"));
        assert!(s.contains("ollama"));
    }

    #[test]
    fn worker_without_defaults_inherits_orchestrator() {
        let mut a = AgentsConfig::default();
        a.orchestrator_id = Some("hermes".to_string());
        a.default_provider = Some("ollama".to_string());
        a.default_model = Some("llama3.2:latest".to_string());
        a.workers = Some(vec![WorkerConfig {
            id: "apollo".to_string(),
            default_provider: None,
            default_model: None,
            enabled_providers: None,
            skills_enabled: None,
            context_mode: None,
            delegate_allowed_models: None,
        }]);
        let s = build_workers_context(&a);
        assert!(s.contains("apollo"));
        assert!(s.contains("ollama"));
        assert!(s.contains("llama3.2:latest"));
    }
}
