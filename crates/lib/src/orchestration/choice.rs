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
/// Returns the first configured provider if the agent's `defaultProvider` doesn't match any
/// configured provider, or "ollama" as a last resort.
pub fn resolve_provider_choice(providers: &ProvidersConfig, agents: &AgentsConfig) -> ProviderChoice {
    let id = agents
        .default_provider
        .as_deref()
        .and_then(|s| canonical_provider_id(providers, s))
        .or_else(|| providers.entries.first().map(|p| p.id.trim().to_string()))
        .unwrap_or_else(|| "ollama".to_string());
    ProviderChoice::new(id)
}
