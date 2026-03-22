//! OpenAI API client (`https://api.openai.com/v1/...`). Uses shared [`super::openai_compat::OpenAiCompatClient`].
//!
//! Data is sent to OpenAI; not a local-first option. Optional base URL override for Azure OpenAI or proxies.

use crate::providers::openai_compat::{OpenAiCompatClient, OpenAiCompatError};
use crate::providers::{ChatMessage, ChatResponse, ToolDefinition};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// Client for the OpenAI HTTP API (chat completions, list models).
#[derive(Clone)]
pub struct OpenAiClient {
    inner: OpenAiCompatClient,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenAiError {
    #[error("openai request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("openai api error: {0}")]
    Api(String),
}

impl From<OpenAiCompatError> for OpenAiError {
    fn from(e: OpenAiCompatError) -> Self {
        match e {
            OpenAiCompatError::Request(r) => OpenAiError::Request(r),
            OpenAiCompatError::Api(s) => OpenAiError::Api(s),
        }
    }
}

impl OpenAiClient {
    pub fn new(base_url: Option<String>, api_key: Option<String>) -> Self {
        Self {
            inner: OpenAiCompatClient::new(base_url, DEFAULT_BASE_URL, api_key),
        }
    }

    /// List models from `GET /v1/models`.
    pub async fn list_models(&self) -> Result<Vec<OpenAiModel>, OpenAiError> {
        let ids = self.inner.list_models_openai().await?;
        Ok(ids
            .into_iter()
            .map(|name| OpenAiModel { name })
            .collect())
    }

    pub async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, OpenAiError> {
        self.inner
            .chat(model, messages, stream, tools)
            .await
            .map_err(OpenAiError::from)
    }

    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, OpenAiError> {
        self.inner
            .chat_stream(model, messages, tools, on_chunk)
            .await
            .map_err(OpenAiError::from)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OpenAiModel {
    pub name: String,
}
