//! Delegation policy: optional allowlists of (provider, model) pairs (see `.agents/EPIC_ORCHESTRATION.md`).

use crate::config::{
    canonical_provider, resolve_effective_provider_and_model, AgentsConfig, AllowedModelEntry,
};
use crate::session::SessionStore;
use serde_json::{json, Value};

use super::workers_context::effective_worker_defaults;

fn pair_matches_catalog(
    catalog: &[AllowedModelEntry],
    provider_canonical: &str,
    model: &str,
) -> bool {
    catalog.iter().any(|e| {
        canonical_provider(e.provider.as_str()) == Some(provider_canonical)
            && e.model.trim() == model
    })
}

fn matches_effective_default(
    provider_canonical: &str,
    model: &str,
    default_provider: &str,
    default_model: &str,
) -> bool {
    provider_canonical == default_provider && model == default_model.trim()
}

/// When **`delegateAllowedModels`** is **non-empty**, the resolved `(provider, model)` for
/// **`delegate_task`** must appear in that list (worker list if set and non-empty; otherwise
/// orchestrator list). When the effective list is **omitted or empty**, only the **effective
/// default** provider/model for that scope is allowed (`effective_worker_defaults` for a worker,
/// `resolve_effective_provider_and_model` when no worker).
pub fn assert_delegation_pair_allowed(
    agents: &AgentsConfig,
    worker_id: Option<&str>,
    provider_canonical: &str,
    model: &str,
) -> Result<(), String> {
    let model = model.trim();
    if model.is_empty() {
        return Err("resolved model must not be empty".to_string());
    }

    if let Some(wid) = worker_id {
        if let Some(w) = agents
            .workers
            .as_ref()
            .and_then(|ws| ws.iter().find(|w| w.id == wid))
        {
            if let Some(ref list) = w.delegate_allowed_models {
                if !list.is_empty() {
                    if !pair_matches_catalog(list, provider_canonical, model) {
                        return Err(format!(
                            "provider/model not allowed for worker {} (delegateAllowedModels)",
                            wid
                        ));
                    }
                    return Ok(());
                }
            }
            let (def_p, def_m) = effective_worker_defaults(agents, w);
            if matches_effective_default(provider_canonical, model, &def_p, &def_m) {
                return Ok(());
            }
            return Err(format!(
                "provider/model not allowed for worker {} (delegateAllowedModels empty: only default {} / {})",
                wid, def_p, def_m
            ));
        }
    }

    if let Some(ref list) = agents.delegate_allowed_models {
        if !list.is_empty() {
            if !pair_matches_catalog(list, provider_canonical, model) {
                return Err(
                    "provider/model not allowed for delegation (agents.delegateAllowedModels)"
                        .to_string(),
                );
            }
            return Ok(());
        }
    }

    let (def_p, def_m) = resolve_effective_provider_and_model(agents);
    if matches_effective_default(provider_canonical, model, &def_p, &def_m) {
        return Ok(());
    }
    Err(format!(
        "provider/model not allowed for delegation (delegateAllowedModels empty: only default {} / {})",
        def_p, def_m
    ))
}

/// Merge **`delegationInstructionRoutes`**: first matching **`instructionPrefix`** fills missing **`workerId`** / **`provider`** / **`model`**.
pub fn apply_delegation_instruction_routes(agents: &AgentsConfig, args: &Value) -> Value {
    let Some(routes) = agents.delegation_instruction_routes.as_ref() else {
        return args.clone();
    };
    if routes.is_empty() {
        return args.clone();
    }
    let instruction = args
        .get("instruction")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let mut matched: Option<&crate::config::DelegationInstructionRoute> = None;
    for r in routes {
        let prefix = r.instruction_prefix.trim();
        if !prefix.is_empty() && instruction.starts_with(prefix) {
            matched = Some(r);
            break;
        }
    }
    let Some(route) = matched else {
        return args.clone();
    };
    let mut v = args.clone();
    let Some(obj) = v.as_object_mut() else {
        return args.clone();
    };
    if route.worker_id.is_some() && !obj.contains_key("workerId") {
        obj.insert(
            "workerId".to_string(),
            json!(route.worker_id.as_ref().unwrap()),
        );
    }
    if route.provider.is_some() && !obj.contains_key("provider") {
        obj.insert(
            "provider".to_string(),
            json!(route.provider.as_ref().unwrap()),
        );
    }
    if route.model.is_some() && !obj.contains_key("model") {
        obj.insert("model".to_string(), json!(route.model.as_ref().unwrap()));
    }
    v
}

/// Rejects delegation to providers listed in **`delegateBlockedProviders`** (canonical ids).
pub fn assert_delegate_provider_not_blocked(
    agents: &AgentsConfig,
    provider_canonical: &str,
) -> Result<(), String> {
    let Some(ref list) = agents.delegate_blocked_providers else {
        return Ok(());
    };
    if list.is_empty() {
        return Ok(());
    }
    for p in list {
        if canonical_provider(p.as_str()) == Some(provider_canonical) {
            return Err(format!(
                "delegation to provider {} is blocked (delegateBlockedProviders)",
                provider_canonical
            ));
        }
    }
    Ok(())
}

/// Enforces **`maxDelegationsPerSession`** and **`maxDelegationsPerProvider`** before a delegation runs.
pub async fn assert_session_delegation_limits(
    store: &SessionStore,
    session_id: &str,
    agents: &AgentsConfig,
    provider_canonical: &str,
) -> Result<(), String> {
    let session = store
        .get(session_id)
        .await
        .ok_or_else(|| "session not found".to_string())?;

    if let Some(max) = agents.max_delegations_per_session {
        if session.delegation_count >= max {
            return Err(format!(
                "max delegations per session reached (maxDelegationsPerSession={})",
                max
            ));
        }
    }

    if let Some(ref map) = agents.max_delegations_per_provider {
        if let Some(&limit) = map.get(provider_canonical) {
            let n = session
                .delegation_by_provider
                .get(provider_canonical)
                .copied()
                .unwrap_or(0);
            if n >= limit {
                return Err(format!(
                    "max delegations to provider {} for this session reached (configure maxDelegationsPerProvider.{})",
                    provider_canonical, provider_canonical
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DelegationInstructionRoute, WorkerConfig};
    use serde_json::json;

    fn entry(p: &str, m: &str) -> AllowedModelEntry {
        AllowedModelEntry {
            provider: p.to_string(),
            model: m.to_string(),
            local: false,
            tool_capable: None,
        }
    }

    #[test]
    fn no_lists_allows_only_orchestrator_default() {
        let agents = AgentsConfig::default();
        let (def_p, def_m) = crate::config::resolve_effective_provider_and_model(&agents);
        assert!(
            assert_delegation_pair_allowed(&agents, None, def_p.as_str(), def_m.as_str()).is_ok()
        );
        assert!(assert_delegation_pair_allowed(&agents, None, "lms", "x").is_err());
    }

    #[test]
    fn orchestrator_list_enforced_without_worker() {
        let mut agents = AgentsConfig::default();
        agents.delegate_allowed_models = Some(vec![entry("ollama", "llama3.2:latest")]);
        assert!(assert_delegation_pair_allowed(&agents, None, "ollama", "llama3.2:latest").is_ok());
        assert!(assert_delegation_pair_allowed(&agents, None, "lms", "x").is_err());
    }

    #[test]
    fn worker_list_overrides_global_when_non_empty() {
        let mut agents = AgentsConfig::default();
        agents.delegate_allowed_models = Some(vec![entry("ollama", "llama3.2:latest")]);
        agents.workers = Some(vec![WorkerConfig {
            id: "w".to_string(),
            default_provider: None,
            default_model: None,
            enabled_providers: None,
            delegate_allowed_models: Some(vec![entry("lms", "granite")]),
        }]);
        assert!(assert_delegation_pair_allowed(&agents, Some("w"), "lms", "granite").is_ok());
        assert!(assert_delegation_pair_allowed(&agents, Some("w"), "ollama", "llama3.2:latest").is_err());
    }

    #[test]
    fn worker_empty_list_uses_worker_defaults_not_orchestrator_list() {
        let mut agents = AgentsConfig::default();
        agents.delegate_allowed_models = Some(vec![entry("ollama", "llama3.2:latest")]);
        agents.workers = Some(vec![WorkerConfig {
            id: "w".to_string(),
            default_provider: Some("lms".to_string()),
            default_model: Some("granite".to_string()),
            enabled_providers: None,
            delegate_allowed_models: Some(vec![]),
        }]);
        assert!(assert_delegation_pair_allowed(&agents, Some("w"), "lms", "granite").is_ok());
        assert!(assert_delegation_pair_allowed(&agents, Some("w"), "ollama", "llama3.2:latest").is_err());
    }

    #[test]
    fn instruction_route_injects_worker_id() {
        let agents = AgentsConfig {
            delegation_instruction_routes: Some(vec![DelegationInstructionRoute {
                instruction_prefix: "[fast]".to_string(),
                worker_id: Some("w".to_string()),
                provider: None,
                model: None,
            }]),
            ..AgentsConfig::default()
        };
        let args = json!({ "instruction": "[fast] do thing" });
        let merged = apply_delegation_instruction_routes(&agents, &args);
        assert_eq!(merged["workerId"], "w");
    }

    #[test]
    fn blocked_provider_rejects() {
        let mut agents = AgentsConfig::default();
        agents.delegate_blocked_providers = Some(vec!["nim".to_string()]);
        assert!(assert_delegate_provider_not_blocked(&agents, "ollama").is_ok());
        assert!(assert_delegate_provider_not_blocked(&agents, "nim").is_err());
    }
}
