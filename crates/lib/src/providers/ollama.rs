//! Ollama API client (http://127.0.0.1:11434 by default).
//! Supports non-streaming and streaming chat (NDJSON).

use anyhow::Result;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:11434";

/// Client for Ollama HTTP API.
#[derive(Clone)]
pub struct OllamaClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum OllamaError {
    #[error("ollama request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("ollama api error: {0}")]
    Api(String),
}

impl OllamaClient {
    pub fn new(base_url: Option<String>) -> Self {
        let base_url = base_url
            .map(|u| u.trim_end_matches('/').to_string())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    /// GET /api/tags — list available models.
    pub async fn list_models(&self) -> Result<Vec<OllamaModel>, OllamaError> {
        let url = format!("{}/api/tags", self.base_url);
        let res = self.client.get(&url).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(OllamaError::Api(format!("{} {}", status, body)));
        }
        let data: TagsResponse = res.json().await?;
        Ok(data.models.unwrap_or_default())
    }

    /// POST /api/chat — non-streaming chat completion. Optionally pass tools for function calling.
    pub async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, OllamaError> {
        let url = format!("{}/api/chat", self.base_url);
        let body = ChatRequest {
            model: model.to_string(),
            messages,
            stream,
            tools,
        };
        let res = self.client.post(&url).json(&body).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(OllamaError::Api(format!("{} {}", status, body)));
        }
        let mut data: ChatResponse = res.json().await?;
        data.resolve_usage();
        Ok(data)
    }

    /// POST /api/chat with stream: true. Parses NDJSON and calls on_chunk for each content delta; returns accumulated message and done.
    /// Tool calls are taken from the last chunk that contains them.
    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, OllamaError> {
        let url = format!("{}/api/chat", self.base_url);
        let body = ChatRequest {
            model: model.to_string(),
            messages,
            stream: true,
            tools,
        };
        let res = self.client.post(&url).json(&body).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(OllamaError::Api(format!("{} {}", status, body)));
        }
        let mut stream = res.bytes_stream();
        let mut buffer = Vec::new();
        let mut content = String::new();
        let mut last_message: Option<ChatMessage> = None;
        let mut finish_reason: Option<FinishReason> = None;
        let mut eval_count: Option<u64> = None;
        let mut prompt_eval_count: Option<u64> = None;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(OllamaError::Request)?;
            buffer.extend_from_slice(&chunk);
            while let Some(i) = buffer.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = buffer.drain(..i).collect();
                buffer.drain(..1);
                let line = String::from_utf8_lossy(&line_bytes).trim().to_string();
                if line.is_empty() {
                    continue;
                }
                let event: ChatStreamEvent = match serde_json::from_str(&line) {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                if let Some(fr) = event.finish_reason {
                    finish_reason = Some(fr);
                }
                if event.eval_count.is_some() {
                    eval_count = event.eval_count;
                }
                if event.prompt_eval_count.is_some() {
                    prompt_eval_count = event.prompt_eval_count;
                }
                if let Some(ref msg) = event.message {
                    if !msg.content.is_empty() {
                        on_chunk(&msg.content);
                        content.push_str(&msg.content);
                    }
                    if msg.tool_calls.is_some() {
                        last_message = Some(msg.clone());
                    }
                }
                if event.done {
                    let message = last_message.take().unwrap_or_else(|| ChatMessage {
                        role: "assistant".to_string(),
                        content: content.clone(),
                        tool_calls: None,
                        tool_name: None,
                    });
                    let message = ChatMessage {
                        content: content.clone(),
                        tool_calls: message.tool_calls,
                        ..message
                    };
                    let mut resp = ChatResponse {
                        message: Some(message),
                        done: true,
                        finish_reason,
                        eval_count,
                        prompt_eval_count,
                        usage: None,
                    };
                    resp.resolve_usage();
                    return Ok(resp);
                }
            }
        }
        let mut resp = ChatResponse {
            message: Some(ChatMessage {
                role: "assistant".to_string(),
                content,
                tool_calls: last_message.and_then(|m| m.tool_calls),
                tool_name: None,
            }),
            done: true,
            finish_reason,
            eval_count,
            prompt_eval_count,
            usage: None,
        };
        resp.resolve_usage();
        Ok(resp)
    }
}

#[derive(Debug, Deserialize)]
struct ChatStreamEvent {
    #[serde(default)]
    message: Option<ChatMessage>,
    #[serde(default)]
    done: bool,
    /// Ollama reports `done_reason` on the final chunk (e.g. "length", "stop", "load").
    #[serde(default, rename = "done_reason")]
    finish_reason: Option<FinishReason>,
    /// Number of tokens in the completion (Ollama final chunk).
    #[serde(default, rename = "eval_count")]
    eval_count: Option<u64>,
    /// Number of tokens in the prompt (Ollama final chunk).
    #[serde(default, rename = "prompt_eval_count")]
    prompt_eval_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Option<Vec<OllamaModel>>,
}

/// One tool/function call in an assistant message (Ollama format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    #[serde(rename = "type", default)]
    pub typ: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    #[serde(default)]
    pub index: Option<u32>,
    pub name: String,
    /// Arguments as JSON object or string (model-dependent).
    #[serde(default)]
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// When role is "tool", the name of the tool this result is for (Ollama expects "tool_name").
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "tool_name")]
    pub tool_name: Option<String>,
}

/// Tool definition for Ollama chat (function-calling).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub typ: String,
    pub function: ToolFunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunctionDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDefinition>>,
}

/// Reason the model finished generating, if reported by the provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinishReason {
    /// Model finished normally (natural stop or end-of-sequence token).
    #[serde(rename = "stop")]
    Stop,
    /// Model hit the maximum output token limit and was truncated.
    #[serde(rename = "length")]
    Length,
    /// Model triggered a tool call (some providers use this instead of "stop" when tool calls are present).
    #[serde(rename = "tool_calls")]
    ToolCalls,
    /// Any other/unknown reason string from the provider.
    #[serde(untagged)]
    Other(String),
}

impl FinishReason {
    /// Returns `true` if the model's output was truncated due to hitting the token limit.
    pub fn is_truncated(&self) -> bool {
        matches!(self, FinishReason::Length)
    }
}

impl std::fmt::Display for FinishReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FinishReason::Stop => write!(f, "stop"),
            FinishReason::Length => write!(f, "length"),
            FinishReason::ToolCalls => write!(f, "tool_calls"),
            FinishReason::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Token usage information from the provider response. Not all providers
/// return this data, so all fields are optional.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Usage {
    /// Number of tokens in the prompt (input). Ollama calls this `prompt_eval_count`.
    pub prompt_tokens: Option<u64>,
    /// Number of tokens in the completion (output). Ollama calls this `eval_count`.
    pub completion_tokens: Option<u64>,
    /// Total tokens (prompt + completion), when provided directly by the provider.
    pub total_tokens: Option<u64>,
}

impl std::fmt::Display for Usage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.prompt_tokens, self.completion_tokens) {
            (Some(p), Some(c)) => write!(f, "prompt={}, completion={}", p, c),
            (Some(p), None) => write!(f, "prompt={}, completion=?", p),
            (None, Some(c)) => write!(f, "prompt=?, completion={}", c),
            (None, None) => write!(f, "prompt=?, completion=?"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    pub message: Option<ChatMessage>,
    #[serde(default)]
    pub done: bool,
    /// Why the model stopped generating. Ollama reports `done_reason`; OpenAI-compatible
    /// providers report `finish_reason`. `None` when the provider does not expose this field.
    #[serde(rename = "done_reason", default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
    /// Ollama: number of tokens in the completion output.
    #[serde(rename = "eval_count", default, skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<u64>,
    /// Ollama: number of tokens in the prompt input.
    #[serde(rename = "prompt_eval_count", default, skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<u64>,
    /// Token usage information. Populated from `usage` (OpenAI-compatible) or from
    /// `eval_count`/`prompt_eval_count` (Ollama) via `resolve_usage()`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

impl ChatResponse {
    /// Resolves usage information from provider-specific fields.
    /// For Ollama: populates `usage` from `eval_count`/`prompt_eval_count` if not already set.
    /// For OpenAI-compatible: `usage` is already populated from the response.
    pub fn resolve_usage(&mut self) {
        if self.usage.is_none() && (self.eval_count.is_some() || self.prompt_eval_count.is_some()) {
            self.usage = Some(Usage {
                prompt_tokens: self.prompt_eval_count,
                completion_tokens: self.eval_count,
                total_tokens: None,
            });
        }
    }

    /// Text content of the assistant message, if any.
    pub fn content(&self) -> &str {
        self.message
            .as_ref()
            .map(|m| m.content.as_str())
            .unwrap_or("")
    }

    /// Parsed tool/function calls from the assistant message, if any.
    pub fn tool_calls(&self) -> &[ToolCall] {
        self.message
            .as_ref()
            .and_then(|m| m.tool_calls.as_deref())
            .unwrap_or(&[])
    }

    /// Returns `true` if the model's output was truncated due to hitting the token limit.
    pub fn is_truncated(&self) -> bool {
        self.finish_reason.as_ref().map_or(false, |r| r.is_truncated())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    // --- FinishReason deserialization ---

    #[test]
    fn finish_reason_deserialize_stop() {
        let fr: FinishReason = serde_json::from_str(r#""stop""#).unwrap();
        assert_eq!(fr, FinishReason::Stop);
        assert!(!fr.is_truncated());
    }

    #[test]
    fn finish_reason_deserialize_length() {
        let fr: FinishReason = serde_json::from_str(r#""length""#).unwrap();
        assert_eq!(fr, FinishReason::Length);
        assert!(fr.is_truncated());
    }

    #[test]
    fn finish_reason_deserialize_tool_calls() {
        let fr: FinishReason = serde_json::from_str(r#""tool_calls""#).unwrap();
        assert_eq!(fr, FinishReason::ToolCalls);
        assert!(!fr.is_truncated());
    }

    #[test]
    fn finish_reason_deserialize_unknown_falls_back_to_other() {
        let fr: FinishReason = serde_json::from_str(r#""content_filter""#).unwrap();
        assert!(matches!(fr, FinishReason::Other(ref s) if s == "content_filter"));
        assert!(!fr.is_truncated());
    }

    #[test]
    fn finish_reason_display() {
        assert_eq!(FinishReason::Stop.to_string(), "stop");
        assert_eq!(FinishReason::Length.to_string(), "length");
        assert_eq!(FinishReason::ToolCalls.to_string(), "tool_calls");
        assert_eq!(FinishReason::Other("content_filter".to_string()).to_string(), "content_filter");
    }

    // --- ChatResponse.is_truncated() ---

    #[test]
    fn chat_response_is_truncated_when_length() {
        let resp = ChatResponse {
            message: None,
            done: true,
            finish_reason: Some(FinishReason::Length),
            eval_count: None,
            prompt_eval_count: None,
            usage: None,
        };
        assert!(resp.is_truncated());
    }

    #[test]
    fn chat_response_not_truncated_when_stop() {
        let resp = ChatResponse {
            message: None,
            done: true,
            finish_reason: Some(FinishReason::Stop),
            eval_count: None,
            prompt_eval_count: None,
            usage: None,
        };
        assert!(!resp.is_truncated());
    }

    #[test]
    fn chat_response_not_truncated_when_none() {
        let resp = ChatResponse {
            message: None,
            done: true,
            finish_reason: None,
            eval_count: None,
            prompt_eval_count: None,
            usage: None,
        };
        assert!(!resp.is_truncated());
    }

    // --- Ollama non-streaming: done_reason deserialization ---

    #[test]
    fn ollama_chat_response_deserialize_done_reason_length() {
        // Simulates Ollama's non-streaming /api/chat response with done_reason: "length"
        let json = r#"{"message":{"role":"assistant","content":"partial"},"done":true,"done_reason":"length"}"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert!(resp.is_truncated());
        assert_eq!(resp.finish_reason, Some(FinishReason::Length));
        assert_eq!(resp.content(), "partial");
    }

    #[test]
    fn ollama_chat_response_deserialize_done_reason_stop() {
        let json = r#"{"message":{"role":"assistant","content":"hello"},"done":true,"done_reason":"stop"}"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert!(!resp.is_truncated());
        assert_eq!(resp.finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn ollama_chat_response_deserialize_no_done_reason() {
        // Older Ollama versions or responses that omit done_reason
        let json = r#"{"message":{"role":"assistant","content":"hello"},"done":true}"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert!(!resp.is_truncated());
        assert_eq!(resp.finish_reason, None);
    }

    // --- Ollama streaming: ChatStreamEvent done_reason ---

    #[test]
    fn ollama_stream_event_done_reason_length() {
        let json = r#"{"message":{"role":"assistant","content":""},"done":true,"done_reason":"length"}"#;
        let event: ChatStreamEvent = serde_json::from_str(json).unwrap();
        assert!(event.done);
        assert_eq!(event.finish_reason, Some(FinishReason::Length));
    }

    #[test]
    fn ollama_stream_event_no_done_reason() {
        let json = r#"{"message":{"role":"assistant","content":"hi"},"done":false}"#;
        let event: ChatStreamEvent = serde_json::from_str(json).unwrap();
        assert!(!event.done);
        assert_eq!(event.finish_reason, None);
    }
}
