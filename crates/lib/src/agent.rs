//! Agent turn: load session history, call LLM (Ollama or LM Studio), append reply.
//! Supports optional tools: when the model returns tool_calls, we execute them and re-call the model until done.

use crate::llm::{ChatMessage, LlmBackend, LlmError, ToolCall, ToolDefinition};
use crate::session::SessionStore;

const MAX_TOOL_LOOP: usize = 5;

/// Result of one agent turn: text content and any parsed tool/function calls (from the final message).
#[derive(Debug, Clone)]
pub struct AgentTurnResult {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

/// Executes a tool by name and JSON arguments. Returns output or error string.
pub trait ToolExecutor: Send + Sync {
    fn execute(&self, name: &str, args: &serde_json::Value) -> Result<String, String>;
}

/// Run one agent turn: load session messages, call the given LLM backend (streaming when on_chunk is Some); if tools are provided and the model returns tool_calls, execute them and re-call until no more tool_calls or max iterations.
/// `model` is the backend-specific model name (no prefix; e.g. `llama3.2:latest` for Ollama, `gpt-oss-20b` for LM Studio).
pub async fn run_turn<B: LlmBackend>(
    store: &SessionStore,
    session_id: &str,
    backend: &B,
    model: &str,
    system_context: Option<&str>,
    tools: Option<Vec<ToolDefinition>>,
    tool_executor: Option<&dyn ToolExecutor>,
    mut on_chunk: Option<&mut (dyn FnMut(&str) + Send)>,
) -> Result<AgentTurnResult, LlmError> {
    let session = store
        .get(session_id)
        .await
        .ok_or_else(|| LlmError::Session("session not found".to_string()))?;

    let mut messages: Vec<ChatMessage> = session
        .messages
        .iter()
        .map(|m| ChatMessage {
            role: m.role.clone(),
            content: m.content.clone(),
            tool_calls: m.tool_calls.clone(),
            tool_name: m.tool_name.clone(),
        })
        .collect();

    if let Some(ctx) = system_context {
        if !ctx.trim().is_empty() {
            messages.insert(
                0,
                ChatMessage {
                    role: "system".to_string(),
                    content: ctx.to_string(),
                    tool_calls: None,
                    tool_name: None,
                },
            );
        }
    }

    let model_name = model.trim();
    let model_name = if model_name.is_empty() {
        log::warn!("agent: configured model was empty, using fallback");
        "llama3.2:latest"
    } else {
        model_name
    };
    log::info!("agent: using model {}", model_name);
    let tools_ref = tools.as_ref();
    let mut loop_count = 0;
    let mut last_content;
    let mut last_tool_calls;

    loop {
        let use_stream = on_chunk.is_some() && loop_count == 0;
        let res = if use_stream {
            let cb = on_chunk.as_mut().unwrap();
            let mut delta_cb = |s: &str| cb(s);
            backend
                .chat_stream(model_name, messages.clone(), tools_ref.cloned(), &mut delta_cb)
                .await?
        } else {
            backend
                .chat(model_name, messages.clone(), false, tools_ref.cloned())
                .await?
        };
        last_content = res.content().to_string();
        last_tool_calls = res.tool_calls().to_vec();

        let assistant_msg = ChatMessage {
            role: "assistant".to_string(),
            content: last_content.clone(),
            tool_calls: if last_tool_calls.is_empty() {
                None
            } else {
                Some(last_tool_calls.clone())
            },
            tool_name: None,
        };

        store
            .append_message_full(
                session_id,
                "assistant",
                &assistant_msg.content,
                assistant_msg.tool_calls.clone(),
                None,
            )
            .await
            .map_err(|e| LlmError::Session(e.to_string()))?;

        if last_tool_calls.is_empty() {
            break;
        }

        loop_count += 1;
        if loop_count >= MAX_TOOL_LOOP {
            log::debug!("agent: max tool loop iterations reached");
            break;
        }

        let executor = match tool_executor {
            Some(e) => e,
            None => {
                log::debug!("agent: tool_calls returned but no executor");
                break;
            }
        };

        messages.push(assistant_msg);
        for call in &last_tool_calls {
            let name = call.function.name.as_str();
            let args = &call.function.arguments;
            let result = match executor.execute(name, args) {
                Ok(out) => out.clone(),
                Err(e) => {
                    log::warn!("agent: tool {} failed: {}", name, e);
                    format!("error: {}", e)
                }
            };
            messages.push(ChatMessage {
                role: "tool".to_string(),
                content: result.clone(),
                tool_calls: None,
                tool_name: Some(name.to_string()),
            });
            store
                .append_message_full(session_id, "tool", &result, None, Some(name.to_string()))
                .await
                .map_err(|e| LlmError::Session(e.to_string()))?;
        }
    }

    Ok(AgentTurnResult {
        content: last_content,
        tool_calls: last_tool_calls,
    })
}
