//! Model providers: endpoint types and the [`Provider`] trait.
//!
//! The active provider is selected via config (`agents.defaultProvider` referencing a provider `id`
//! from the `providers` array). Client construction uses the provider's `endpoint` type to select
//! the appropriate adapter. Model id (`agents.defaultModel`) is passed as-is to the provider.

mod ollama;
mod openai_compat;

use async_trait::async_trait;

pub use ollama::{
    ChatMessage, ChatResponse, FinishReason, OllamaClient, OllamaError, OllamaModel, ToolCall,
    ToolCallFunction, ToolDefinition, ToolFunctionDefinition, Usage,
};
pub use openai_compat::OpenAiCompatClient;

use crate::config::{EndpointType, ProviderDefinition, ProvidersConfig};
use std::sync::Arc;

/// Common error type for any provider and for agent/session errors.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("ollama: {0}")]
    Ollama(#[from] OllamaError),
    #[error("provider: {0}")]
    Provider(String),
    /// Agent or session store error (not from a provider).
    #[error("session: {0}")]
    Session(String),
}

/// Provider interface for chat and chat_stream.
#[async_trait]
pub trait Provider: Send + Sync {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, ProviderError>;

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, ProviderError>;
}

/// Build a [`Provider`] client from a [`ProviderDefinition`] based on its [`EndpointType`].
pub fn build_provider_client(
    def: &ProviderDefinition,
    providers: &ProvidersConfig,
) -> Result<Arc<dyn Provider>, String> {
    let base_url = crate::config::resolve_provider_base_url(providers, &def.id);
    let api_key = crate::config::resolve_provider_api_key(providers, &def.id);

    match def.endpoint {
        EndpointType::Ollama => {
            Ok(Arc::new(OllamaClient::new(base_url)))
        }
        EndpointType::OpenaiCompat => {
            let base = base_url.ok_or_else(|| {
                format!("provider '{}' uses endpoint 'openai-compat' but baseUrl could not be resolved", def.id)
            })?;
            Ok(Arc::new(OpenAiCompatClient::new_with_auto_load(base, api_key, def.auto_load)))
        }
    }
}
