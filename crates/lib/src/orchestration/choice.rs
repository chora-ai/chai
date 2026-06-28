//! Provider choice: resolved provider id string for dispatch.

use crate::config::{canonical_provider_id, AgentsConfig, ProvidersConfig};

/// Resolved provider id for dispatch. This is a string that matches a `ProviderDefinition::id`
/// in the configured `ProvidersConfig`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderChoice {
    pub id: String,
}

impl ProviderChoice {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    /// Wire / status string for this provider.
    pub fn as_str(&self) -> &str {
        &self.id
    }
}

impl std::fmt::Display for ProviderChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// Resolve default provider from [`AgentsConfig`] + [`ProvidersConfig`].
/// Uses the default (first) orchestrator's `default_provider`.
/// Returns the first configured provider if the agent's `defaultProvider` doesn't match any
/// configured provider, or "ollama" as a last resort.
pub fn resolve_provider_choice(providers: &ProvidersConfig, agents: &AgentsConfig) -> ProviderChoice {
    resolve_orchestrator_provider_choice(providers, agents.default_orchestrator())
}

/// Resolve default provider for a specific [`OrchestratorConfig`].
/// Same fallback logic as [`resolve_provider_choice`] but scoped to one orchestrator.
pub fn resolve_orchestrator_provider_choice(
    providers: &ProvidersConfig,
    orch: &crate::config::OrchestratorConfig,
) -> ProviderChoice {
    let id = orch
        .default_provider
        .as_deref()
        .and_then(|s| canonical_provider_id(providers, s))
        .or_else(|| providers.entries.first().map(|p| p.id.trim().to_string()))
        .unwrap_or_else(|| "ollama".to_string());
    ProviderChoice::new(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn resolve_orchestrator_provider_choice_uses_orchestrator_default() {
        let j = r#"{"providers":[
            {"id":"nearai","endpointType":"openai-compat","baseUrl":"https://cloud-api.near.ai/v1"},
            {"id":"ollama","endpointType":"ollama"}
        ],"agents":[
            {"id":"developer","role":"orchestrator","defaultProvider":"nearai"},
            {"id":"reviewer","role":"orchestrator","defaultProvider":"ollama"}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        let dev = c.agents.orchestrator(Some("developer")).unwrap();
        let rev = c.agents.orchestrator(Some("reviewer")).unwrap();
        assert_eq!(
            resolve_orchestrator_provider_choice(&c.providers, dev),
            ProviderChoice::new("nearai")
        );
        assert_eq!(
            resolve_orchestrator_provider_choice(&c.providers, rev),
            ProviderChoice::new("ollama")
        );
    }

    #[test]
    fn resolve_provider_choice_uses_default_orchestrator() {
        let j = r#"{"providers":[
            {"id":"nearai","endpointType":"openai-compat","baseUrl":"https://cloud-api.near.ai/v1"},
            {"id":"ollama","endpointType":"ollama"}
        ],"agents":[
            {"id":"developer","role":"orchestrator","defaultProvider":"nearai"},
            {"id":"reviewer","role":"orchestrator","defaultProvider":"ollama"}
        ]}"#;
        let c: Config = serde_json::from_str(j).expect("parse");
        // resolve_provider_choice uses the default (first) orchestrator.
        assert_eq!(
            resolve_provider_choice(&c.providers, &c.agents),
            ProviderChoice::new("nearai")
        );
    }
}
