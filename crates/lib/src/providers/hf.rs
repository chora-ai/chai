//! Hugging Face Inference Endpoints (and other OpenAI-compatible HF deployments) via shared [`super::openai_compat::OpenAiCompatClient`].
//!
//! Set `providers.hf.baseUrl` to your endpoint base including `/v1` (e.g. `https://....endpoints.huggingface.cloud/v1`). Self-hosted TGI often exposes the same routes.

use crate::providers::openai_compat::{OpenAiCompatClient, OpenAiCompatError};
use crate::providers::{ChatMessage, ChatResponse, ToolDefinition};

/// Default when `providers.hf.baseUrl` is unset: local placeholder; configure a real endpoint for Inference Endpoints or TGI.
const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8080/v1";

/// Client for an OpenAI-compatible Hugging Face endpoint.
#[derive(Clone)]
pub struct HfClient {
    inner: OpenAiCompatClient,
}

#[derive(Debug, thiserror::Error)]
pub enum HfError {
    #[error("huggingface request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("huggingface api error: {0}")]
    Api(String),
}

impl From<OpenAiCompatError> for HfError {
    fn from(e: OpenAiCompatError) -> Self {
        match e {
            OpenAiCompatError::Request(r) => HfError::Request(r),
            OpenAiCompatError::Api(s) => HfError::Api(s),
        }
    }
}

impl HfClient {
    pub fn new(base_url: Option<String>, api_key: Option<String>) -> Self {
        Self {
            inner: OpenAiCompatClient::new(base_url, DEFAULT_BASE_URL, api_key),
        }
    }

    /// List models from `GET /v1/models` when the endpoint implements it.
    pub async fn list_models(&self) -> Result<Vec<HfModel>, HfError> {
        let ids = self.inner.list_models_openai().await?;
        Ok(ids.into_iter().map(|name| HfModel { name }).collect())
    }

    pub async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, HfError> {
        self.inner
            .chat(model, messages, stream, tools)
            .await
            .map_err(HfError::from)
    }

    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, HfError> {
        self.inner
            .chat_stream(model, messages, tools, on_chunk)
            .await
            .map_err(HfError::from)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HfModel {
    pub name: String,
}
