//! Shared OpenAI-compatible HTTP client (chat completions, streaming, list models).
//!
//! This is the wire implementation for the `openai-compat` endpoint type. It supports
//! configurable behaviors:
//!
//! - **Model discovery**: standard `GET /v1/models`, LM Studio native `GET /api/v1/models`,
//!   or static model lists from config.
//! - **Auto-load**: On "unloaded" error (LM Studio), call `POST /api/v1/models/load` and
//!   retry the chat request once.

use crate::config::AutoLoad;
use crate::providers::{ChatMessage, ChatResponse, FinishReason, Provider, ProviderError, ToolCall, ToolCallFunction, ToolDefinition, Usage};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum OpenAiCompatError {
    #[error("openai-compat request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("openai-compat api error: {0}")]
    Api(String),
}

/// HTTP client for `POST /v1/chat/completions`, `GET /v1/models`, and optional LM Studio
/// auto-load (`POST /api/v1/models/load`) and native model list (`GET /api/v1/models`).
#[derive(Clone)]
pub struct OpenAiCompatClient {
    base_url: String,
    client: reqwest::Client,
    api_key: Option<String>,
    auto_load: AutoLoad,
}

impl OpenAiCompatClient {
    /// Constructor that accepts auto-load config for LM Studio behavior.
    pub fn new_with_auto_load(base_url: String, api_key: Option<String>, auto_load: AutoLoad) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        let api_key = api_key
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        Self {
            base_url,
            client: reqwest::Client::new(),
            api_key,
            auto_load,
        }
    }

    /// Constructor for direct use as an `openai-compat` endpoint type provider (no auto-load).
    pub fn new_adapter(base_url: String, api_key: Option<String>) -> Self {
        Self::new_with_auto_load(base_url, api_key, AutoLoad::None)
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

    /// Root URL for the LM Studio server (no /v1 suffix). Used for `/api/v1/models` and
    /// `/api/v1/models/load` (LM Studio native endpoints that sit outside /v1).
    fn api_server_root(&self) -> String {
        if self.base_url.ends_with("/v1") {
            self.base_url.trim_end_matches("/v1").to_string()
        } else {
            self.base_url.to_string()
        }
    }

    // --- Model discovery ---

    /// `GET /v1/models` — OpenAI list models response (`data[].id`).
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

    /// `GET /api/v1/models` — LM Studio native model list. Filters `type == "llm"` and
    /// uses `key` as the model id (compatible with `/v1/chat/completions`).
    pub async fn list_models_lmstudio(&self) -> Result<Vec<String>, OpenAiCompatError> {
        let root = self.api_server_root();
        let url = format!("{}/api/v1/models", root);
        let res = self.client.get(&url).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(OpenAiCompatError::Api(format!("{} {}", status, body)));
        }
        let data: LmStudioNativeModelsResponse = res.json().await?;
        Ok(data
            .models
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.typ.as_deref() == Some("llm"))
            .filter(|m| {
                let k = m.key.as_deref().unwrap_or("").trim();
                !k.is_empty() && !k.starts_with("text-embedding-")
            })
            .map(|m| m.key.unwrap_or_default().trim().to_string())
            .collect())
    }

    // --- Auto-load ---

    /// Ensure the model is loaded in LM Studio before chat. Calls `POST /api/v1/models/load`
    /// with only the model id for compatibility.
    async fn ensure_model_loaded(&self, model: &str) -> Result<(), OpenAiCompatError> {
        let root = self.api_server_root();
        let url = format!("{}/api/v1/models/load", root);
        let body = serde_json::json!({ "model": model });
        let res = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(OpenAiCompatError::Api(format!("{} {}", status, body)));
        }
        Ok(())
    }

    /// Returns true if the error message indicates an "unloaded" model (LM Studio).
    fn is_unloaded_error(msg: &str) -> bool {
        msg.to_lowercase().contains("unloaded")
    }

    // --- Chat ---

    /// Non-streaming chat via `/v1/chat/completions`. When `autoLoad` is `"lmstudio"` and the
    /// error indicates "unloaded", loads the model and retries once.
    pub async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        _stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, OpenAiCompatError> {
        match self.chat_openai(model, &messages, tools.clone()).await {
            Ok(r) => Ok(r),
            Err(e) => {
                if self.auto_load == AutoLoad::Lmstudio && Self::is_unloaded_error(&e.to_string()) {
                    self.ensure_model_loaded(model).await?;
                    self.chat_openai(model, &messages, tools).await
                } else {
                    Err(e)
                }
            }
        }
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

    /// Streaming chat via `/v1/chat/completions` with `stream: true`. When `autoLoad` is
    /// `"lmstudio"` and the error indicates "unloaded", loads the model and retries once with
    /// a single non-streaming call (to avoid invoking `on_chunk` twice if partial data was
    /// already streamed).
    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, OpenAiCompatError> {
        match self.chat_stream_openai(model, &messages, tools.clone(), on_chunk).await {
            Ok(r) => Ok(r),
            Err(e) => {
                if self.auto_load == AutoLoad::Lmstudio && Self::is_unloaded_error(&e.to_string()) {
                    self.ensure_model_loaded(model).await?;
                    // Retry with a single non-streaming call so we don't invoke on_chunk twice.
                    let out = self.chat_openai(model, &messages, tools).await?;
                    if let Some(ref m) = out.message {
                        if !m.content.is_empty() {
                            on_chunk(&m.content);
                        }
                    }
                    Ok(out)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn chat_stream_openai(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, OpenAiCompatError> {
        let url = format!("{}/chat/completions", self.base_url);
        let (openai_messages, _) = messages_to_openai(messages);
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
        let mut finish_reason: Option<FinishReason> = None;

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
                            if let Some(fr) = choice.finish_reason {
                                finish_reason = Some(fr);
                            }
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
            finish_reason,
            eval_count: None,
            prompt_eval_count: None,
            usage: None,
        })
    }
}

// --- LM Studio native model list wire types ---

#[derive(Debug, Deserialize)]
struct LmStudioNativeModelsResponse {
    models: Option<Vec<LmStudioNativeModelObject>>,
}

#[derive(Debug, Deserialize)]
struct LmStudioNativeModelObject {
    key: Option<String>,
    #[serde(rename = "type")]
    typ: Option<String>,
}

// --- OpenAI wire types ---

#[derive(Debug, Deserialize)]
struct OpenAiListModelsResponse {
    data: Option<Vec<OpenAiModelId>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelId {
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
    System {
        content: String,
    },
    User {
        content: String,
    },
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
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: Option<OpenAiResponseMessage>,
    finish_reason: Option<FinishReason>,
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

fn openai_response_to_chat_response(
    data: OpenAiChatResponse,
) -> Result<ChatResponse, OpenAiCompatError> {
    let choice = data.choices.and_then(|c| c.into_iter().next());
    let finish_reason = choice.as_ref().and_then(|c| c.finish_reason.clone());
    let message = choice.and_then(|c| c.message);
    let usage = data.usage.map(|u| Usage {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
    });
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
        finish_reason,
        eval_count: None,
        prompt_eval_count: None,
        usage,
    })
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    choices: Option<Vec<OpenAiStreamChoice>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    delta: Option<OpenAiStreamDelta>,
    finish_reason: Option<FinishReason>,
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


#[cfg(test)]
mod tests {
    use super::*;

    // --- OpenAI non-streaming: finish_reason propagation via openai_response_to_chat_response ---

    #[test]
    fn openai_response_finish_reason_length() {
        let json = r#"{"choices":[{"message":{"role":"assistant","content":"cut off"},"finish_reason":"length"}]}"#;
        let data: OpenAiChatResponse = serde_json::from_str(json).unwrap();
        let resp = openai_response_to_chat_response(data).unwrap();
        assert!(resp.is_truncated());
        assert_eq!(resp.finish_reason, Some(FinishReason::Length));
        assert_eq!(resp.content(), "cut off");
    }

    #[test]
    fn openai_response_finish_reason_stop() {
        let json = r#"{"choices":[{"message":{"role":"assistant","content":"done"},"finish_reason":"stop"}]}"#;
        let data: OpenAiChatResponse = serde_json::from_str(json).unwrap();
        let resp = openai_response_to_chat_response(data).unwrap();
        assert!(!resp.is_truncated());
        assert_eq!(resp.finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn openai_response_finish_reason_tool_calls() {
        let json = r#"{
            "choices":[{
                "message":{
                    "role":"assistant",
                    "content":null,
                    "tool_calls":[{"id":"call_0","type":"function","function":{"name":"read_file","arguments":"{}"}}]
                },
                "finish_reason":"tool_calls"
            }]
        }"#;
        let data: OpenAiChatResponse = serde_json::from_str(json).unwrap();
        let resp = openai_response_to_chat_response(data).unwrap();
        assert!(!resp.is_truncated());
        assert_eq!(resp.finish_reason, Some(FinishReason::ToolCalls));
        assert_eq!(resp.tool_calls().len(), 1);
        assert_eq!(resp.tool_calls()[0].function.name, "read_file");
    }

    #[test]
    fn openai_response_no_finish_reason() {
        let json = r#"{"choices":[{"message":{"role":"assistant","content":"hello"}}]}"#;
        let data: OpenAiChatResponse = serde_json::from_str(json).unwrap();
        let resp = openai_response_to_chat_response(data).unwrap();
        assert!(!resp.is_truncated());
        assert_eq!(resp.finish_reason, None);
    }

    #[test]
    fn openai_response_no_choices() {
        let json = r#"{"choices":[]}"#;
        let data: OpenAiChatResponse = serde_json::from_str(json).unwrap();
        let resp = openai_response_to_chat_response(data).unwrap();
        assert!(!resp.is_truncated());
        assert_eq!(resp.finish_reason, None);
        assert_eq!(resp.content(), "");
    }

    // --- OpenAI streaming: OpenAiStreamChoice finish_reason deserialization ---

    #[test]
    fn openai_stream_choice_finish_reason_length() {
        let json = r#"{"choices":[{"delta":{"content":"partial"},"finish_reason":"length"}]}"#;
        let chunk: OpenAiStreamChunk = serde_json::from_str(json).unwrap();
        let choice = chunk.choices.unwrap().into_iter().next().unwrap();
        assert_eq!(choice.finish_reason, Some(FinishReason::Length));
    }

    #[test]
    fn openai_stream_choice_finish_reason_stop() {
        let json = r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#;
        let chunk: OpenAiStreamChunk = serde_json::from_str(json).unwrap();
        let choice = chunk.choices.unwrap().into_iter().next().unwrap();
        assert_eq!(choice.finish_reason, Some(FinishReason::Stop));
    }

    // --- Truncation with partial tool calls (the phantom edit scenario) ---

    #[test]
    fn openai_response_truncated_with_partial_tool_calls() {
        let json = r#"{
            "choices":[{
                "message":{
                    "role":"assistant",
                    "content":"I will write both files.",
                    "tool_calls":[{"id":"call_0","type":"function","function":{"name":"write_file","arguments":"{\"path\":\"a.txt\"}"}}]
                },
                "finish_reason":"length"
            }]
        }"#;
        let data: OpenAiChatResponse = serde_json::from_str(json).unwrap();
        let resp = openai_response_to_chat_response(data).unwrap();
        assert!(resp.is_truncated());
        assert_eq!(resp.tool_calls().len(), 1);
    }

    // --- AutoLoad ---

    #[test]
    fn is_unloaded_error_detects_unloaded() {
        assert!(OpenAiCompatClient::is_unloaded_error("Model is unloaded"));
        assert!(OpenAiCompatClient::is_unloaded_error("error: model Unloaded"));
        assert!(!OpenAiCompatClient::is_unloaded_error("rate limit exceeded"));
    }
}

// --- Provider trait impl for OpenAiCompatClient ---

#[async_trait]
impl Provider for OpenAiCompatClient {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, ProviderError> {
        OpenAiCompatClient::chat(self, model, messages, stream, tools)
            .await
            .map_err(|e| ProviderError::Provider(e.to_string()))
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, ProviderError> {
        OpenAiCompatClient::chat_stream(self, model, messages, tools, on_chunk)
            .await
            .map_err(|e| ProviderError::Provider(e.to_string()))
    }
}
