//! Shared OpenAI-compatible HTTP client (chat completions, streaming, list models).
//! This module is the **wire implementation** used by several named providers (`lms`, `vllm`, `openai`, `hf`).
//! It is **not** merged with [`super::openai::OpenAiClient`]: that crate module supplies OpenAI-specific defaults and the [`crate::providers::Provider`] impl; this module stays provider-agnostic so one implementation serves every OpenAI-shaped backend.

use crate::providers::{ChatMessage, ChatResponse, ToolCall, ToolCallFunction, ToolDefinition};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum OpenAiCompatError {
    #[error("openai-compat request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("openai-compat api error: {0}")]
    Api(String),
}

/// HTTP client for `POST /v1/chat/completions` and `GET /v1/models` (OpenAI-compatible servers).
#[derive(Clone)]
pub struct OpenAiCompatClient {
    base_url: String,
    client: reqwest::Client,
    api_key: Option<String>,
}

impl OpenAiCompatClient {
    pub fn new(base_url: Option<String>, default_base: &'static str, api_key: Option<String>) -> Self {
        let base_url = base_url
            .map(|u| u.trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_base.to_string());
        let api_key = api_key
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        Self {
            base_url,
            client: reqwest::Client::new(),
            api_key,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn http_client(&self) -> &reqwest::Client {
        &self.client
    }

    fn apply_auth(&self, mut req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }
        req
    }

    /// GET `/v1/models` — OpenAI list models response (`data[].id`).
    pub async fn list_models_openai(&self) -> Result<Vec<String>, OpenAiCompatError> {
        let url = format!("{}/models", self.base_url);
        let req = self.client.get(&url);
        let res = self.apply_auth(req).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(OpenAiCompatError::Api(format!("{} {}", status, body)));
        }
        let data: OpenAiListModelsResponse = res.json().await?;
        Ok(data
            .data
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.id)
            .collect())
    }

    /// Non-streaming chat via `/v1/chat/completions`.
    pub async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        _stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, OpenAiCompatError> {
        self.chat_openai(model, &messages, tools).await
    }

    async fn chat_openai(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, OpenAiCompatError> {
        let url = format!("{}/chat/completions", self.base_url);
        let (openai_messages, _) = messages_to_openai(messages);
        let body = OpenAiChatRequest {
            model: model.to_string(),
            messages: openai_messages,
            stream: false,
            tools: tools.map(tool_definitions_to_openai),
        };
        let req = self.client.post(&url).json(&body);
        let res = self.apply_auth(req).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(OpenAiCompatError::Api(format!("{} {}", status, body)));
        }
        let data: OpenAiChatResponse = res.json().await?;
        openai_response_to_chat_response(data)
    }

    /// Streaming chat via `/v1/chat/completions` with `stream: true`.
    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, OpenAiCompatError> {
        let url = format!("{}/chat/completions", self.base_url);
        let (openai_messages, _) = messages_to_openai(&messages);
        let body = OpenAiChatRequest {
            model: model.to_string(),
            messages: openai_messages,
            stream: true,
            tools: tools.map(tool_definitions_to_openai),
        };
        let req = self.client.post(&url).json(&body);
        let res = self.apply_auth(req).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(OpenAiCompatError::Api(format!("{} {}", status, body)));
        }
        let mut stream = res.bytes_stream();
        let mut buffer = Vec::new();
        let mut content = String::new();
        let mut tool_calls: Vec<OpenAiStreamToolCall> = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(OpenAiCompatError::Request)?;
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

#[derive(Debug, Deserialize)]
struct OpenAiListModelsResponse {
    data: Option<Vec<OpenAiModelId>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelId {
    id: String,
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

fn openai_response_to_chat_response(data: OpenAiChatResponse) -> Result<ChatResponse, OpenAiCompatError> {
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
