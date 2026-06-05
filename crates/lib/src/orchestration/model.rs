//! Default model ids when `agents.defaultModel` and `ProviderDefinition.defaultModel` are unset.

use crate::config::{ProvidersConfig, resolve_provider_default_model};

use super::choice::ProviderChoice;

/// Fallback model when no provider matches at all (should not happen in normal use).
pub const DEFAULT_MODEL_FALLBACK: &str = "llama3.2:3b";

/// Resolve model id from config and optional request override. No prefix stripping — model id
/// is passed as-is to the provider.
pub fn resolve_model(
    providers: &ProvidersConfig,
    config_model: Option<&str>,
    param_model: Option<&str>,
    provider: &ProviderChoice,
) -> String {
    let s = param_model
        .or(config_model)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    match s {
        Some(name) => name,
        None => resolve_provider_default_model(providers, provider.as_str()),
    }
}
