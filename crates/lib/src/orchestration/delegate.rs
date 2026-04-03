//! Built-in `delegate_task` tool: run a worker turn on another provider/model (see `.agents/epic/ORCHESTRATION.md`).

use crate::agent::{run_turn_with_messages_dyn, ToolExecutor};
use crate::config::{canonical_provider, provider_discovery_enabled, AgentsConfig, SkillContextMode};
use crate::providers::{ChatMessage, ToolCall, ToolDefinition, ToolFunctionDefinition};
use crate::session::SessionStore;
use crate::skills::Skill;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use super::choice::provider_choice_from_canonical;
use super::dispatch::ProviderClients;
use super::model::resolve_model;
use super::policy::{
    apply_delegation_instruction_routes, assert_delegation_pair_allowed,
    assert_delegate_provider_not_blocked, assert_session_delegation_limits,
};
use serde_json::json;
use tokio::sync::broadcast;

pub const DELEGATE_TASK_TOOL_NAME: &str = "delegate_task";

/// WebSocket event name: delegation started (worker turn about to run).
pub const EVENT_DELEGATE_START: &str = "orchestration.delegate.start";
/// WebSocket event name: delegation finished successfully.
pub const EVENT_DELEGATE_COMPLETE: &str = "orchestration.delegate.complete";
/// WebSocket event name: delegation failed (resolution or worker turn error).
pub const EVENT_DELEGATE_ERROR: &str = "orchestration.delegate.error";
/// WebSocket event name: delegation not run (e.g. max delegations per turn exceeded).
pub const EVENT_DELEGATE_REJECTED: &str = "orchestration.delegate.rejected";

/// Optional broadcast of structured orchestration events to gateway WebSocket clients (`type`: `event`).
#[derive(Clone)]
pub struct DelegateObservability {
    pub event_tx: broadcast::Sender<String>,
    pub session_id: Option<String>,
}

impl DelegateObservability {
    fn base_payload(&self) -> serde_json::Value {
        match &self.session_id {
            Some(id) => json!({ "sessionId": id }),
            None => json!({}),
        }
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
            let _ = self.event_tx.send(text);
        }
    }

    /// Emits [`EVENT_DELEGATE_REJECTED`] (e.g. max delegations per turn).
    pub fn emit_rejected(&self, args: &serde_json::Value, reason: &str, max_delegations: Option<usize>) {
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
}

fn optional_worker_id_from_args(args: &serde_json::Value) -> Option<String> {
    args.as_object()
        .and_then(|o| o.get("workerId"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Prepend today's date and capability hints to static system context (same as gateway main turn).
///
/// **`WORKERS_ENABLED`** — only when **`workers_enabled`** is **`Some`**: orchestrator may delegate (`delegate_task`).
/// Omitted entirely for worker turns (**`None`**) so worker prompts never mention delegation.
/// **`SKILLS_ENABLED`** — this agent has at least one loaded skill package for tools / context.
pub fn system_context_with_today(
    static_ctx: &str,
    workers_enabled: Option<bool>,
    skills_enabled: bool,
) -> String {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let s = if skills_enabled { "true" } else { "false" };
    let mut header = format!("TODAYS_DATE={}", today);
    if let Some(w) = workers_enabled {
        header.push_str(&format!(
            "\nWORKERS_ENABLED={}",
            if w { "true" } else { "false" }
        ));
    }
    header.push_str(&format!("\nSKILLS_ENABLED={}", s));
    if static_ctx.trim().is_empty() {
        header
    } else {
        format!("{}\n\n{}", header, static_ctx)
    }
}

/// Per-worker skill bundle for `delegate_task` when `workerId` is set (built at gateway startup).
pub struct WorkerDelegateRuntime {
    /// Static system context without date (no orchestrator roster block).
    pub system_context_static: String,
    /// **`AGENTS.md`** directory for this worker (`<profile>/agents/<id>/`), if resolved.
    pub context_directory: Option<PathBuf>,
    pub skills: Arc<Vec<Skill>>,
    pub tools_list: Option<Vec<ToolDefinition>>,
    pub tool_executor: Option<Arc<dyn ToolExecutor>>,
    pub context_mode: SkillContextMode,
}

/// References needed to run a worker turn from the main agent loop.
#[derive(Clone)]
pub struct DelegateContext<'a> {
    pub clients: ProviderClients<'a>,
    pub agents: &'a AgentsConfig,
    /// Full orchestrator system message (with date) for `delegate_task` without `workerId`.
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
            description: Some(
                "delegate a task to a worker"
                    .to_string(),
            ),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "workerId": {
                        "type": "string",
                        "description": "the id of the worker"
                    },
                    "provider": {
                        "type": "string",
                        "description": "the provider to use"
                    },
                    "model": {
                        "type": "string",
                        "description": "the model to use"
                    },
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
    tool_calls: Vec<ToolCall>,
    tool_results: Vec<String>,
    provider_canonical: &str,
    model: &str,
) -> String {
    let payload = serde_json::json!({
        "reply": reply,
        "toolCalls": tool_calls,
        "toolResults": tool_results,
        "worker": {
            "provider": provider_canonical,
            "model": model,
        }
    });
    payload.to_string()
}

/// Parse the JSON string produced by [`format_delegate_result`] and extract the worker's `toolCalls`.
pub fn parse_delegate_tool_calls(payload: &str) -> Result<Vec<ToolCall>, String> {
    let v: serde_json::Value = serde_json::from_str(payload)
        .map_err(|e| format!("failed to parse delegate_task payload json: {}", e))?;
    let tool_calls_v = v
        .get("toolCalls")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    serde_json::from_value::<Vec<ToolCall>>(tool_calls_v)
        .map_err(|e| format!("failed to parse worker toolCalls: {}", e))
}

/// Parse the JSON string produced by [`format_delegate_result`] and extract the worker's `toolResults`.
pub fn parse_delegate_tool_results(payload: &str) -> Result<Vec<String>, String> {
    let v: serde_json::Value = serde_json::from_str(payload)
        .map_err(|e| format!("failed to parse delegate_task payload json: {}", e))?;
    let tool_results_v = v
        .get("toolResults")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    serde_json::from_value::<Vec<String>>(tool_results_v)
        .map_err(|e| format!("failed to parse worker toolResults: {}", e))
}

#[derive(Debug)]
struct DelegateTarget {
    provider_canonical: &'static str,
    provider_choice: super::choice::ProviderChoice,
    model: String,
}

fn resolve_delegate_target(agents: &AgentsConfig, args: &serde_json::Value) -> Result<DelegateTarget, String> {
    let obj = args
        .as_object()
        .ok_or_else(|| "arguments must be an object".to_string())?;

    let worker_id = obj
        .get("workerId")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let provider_raw = obj
        .get("provider")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let model_param = obj
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let global_default_provider = agents
        .default_provider
        .as_deref()
        .and_then(canonical_provider)
        .unwrap_or("ollama");

    let (provider_canonical, model) = if let Some(ref worker_id) = worker_id {
        let worker = agents
            .workers
            .as_ref()
            .and_then(|ws| ws.iter().find(|w| w.id == worker_id.as_str()))
            .ok_or_else(|| format!("unknown workerId: {}", worker_id))?;

        let default_provider = worker
            .default_provider
            .as_deref()
            .and_then(canonical_provider)
            .unwrap_or(global_default_provider);

        let provider_canonical = match provider_raw {
            Some(p) => canonical_provider(p).ok_or_else(|| format!("unknown provider: {}", p))?,
            None => default_provider,
        };

        let allowed = match &worker.enabled_providers {
            None => true,
            Some(list) if list.is_empty() => provider_canonical == default_provider,
            Some(list) => list
                .iter()
                .filter_map(|p| canonical_provider(p))
                .any(|p| p == provider_canonical),
        };
        if !allowed {
            return Err(format!(
                "provider {} is not enabled for workerId {} (workers.enabledProviders)",
                provider_canonical, worker_id
            ));
        }

        if !provider_discovery_enabled(agents, provider_canonical) {
            return Err(format!(
                "provider {} is not enabled for this agent (agents.enabledProviders)",
                provider_canonical
            ));
        }

        let provider_choice = provider_choice_from_canonical(provider_canonical);
        let config_model = worker.default_model.as_deref().or(agents.default_model.as_deref());
        let model = resolve_model(config_model, model_param, provider_choice);
        (provider_canonical, model)
    } else {
        let provider_canonical = match provider_raw {
            Some(p) => canonical_provider(p).ok_or_else(|| format!("unknown provider: {}", p))?,
            None => global_default_provider,
        };

        if !provider_discovery_enabled(agents, provider_canonical) {
            return Err(format!(
                "provider {} is not enabled for this agent (agents.enabledProviders)",
                provider_canonical
            ));
        }

        let provider_choice = provider_choice_from_canonical(provider_canonical);
        let model = resolve_model(agents.default_model.as_deref(), model_param, provider_choice);
        (provider_canonical, model)
    };

    assert_delegate_provider_not_blocked(agents, provider_canonical)?;

    assert_delegation_pair_allowed(
        agents,
        worker_id.as_deref(),
        provider_canonical,
        &model,
    )?;

    let provider_choice = provider_choice_from_canonical(provider_canonical);
    Ok(DelegateTarget {
        provider_canonical,
        provider_choice,
        model,
    })
}

/// Run a worker turn: delegates to [`crate::agent::run_turn_with_messages_dyn`] (nested `delegate_task` is disabled there).
pub async fn execute_delegate_task(
    ctx: &DelegateContext<'_>,
    args: &serde_json::Value,
) -> Result<String, String> {
    let merged = apply_delegation_instruction_routes(ctx.agents, args);
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

    let target = match resolve_delegate_target(ctx.agents, &merged) {
        Ok(t) => t,
        Err(e) => {
            if let Some(ref obs) = ctx.observability {
                let mut extra = json!({ "error": e });
                if let Some(w) = optional_worker_id_from_args(&merged) {
                    extra["workerId"] = json!(w);
                }
                obs.send(EVENT_DELEGATE_ERROR, obs.merge_base(extra));
            }
            return Err(e);
        }
    };
    let provider_canonical = target.provider_canonical;
    let choice = target.provider_choice;
    let model = target.model;

    if let (Some(store), Some(sid)) = (ctx.session_store, ctx.session_id) {
        if let Err(e) =
            assert_session_delegation_limits(store, sid, ctx.agents, provider_canonical).await
        {
            if let Some(ref obs) = ctx.observability {
                let reason = if e.contains("maxDelegationsPerSession") {
                    "max_delegations_per_session"
                } else {
                    "max_delegations_per_provider"
                };
                obs.emit_rejected(&merged, reason, None);
            }
            return Err(e);
        }
    }

    let worker_id = obj
        .get("workerId")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    if let Some(ref obs) = ctx.observability {
        let mut extra = json!({
            "provider": provider_canonical,
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
            provider_canonical,
            model
        );
    } else {
        log::info!(
            "orchestration: delegate_task provider={} model={}",
            provider_canonical,
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
            let worker_skills_enabled = !rt.skills.is_empty();
            let sys = system_context_with_today(&rt.system_context_static, None, worker_skills_enabled);
            let s = sys.trim();
            if !s.is_empty() {
                messages.push(ChatMessage {
                    role: "system".to_string(),
                    content: s.to_string(),
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
    let provider = ctx.clients.as_dyn(choice);
    let result = match run_turn_with_messages_dyn(
        provider,
        &model,
        messages,
        worker_tools,
        tool_exec,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            let msg = e.to_string();
            if let Some(ref obs) = ctx.observability {
                let mut extra = json!({
                    "error": msg,
                    "provider": provider_canonical,
                    "model": model,
                });
                if let Some(w) = optional_worker_id_from_args(&merged) {
                    extra["workerId"] = json!(w);
                }
                obs.send(EVENT_DELEGATE_ERROR, obs.merge_base(extra));
            }
            return Err(msg);
        }
    };

    if let (Some(store), Some(sid)) = (ctx.session_store, ctx.session_id) {
        if let Err(e) = store.record_delegation(sid, provider_canonical).await {
            log::warn!("orchestration: record_delegation failed: {}", e);
        }
    }

    if let Some(ref obs) = ctx.observability {
        let mut extra = json!({
            "provider": provider_canonical,
            "model": model,
            "workerToolCalls": result.tool_calls.len(),
            "workerToolResults": result.tool_results.len(),
        });
        if let Some(w) = optional_worker_id_from_args(&merged) {
            extra["workerId"] = json!(w);
        }
        obs.send(EVENT_DELEGATE_COMPLETE, obs.merge_base(extra));
    }

    Ok(format_delegate_result(
        result.content,
        result.tool_calls,
        result.tool_results,
        provider_canonical,
        &model,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ToolFunctionDefinition;
    use crate::providers::ToolCallFunction;
    use crate::config::{AllowedModelEntry, WorkerConfig};
    use serde_json::json;

    #[test]
    fn system_context_with_today_includes_flags() {
        let s = system_context_with_today("hello", Some(true), false);
        assert!(s.contains("TODAYS_DATE="));
        assert!(s.contains("WORKERS_ENABLED=true"));
        assert!(s.contains("SKILLS_ENABLED=false"));
        assert!(s.contains("hello"));
        let empty = system_context_with_today("", Some(false), false);
        assert!(empty.contains("WORKERS_ENABLED=false"));
        assert!(empty.contains("SKILLS_ENABLED=false"));
        assert!(!empty.contains("\n\n\n"));
    }

    #[test]
    fn system_context_with_today_worker_omits_workers_line() {
        let s = system_context_with_today("body", None, true);
        assert!(s.contains("TODAYS_DATE="));
        assert!(!s.contains("WORKERS_ENABLED"));
        assert!(s.contains("SKILLS_ENABLED=true"));
        assert!(s.contains("body"));
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
    fn format_delegate_result_includes_reply_and_tool_calls() {
        let tool_call = ToolCall {
            typ: "function".to_string(),
            function: ToolCallFunction {
                index: None,
                name: "search".to_string(),
                arguments: serde_json::json!({"query": "hi"}),
            },
        };

        let payload = format_delegate_result(
            "worker reply".to_string(),
            vec![tool_call],
            vec!["tool output".to_string()],
            "ollama",
            "llama3.2:latest",
        );

        let v: serde_json::Value = serde_json::from_str(&payload).expect("valid json");
        assert_eq!(v["reply"], "worker reply");
        assert_eq!(v["toolCalls"].as_array().unwrap().len(), 1);
        assert_eq!(v["worker"]["provider"], "ollama");
        assert_eq!(v["worker"]["model"], "llama3.2:latest");
        assert_eq!(v["toolResults"].as_array().unwrap().len(), 1);
        assert_eq!(v["toolResults"].as_array().unwrap()[0], "tool output");
    }

    #[test]
    fn parse_delegate_tool_calls_round_trip() {
        let tool_call = ToolCall {
            typ: "function".to_string(),
            function: ToolCallFunction {
                index: None,
                name: "search".to_string(),
                arguments: serde_json::json!({"query": "hi"}),
            },
        };

        let payload = format_delegate_result(
            "worker reply".to_string(),
            vec![tool_call.clone()],
            vec!["tool output".to_string()],
            "ollama",
            "llama3.2:latest",
        );

        let out = parse_delegate_tool_calls(&payload).expect("parsed");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].function.name, "search");
        assert_eq!(
            out[0].function.arguments["query"],
            serde_json::json!("hi")
        );
    }

    #[test]
    fn parse_delegate_tool_results_round_trip() {
        let payload = format_delegate_result(
            "worker reply".to_string(),
            vec![],
            vec!["tool output 1".to_string(), "tool output 2".to_string()],
            "ollama",
            "llama3.2:latest",
        );

        let out = parse_delegate_tool_results(&payload).expect("parsed");
        assert_eq!(out, vec!["tool output 1", "tool output 2"]);
    }

    #[test]
    fn resolve_delegate_target_uses_worker_defaults() {
        let agents = AgentsConfig {
            orchestrator_id: None,
            default_provider: Some("ollama".to_string()),
            default_model: Some("global-default".to_string()),
            enabled_providers: Some(vec!["lms".to_string(), "ollama".to_string()]),
            workers: Some(vec![WorkerConfig {
                id: "fast".to_string(),
                default_provider: Some("lms".to_string()),
                default_model: Some("worker-model".to_string()),
                enabled_providers: None,
                skills_enabled: None,
                context_mode: None,
                delegate_allowed_models: None,
            }]),
            ..AgentsConfig::default()
        };

        let args = json!({
            "workerId": "fast",
            "instruction": "do the thing"
        });

        let target = resolve_delegate_target(&agents, &args).expect("resolved");
        assert_eq!(target.provider_canonical, "lms");
        assert_eq!(target.model, "worker-model");
    }

    #[test]
    fn delegate_observability_event_frame_shape() {
        let (tx, _rx) = broadcast::channel::<String>(4);
        let obs = DelegateObservability {
            event_tx: tx,
            session_id: Some("sess-1".to_string()),
        };
        obs.send(
            EVENT_DELEGATE_START,
            obs.merge_base(json!({
                "provider": "ollama",
                "model": "m",
                "workerId": "w",
            })),
        );
        // If this were used in tests with a receiver, we'd parse — here we only ensure merge_base includes sessionId.
        let merged = obs.merge_base(json!({ "provider": "ollama" }));
        assert_eq!(merged["sessionId"], "sess-1");
        assert_eq!(merged["provider"], "ollama");
    }

    #[test]
    fn resolve_delegate_target_rejects_disallowed_delegate_pair() {
        let agents = AgentsConfig {
            default_provider: Some("ollama".to_string()),
            default_model: Some("global-default".to_string()),
            enabled_providers: Some(vec!["ollama".to_string(), "lms".to_string()]),
            delegate_allowed_models: Some(vec![AllowedModelEntry {
                provider: "ollama".to_string(),
                model: "allowed-only".to_string(),
                local: false,
                tool_capable: None,
            }]),
            ..AgentsConfig::default()
        };
        let args = json!({
            "provider": "lms",
            "model": "some-model",
            "instruction": "do the thing"
        });
        let err = resolve_delegate_target(&agents, &args).expect_err("should reject pair");
        assert!(err.contains("delegateAllowedModels"), "{}", err);
    }

    #[test]
    fn resolve_delegate_target_enforces_worker_enabled_providers() {
        let agents = AgentsConfig {
            default_provider: Some("ollama".to_string()),
            default_model: Some("global-default".to_string()),
            enabled_providers: Some(vec!["lms".to_string(), "ollama".to_string()]),
            workers: Some(vec![WorkerConfig {
                id: "strict".to_string(),
                default_provider: Some("lms".to_string()),
                default_model: Some("worker-model".to_string()),
                enabled_providers: Some(vec!["lms".to_string()]),
                skills_enabled: None,
                context_mode: None,
                delegate_allowed_models: None,
            }]),
            ..AgentsConfig::default()
        };

        let args = json!({
            "workerId": "strict",
            "provider": "ollama",
            "instruction": "do the thing"
        });

        let err = resolve_delegate_target(&agents, &args).expect_err("should reject provider");
        assert!(err.contains("workers.enabledProviders"));
    }
}
