//! Default model ids when `agents.defaultModel` is unset.

use super::choice::ProviderChoice;

/// Ollama tag for Llama 3.2 3B instruct (aligns with LMS/NIM defaults below).
pub const DEFAULT_MODEL_FALLBACK: &str = "llama3.2:3b";
/// LM Studio id for the same weight class as [`DEFAULT_MODEL_FALLBACK`].
pub const DEFAULT_MODEL_FALLBACK_LMS: &str = "llama-3.2-3B-instruct";
/// NIM catalog id for the same weight class as [`DEFAULT_MODEL_FALLBACK`].
pub const DEFAULT_MODEL_FALLBACK_NIM: &str = "meta/llama-3.2-3b-instruct";
pub const DEFAULT_MODEL_FALLBACK_VLLM: &str = "Qwen/Qwen2.5-7B-Instruct";
pub const DEFAULT_MODEL_FALLBACK_OPENAI: &str = "gpt-4o-mini";
pub const DEFAULT_MODEL_FALLBACK_HF: &str = "meta-llama/Llama-3.1-8B-Instruct";

/// Resolve model id from config and optional request override. No prefix stripping—model id is passed as-is to the provider.
pub fn resolve_model(
    config_model: Option<&str>,
    param_model: Option<&str>,
    provider: ProviderChoice,
) -> String {
    let s = param_model
        .or(config_model)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    match (s, provider) {
        (Some(name), _) => name,
        (None, ProviderChoice::Ollama) => DEFAULT_MODEL_FALLBACK.to_string(),
        (None, ProviderChoice::Lms) => DEFAULT_MODEL_FALLBACK_LMS.to_string(),
        (None, ProviderChoice::Vllm) => DEFAULT_MODEL_FALLBACK_VLLM.to_string(),
        (None, ProviderChoice::Nim) => DEFAULT_MODEL_FALLBACK_NIM.to_string(),
        (None, ProviderChoice::OpenAi) => DEFAULT_MODEL_FALLBACK_OPENAI.to_string(),
        (None, ProviderChoice::Hf) => DEFAULT_MODEL_FALLBACK_HF.to_string(),
    }
}
