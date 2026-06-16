//! Merged `(provider, model)` catalog for discovery (see `base/adr/ORCHESTRATION.md`).

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::config::{AgentsConfig, ProvidersConfig};

/// One row in the merged orchestration catalog (WebSocket **`status`** payload **`agents.orchestrationCatalog`**).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationCatalogEntry {
    pub provider: String,
    pub model: String,
    /// Present in provider discovery (or static model list) at last refresh.
    pub discovered: bool,
}

/// Build a merged list from discovered models per provider.
/// The `discovered_models` map uses provider id → model name list.
pub fn build_orchestration_catalog(
    _providers: &ProvidersConfig,
    _agents: &AgentsConfig,
    discovered_models: &HashMap<String, Vec<String>>,
) -> Vec<OrchestrationCatalogEntry> {
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut out: Vec<OrchestrationCatalogEntry> = Vec::new();

    // Iterate over discovered models by provider id.
    let mut provider_ids: Vec<&String> = discovered_models.keys().collect();
    provider_ids.sort();
    for provider_id in provider_ids {
        let models = discovered_models.get(provider_id).map(|v| v.as_slice()).unwrap_or(&[]);
        for model in models {
            let model = model.trim();
            if model.is_empty() {
                continue;
            }
            let key = (provider_id.to_string(), model.to_string());
            if !seen.insert(key) {
                continue;
            }
            out.push(OrchestrationCatalogEntry {
                provider: provider_id.to_string(),
                model: model.to_string(),
                discovered: true,
            });
        }
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
    use crate::config::ProvidersConfig;

    fn test_providers(ids: &[&str]) -> ProvidersConfig {
        crate::config::ProvidersConfig {
            entries: ids.iter().map(|id| {
                let endpoint_type = match *id {
                    "ollama" => crate::config::EndpointType::Ollama,
                    _ => crate::config::EndpointType::OpenaiCompat,
                };
                crate::config::ProviderDefinition {
                    id: id.to_string(),
                    endpoint_type,
                    base_url: if endpoint_type == crate::config::EndpointType::OpenaiCompat {
                        Some(format!("http://localhost/{}", id))
                    } else {
                        None
                    },
                    api_key: None,
                    default_model: None,
                    model_discovery: Default::default(),
                    static_models: Vec::new(),
                }
            }).collect(),
        }
    }

    #[test]
    fn merges_discovered_models() {
        let providers = test_providers(&["ollama"]);
        let agents = crate::config::AgentsConfig::default();
        let mut discovered = HashMap::new();
        discovered.insert("ollama".to_string(), vec!["llama3.2:3b".to_string()]);
        let cat = build_orchestration_catalog(&providers, &agents, &discovered);
        assert!(cat
            .iter()
            .any(|r| r.model == "llama3.2:3b" && r.discovered));
    }

    #[test]
    fn deduplicates_same_model_across_providers() {
        let providers = test_providers(&["ollama", "lms"]);
        let agents = crate::config::AgentsConfig::default();
        let mut discovered = HashMap::new();
        discovered.insert("ollama".to_string(), vec!["llama3.2:3b".to_string()]);
        discovered.insert("lms".to_string(), vec!["llama3.2:3b".to_string()]);
        let cat = build_orchestration_catalog(&providers, &agents, &discovered);
        // Each (provider, model) is a distinct entry — same model name on different providers
        // is not deduplicated.
        assert_eq!(cat.len(), 2);
    }
}
