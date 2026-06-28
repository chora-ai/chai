//! Built-in `delegate_task` tool: run a worker turn on the worker's single `(provider, model)` pair (see `base/adr/ORCHESTRATION.md`).

use super::choice::ProviderChoice;
use super::dispatch::ProviderClients;
use super::model::resolve_model;
use super::policy::{apply_delegation_bracket_match, assert_session_delegation_limits};
use crate::agent::{run_turn_with_messages_dyn, ToolExecutor};
use crate::config::{
    canonical_provider_id, provider_discovery_enabled, AgentsConfig, ProvidersConfig,
    SkillContextMode,
};
use crate::providers::{ChatMessage, ToolDefinition, ToolFunctionDefinition};
use crate::session::SessionStore;
use crate::skills::Skill;
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

pub const DELEGATE_TASK_TOOL_NAME: &str = "delegate_task";

/// WebSocket event name: delegation started (worker turn about to run).
pub const EVENT_DELEGATE_START: &str = "orchestration.delegate.start";
/// WebSocket event name: delegation finished successfully.
pub const EVENT_DELEGATE_COMPLETE: &str = "orchestration.delegate.complete";
/// WebSocket event name: delegation failed (resolution or worker turn error).
pub const EVENT_DELEGATE_ERROR: &str = "orchestration.delegate.error";
/// WebSocket event name: delegation rejected (policy limit).
pub const EVENT_DELEGATE_REJECTED: &str = "orchestration.delegate.rejected";

/// WebSocket event name: tool call started (tool about to execute).
pub const EVENT_TOOL_CALL: &str = "session.tool_call";
/// WebSocket event name: tool call completed (result available).
pub const EVENT_TOOL_RESULT: &str = "session.tool_result";
/// WebSocket event name: intermediate assistant message content during tool loop iterations.
pub const EVENT_ASSISTANT_PROGRESS: &str = "session.assistant_progress";
/// WebSocket event name: tool loop iteration limit reached, some tool calls were not executed.
pub const EVENT_TOOL_LOOP_LIMIT: &str = "session.tool_loop_limit";
/// WebSocket event name: agent turn stopped by user (pause after current iteration).
pub const EVENT_TURN_STOPPED: &str = "session.turn_stopped";
/// WebSocket event name: gateway configuration changed (e.g. model discovery updated provider models).
pub const EVENT_CONFIG_CHANGED: &str = "gateway.config.changed";
/// Optional broadcast of structured orchestration events to gateway WebSocket clients (`type`: `event`).
pub struct DelegateObservability {
    pub event_tx: broadcast::Sender<String>,
    pub session_id: Option<String>,
    /// Source label included in tool call/result events — the agent id (e.g.
    /// `"orchestrator"` or a worker id like `"engineer"`) so the desktop can
    /// display the author and style worker messages differently.
    pub source: Option<String>,
    /// Offset added to tool call/result `index` values emitted by this observability instance.
    /// Copied from [`DelegateContext::tool_index_offset`] when the worker is spawned. The
    /// orchestrator accumulates this value after each delegation so that successive workers
    /// produce non-overlapping tool indices.
    pub tool_index_offset: usize,
    /// Number of tool_call events emitted through this observability instance.
    /// Incremented atomically by `emit_tool_call` so the count is available even
    /// when the worker turn fails partway through (e.g. provider timeout after
    /// some tool calls have already been emitted). The orchestrator reads this
    /// via [`DelegateObservability::emitted_tool_call_count`] to accumulate
    /// `tool_index_offset` correctly in the error path.
    /// Always initialize to 0 when constructing a new instance.
    pub emitted_tool_calls: AtomicUsize,
}

impl Clone for DelegateObservability {
    fn clone(&self) -> Self {
        Self {
            event_tx: self.event_tx.clone(),
            session_id: self.session_id.clone(),
            source: self.source.clone(),
            tool_index_offset: self.tool_index_offset,
            // A cloned observability is a fresh instance for a new worker;
            // the emitted count starts at zero.
            emitted_tool_calls: AtomicUsize::new(0),
        }
    }
}

impl DelegateObservability {
    fn base_payload(&self) -> serde_json::Value {
        let mut base = match &self.session_id {
            Some(id) => json!({ "sessionId": id }),
            None => json!({}),
        };
        if let Some(obj) = base.as_object_mut() {
            if let Some(ref s) = self.source {
                obj.insert("source".to_string(), json!(s));
            }
        }
        base
    }

    fn merge_base(&self, extra: serde_json::Value) -> serde_json::Value {
        let mut base = self.base_payload();
        if let Some(obj) = base.as_object_mut() {
            if let Some(e) = extra.as_object() {
                for (k, v) in e {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        base
    }

    fn send(&self, event: &str, payload: serde_json::Value) {
        let frame = json!({
            "type": "event",
            "event": event,
            "payload": payload,
        });
        if let Ok(text) = serde_json::to_string(&frame) {
            if let Err(e) = self.event_tx.send(text) {
                log::debug!(
                    "delegate observability: failed to send {} event: {}",
                    event, e
                );
            }
        }
    }

    /// Emits [`EVENT_DELEGATE_REJECTED`] (e.g. max delegations per turn).
    pub fn emit_rejected(
        &self,
        args: &serde_json::Value,
        reason: &str,
        max_delegations: Option<usize>,
    ) {
        let worker_id = args
            .as_object()
            .and_then(|o| o.get("workerId"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let mut extra = json!({
            "reason": reason,
        });
        if let Some(m) = max_delegations {
            extra["maxDelegationsPerTurn"] = json!(m);
        }
        if let Some(w) = worker_id {
            extra["workerId"] = json!(w);
        }
        self.send(EVENT_DELEGATE_REJECTED, self.merge_base(extra));
    }

    /// Emits [`EVENT_TOOL_CALL`] when a tool is about to execute.
    pub fn emit_tool_call(
        &self,
        tool_name: &str,
        tool_args: &serde_json::Value,
        index: usize,
    ) {
        let effective_index = self.tool_index_offset + index;
        let payload = self.merge_base(json!({
            "toolName": tool_name,
            "toolArgs": tool_args,
            "index": effective_index,
        }));
        self.send(EVENT_TOOL_CALL, payload);
        self.emitted_tool_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Returns the number of tool_call events emitted through this observability
    /// instance. Used by the orchestrator to accumulate `tool_index_offset` when
    /// a worker turn fails partway through — the worker may have already emitted
    /// some tool calls before the error, and those indices must not collide with
    /// the next delegation's indices.
    pub fn emitted_tool_call_count(&self) -> usize {
        self.emitted_tool_calls.load(Ordering::Relaxed)
    }

    /// Emits [`EVENT_TOOL_RESULT`] when a tool execution completes.
    pub fn emit_tool_result(
        &self,
        tool_name: &str,
        tool_result: &str,
        index: usize,
    ) {
        let effective_index = self.tool_index_offset + index;
        let payload = self.merge_base(json!({
            "toolName": tool_name,
            "toolResult": tool_result,
            "index": effective_index,
        }));
        self.send(EVENT_TOOL_RESULT, payload);
    }

    /// Emits [`EVENT_ASSISTANT_PROGRESS`] when the model produces content alongside
    /// tool calls during a loop iteration. This content would otherwise be invisible
    /// to the user since only the final iteration's content is sent as the assistant reply.
    pub fn emit_assistant_message(&self, content: &str, iteration: u32) {
        let payload = self.merge_base(json!({
            "content": content,
            "iteration": iteration,
        }));
        self.send(EVENT_ASSISTANT_PROGRESS, payload);
    }

    /// Emits [`EVENT_TOOL_LOOP_LIMIT`] when the tool loop iteration limit is reached
    /// and some tool calls were not executed. Includes the pending tool calls so
    /// connected clients can inform the user what was interrupted.
    pub fn emit_tool_loop_limit(&self, pending_tool_calls: &[crate::providers::ToolCall]) {
        let pending = serde_json::to_value(pending_tool_calls).unwrap_or_else(|_| json!([]));
        let payload = self.merge_base(json!({
            "pendingToolCalls": pending,
        }));
        self.send(EVENT_TOOL_LOOP_LIMIT, payload);
    }

    /// Emits [`EVENT_TURN_STOPPED`] when the agent turn is stopped by the user.
    /// The session transcript remains valid — the user can send a new message
    /// to continue from where the turn was paused.
    pub fn emit_turn_stopped(&self) {
        let payload = self.base_payload();
        self.send(EVENT_TURN_STOPPED, payload);
    }
}

fn optional_worker_id_from_args(args: &serde_json::Value) -> Option<String> {
    args.as_object()
        .and_then(|o| o.get("workerId"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Per-worker skill bundle for `delegate_task` when `workerId` is set (built at gateway startup).
pub struct WorkerDelegateRuntime {
    /// Static system context (no orchestrator roster block).
    pub system_context: String,
    pub skills: Arc<Vec<Skill>>,
    pub tools_list: Option<Vec<ToolDefinition>>,
    pub tool_executor: Option<Arc<dyn ToolExecutor>>,
    pub context_mode: SkillContextMode,
}

/// References needed to run a worker turn from the main agent loop.
#[derive(Clone)]
pub struct DelegateContext<'a> {
    pub clients: &'a ProviderClients,
    pub providers: &'a ProvidersConfig,
    pub agents: &'a AgentsConfig,
    /// Full orchestrator system message for `delegate_task` without `workerId`.
    pub orchestrator_system_context: Option<&'a str>,
    /// Skill tools for the orchestrator path (no `delegate_task`); used when `workerId` is absent.
    pub orchestrator_worker_tools: Option<Vec<ToolDefinition>>,
    pub orchestrator_tool_executor: Option<&'a dyn ToolExecutor>,
    /// When `workerId` is set, the worker turn uses this bundle instead of the orchestrator copies above.
    pub worker_runtimes: Option<&'a HashMap<String, WorkerDelegateRuntime>>,
    /// When set, emits gateway WebSocket events for delegate lifecycle (see [`DelegateObservability`]).
    pub observability: Option<DelegateObservability>,
    /// When set with [`DelegateContext::session_id`], session policy caps and [`SessionStore::record_delegation`] apply.
    pub session_store: Option<&'a SessionStore>,
    pub session_id: Option<&'a str>,
    /// When set, the worker turn checks this flag at the top of each loop iteration
    /// and stops gracefully when the flag becomes true.
    pub stop_flag: Option<Arc<AtomicBool>>,
    /// Offset added to tool call/result `index` values emitted by the worker's
    /// observability. Initialized to 0 and accumulated by the orchestrator after
    /// each delegation by adding the worker's `tool_call_count`. This ensures
    /// successive delegations produce non-overlapping tool indices even when
    /// workers share the same `source` label.
    pub tool_index_offset: usize,
}

/// Tool list passed to the worker: same definitions as the orchestrator minus `delegate_task` (nested delegation is disabled).
pub fn worker_tool_list(tools: Option<&Vec<ToolDefinition>>) -> Option<Vec<ToolDefinition>> {
    let v: Vec<ToolDefinition> = tools?
        .iter()
        .filter(|t| t.function.name != DELEGATE_TASK_TOOL_NAME)
        .cloned()
        .collect();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

pub fn delegate_task_tool_definition() -> ToolDefinition {
    ToolDefinition {
        typ: "function".to_string(),
        function: ToolFunctionDefinition {
            name: DELEGATE_TASK_TOOL_NAME.to_string(),
            description: Some("Delegate a task to a worker agent. Use this when the task benefits from a separate context and the worker has the relevant skills. Start your instruction with the worker's bracket prefix to target that worker. The worker runs one turn with your instruction and returns the result as a synthesized reply, not raw tool output.".to_string()),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "instruction": {
                        "type": "string",
                        "description": "the instructions for the worker"
                    }
                },
                "required": ["instruction"]
            }),
        },
    }
}

/// Prepend the delegate tool when at least one worker is configured, unless the list already contains `delegate_task`.
pub fn merge_delegate_task(
    tools: Option<Vec<ToolDefinition>>,
    has_workers: bool,
) -> Option<Vec<ToolDefinition>> {
    if !has_workers {
        return tools;
    }
    let def = delegate_task_tool_definition();
    match tools {
        None => Some(vec![def]),
        Some(mut v) => {
            if v.iter().any(|t| t.function.name == DELEGATE_TASK_TOOL_NAME) {
                return Some(v);
            }
            v.insert(0, def);
            Some(v)
        }
    }
}

fn format_delegate_result(
    reply: String,
    provider_id: &str,
    model: &str,
) -> String {
    let payload = serde_json::json!({
        "reply": reply,
        "worker": {
            "provider": provider_id,
            "model": model,
        }
    });
    payload.to_string()
}

#[derive(Debug)]
struct DelegateTarget {
    provider_id: String,
    provider_choice: ProviderChoice,
    model: String,
}

fn resolve_delegate_target(
    providers: &ProvidersConfig,
    agents: &AgentsConfig,
    args: &serde_json::Value,
) -> Result<DelegateTarget, String> {
    let obj = args
        .as_object()
        .ok_or_else(|| "arguments must be an object".to_string())?;

    let worker_id = obj
        .get("workerId")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Default to the first configured provider or "ollama".
    let orch = agents.default_orchestrator();
    let global_default_provider = orch
        .default_provider
        .as_deref()
        .and_then(|s| canonical_provider_id(providers, s))
        .or_else(|| providers.entries.first().map(|p| p.id.trim().to_string()))
        .unwrap_or_else(|| "ollama".to_string());

    let (provider_id, model) = if let Some(ref worker_id) = worker_id {
        let worker = agents
            .workers
            .as_ref()
            .and_then(|ws| ws.iter().find(|w| w.id == worker_id.as_str()))
            .ok_or_else(|| format!("unknown workerId: {}", worker_id))?;

        let provider_id = worker
            .default_provider
            .as_deref()
            .and_then(|s| canonical_provider_id(providers, s))
            .unwrap_or(global_default_provider.clone());

        if !provider_discovery_enabled(providers, agents, &provider_id) {
            return Err(format!(
                "provider {} is not enabled for this agent (agents.enabledProviders)",
                provider_id
            ));
        }

        let provider_choice = ProviderChoice::new(&provider_id);
        let config_model = worker
            .default_model
            .as_deref()
            .or(orch.default_model.as_deref());
        let model = resolve_model(providers, config_model, None, &provider_choice);
        (provider_id, model)
    } else {
        let provider_id = global_default_provider;

        if !provider_discovery_enabled(providers, agents, &provider_id) {
            return Err(format!(
                "provider {} is not enabled for this agent (agents.enabledProviders)",
                provider_id
            ));
        }

        let provider_choice = ProviderChoice::new(&provider_id);
        let model = resolve_model(
            providers,
            orch.default_model.as_deref(),
            None,
            &provider_choice,
        );
        (provider_id, model)
    };

    let provider_choice = ProviderChoice::new(&provider_id);
    Ok(DelegateTarget {
        provider_id,
        provider_choice,
        model,
    })
}

/// Result of a `delegate_task` tool execution.
pub struct DelegateTaskResult {
    /// Formatted JSON result string returned to the orchestrator as the tool output.
    pub output: String,
    /// Whether the worker turn was stopped by a stop signal mid-execution.
    pub stopped: bool,
    /// Number of tool calls executed by the worker during this delegation.
    /// The orchestrator uses this to accumulate `tool_index_offset` so that
    /// successive delegations produce non-overlapping tool indices.
    pub tool_call_count: usize,
}

/// Run a worker turn: delegates to [`crate::agent::run_turn_with_messages_dyn`] (nested `delegate_task` is disabled there).
pub async fn execute_delegate_task(
    ctx: &DelegateContext<'_>,
    args: &serde_json::Value,
) -> Result<DelegateTaskResult, String> {
    let merged = apply_delegation_bracket_match(ctx.agents, args);
    let obj = merged
        .as_object()
        .ok_or_else(|| "arguments must be an object".to_string())?;

    let instruction = obj
        .get("instruction")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing instruction".to_string())?;
    let instruction = instruction.trim();
    if instruction.is_empty() {
        return Err("instruction must not be empty".to_string());
    }

    let worker_id = obj
        .get("workerId")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let target = match resolve_delegate_target(ctx.providers, ctx.agents, &merged) {
        Ok(t) => t,
        Err(e) => {
            if let Some(ref obs) = ctx.observability {
                let mut extra = json!({ "error": e });
                if let Some(w) = worker_id {
                    extra["workerId"] = json!(w);
                }
                obs.send(EVENT_DELEGATE_ERROR, obs.merge_base(extra));
            }
            return Err(e);
        }
    };
    let provider_id = &target.provider_id;
    let choice = &target.provider_choice;
    let model = target.model;

    if let (Some(store), Some(sid)) = (ctx.session_store, ctx.session_id) {
        let wid_for_policy = worker_id.unwrap_or(provider_id);
        if let Err(e) =
            assert_session_delegation_limits(store, sid, ctx.agents, wid_for_policy).await
        {
            if let Some(ref obs) = ctx.observability {
                let reason = if e.contains("maxDelegationsPerSession") {
                    "max_delegations_per_session"
                } else {
                    "max_delegations_per_worker"
                };
                obs.emit_rejected(&merged, reason, None);
            }
            return Err(e);
        }
    }

    if let Some(ref obs) = ctx.observability {
        let mut extra = json!({
            "provider": provider_id,
            "model": model,
        });
        if let Some(wid) = worker_id {
            extra["workerId"] = json!(wid);
        }
        obs.send(EVENT_DELEGATE_START, obs.merge_base(extra));
    }

    if let Some(wid) = worker_id {
        log::info!(
            "orchestration: delegate_task workerId={} provider={} model={}",
            wid,
            provider_id,
            model
        );
    } else {
        log::info!(
            "orchestration: delegate_task provider={} model={}",
            provider_id,
            model
        );
    }

    let mut messages: Vec<ChatMessage> = Vec::new();
    let (worker_tools, tool_exec): (Option<Vec<ToolDefinition>>, Option<&dyn ToolExecutor>) =
        if let Some(wid) = worker_id {
            let rt = ctx
                .worker_runtimes
                .and_then(|m| m.get(wid))
                .ok_or_else(|| format!("no worker runtime for workerId: {}", wid))?;
            let sys = rt.system_context.trim();
            if !sys.is_empty() {
                messages.push(ChatMessage {
                    role: "system".to_string(),
                    content: sys.to_string(),
                    tool_calls: None,
                    tool_name: None,
                });
            }
            (
                worker_tool_list(rt.tools_list.as_ref()),
                rt.tool_executor.as_deref(),
            )
        } else {
            if let Some(sys) = ctx.orchestrator_system_context {
                let s = sys.trim();
                if !s.is_empty() {
                    messages.push(ChatMessage {
                        role: "system".to_string(),
                        content: s.to_string(),
                        tool_calls: None,
                        tool_name: None,
                    });
                }
            }
            (
                ctx.orchestrator_worker_tools.clone(),
                ctx.orchestrator_tool_executor,
            )
        };
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: instruction.to_string(),
        tool_calls: None,
        tool_name: None,
    });
    let provider = ctx.clients.get(choice).ok_or_else(|| {
        format!("no client registered for provider '{}'", choice.as_str())
    })?;
    let max_iterations = ctx.agents.default_orchestrator().max_tool_loops_per_turn;
    let worker_obs = ctx.observability.as_ref().map(|obs| DelegateObservability {
        event_tx: obs.event_tx.clone(),
        session_id: obs.session_id.clone(),
        source: Some(worker_id.unwrap_or("worker").to_string()),
        tool_index_offset: ctx.tool_index_offset,
        emitted_tool_calls: AtomicUsize::new(0),
    });
    let result =
        match run_turn_with_messages_dyn(provider, &model, messages, worker_tools, tool_exec, max_iterations, worker_obs.as_ref(), ctx.stop_flag.clone()).await
        {
            Ok(r) => r,
            Err(e) => {
                let msg = e.to_string();
                let partial_tool_calls = worker_obs
                    .as_ref()
                    .map(|obs| obs.emitted_tool_call_count())
                    .unwrap_or(0);
                if let Some(ref obs) = ctx.observability {
                    let mut extra = json!({
                        "error": msg,
                        "provider": provider_id,
                        "model": model,
                    });
                    if let Some(w) = optional_worker_id_from_args(&merged) {
                        extra["workerId"] = json!(w);
                    }
                    obs.send(EVENT_DELEGATE_ERROR, obs.merge_base(extra));
                }
                return Ok(DelegateTaskResult {
                    output: format!("error: {}", msg),
                    stopped: false,
                    tool_call_count: partial_tool_calls,
                });
            }
        };

    if let (Some(store), Some(sid)) = (ctx.session_store, ctx.session_id) {
        let wid_for_record = worker_id.unwrap_or(provider_id);
        if let Err(e) = store.record_delegation(sid, wid_for_record).await {
            log::warn!("orchestration: record_delegation failed: {}", e);
        }
    }

    if let Some(ref obs) = ctx.observability {
        // When the worker was stopped mid-loop, the last iteration's content was
        // already emitted via `session.assistant_progress` (because that iteration
        // had tool calls + non-empty content). Since the worker didn't get to make
        // another model request, `result.content` is the same content — including it
        // here would duplicate what the desktop already displayed. Omit `reply` when
        // stopped so the desktop only shows the `assistant_progress` version.
        let mut extra = if result.stopped {
            json!({
                "provider": provider_id,
                "model": model,
                "workerToolCalls": result.tool_calls.len(),
                "workerToolResults": result.tool_results.len(),
                "stopped": true,
            })
        } else {
            json!({
                "provider": provider_id,
                "model": model,
                "workerToolCalls": result.tool_calls.len(),
                "workerToolResults": result.tool_results.len(),
                "reply": result.content,
            })
        };
        if let Some(w) = optional_worker_id_from_args(&merged) {
            extra["workerId"] = json!(w);
        }
        obs.send(EVENT_DELEGATE_COMPLETE, obs.merge_base(extra));
    }

    Ok(DelegateTaskResult {
        output: format_delegate_result(
            result.content,
            provider_id,
            &model,
        ),
        stopped: result.stopped,
        tool_call_count: result.tool_calls.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{OrchestratorConfig, ProviderDefinition, WorkerConfig, EndpointType};
    use crate::providers::ToolFunctionDefinition;
    use serde_json::json;

    /// Helper to build a ProvidersConfig with the given ids.
    fn test_providers(ids: &[&str]) -> ProvidersConfig {
        ProvidersConfig {
            entries: ids.iter().map(|id| {
                let endpoint_type = match *id {
                    "ollama" => EndpointType::Ollama,
                    _ => EndpointType::OpenaiCompat,
                };
                ProviderDefinition {
                    id: id.to_string(),
                    endpoint_type,
                    base_url: if endpoint_type == EndpointType::OpenaiCompat {
                        Some(format!("http://localhost/{}", id))
                    } else {
                        None
                    },
                    api_key: None,
                    default_model: None,
                    model_discovery: Default::default(),
                    static_models: Vec::new(),
                }
            }).collect(),
        }
    }

    #[test]
    fn merge_delegate_task_skipped_without_workers() {
        assert!(merge_delegate_task(None, false).is_none());
        let t = ToolDefinition {
            typ: "function".to_string(),
            function: ToolFunctionDefinition {
                name: "x".to_string(),
                description: None,
                parameters: serde_json::json!({"type": "object"}),
            },
        };
        let out = merge_delegate_task(Some(vec![t.clone()]), false).expect("some");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].function.name, "x");
    }

    #[test]
    fn merge_delegate_task_prepends_when_workers() {
        let out = merge_delegate_task(None, true).expect("some");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].function.name, DELEGATE_TASK_TOOL_NAME);
    }

    #[test]
    fn worker_tool_list_strips_delegate_task() {
        let tools = vec![
            delegate_task_tool_definition(),
            ToolDefinition {
                typ: "function".to_string(),
                function: ToolFunctionDefinition {
                    name: "other".to_string(),
                    description: None,
                    parameters: serde_json::json!({"type": "object"}),
                },
            },
        ];
        let out = worker_tool_list(Some(&tools)).expect("expected one tool");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].function.name, "other");
    }

    #[test]
    fn format_delegate_result_includes_reply_and_worker() {
        let payload = format_delegate_result(
            "worker reply".to_string(),
            "ollama",
            "llama3.2:3b",
        );

        let v: serde_json::Value = serde_json::from_str(&payload).expect("valid json");
        assert_eq!(v["reply"], "worker reply");
        assert_eq!(v["worker"]["provider"], "ollama");
        assert_eq!(v["worker"]["model"], "llama3.2:3b");
        assert!(v.get("toolCalls").is_none(), "toolCalls should not be present");
        assert!(v.get("toolResults").is_none(), "toolResults should not be present");
    }

    #[test]
    fn resolve_delegate_target_uses_worker_defaults() {
        let providers = test_providers(&["ollama", "lms"]);
        let agents = AgentsConfig {
            orchestrators: vec![OrchestratorConfig {
                id: "orchestrator".to_string(),
                default_provider: Some("ollama".to_string()),
                default_model: Some("global-default".to_string()),
                enabled_providers: Some(vec!["lms".to_string(), "ollama".to_string()]),
                ..Default::default()
            }],
            workers: Some(vec![WorkerConfig {
                id: "fast".to_string(),
                default_provider: Some("lms".to_string()),
                default_model: Some("worker-model".to_string()),
                enabled_skills: None,
                context_mode: None,
            }]),
        };

        let args = json!({
            "workerId": "fast",
            "instruction": "do the thing"
        });

        let target = resolve_delegate_target(&providers, &agents, &args).expect("resolved");
        assert_eq!(target.provider_id, "lms");
        assert_eq!(target.model, "worker-model");
    }

    #[test]
    fn delegate_observability_event_frame_shape() {
        let (tx, _rx) = broadcast::channel::<String>(4);
        let obs = DelegateObservability {
            event_tx: tx,
            session_id: Some("sess-1".to_string()),
            source: None,
            tool_index_offset: 0,
            emitted_tool_calls: AtomicUsize::new(0),
        };
        obs.send(
            EVENT_DELEGATE_START,
            obs.merge_base(json!({
                "provider": "ollama",
                "model": "m",
                "workerId": "w",
            })),
        );
        let merged = obs.merge_base(json!({ "provider": "ollama" }));
        assert_eq!(merged["sessionId"], "sess-1");
        assert_eq!(merged["provider"], "ollama");
        // source is None — should not appear in the merged payload.
        assert!(merged.get("source").is_none(), "source should be absent when None");
    }

    #[test]
    fn delegate_observability_includes_source_in_payload() {
        let (tx, _rx) = broadcast::channel::<String>(4);
        let obs = DelegateObservability {
            event_tx: tx,
            session_id: Some("sess-2".to_string()),
            source: Some("worker".to_string()),
            tool_index_offset: 0,
            emitted_tool_calls: AtomicUsize::new(0),
        };
        let merged = obs.merge_base(json!({ "toolName": "read_file" }));
        assert_eq!(merged["sessionId"], "sess-2");
        assert_eq!(merged["source"], "worker");
        assert_eq!(merged["toolName"], "read_file");
    }

    #[test]
    fn delegate_complete_payload_includes_reply() {
        let (tx, mut rx) = broadcast::channel::<String>(4);
        let obs = DelegateObservability {
            event_tx: tx,
            session_id: Some("sess-3".to_string()),
            source: None,
            tool_index_offset: 0,
            emitted_tool_calls: AtomicUsize::new(0),
        };
        obs.send(
            EVENT_DELEGATE_COMPLETE,
            obs.merge_base(json!({
                "provider": "ollama",
                "model": "llama3.2:3b",
                "workerToolCalls": 0,
                "workerToolResults": 0,
                "reply": "the worker's text response",
            })),
        );
        let frame = rx.try_recv().unwrap();
        let v: serde_json::Value = serde_json::from_str(&frame).unwrap();
        assert_eq!(v["event"], EVENT_DELEGATE_COMPLETE);
        let payload = &v["payload"];
        assert_eq!(payload["reply"], "the worker's text response");
        assert_eq!(payload["provider"], "ollama");
        assert_eq!(payload["workerToolCalls"], 0);
    }

    #[test]
    fn delegate_complete_payload_stopped_omits_reply() {
        let (tx, mut rx) = broadcast::channel::<String>(4);
        let obs = DelegateObservability {
            event_tx: tx,
            session_id: Some("sess-4".to_string()),
            source: None,
            tool_index_offset: 0,
            emitted_tool_calls: AtomicUsize::new(0),
        };
        // When the worker was stopped, `delegate.complete` omits `reply` and
        // includes `stopped: true` to avoid duplicating the content already
        // emitted via `session.assistant_progress`.
        obs.send(
            EVENT_DELEGATE_COMPLETE,
            obs.merge_base(json!({
                "provider": "ollama",
                "model": "llama3.2:3b",
                "workerToolCalls": 1,
                "workerToolResults": 1,
                "stopped": true,
            })),
        );
        let frame = rx.try_recv().unwrap();
        let v: serde_json::Value = serde_json::from_str(&frame).unwrap();
        assert_eq!(v["event"], EVENT_DELEGATE_COMPLETE);
        let payload = &v["payload"];
        assert!(payload.get("reply").is_none(), "reply should be absent when stopped");
        assert_eq!(payload["stopped"], true);
        assert_eq!(payload["provider"], "ollama");
    }

    #[test]
    fn delegate_task_tool_has_no_provider_or_model_params() {
        let def = delegate_task_tool_definition();
        let params = &def.function.parameters;
        let props = params.get("properties").expect("properties");
        assert!(props.get("instruction").is_some(), "instruction param should exist");
        assert!(props.get("provider").is_none(), "provider param should not exist");
        assert!(props.get("model").is_none(), "model param should not exist");
    }
}
