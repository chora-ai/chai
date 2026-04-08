//! NVIDIA NIM hosted API client (OpenAI-compatible).
//!
//! Uses the free tier at https://integrate.api.nvidia.com. Requires an API key from
//! build.nvidia.com. This is not a privacy-preserving option; all requests are sent to NVIDIA.

use crate::config::Config;
use crate::providers::{ChatMessage, ChatResponse, ToolCall, ToolCallFunction, ToolDefinition};
use anyhow::Result;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

const NIM_BASE_URL: &str = "https://integrate.api.nvidia.com/v1";

/// Client for NVIDIA NIM hosted API (OpenAI-compat). Requires API key.
#[derive(Clone)]
pub struct NimClient {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

#[derive(Debug, Clone, Serialize)]
pub struct NimModel {
    pub name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum NimError {
    #[error("nim request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("nim api error: {0}")]
    Api(String),
}

impl NimClient {
    pub fn new(api_key: Option<String>) -> Self {
        let api_key = api_key
            .or_else(|| std::env::var("NVIDIA_API_KEY").ok())
            .unwrap_or_default()
            .trim()
            .to_string();
        Self {
            base_url: NIM_BASE_URL.trim_end_matches('/').to_string(),
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Returns a static list of known NIM model ids (NVIDIA does not expose a list endpoint).
    pub async fn list_models(&self) -> Result<Vec<NimModel>, NimError> {
        Ok(Self::static_model_list())
    }

    /// Static catalog plus optional [`crate::config::NimProviderEntry::extra_models`] from config.
    /// Deduped by exact id; sorted by name for stable **`status`** / UI.
    pub(crate) fn gateway_model_list(config: &Config) -> Vec<NimModel> {
        let mut names: Vec<String> = Self::static_model_list()
            .into_iter()
            .map(|m| m.name)
            .collect();
        if let Some(extra) = config
            .providers
            .as_ref()
            .and_then(|p| p.nim.as_ref())
            .and_then(|n| n.extra_models.as_ref())
        {
            for s in extra {
                let t = s.trim();
                if !t.is_empty() && !names.iter().any(|n| n == t) {
                    names.push(t.to_string());
                }
            }
        }
        names.sort();
        names.into_iter().map(|name| NimModel { name }).collect()
    }

    /// Returns the static list of known NIM model ids (for discovery/status). Not part of public API.
    /// Chosen to give a small, representative set for the UI: one or two chat/instruct models per
    /// major vendor (Meta, Mistral, Google, Qwen, Microsoft, NVIDIA), mix of small and large, so
    /// users can try NIM without maintaining the full catalog. Any model id from the NIM docs works
    /// when set in config or request even if not listed here.
    pub(crate) fn static_model_list() -> Vec<NimModel> {
        [
            "deepseek-ai/deepseek-v3.1",
            "deepseek-ai/deepseek-v3.1-terminus",
            "deepseek-ai/deepseek-v3.2",
            "meta/llama3-70b-instruct",
            "meta/llama-3.1-8b-instruct",
            "meta/llama-3.1-70b-instruct",
            "meta/llama-3.1-405b-instruct",
            "meta/llama-3.2-3b-instruct",
            "meta/llama-3.3-70b-instruct",
            "meta/llama-4-maverick-17b-128e-instruct",
            "qwen/qwen3-coder-480b-a35b-instruct",
            "qwen/qwen3-next-80b-a3b-instruct",
            "qwen/qwen3-next-80b-a3b-thinking",
            "qwen/qwen3.5-122b-a10b",
        ]
        .into_iter()
        .map(|s| NimModel {
            name: s.to_string(),
        })
        .collect()
    }

    fn auth_header(&self) -> Option<(String, String)> {
        if self.api_key.is_empty() {
            None
        } else {
            Some((
                "Authorization".to_string(),
                format!("Bearer {}", self.api_key),
            ))
        }
    }

    pub async fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        _stream: bool,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, NimError> {
        let url = format!("{}/chat/completions", self.base_url);
        let (openai_messages, _) = messages_to_openai(&messages);
        let body = OpenAiChatRequest {
            model: model.to_string(),
            messages: openai_messages,
            stream: false,
            tools: tools.map(tool_definitions_to_openai),
        };
        let mut req = self.client.post(&url).json(&body);
        if let Some((k, v)) = self.auth_header() {
            req = req.header(k, v);
        }
        let res = req.send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(NimError::Api(format!("{} {}", status, body)));
        }
        let data: OpenAiChatResponse = res.json().await?;
        openai_response_to_chat_response(data)
    }

    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        on_chunk: &mut (dyn for<'a> FnMut(&'a str) + Send),
    ) -> Result<ChatResponse, NimError> {
        let url = format!("{}/chat/completions", self.base_url);
        let (openai_messages, _) = messages_to_openai(&messages);
        let body = OpenAiChatRequest {
            model: model.to_string(),
            messages: openai_messages,
            stream: true,
            tools: tools.map(tool_definitions_to_openai),
        };
        let mut req = self.client.post(&url).json(&body);
        if let Some((k, v)) = self.auth_header() {
            req = req.header(k, v);
        }
        let res = req.send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(NimError::Api(format!("{} {}", status, body)));
        }
        let mut stream = res.bytes_stream();
        let mut buffer = Vec::new();
        let mut content = String::new();
        let mut tool_calls: Vec<OpenAiStreamToolCall> = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(NimError::Request)?;
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

// --- OpenAI wire types (same shape as LM Studio / OpenAI) ---

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
struct OpenAiResponseMessage {
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

fn openai_response_to_chat_response(data: OpenAiChatResponse) -> Result<ChatResponse, NimError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, NimProviderEntry, ProvidersConfig};

    #[test]
    fn gateway_model_list_appends_extra_models_and_dedupes() {
        let mut c = Config::default();
        c.providers = Some(ProvidersConfig {
            nim: Some(NimProviderEntry {
                extra_models: Some(vec![
                    "vendor/extra-model".to_string(),
                    "meta/llama-3.2-3b-instruct".to_string(),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        });
        let list = NimClient::gateway_model_list(&c);
        assert!(list.iter().any(|m| m.name == "vendor/extra-model"));
        assert_eq!(
            list.iter()
                .filter(|m| m.name == "meta/llama-3.2-3b-instruct")
                .count(),
            1
        );
    }
}
