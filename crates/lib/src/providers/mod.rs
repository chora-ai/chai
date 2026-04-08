//! Model providers: Ollama, LM Studio (OpenAI-compat), vLLM (OpenAI-compat), OpenAI API, Hugging Face OpenAI-compat endpoints, and NVIDIA NIM hosted API.
//!
//! The active provider is selected via config (agents.defaultProvider: "ollama" | "lms" | "vllm" | "nim" | "openai" | "hf"). Model id
//! (agents.default_model) is passed as-is to the provider.

mod hf;
mod lms;
mod nim;
mod ollama;
mod openai;
mod openai_compat;
mod vllm;

use async_trait::async_trait;

pub use hf::{HfClient, HfError, HfModel};
pub use lms::{LmsClient, LmsError, LmsModel};
pub use nim::{NimClient, NimError, NimModel};
pub use ollama::{
    ChatMessage, ChatResponse, OllamaClient, OllamaError, OllamaModel, ToolCall, ToolCallFunction,
    ToolDefinition, ToolFunctionDefinition,
};
pub use openai::{OpenAiClient, OpenAiError, OpenAiModel};
pub use vllm::{VllmClient, VllmError, VllmModel};

/// Common error type for any provider and for agent/session errors.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("ollama: {0}")]
    Ollama(#[from] OllamaError),
    #[error("lms: {0}")]
    Lms(#[from] LmsError),
    #[error("vllm: {0}")]
    Vllm(#[from] VllmError),
    #[error("nim: {0}")]
    Nim(#[from] NimError),
    #[error("openai: {0}")]
    OpenAi(#[from] OpenAiError),
    #[error("hf: {0}")]
    Hf(#[from] HfError),
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

#[async_trait]
impl Provider for OllamaClient {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, ProviderError> {
        OllamaClient::chat(self, model, messages, stream, tools)
            .await
            .map_err(ProviderError::Ollama)
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, ProviderError> {
        OllamaClient::chat_stream(self, model, messages, tools, on_chunk)
            .await
            .map_err(ProviderError::Ollama)
    }
}

#[async_trait]
impl Provider for LmsClient {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, ProviderError> {
        LmsClient::chat(self, model, messages, stream, tools)
            .await
            .map_err(ProviderError::Lms)
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, ProviderError> {
        LmsClient::chat_stream(self, model, messages, tools, on_chunk)
            .await
            .map_err(ProviderError::Lms)
    }
}

#[async_trait]
impl Provider for NimClient {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, ProviderError> {
        NimClient::chat(self, model, messages, stream, tools)
            .await
            .map_err(ProviderError::Nim)
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, ProviderError> {
        NimClient::chat_stream(self, model, messages, tools, on_chunk)
            .await
            .map_err(ProviderError::Nim)
    }
}

#[async_trait]
impl Provider for VllmClient {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, ProviderError> {
        VllmClient::chat(self, model, messages, stream, tools)
            .await
            .map_err(ProviderError::Vllm)
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, ProviderError> {
        VllmClient::chat_stream(self, model, messages, tools, on_chunk)
            .await
            .map_err(ProviderError::Vllm)
    }
}

#[async_trait]
impl Provider for OpenAiClient {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, ProviderError> {
        OpenAiClient::chat(self, model, messages, stream, tools)
            .await
            .map_err(ProviderError::OpenAi)
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, ProviderError> {
        OpenAiClient::chat_stream(self, model, messages, tools, on_chunk)
            .await
            .map_err(ProviderError::OpenAi)
    }
}

#[async_trait]
impl Provider for HfClient {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, ProviderError> {
        HfClient::chat(self, model, messages, stream, tools)
            .await
            .map_err(ProviderError::Hf)
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, ProviderError> {
        HfClient::chat_stream(self, model, messages, tools, on_chunk)
            .await
            .map_err(ProviderError::Hf)
    }
}
