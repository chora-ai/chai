//! Merged `(provider, model)` catalog for discovery + delegation allowlists (see `.agents/EPIC_ORCHESTRATION.md`).

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::config::{canonical_provider, AgentsConfig, AllowedModelEntry};
use crate::providers::{HfModel, LmsModel, NimModel, OllamaModel, OpenAiModel, VllmModel};

/// One row in the merged orchestration catalog (WebSocket **`status`** payload **`orchestrationCatalog`**).
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

fn insert_allowlist(map: &mut HashMap<(String, String), Hint>, e: &AllowedModelEntry) {
    let Some(p) = canonical_provider(e.provider.as_str()) else {
        return;
    };
    let model = e.model.trim().to_string();
    if model.is_empty() {
        return;
    }
    let key = (p.to_string(), model);
    map.insert(
        key,
        (Some(e.local), e.tool_capable),
    );
}

/// Collect hints: worker allowlist entries override orchestrator for the same `(provider, model)`.
fn allowlist_hints(agents: &AgentsConfig) -> HashMap<(String, String), Hint> {
    let mut map = HashMap::new();
    if let Some(ref list) = agents.delegate_allowed_models {
        for e in list {
            insert_allowlist(&mut map, e);
        }
    }
    if let Some(ref workers) = agents.workers {
        for w in workers {
            if let Some(ref list) = w.delegate_allowed_models {
                for e in list {
                    insert_allowlist(&mut map, e);
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
pub fn build_orchestration_catalog(
    agents: &AgentsConfig,
    ollama: &[OllamaModel],
    lms: &[LmsModel],
    vllm: &[VllmModel],
    nim: &[NimModel],
    openai: &[OpenAiModel],
    hf: &[HfModel],
) -> Vec<OrchestrationCatalogEntry> {
    let hints = allowlist_hints(agents);
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut out: Vec<OrchestrationCatalogEntry> = Vec::new();

    for m in ollama {
        push_discovered(&mut out, &mut seen, "ollama", &m.name, &hints);
    }
    for m in lms {
        push_discovered(&mut out, &mut seen, "lms", &m.name, &hints);
    }
    for m in vllm {
        push_discovered(&mut out, &mut seen, "vllm", &m.name, &hints);
    }
    for m in nim {
        push_discovered(&mut out, &mut seen, "nim", &m.name, &hints);
    }
    for m in openai {
        push_discovered(&mut out, &mut seen, "openai", &m.name, &hints);
    }
    for m in hf {
        push_discovered(&mut out, &mut seen, "hf", &m.name, &hints);
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
    use crate::config::WorkerConfig;

    #[test]
    fn merges_discovery_and_allowlist_only() {
        let agents = AgentsConfig {
            delegate_allowed_models: Some(vec![
                AllowedModelEntry {
                    provider: "ollama".to_string(),
                    model: "offline-only".to_string(),
                    local: true,
                    tool_capable: Some(false),
                },
            ]),
            ..AgentsConfig::default()
        };
        let ollama = vec![OllamaModel {
            name: "llama3.2:latest".to_string(),
            size: None,
        }];
        let cat = build_orchestration_catalog(&agents, &ollama, &[], &[], &[], &[], &[]);
        assert!(cat.iter().any(|r| r.model == "llama3.2:latest" && r.discovered));
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
                delegate_allowed_models: Some(vec![AllowedModelEntry {
                    provider: "ollama".to_string(),
                    model: "m".to_string(),
                    local: true,
                    tool_capable: None,
                }]),
            }]),
            ..AgentsConfig::default()
        };
        let ollama = vec![OllamaModel {
            name: "m".to_string(),
            size: None,
        }];
        let cat = build_orchestration_catalog(&agents, &ollama, &[], &[], &[], &[], &[]);
        let row = cat.iter().find(|r| r.model == "m").expect("row");
        assert_eq!(row.local, Some(true));
    }
}
