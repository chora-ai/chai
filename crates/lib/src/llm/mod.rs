//! LLM abstraction: Ollama, LM Studio (OpenAI-compat), and NVIDIA NIM hosted API.
//!
//! Backend is selected via config (agents.defaultBackend: "ollama" | "lmstudio" | "nim"). Model id
//! (agents.default_model) is passed as-is to the backend.

mod lm_studio;
mod nim;
mod ollama;

use async_trait::async_trait;

pub use lm_studio::{LmStudioClient, LmStudioError, LmStudioModel};
pub use nim::{NimClient, NimError, NimModel};
pub use ollama::{
    ChatMessage, ChatResponse, OllamaClient, OllamaError, OllamaModel, ToolCall, ToolCallFunction,
    ToolDefinition, ToolFunctionDefinition,
};

/// Common error type for any LLM backend (Ollama, LM Studio, NIM) and for agent/session errors.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("ollama: {0}")]
    Ollama(#[from] OllamaError),
    #[error("lm studio: {0}")]
    LmStudio(#[from] LmStudioError),
    #[error("nim: {0}")]
    Nim(#[from] NimError),
    /// Agent or session store error (not from an LLM backend).
    #[error("session: {0}")]
    Session(String),
}

/// Backend interface for chat and chat_stream so the agent can use Ollama or LM Studio.
#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, LlmError>;

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, LlmError>;
}

#[async_trait]
impl LlmBackend for OllamaClient {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, LlmError> {
        OllamaClient::chat(self, model, messages, stream, tools)
            .await
            .map_err(LlmError::Ollama)
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, LlmError> {
        OllamaClient::chat_stream(self, model, messages, tools, on_chunk)
            .await
            .map_err(LlmError::Ollama)
    }
}

#[async_trait]
impl LlmBackend for LmStudioClient {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, LlmError> {
        LmStudioClient::chat(self, model, messages, stream, tools)
            .await
            .map_err(LlmError::LmStudio)
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, LlmError> {
        LmStudioClient::chat_stream(self, model, messages, tools, on_chunk)
            .await
            .map_err(LlmError::LmStudio)
    }
}

#[async_trait]
impl LlmBackend for NimClient {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, LlmError> {
        NimClient::chat(self, model, messages, stream, tools)
            .await
            .map_err(LlmError::Nim)
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, LlmError> {
        NimClient::chat_stream(self, model, messages, tools, on_chunk)
            .await
            .map_err(LlmError::Nim)
    }
}