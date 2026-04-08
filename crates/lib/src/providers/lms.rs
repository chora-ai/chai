//! LM Studio client: OpenAI-compatible API only.
//!
//! We use /v1/chat/completions (and /api/v1/models for listing when needed). On 500 "Model is unloaded"
//! we call POST /api/v1/models/load and retry once (aligns with Ollama). All other errors are returned.

use crate::providers::openai_compat::{OpenAiCompatClient, OpenAiCompatError};
use crate::providers::{ChatMessage, ChatResponse, ToolDefinition};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:1234/v1";

/// Client for LM Studio (provider id `lms`): OpenAI-compat chat only; errors are returned to the caller.
#[derive(Clone)]
pub struct LmsClient {
    inner: OpenAiCompatClient,
}

#[derive(Debug, thiserror::Error)]
pub enum LmsError {
    #[error("lms request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("lms api error: {0}")]
    Api(String),
}

impl From<OpenAiCompatError> for LmsError {
    fn from(e: OpenAiCompatError) -> Self {
        match e {
            OpenAiCompatError::Request(r) => LmsError::Request(r),
            OpenAiCompatError::Api(s) => LmsError::Api(s),
        }
    }
}

impl LmsClient {
    pub fn new(base_url: Option<String>) -> Self {
        Self {
            inner: OpenAiCompatClient::new(base_url, DEFAULT_BASE_URL, None),
        }
    }

    /// Root URL for the LM Studio server (no /v1 suffix). Used for /api/v1/models and /api/v1/models/load.
    fn api_server_root(&self) -> String {
        let base_url = self.inner.base_url();
        if base_url.ends_with("/v1") {
            base_url.trim_end_matches("/v1").to_string()
        } else {
            base_url.to_string()
        }
    }

    /// Ensure the model is loaded in LM Studio before chat. Calls POST /api/v1/models/load with
    /// only the model id for compatibility (many LM Studio versions do not accept gpu/offload_kv_cache_to_gpu).
    /// For VRAM-friendly load use `lms load <model> --gpu 0.5` before starting the app.
    async fn ensure_model_loaded(&self, model: &str) -> Result<(), LmsError> {
        let root = self.api_server_root();
        let url = format!("{}/api/v1/models/load", root);
        let body = serde_json::json!({ "model": model });
        let res = self
            .inner
            .http_client()
            .post(&url)
            .json(&body)
            .send()
            .await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(LmsError::Api(format!("{} {}", status, body)));
        }
        Ok(())
    }

    /// List available models via GET /api/v1/models; returned ids are the model `key` (e.g. publisher/model-name) for use with /v1/chat/completions.
    pub async fn list_models(&self) -> Result<Vec<LmsModel>, LmsError> {
        self.list_models_native().await
    }

    /// GET /api/v1/models — list models; returned key is the model id for chat.
    async fn list_models_native(&self) -> Result<Vec<LmsModel>, LmsError> {
        let root = self.api_server_root();
        let url = format!("{}/api/v1/models", root);
        let res = self.inner.http_client().get(&url).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(LmsError::Api(format!("{} {}", status, body)));
        }
        let data: NativeModelsResponse = res.json().await?;
        Ok(data
            .models
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.typ.as_deref() == Some("llm"))
            .filter(|m| {
                let k = m.key.as_deref().unwrap_or("").trim();
                !k.is_empty() && !k.starts_with("text-embedding-")
            })
            .map(|m| LmsModel {
                name: m.key.clone().unwrap_or_default().trim().to_string(),
            })
            .collect())
    }

    /// Non-streaming chat via /v1/chat/completions. On 500 "Model is unloaded" we load and retry once (aligns with Ollama); all other errors are returned.
    pub async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, LmsError> {
        match self
            .inner
            .chat(model, messages.clone(), stream, tools.clone())
            .await
        {
            Ok(r) => Ok(r),
            Err(e) => {
                let msg = e.to_string();
                if msg.to_lowercase().contains("unloaded") {
                    self.ensure_model_loaded(model).await?;
                    self.inner
                        .chat(model, messages, false, tools)
                        .await
                        .map_err(LmsError::from)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    /// Streaming chat via /v1/chat/completions. On 500 "Model is unloaded" we load and retry once with a single non-streaming call so we don't invoke on_chunk twice (avoid duplicate or interleaved output if the first attempt had already streamed partial data).
    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, LmsError> {
        match self
            .inner
            .chat_stream(model, messages.clone(), tools.clone(), on_chunk)
            .await
        {
            Ok(r) => Ok(r),
            Err(e) => {
                let msg = e.to_string();
                if msg.to_lowercase().contains("unloaded") {
                    self.ensure_model_loaded(model).await?;
                    let out = self
                        .inner
                        .chat(model, messages, false, tools)
                        .await
                        .map_err(LmsError::from)?;
                    if let Some(ref m) = out.message {
                        if !m.content.is_empty() {
                            on_chunk(&m.content);
                        }
                    }
                    Ok(out)
                } else {
                    Err(e.into())
                }
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LmsModel {
    pub name: String,
}

// --- Native API wire types (list models only) ---

#[derive(Debug, serde::Deserialize)]
struct NativeModelsResponse {
    models: Option<Vec<NativeModelObject>>,
}

#[derive(Debug, serde::Deserialize)]
struct NativeModelObject {
    key: Option<String>,
    #[serde(rename = "type")]
    typ: Option<String>,
}
