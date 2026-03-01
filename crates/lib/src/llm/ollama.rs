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
        let res = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(OllamaError::Api(format!("{} {}", status, body)));
        }
        let data: ChatResponse = res.json().await?;
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
        let res = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(OllamaError::Api(format!("{} {}", status, body)));
        }
        let mut stream = res.bytes_stream();
        let mut buffer = Vec::new();
        let mut content = String::new();
        let mut last_message: Option<ChatMessage> = None;
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
                    return Ok(ChatResponse {
                        message: Some(message),
                        done: true,
                    });
                }
            }
        }
        Ok(ChatResponse {
            message: Some(ChatMessage {
                role: "assistant".to_string(),
                content,
                tool_calls: last_message.and_then(|m| m.tool_calls),
                tool_name: None,
            }),
            done: true,
        })
    }
}

#[derive(Debug, Deserialize)]
struct ChatStreamEvent {
    #[serde(default)]
    message: Option<ChatMessage>,
    #[serde(default)]
    done: bool,
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

#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    pub message: Option<ChatMessage>,
    #[serde(default)]
    pub done: bool,
}

impl ChatResponse {
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
}
