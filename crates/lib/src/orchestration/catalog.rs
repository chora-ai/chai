//! Merged `(provider, model)` catalog for discovery + delegation allowlists (see `base/epic/ORCHESTRATION.md`).

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::config::{canonical_provider_id, AgentsConfig, ProvidersConfig, AllowedModelEntry};

/// One row in the merged orchestration catalog (WebSocket **`status`** payload **`agents.orchestrationCatalog`**).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationCatalogEntry {
    pub provider: String,
    pub model: String,
    /// Present in provider discovery (or static NIM list) at last refresh.
    pub discovered: bool,
    /// From an allowlist entry when this `(provider, model)` matches; omitted when unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_capable: Option<bool>,
}

type Hint = (Option<bool>, Option<bool>);

fn insert_allowlist(map: &mut HashMap<(String, String), Hint>, providers: &ProvidersConfig, e: &AllowedModelEntry) {
    let Some(p) = canonical_provider_id(providers, &e.provider) else {
        return;
    };
    let model = e.model.trim().to_string();
    if model.is_empty() {
        return;
    }
    let key = (p, model);
    map.insert(key, (Some(e.local), e.tool_capable));
}

/// Collect hints: worker allowlist entries override orchestrator for the same `(provider, model)`.
fn allowlist_hints(providers: &ProvidersConfig, agents: &AgentsConfig) -> HashMap<(String, String), Hint> {
    let mut map = HashMap::new();
    if let Some(ref list) = agents.delegate_allowed_models {
        for e in list {
            insert_allowlist(&mut map, providers, e);
        }
    }
    if let Some(ref workers) = agents.workers {
        for w in workers {
            if let Some(ref list) = w.delegate_allowed_models {
                for e in list {
                    insert_allowlist(&mut map, providers, e);
                }
            }
        }
    }
    map
}

fn push_discovered(
    out: &mut Vec<OrchestrationCatalogEntry>,
    seen: &mut HashSet<(String, String)>,
    provider: &str,
    model: &str,
    hints: &HashMap<(String, String), Hint>,
) {
    let model = model.trim();
    if model.is_empty() {
        return;
    }
    let key = (provider.to_string(), model.to_string());
    if !seen.insert(key.clone()) {
        return;
    }
    let hint = hints.get(&(provider.to_string(), model.to_string()));
    let (local, tool_capable) = match hint {
        Some((l, tc)) => (*l, *tc),
        None => (None, None),
    };
    out.push(OrchestrationCatalogEntry {
        provider: provider.to_string(),
        model: model.to_string(),
        discovered: true,
        local,
        tool_capable,
    });
}

/// Build a merged list: discovered models per provider, then allowlist-only pairs not seen in discovery.
/// The `discovered_models` map uses provider id → model name list.
pub fn build_orchestration_catalog(
    providers: &ProvidersConfig,
    agents: &AgentsConfig,
    discovered_models: &HashMap<String, Vec<String>>,
) -> Vec<OrchestrationCatalogEntry> {
    let hints = allowlist_hints(providers, agents);
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut out: Vec<OrchestrationCatalogEntry> = Vec::new();

    // Iterate over discovered models by provider id.
    let mut provider_ids: Vec<&String> = discovered_models.keys().collect();
    provider_ids.sort();
    for provider_id in provider_ids {
        let models = discovered_models.get(provider_id).map(|v| v.as_slice()).unwrap_or(&[]);
        for model in models {
            push_discovered(&mut out, &mut seen, provider_id, model, &hints);
        }
    }

    // Allowlist-only rows (configured but not in current discovery lists).
    let mut allow_keys: Vec<(String, String)> = hints.keys().cloned().collect();
    allow_keys.sort();
    for (prov, model) in allow_keys {
        let key = (prov.clone(), model.clone());
        if seen.contains(&key) {
            continue;
        }
        let hint = hints.get(&key);
        let (local, tool_capable) = match hint {
            Some((l, tc)) => (*l, *tc),
            None => (None, None),
        };
        out.push(OrchestrationCatalogEntry {
            provider: prov,
            model,
            discovered: false,
            local,
            tool_capable,
        });
    }

    out.sort_by(|a, b| {
        a.provider
            .cmp(&b.provider)
            .then_with(|| a.model.cmp(&b.model))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{EndpointType, ProviderDefinition, WorkerConfig};

    fn test_providers(ids: &[&str]) -> ProvidersConfig {
        ProvidersConfig {
            entries: ids.iter().map(|id| {
                let endpoint = match *id {
                    "ollama" => EndpointType::Ollama,
                    _ => EndpointType::OpenaiCompat,
                };
                ProviderDefinition {
                    id: id.to_string(),
                    endpoint,
                    base_url: if endpoint == EndpointType::OpenaiCompat {
                        Some(format!("http://localhost/{}", id))
                    } else {
                        None
                    },
                    api_key: None,
                    default_model: None,
                    model_discovery: Default::default(),
                    static_models: Vec::new(),
                    auto_load: Default::default(),
                }
            }).collect(),
        }
    }

    #[test]
    fn merges_discovery_and_allowlist_only() {
        let providers = test_providers(&["ollama"]);
        let agents = AgentsConfig {
            delegate_allowed_models: Some(vec![AllowedModelEntry {
                provider: "ollama".to_string(),
                model: "offline-only".to_string(),
                local: true,
                tool_capable: Some(false),
            }]),
            ..AgentsConfig::default()
        };
        let mut discovered = HashMap::new();
        discovered.insert("ollama".to_string(), vec!["llama3.2:latest".to_string()]);
        let cat = build_orchestration_catalog(&providers, &agents, &discovered);
        assert!(cat
            .iter()
            .any(|r| r.model == "llama3.2:latest" && r.discovered));
        let off = cat
            .iter()
            .find(|r| r.model == "offline-only")
            .expect("allowlist-only row");
        assert!(!off.discovered);
        assert_eq!(off.local, Some(true));
        assert_eq!(off.tool_capable, Some(false));
    }

    #[test]
    fn worker_hint_overrides_orchestrator_for_same_pair() {
        let providers = test_providers(&["ollama"]);
        let agents = AgentsConfig {
            delegate_allowed_models: Some(vec![AllowedModelEntry {
                provider: "ollama".to_string(),
                model: "m".to_string(),
                local: false,
                tool_capable: None,
            }]),
            workers: Some(vec![WorkerConfig {
                id: "w".to_string(),
                default_provider: None,
                default_model: None,
                enabled_providers: None,
                skills_enabled: None,
                context_mode: None,
                delegate_allowed_models: Some(vec![AllowedModelEntry {
                    provider: "ollama".to_string(),
                    model: "m".to_string(),
                    local: true,
                    tool_capable: None,
                }]),
            }]),
            ..AgentsConfig::default()
        };
        let mut discovered = HashMap::new();
        discovered.insert("ollama".to_string(), vec!["m".to_string()]);
        let cat = build_orchestration_catalog(&providers, &agents, &discovered);
        let row = cat.iter().find(|r| r.model == "m").expect("row");
        assert_eq!(row.local, Some(true));
    }
}
