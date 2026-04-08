//! Agent turn: load session history, call the configured provider (Ollama, LM Studio, vLLM, NIM), append reply.
//! Supports optional tools: when the model returns tool_calls, we execute them and re-call the model until done.
//!
//! For orchestration, see [`run_turn_with_messages`] — in-memory turns without session persistence (worker / delegation).
//! Gateway dispatch uses [`run_turn_dyn`] with [`crate::orchestration::ProviderClients::as_dyn`].
//! When the gateway passes [`crate::orchestration::DelegateContext`], the built-in tool **`delegate_task`** runs a worker
//! via [`crate::orchestration::execute_delegate_task`] on another enabled provider: per-worker system context and tools
//! when **`workerId`** is set, otherwise the orchestrator’s skill bundle; nested **`delegate_task`** is disabled (see epic).

use crate::orchestration::{
    execute_delegate_task, parse_delegate_tool_calls, parse_delegate_tool_results, DelegateContext,
    DELEGATE_TASK_TOOL_NAME,
};
use crate::providers::{ChatMessage, Provider, ProviderError, ToolCall, ToolDefinition};
use crate::session::SessionStore;

const MAX_TOOL_LOOP: usize = 5;

/// Result of one agent turn: final text content and any tool/function calls that were executed during the turn.
#[derive(Debug, Clone)]
pub struct AgentTurnResult {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    /// Tool/function call outputs (as strings) executed during the turn.
    ///
    /// This is parallel to [`AgentTurnResult::tool_calls`] by index: `tool_results[i]` is the output
    /// for `tool_calls[i]`.
    pub tool_results: Vec<String>,
}

/// Executes a tool by name and JSON arguments. Returns output or error string.
pub trait ToolExecutor: Send + Sync {
    fn execute(&self, name: &str, args: &serde_json::Value) -> Result<String, String>;
}

/// Run one agent turn: load session messages, call the given provider (streaming when on_chunk is Some); if tools are provided and the model returns tool_calls, execute them and re-call until no more tool_calls or max iterations.
/// `model` is the provider-specific model name (no prefix; e.g. `llama3.2:3b` for Ollama, `llama-3.2-3B-instruct` for LM Studio).
pub async fn run_turn<B: Provider>(
    store: &SessionStore,
    session_id: &str,
    provider: &B,
    model: &str,
    system_context: Option<&str>,
    max_session_messages: Option<usize>,
    tools: Option<Vec<ToolDefinition>>,
    tool_executor: Option<&dyn ToolExecutor>,
    delegate: Option<DelegateContext<'_>>,
    on_chunk: Option<&mut (dyn FnMut(&str) + Send)>,
) -> Result<AgentTurnResult, ProviderError> {
    run_turn_dyn(
        store,
        session_id,
        provider as &dyn Provider,
        model,
        system_context,
        max_session_messages,
        tools,
        tool_executor,
        delegate,
        on_chunk,
    )
    .await
}

/// Same as [`run_turn`] but accepts a [`Provider`] trait object (e.g. from [`crate::orchestration::ProviderClients::as_dyn`]).
pub async fn run_turn_dyn(
    store: &SessionStore,
    session_id: &str,
    provider: &dyn Provider,
    model: &str,
    system_context: Option<&str>,
    max_session_messages: Option<usize>,
    tools: Option<Vec<ToolDefinition>>,
    tool_executor: Option<&dyn ToolExecutor>,
    delegate: Option<DelegateContext<'_>>,
    mut on_chunk: Option<&mut (dyn FnMut(&str) + Send)>,
) -> Result<AgentTurnResult, ProviderError> {
    let session = store
        .get(session_id)
        .await
        .ok_or_else(|| ProviderError::Session("session not found".to_string()))?;

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

    if let Some(limit) = max_session_messages {
        if limit > 0 && messages.len() > limit {
            let start = messages.len() - limit;
            messages = messages[start..].to_vec();
        }
    }

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

    execute_turn_main(
        provider,
        model,
        &mut messages,
        tools,
        tool_executor,
        &mut on_chunk,
        Some((store, session_id)),
        delegate,
    )
    .await
}

/// Run a single turn with an explicit message list. **Does not** read or write [`SessionStore`].
///
/// Use this for **worker** or **delegated** subtasks: build `messages` (e.g. system + user instruction), pick a
/// [`Provider`] and model id, then consume [`AgentTurnResult`]. The same tool loop as [`run_turn`] applies
/// (`MAX_TOOL_LOOP` iterations); nothing is persisted — the orchestrator merges results into the main session.
///
/// Streaming is not supported here (`on_chunk` is unused); add later if needed.
pub async fn run_turn_with_messages<B: Provider>(
    provider: &B,
    model: &str,
    messages: Vec<ChatMessage>,
    tools: Option<Vec<ToolDefinition>>,
    tool_executor: Option<&dyn ToolExecutor>,
) -> Result<AgentTurnResult, ProviderError> {
    run_turn_with_messages_dyn(
        provider as &dyn Provider,
        model,
        messages,
        tools,
        tool_executor,
    )
    .await
}

/// Same as [`run_turn_with_messages`] but accepts a [`Provider`] trait object.
pub async fn run_turn_with_messages_dyn(
    provider: &dyn Provider,
    model: &str,
    mut messages: Vec<ChatMessage>,
    tools: Option<Vec<ToolDefinition>>,
    tool_executor: Option<&dyn ToolExecutor>,
) -> Result<AgentTurnResult, ProviderError> {
    let mut on_chunk: Option<&mut (dyn FnMut(&str) + Send)> = None;
    execute_turn_worker(
        provider,
        model,
        &mut messages,
        tools,
        tool_executor,
        &mut on_chunk,
        None,
    )
    .await
}

/// In-memory tool loop without `delegate_task` handling (nested delegation). Used by [`run_turn_with_messages_dyn`].
async fn execute_turn_worker(
    provider: &dyn Provider,
    model: &str,
    messages: &mut Vec<ChatMessage>,
    tools: Option<Vec<ToolDefinition>>,
    tool_executor: Option<&dyn ToolExecutor>,
    on_chunk: &mut Option<&mut (dyn FnMut(&str) + Send)>,
    persist: Option<(&SessionStore, &str)>,
) -> Result<AgentTurnResult, ProviderError> {
    let model_name = model.trim();
    let model_name = if model_name.is_empty() {
        log::warn!("agent: configured model was empty, using fallback");
        crate::orchestration::DEFAULT_MODEL_FALLBACK
    } else {
        model_name
    };
    log::info!("agent: using model {}", model_name);
    let tools_ref = tools.as_ref();
    let mut loop_count = 0;
    let mut executed_tool_calls: Vec<ToolCall> = Vec::new();
    let mut executed_tool_results: Vec<String> = Vec::new();
    let mut last_content: String;
    let mut last_tool_calls: Vec<ToolCall>;

    loop {
        let use_stream = on_chunk.is_some() && loop_count == 0;
        let res = if use_stream {
            let cb = on_chunk.as_mut().unwrap();
            let mut delta_cb = |s: &str| cb(s);
            provider
                .chat_stream(
                    model_name,
                    messages.clone(),
                    tools_ref.cloned(),
                    &mut delta_cb,
                )
                .await?
        } else {
            provider
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

        if let Some((store, session_id)) = persist {
            store
                .append_message_full(
                    session_id,
                    "assistant",
                    &assistant_msg.content,
                    assistant_msg.tool_calls.clone(),
                    None,
                )
                .await
                .map_err(|e| ProviderError::Session(e.to_string()))?;
        }

        if last_tool_calls.is_empty() {
            break;
        }

        loop_count += 1;
        if loop_count >= MAX_TOOL_LOOP {
            log::debug!("agent: max tool loop iterations reached");
            break;
        }

        let needs_executor = last_tool_calls
            .iter()
            .any(|c| c.function.name != DELEGATE_TASK_TOOL_NAME);
        if needs_executor && tool_executor.is_none() {
            log::debug!("agent: tool_calls returned but no executor");
            break;
        }

        messages.push(assistant_msg);
        for call in &last_tool_calls {
            let name = call.function.name.as_str();
            let args = &call.function.arguments;
            let result = if name == DELEGATE_TASK_TOOL_NAME {
                log::debug!("agent: delegate_task not available in worker turn");
                "error: delegate_task is not available in this context".to_string()
            } else {
                match tool_executor {
                    Some(executor) => match executor.execute(name, args) {
                        Ok(out) => out.clone(),
                        Err(e) => {
                            log::warn!("agent: tool {} failed: {}", name, e);
                            format!("error: {}", e)
                        }
                    },
                    None => {
                        log::debug!("agent: missing executor for tool");
                        format!("error: no executor for tool {}", name)
                    }
                }
            };
            executed_tool_results.push(result.clone());
            messages.push(ChatMessage {
                role: "tool".to_string(),
                content: result.clone(),
                tool_calls: None,
                tool_name: Some(name.to_string()),
            });
            if let Some((store, session_id)) = persist {
                store
                    .append_message_full(session_id, "tool", &result, None, Some(name.to_string()))
                    .await
                    .map_err(|e| ProviderError::Session(e.to_string()))?;
            }
        }

        executed_tool_calls.extend(last_tool_calls.clone());
    }

    Ok(AgentTurnResult {
        content: last_content,
        tool_calls: executed_tool_calls,
        tool_results: executed_tool_results,
    })
}

/// Session-backed tool loop with `delegate_task` (nested worker turns use [`execute_turn_worker`] only).
async fn execute_turn_main(
    provider: &dyn Provider,
    model: &str,
    messages: &mut Vec<ChatMessage>,
    tools: Option<Vec<ToolDefinition>>,
    tool_executor: Option<&dyn ToolExecutor>,
    on_chunk: &mut Option<&mut (dyn FnMut(&str) + Send)>,
    persist: Option<(&SessionStore, &str)>,
    delegate: Option<DelegateContext<'_>>,
) -> Result<AgentTurnResult, ProviderError> {
    let model_name = model.trim();
    let model_name = if model_name.is_empty() {
        log::warn!("agent: configured model was empty, using fallback");
        crate::orchestration::DEFAULT_MODEL_FALLBACK
    } else {
        model_name
    };
    log::info!("agent: using model {}", model_name);
    let tools_ref = tools.as_ref();
    let mut loop_count = 0;
    let mut executed_tool_calls: Vec<ToolCall> = Vec::new();
    let mut executed_tool_results: Vec<String> = Vec::new();
    let mut last_content: String;
    let mut last_tool_calls: Vec<ToolCall>;
    let max_delegations_per_turn = delegate
        .as_ref()
        .and_then(|d| d.agents.max_delegations_per_turn);
    let mut delegate_calls_this_turn: usize = 0;

    loop {
        let use_stream = on_chunk.is_some() && loop_count == 0;
        let res = if use_stream {
            let cb = on_chunk.as_mut().unwrap();
            let mut delta_cb = |s: &str| cb(s);
            provider
                .chat_stream(
                    model_name,
                    messages.clone(),
                    tools_ref.cloned(),
                    &mut delta_cb,
                )
                .await?
        } else {
            provider
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

        if let Some((store, session_id)) = persist {
            store
                .append_message_full(
                    session_id,
                    "assistant",
                    &assistant_msg.content,
                    assistant_msg.tool_calls.clone(),
                    None,
                )
                .await
                .map_err(|e| ProviderError::Session(e.to_string()))?;
        }

        if last_tool_calls.is_empty() {
            break;
        }

        loop_count += 1;
        if loop_count >= MAX_TOOL_LOOP {
            log::debug!("agent: max tool loop iterations reached");
            break;
        }

        let needs_executor = last_tool_calls
            .iter()
            .any(|c| c.function.name != DELEGATE_TASK_TOOL_NAME);
        if needs_executor && tool_executor.is_none() {
            log::debug!("agent: tool_calls returned but no executor");
            break;
        }

        messages.push(assistant_msg);
        let mut worker_tool_calls: Vec<ToolCall> = Vec::new();
        let mut worker_tool_results: Vec<String> = Vec::new();
        for call in &last_tool_calls {
            let name = call.function.name.as_str();
            let args = &call.function.arguments;
            let result = if name == DELEGATE_TASK_TOOL_NAME {
                delegate_calls_this_turn += 1;
                if let Some(max) = max_delegations_per_turn {
                    if delegate_calls_this_turn > max {
                        log::warn!(
                            "agent: delegate_task rejected (max delegations per turn: {})",
                            max
                        );
                        if let Some(ref d) = delegate {
                            if let Some(ref obs) = d.observability {
                                obs.emit_rejected(args, "max_delegations_per_turn", Some(max));
                            }
                        }
                        format!(
                            "error: max delegations per turn exceeded (maxDelegationsPerTurn={})",
                            max
                        )
                    } else {
                        match delegate {
                            Some(ref ctx) => match execute_delegate_task(ctx, args).await {
                                Ok(s) => s,
                                Err(e) => {
                                    log::warn!("agent: delegate_task failed: {}", e);
                                    format!("error: {}", e)
                                }
                            },
                            None => {
                                log::debug!("agent: delegate_task not available");
                                "error: delegate_task is not available in this context".to_string()
                            }
                        }
                    }
                } else {
                    match delegate {
                        Some(ref ctx) => match execute_delegate_task(ctx, args).await {
                            Ok(s) => s,
                            Err(e) => {
                                log::warn!("agent: delegate_task failed: {}", e);
                                format!("error: {}", e)
                            }
                        },
                        None => {
                            log::debug!("agent: delegate_task not available");
                            "error: delegate_task is not available in this context".to_string()
                        }
                    }
                }
            } else {
                match tool_executor {
                    Some(executor) => match executor.execute(name, args) {
                        Ok(out) => out.clone(),
                        Err(e) => {
                            log::warn!("agent: tool {} failed: {}", name, e);
                            format!("error: {}", e)
                        }
                    },
                    None => {
                        log::debug!("agent: missing executor for tool");
                        format!("error: no executor for tool {}", name)
                    }
                }
            };
            executed_tool_results.push(result.clone());
            messages.push(ChatMessage {
                role: "tool".to_string(),
                content: result.clone(),
                tool_calls: None,
                tool_name: Some(name.to_string()),
            });
            if let Some((store, session_id)) = persist {
                store
                    .append_message_full(session_id, "tool", &result, None, Some(name.to_string()))
                    .await
                    .map_err(|e| ProviderError::Session(e.to_string()))?;
            }

            if name == DELEGATE_TASK_TOOL_NAME {
                if let Ok(tool_calls) = parse_delegate_tool_calls(&result) {
                    worker_tool_calls.extend(tool_calls);
                }
                if let Ok(tool_results) = parse_delegate_tool_results(&result) {
                    worker_tool_results.extend(tool_results);
                }
            }
        }

        executed_tool_calls.extend(last_tool_calls.clone());
        executed_tool_calls.extend(worker_tool_calls);
        executed_tool_results.extend(worker_tool_results);
    }

    Ok(AgentTurnResult {
        content: last_content,
        tool_calls: executed_tool_calls,
        tool_results: executed_tool_results,
    })
}
