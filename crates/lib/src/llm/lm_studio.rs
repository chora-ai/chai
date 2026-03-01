//! LM Studio client: OpenAI-compatible API or native API.
//!
//! Endpoint type is set at construction. **OpenAI** uses /v1/models and /v1/chat/completions
//! (supports tools). **Native** uses /api/v1/models and /api/v1/chat (no custom tools in this implementation).

use crate::config::LmStudioEndpointType;
use crate::llm::{ChatMessage, ChatResponse, ToolCall, ToolCallFunction, ToolDefinition};
use anyhow::Result;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:1234/v1";

/// Client for LM Studio: OpenAI-compat or native API depending on endpoint type.
#[derive(Clone)]
pub struct LmStudioClient {
    base_url: String,
    endpoint_type: LmStudioEndpointType,
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
    pub fn new(base_url: Option<String>, endpoint_type: LmStudioEndpointType) -> Self {
        let base_url = base_url
            .map(|u| u.trim_end_matches('/').to_string())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        Self {
            base_url,
            endpoint_type,
            client: reqwest::Client::new(),
        }
    }

    /// Base URL as configured (OpenAI-compat base for openai, or server root for native).
    fn server_root(&self) -> String {
        if self.endpoint_type == LmStudioEndpointType::Native && self.base_url.ends_with("/v1") {
            self.base_url.trim_end_matches("/v1").to_string()
        } else {
            self.base_url.clone()
        }
    }

    /// List available models (OpenAI /v1/models or native /api/v1/models).
    pub async fn list_models(&self) -> Result<Vec<LmStudioModel>, LmStudioError> {
        match self.endpoint_type {
            LmStudioEndpointType::Openai => self.list_models_openai().await,
            LmStudioEndpointType::Native => self.list_models_native().await,
        }
    }

    /// GET /v1/models — list available models (OpenAI-compat).
    async fn list_models_openai(&self) -> Result<Vec<LmStudioModel>, LmStudioError> {
        let url = format!("{}/models", self.base_url);
        let res = self.client.get(&url).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(LmStudioError::Api(format!("{} {}", status, body)));
        }
        let data: OpenAiModelsResponse = res.json().await?;
        Ok(data
            .data
            .unwrap_or_default()
            .into_iter()
            .map(|m| LmStudioModel { name: m.id })
            .collect())
    }

    /// GET /api/v1/models — list available models (native).
    async fn list_models_native(&self) -> Result<Vec<LmStudioModel>, LmStudioError> {
        let root = self.server_root();
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
            .map(|m| LmStudioModel {
                name: m.key.clone().unwrap_or_default(),
            })
            .collect())
    }

    /// Non-streaming chat. OpenAI: tools supported. Native: no custom tools, message content only.
    pub async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        _stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, LmStudioError> {
        match self.endpoint_type {
            LmStudioEndpointType::Openai => self.chat_openai(model, &messages, tools).await,
            LmStudioEndpointType::Native => self.chat_native(model, &messages).await,
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

    /// POST /api/v1/chat — non-streaming chat (native). Tools are ignored; only message content is returned.
    async fn chat_native(&self, model: &str, messages: &[ChatMessage]) -> Result<ChatResponse, LmStudioError> {
        let root = self.server_root();
        let url = format!("{}/api/v1/chat", root);
        let (system_prompt, input) = messages_to_native_input(messages);
        let body = NativeChatRequest {
            model: model.to_string(),
            input,
            system_prompt: system_prompt.or_else(|| Some(String::new())),
            stream: false,
        };
        let res = self.client.post(&url).json(&body).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(LmStudioError::Api(format!("{} {}", status, body)));
        }
        let data: NativeChatResponse = res.json().await?;
        native_response_to_chat_response(data)
    }

    /// Streaming chat. OpenAI: SSE. Native: single call then one on_chunk with full content.
    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, LmStudioError> {
        match self.endpoint_type {
            LmStudioEndpointType::Openai => {
                self.chat_stream_openai(model, messages, tools, on_chunk)
                    .await
            }
            LmStudioEndpointType::Native => {
                let out = self.chat_native(model, &messages).await?;
                if let Some(ref msg) = out.message {
                    if !msg.content.is_empty() {
                        on_chunk(&msg.content);
                    }
                }
                Ok(out)
            }
        }
    }

    /// POST /v1/chat/completions with stream: true (OpenAI-compat).
    async fn chat_stream_openai(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, LmStudioError> {
        let url = format!("{}/chat/completions", self.base_url);
        let (openai_messages, _) = messages_to_openai(&messages);
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

// --- Native API wire types ---

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

#[derive(Debug, Serialize)]
struct NativeChatRequest {
    model: String,
    input: Vec<NativeInputItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_prompt: Option<String>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct NativeInputItem {
    #[serde(rename = "type")]
    typ: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct NativeChatResponse {
    output: Option<Vec<NativeOutputItem>>,
}

#[derive(Debug, Deserialize)]
struct NativeOutputItem {
    #[serde(rename = "type")]
    typ: Option<String>,
    content: Option<String>,
}

fn messages_to_native_input(messages: &[ChatMessage]) -> (Option<String>, Vec<NativeInputItem>) {
    let mut system_prompt: Option<String> = None;
    let mut input = Vec::new();
    for m in messages {
        match m.role.as_str() {
            "system" => {
                if system_prompt.is_none() {
                    system_prompt = Some(m.content.clone());
                }
            }
            _ => {
                let content = if m.role == "tool" {
                    format!("[Tool result] {}", m.content)
                } else {
                    m.content.clone()
                };
                if !content.is_empty() || input.is_empty() {
                    input.push(NativeInputItem {
                        typ: "message".to_string(),
                        content,
                    });
                }
            }
        }
    }
    (system_prompt, input)
}

fn native_response_to_chat_response(data: NativeChatResponse) -> Result<ChatResponse, LmStudioError> {
    let content: String = data
        .output
        .unwrap_or_default()
        .into_iter()
        .filter_map(|o| {
            if o.typ.as_deref() == Some("message") {
                o.content
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("");
    Ok(ChatResponse {
        message: Some(ChatMessage {
            role: "assistant".to_string(),
            content,
            tool_calls: None,
            tool_name: None,
        }),
        done: true,
    })
}

// --- OpenAI wire types (for LM Studio OpenAI-compat endpoint) ---

#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Option<Vec<OpenAiModelObject>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelObject {
    id: String,
}

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
