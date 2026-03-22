//! Canonical provider id (`"ollama"`, `"lms"`, …) and [`ProviderChoice`] for dispatch.

use crate::config::{canonical_provider, AgentsConfig};

/// Which concrete model provider implementation to use (maps to a client in [`super::dispatch::ProviderClients`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderChoice {
    Ollama,
    Lms,
    Vllm,
    Nim,
    OpenAi,
    Hf,
}

/// Wire / status string for this provider (matches `canonical_provider` ids).
pub fn provider_id(choice: ProviderChoice) -> &'static str {
    match choice {
        ProviderChoice::Ollama => "ollama",
        ProviderChoice::Lms => "lms",
        ProviderChoice::Vllm => "vllm",
        ProviderChoice::Nim => "nim",
        ProviderChoice::OpenAi => "openai",
        ProviderChoice::Hf => "hf",
    }
}

/// Map a canonical provider string (from [`canonical_provider`]) to [`ProviderChoice`].
pub fn provider_choice_from_canonical(s: &str) -> ProviderChoice {
    match s {
        "lms" => ProviderChoice::Lms,
        "vllm" => ProviderChoice::Vllm,
        "nim" => ProviderChoice::Nim,
        "openai" => ProviderChoice::OpenAi,
        "hf" => ProviderChoice::Hf,
        _ => ProviderChoice::Ollama,
    }
}

/// Resolve default provider from [`AgentsConfig`]. Invalid or unknown values default to Ollama.
pub fn resolve_provider_choice(agents: &AgentsConfig) -> ProviderChoice {
    let canonical = agents
        .default_provider
        .as_deref()
        .and_then(canonical_provider)
        .unwrap_or("ollama");
    provider_choice_from_canonical(canonical)
}
