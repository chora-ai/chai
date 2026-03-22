//! vLLM client: OpenAI-compatible HTTP API (`vllm serve`).
//!
//! Uses `/v1/chat/completions` and `GET /v1/models`. Optional bearer auth when the server is started with `--api-key`.

use crate::providers::openai_compat::{OpenAiCompatClient, OpenAiCompatError};
use crate::providers::{ChatMessage, ChatResponse, ToolDefinition};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8000/v1";

/// Client for a vLLM OpenAI-compatible server.
#[derive(Clone)]
pub struct VllmClient {
    inner: OpenAiCompatClient,
}

#[derive(Debug, thiserror::Error)]
pub enum VllmError {
    #[error("vllm request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("vllm api error: {0}")]
    Api(String),
}

impl From<OpenAiCompatError> for VllmError {
    fn from(e: OpenAiCompatError) -> Self {
        match e {
            OpenAiCompatError::Request(r) => VllmError::Request(r),
            OpenAiCompatError::Api(s) => VllmError::Api(s),
        }
    }
}

impl VllmClient {
    pub fn new(base_url: Option<String>, api_key: Option<String>) -> Self {
        Self {
            inner: OpenAiCompatClient::new(base_url, DEFAULT_BASE_URL, api_key),
        }
    }

    /// List models from `GET /v1/models` (typically the model id(s) served by this process).
    pub async fn list_models(&self) -> Result<Vec<VllmModel>, VllmError> {
        let ids = self.inner.list_models_openai().await?;
        Ok(ids
            .into_iter()
            .map(|name| VllmModel { name })
            .collect())
    }

    pub async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, VllmError> {
        self.inner
            .chat(model, messages, stream, tools)
            .await
            .map_err(VllmError::from)
    }

    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, VllmError> {
        self.inner
            .chat_stream(model, messages, tools, on_chunk)
            .await
            .map_err(VllmError::from)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VllmModel {
    pub name: String,
}
