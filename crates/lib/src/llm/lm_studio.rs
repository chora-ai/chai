//! LM Studio client: OpenAI-compatible API only.
//!
//! We use /v1/chat/completions (and /v1/models for listing when needed). On 500 "Model is unloaded"
//! we call POST /api/v1/models/load and retry once (aligns with Ollama). All other errors are returned.

use crate::llm::{ChatMessage, ChatResponse, ToolCall, ToolCallFunction, ToolDefinition};
use anyhow::Result;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:1234/v1";

/// Client for LM Studio: OpenAI-compat chat only; errors are returned to the caller.
#[derive(Clone)]
pub struct LmStudioClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum LmStudioError {
    #[error("lm studio request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("lm studio api error: {0}")]
    Api(String),
}

impl LmStudioClient {
    pub fn new(base_url: Option<String>) -> Self {
        let base_url = base_url
            .map(|u| u.trim_end_matches('/').to_string())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    /// Root URL for the LM Studio server (no /v1 suffix). Used for /api/v1/models and /api/v1/models/load.
    fn api_server_root(&self) -> String {
        if self.base_url.ends_with("/v1") {
            self.base_url.trim_end_matches("/v1").to_string()
        } else {
            self.base_url.clone()
        }
    }

    /// Ensure the model is loaded in LM Studio before chat. Calls POST /api/v1/models/load with
    /// only the model id for compatibility (many LM Studio versions do not accept gpu/offload_kv_cache_to_gpu).
    /// For VRAM-friendly load use `lms load <model> --gpu 0.5` before starting the app.
    async fn ensure_model_loaded(&self, model: &str) -> Result<(), LmStudioError> {
        let root = self.api_server_root();
        let url = format!("{}/api/v1/models/load", root);
        let body = serde_json::json!({ "model": model });
        let res = self.client.post(&url).json(&body).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(LmStudioError::Api(format!("{} {}", status, body)));
        }
        Ok(())
    }

    /// List available models via GET /api/v1/models; returned ids are the model `key` (e.g. publisher/model-name) for use with /v1/chat/completions.
    pub async fn list_models(&self) -> Result<Vec<LmStudioModel>, LmStudioError> {
        self.list_models_native().await
    }

    /// GET /api/v1/models — list models; returned key is the model id for chat.
    async fn list_models_native(&self) -> Result<Vec<LmStudioModel>, LmStudioError> {
        let root = self.api_server_root();
        let url = format!("{}/api/v1/models", root);
        let res = self.client.get(&url).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(LmStudioError::Api(format!("{} {}", status, body)));
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
            .map(|m| LmStudioModel {
                name: m.key.clone().unwrap_or_default().trim().to_string(),
            })
            .collect())
    }

    /// Non-streaming chat via /v1/chat/completions. On 500 "Model is unloaded" we load and retry once (aligns with Ollama); all other errors are returned.
    pub async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        _stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, LmStudioError> {
        match self.chat_openai(model, &messages, tools.clone()).await {
            Ok(r) => Ok(r),
            Err(LmStudioError::Api(ref msg)) if msg.to_lowercase().contains("unloaded") => {
                self.ensure_model_loaded(model).await?;
                self.chat_openai(model, &messages, tools).await
            }
            e => e,
        }
    }

    /// POST /v1/chat/completions — non-streaming chat (OpenAI-compat).
    async fn chat_openai(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, LmStudioError> {
        let url = format!("{}/chat/completions", self.base_url);
        let (openai_messages, _) = messages_to_openai(messages);
        let body = OpenAiChatRequest {
            model: model.to_string(),
            messages: openai_messages,
            stream: false,
            tools: tools.map(tool_definitions_to_openai),
        };
        let res = self.client.post(&url).json(&body).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(LmStudioError::Api(format!("{} {}", status, body)));
        }
        let data: OpenAiChatResponse = res.json().await?;
        openai_response_to_chat_response(data)
    }

    /// Streaming chat via /v1/chat/completions. On 500 "Model is unloaded" we load and retry once with a single non-streaming call so we don't invoke on_chunk twice (avoid duplicate or interleaved output if the first attempt had already streamed partial data).
    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, LmStudioError> {
        match self.chat_stream_openai(model, &messages, tools.clone(), on_chunk).await {
            Ok(r) => Ok(r),
            Err(LmStudioError::Api(ref msg)) if msg.to_lowercase().contains("unloaded") => {
                self.ensure_model_loaded(model).await?;
                let out = self.chat_openai(model, &messages, tools).await?;
                if let Some(ref m) = out.message {
                    if !m.content.is_empty() {
                        on_chunk(&m.content);
                    }
                }
                Ok(out)
            }
            e => e,
        }
    }

    /// POST /v1/chat/completions with stream: true (OpenAI-compat).
    async fn chat_stream_openai(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, LmStudioError> {
        let url = format!("{}/chat/completions", self.base_url);
        let (openai_messages, _) = messages_to_openai(messages);
        let body = OpenAiChatRequest {
            model: model.to_string(),
            messages: openai_messages,
            stream: true,
            tools: tools.map(tool_definitions_to_openai),
        };
        let res = self.client.post(&url).json(&body).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(LmStudioError::Api(format!("{} {}", status, body)));
        }
        let mut stream = res.bytes_stream();
        let mut buffer = Vec::new();
        let mut content = String::new();
        let mut tool_calls: Vec<OpenAiStreamToolCall> = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(LmStudioError::Request)?;
            buffer.extend_from_slice(&chunk);
            while let Some(pos) = buffer.windows(2).position(|w| w == b"\n\n") {
                let line_bytes: Vec<u8> = buffer.drain(..pos).collect();
                buffer.drain(..2);
                let line = String::from_utf8_lossy(&line_bytes);
                let line = line.trim();
                if line.starts_with("data: ") {
                    let data = line.trim_start_matches("data: ");
                    if data == "[DONE]" {
                        break;
                    }
                    if let Ok(ev) = serde_json::from_str::<OpenAiStreamChunk>(data) {
                        if let Some(choice) = ev.choices.and_then(|c| c.into_iter().next()) {
                            if let Some(delta) = choice.delta {
                                if let Some(c) = delta.content {
                                    on_chunk(&c);
                                    content.push_str(&c);
                                }
                                if let Some(tc_list) = delta.tool_calls {
                                    for tc in tc_list {
                                        if let Some(idx) = tc.index {
                                            while tool_calls.len() <= idx as usize {
                                                tool_calls.push(OpenAiStreamToolCall::default());
                                            }
                                            if let Some(id) = tc.id {
                                                tool_calls[idx as usize].id = id;
                                            }
                                            if let Some(typ) = tc.typ {
                                                tool_calls[idx as usize].typ = typ;
                                            }
                                            if let Some(f) = tc.function {
                                                if let Some(n) = f.name {
                                                    tool_calls[idx as usize].function.name = n;
                                                }
                                                if let Some(a) = f.arguments {
                                                    tool_calls[idx as usize]
                                                        .function
                                                        .arguments
                                                        .push_str(&a);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let tool_calls_parsed: Option<Vec<ToolCall>> = if tool_calls.is_empty() {
            None
        } else {
            Some(
                tool_calls
                    .into_iter()
                    .map(|tc| ToolCall {
                        typ: tc.typ,
                        function: ToolCallFunction {
                            index: None,
                            name: tc.function.name,
                            arguments: serde_json::from_str(&tc.function.arguments)
                                .unwrap_or(serde_json::Value::Null),
                        },
                    })
                    .collect(),
            )
        };

        Ok(ChatResponse {
            message: Some(ChatMessage {
                role: "assistant".to_string(),
                content,
                tool_calls: tool_calls_parsed,
                tool_name: None,
            }),
            done: true,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LmStudioModel {
    pub name: String,
}

// --- Native API wire types (list models only) ---

#[derive(Debug, Deserialize)]
struct NativeModelsResponse {
    models: Option<Vec<NativeModelObject>>,
}

#[derive(Debug, Deserialize)]
struct NativeModelObject {
    key: Option<String>,
    #[serde(rename = "type")]
    typ: Option<String>,
}

// --- OpenAI wire types (for /v1/chat/completions) ---

#[derive(Debug, Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "role", rename_all = "snake_case")]
enum OpenAiMessage {
    System { content: String },
    User { content: String },
    Assistant {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<OpenAiToolCallRef>>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

#[derive(Debug, Serialize)]
struct OpenAiToolCallRef {
    id: String,
    #[serde(rename = "type")]
    typ: String,
    function: OpenAiToolCallFunctionRef,
}

#[derive(Debug, Serialize)]
struct OpenAiToolCallFunctionRef {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    typ: String,
    function: OpenAiToolFunction,
}

#[derive(Debug, Serialize)]
struct OpenAiToolFunction {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    parameters: serde_json::Value,
}

/// Convert internal messages to OpenAI format. Assigns tool_call_id per assistant tool_calls and matches following tool messages by order.
fn messages_to_openai(messages: &[ChatMessage]) -> (Vec<OpenAiMessage>, Vec<String>) {
    let mut out = Vec::with_capacity(messages.len());
    let mut pending_ids: Vec<String> = Vec::new();
    let mut pending_idx = 0;

    for m in messages {
        match m.role.as_str() {
            "system" => {
                out.push(OpenAiMessage::System {
                    content: m.content.clone(),
                });
            }
            "user" => {
                out.push(OpenAiMessage::User {
                    content: m.content.clone(),
                });
                pending_ids.clear();
                pending_idx = 0;
            }
            "assistant" => {
                let tool_calls = m.tool_calls.as_ref().map(|tcs| {
                    pending_ids.clear();
                    let mut id = pending_idx;
                    let refs: Vec<OpenAiToolCallRef> = tcs
                        .iter()
                        .map(|tc| {
                            let tid = format!("call_{}", id);
                            id += 1;
                            pending_ids.push(tid.clone());
                            let typ = if tc.typ.is_empty() {
                                "function".to_string()
                            } else {
                                tc.typ.clone()
                            };
                            OpenAiToolCallRef {
                                id: tid,
                                typ,
                                function: OpenAiToolCallFunctionRef {
                                    name: tc.function.name.clone(),
                                    arguments: serde_json::to_string(&tc.function.arguments)
                                        .unwrap_or_else(|_| "{}".to_string()),
                                },
                            }
                        })
                        .collect();
                    pending_idx = id;
                    refs
                });
                out.push(OpenAiMessage::Assistant {
                    content: m.content.clone(),
                    tool_calls,
                });
            }
            "tool" => {
                let id = if pending_ids.is_empty() {
                    let fallback = format!("call_{}", pending_idx);
                    pending_idx += 1;
                    fallback
                } else {
                    pending_ids.remove(0)
                };
                out.push(OpenAiMessage::Tool {
                    tool_call_id: id,
                    content: m.content.clone(),
                });
            }
            _ => {
                out.push(OpenAiMessage::User {
                    content: m.content.clone(),
                });
            }
        }
    }
    (out, pending_ids)
}

fn tool_definitions_to_openai(tools: Vec<ToolDefinition>) -> Vec<OpenAiTool> {
    tools
        .into_iter()
        .map(|t| OpenAiTool {
            typ: t.typ,
            function: OpenAiToolFunction {
                name: t.function.name,
                description: t.function.description,
                parameters: t.function.parameters,
            },
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Option<Vec<OpenAiChoice>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: Option<OpenAiResponseMessage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiResponseMessage {
    role: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<OpenAiResponseToolCall>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiResponseToolCall {
    id: Option<String>,
    #[serde(rename = "type")]
    typ: Option<String>,
    function: Option<OpenAiResponseToolCallFunction>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseToolCallFunction {
    name: Option<String>,
    arguments: Option<String>,
}

fn openai_response_to_chat_response(data: OpenAiChatResponse) -> Result<ChatResponse, LmStudioError> {
    let message = data
        .choices
        .and_then(|c| c.into_iter().next())
        .and_then(|c| c.message);
    let (content, tool_calls) = match message {
        Some(m) => {
            let content = m.content.unwrap_or_default();
            let tool_calls = m.tool_calls.map(|tcs| {
                tcs.into_iter()
                    .filter_map(|tc| {
                        tc.function.as_ref().and_then(|f| {
                            f.name.as_ref().map(|name| ToolCall {
                                typ: tc.typ.unwrap_or_else(|| "function".to_string()),
                                function: ToolCallFunction {
                                    index: None,
                                    name: name.clone(),
                                    arguments: tc
                                        .function
                                        .as_ref()
                                        .and_then(|f| f.arguments.as_ref())
                                        .and_then(|s| serde_json::from_str(s).ok())
                                        .unwrap_or(serde_json::Value::Null),
                                },
                            })
                        })
                    })
                    .collect()
            });
            (content, tool_calls)
        }
        None => (String::new(), None),
    };
    Ok(ChatResponse {
        message: Some(ChatMessage {
            role: "assistant".to_string(),
            content,
            tool_calls,
            tool_name: None,
        }),
        done: true,
    })
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    choices: Option<Vec<OpenAiStreamChoice>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    delta: Option<OpenAiStreamDelta>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamDelta {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAiStreamDeltaToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamDeltaToolCall {
    index: Option<u32>,
    id: Option<String>,
    #[serde(rename = "type")]
    typ: Option<String>,
    function: Option<OpenAiStreamDeltaToolCallFunction>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamDeltaToolCallFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Default)]
struct OpenAiStreamToolCall {
    id: String,
    typ: String,
    function: OpenAiStreamToolCallFunction,
}

#[derive(Debug, Default)]
struct OpenAiStreamToolCallFunction {
    name: String,
    arguments: String,
}
