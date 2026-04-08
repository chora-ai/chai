//! Gateway HTTP + WebSocket server (single port).

use crate::agent;
use crate::agent_ctx;
#[cfg(feature = "matrix")]
use crate::channels::{connect_matrix_client, MatrixChannel};
use crate::channels::{
    resolve_signal_daemon_config, ChannelHandle, ChannelRegistry, InboundMessage, SignalChannel,
    TelegramChannel, TelegramTransport, TelegramUpdate,
};
use crate::config::{
    self, orchestrator_context_mode, resolve_hf_api_key, resolve_hf_base_url, resolve_lms_base_url,
    resolve_nim_api_key, resolve_ollama_base_url, resolve_openai_api_key, resolve_openai_base_url,
    resolve_telegram_webhook_secret, resolve_vllm_api_key, resolve_vllm_base_url,
    worker_context_mode, Config, SkillContextMode,
};
#[cfg(feature = "matrix")]
use crate::gateway::matrix_routes;
use crate::gateway::pairing::PairingStore;
use crate::gateway::protocol::{
    AgentParams, ConnectDevice, ConnectParams, HelloAuth, HelloOk, SendParams, WsRequest,
    WsResponse,
};
use crate::init;
use crate::orchestration::{
    build_orchestration_catalog, build_workers_context, effective_worker_defaults,
    merge_delegate_task, provider_choice_from_canonical, provider_id, resolve_model,
    resolve_provider_choice, system_context_with_today, worker_tool_list, DelegateContext,
    DelegateObservability, ProviderChoice, ProviderClients, WorkerDelegateRuntime,
};
use crate::profile::{self, ChaiPaths};
use crate::providers::{
    HfClient, HfModel, LmsClient, LmsModel, NimClient, NimModel, OllamaClient, OllamaModel,
    OpenAiClient, OpenAiModel, ToolDefinition, VllmClient, VllmModel,
};
use crate::routing::SessionBindingStore;
use crate::session::SessionStore;
use crate::skills::{load_skills, validate_skill_composition, Skill, SkillEntry};
use crate::tools::GenericToolExecutor;
use anyhow::{Context, Result};
use axum::{
    body::Bytes,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const PROTOCOL_VERSION: u32 = 1;

const SHUTDOWN_EVENT_JSON: &str = r#"{"type":"event","event":"shutdown","payload":{}}"#;

/// Build connect.challenge event JSON (nonce + ts). Caller sends this as the first frame after WS open; nonce is stored for later device-signing verification.
fn connect_challenge_event(nonce: &str, ts_ms: u64) -> String {
    serde_json::to_string(&json!({
        "type": "event",
        "event": "connect.challenge",
        "payload": { "nonce": nonce, "ts": ts_ms }
    }))
    .unwrap_or_else(|_| r#"{"type":"event","event":"connect.challenge","payload":{}}"#.to_string())
}

/// Canonical payload for device signing: deviceId, client id, client mode, role, scopes, signedAt, token, nonce (newline-separated). Matches order expected for Ed25519 verification.
fn device_signature_payload(
    device: &ConnectDevice,
    client_id: &str,
    client_mode: &str,
    role: &str,
    scopes: &[String],
    token: &str,
) -> String {
    let scopes_str = scopes.join(",");
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        device.id, client_id, client_mode, role, scopes_str, device.signed_at, token, device.nonce
    )
}

/// Verifies that device.nonce matches the challenge nonce and that the Ed25519 signature is valid. Returns an error message on failure.
fn verify_device_signature(
    device: &ConnectDevice,
    params: &ConnectParams,
    challenge_nonce: &str,
) -> Result<(), String> {
    if device.nonce != challenge_nonce {
        return Err("device nonce does not match challenge".to_string());
    }
    let payload = device_signature_payload(
        device,
        params.client.id.as_deref().unwrap_or(""),
        params.client.mode.as_deref().unwrap_or(""),
        &params.role,
        &params.scopes,
        params.auth.token.as_deref().unwrap_or(""),
    );
    let pub_key_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        device.public_key.as_bytes(),
    )
    .map_err(|_| "invalid device publicKey encoding")?;
    let sig_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        device.signature.as_bytes(),
    )
    .map_err(|_| "invalid device signature encoding")?;
    let pk = ed25519_dalek::VerifyingKey::from_bytes(
        pub_key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "invalid device publicKey length")?,
    )
    .map_err(|_| "invalid device publicKey")?;
    let sig = ed25519_dalek::Signature::from_bytes(
        sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "invalid device signature length")?,
    );
    pk.verify_strict(payload.as_bytes(), &sig)
        .map_err(|_| "device signature verification failed".to_string())?;
    Ok(())
}

/// When auth mode is token and a token is configured, returns it for connect validation.
fn require_connect_token(config: &Config) -> Option<String> {
    if config.gateway.auth.mode == config::GatewayAuthMode::Token {
        config::resolve_gateway_token(config)
    } else {
        None
    }
}

/// Shared state for the gateway (config, sessions, channels, agent).
#[derive(Clone)]
pub struct GatewayState {
    pub config: Arc<Config>,
    /// Static portion of the system context built from orchestrator **`AGENTS.md`**, roster, and skills (no date prefix).
    /// The current date is prepended on each turn so the model sees "today" without
    /// rebuilding the rest of the context.
    pub system_context_static: String,
    /// When Some, WebSocket connect must provide params.auth.token matching this.
    pub required_token: Option<String>,
    /// Broadcasts events to connected clients (e.g. shutdown). Subscribers receive JSON event frames.
    pub event_tx: broadcast::Sender<String>,
    /// In-process channel connector tasks; awaited during graceful shutdown.
    pub channel_tasks: Arc<tokio::sync::RwLock<Vec<JoinHandle<()>>>>,
    /// Sender for inbound channel messages (e.g. Telegram webhook POSTs). Processor task receives.
    pub inbound_tx: mpsc::Sender<InboundMessage>,
    pub session_store: Arc<SessionStore>,
    pub channel_registry: Arc<ChannelRegistry>,
    pub bindings: Arc<SessionBindingStore>,
    pub ollama_client: OllamaClient,
    /// Ollama models discovered at startup (or soon after). Empty if Ollama unreachable.
    pub ollama_models: Arc<tokio::sync::RwLock<Vec<OllamaModel>>>,
    pub lms_client: LmsClient,
    /// LM Studio models discovered at startup (or soon after). Empty if LM Studio unreachable.
    pub lms_models: Arc<tokio::sync::RwLock<Vec<LmsModel>>>,
    pub nim_client: NimClient,
    /// NIM models: static list (NVIDIA does not expose a list endpoint). Used for status/UI.
    pub nim_models: Arc<tokio::sync::RwLock<Vec<NimModel>>>,
    pub vllm_client: VllmClient,
    /// vLLM models from GET /v1/models at startup (empty if unreachable).
    pub vllm_models: Arc<tokio::sync::RwLock<Vec<VllmModel>>>,
    pub openai_client: OpenAiClient,
    pub openai_models: Arc<tokio::sync::RwLock<Vec<OpenAiModel>>>,
    pub hf_client: HfClient,
    pub hf_models: Arc<tokio::sync::RwLock<Vec<HfModel>>>,
    /// Loaded skills (name, description, content) for system context. Empty if load failed or no dirs.
    pub skills: Arc<Vec<Skill>>,
    /// Combined tool definitions for the orchestrator: skill tools from tools.json, plus read_skill when context mode is ReadOnDemand, plus delegate_task when at least one worker is configured (same list sent to the model).
    pub tools_list: Option<Vec<ToolDefinition>>,
    /// Generic executor built from skills' tools.json. None when no tools.
    pub tool_executor: Option<Arc<dyn agent::ToolExecutor>>,
    /// Paired devices (deviceId → role, scopes, deviceToken); used for deviceToken auth and issuing new tokens.
    pub pairing_store: Arc<PairingStore>,
    /// Matrix channel handle when Matrix is configured (HTTP verification + allowlist).
    #[cfg(feature = "matrix")]
    pub matrix_channel: Option<Arc<MatrixChannel>>,
    /// Per-worker bundles for `delegate_task` when `workerId` is set.
    pub worker_delegate_runtimes: Arc<HashMap<String, WorkerDelegateRuntime>>,
    /// Skill package root scanned at startup (`~/.chai/skills`); surfaced on `status` only.
    pub skills_discovery_root: PathBuf,
    /// Count of skill packages found on disk before orchestrator filtering.
    pub skill_packages_discovered: usize,
    /// Orchestrator **`AGENTS.md`** directory (`<profile>/agents/<orchestratorId>/`).
    pub orchestrator_context_dir: PathBuf,
}

/// Executor that handles read_skill (lookup by name, return SKILL.md content) and delegates all other tools to the generic executor. Used when context mode is ReadOnDemand.
struct ReadOnDemandExecutor {
    skills: Arc<Vec<Skill>>,
    inner: GenericToolExecutor,
}

impl agent::ToolExecutor for ReadOnDemandExecutor {
    fn execute(&self, name: &str, args: &serde_json::Value) -> Result<String, String> {
        if name == "read_skill" {
            let obj = args
                .as_object()
                .ok_or_else(|| "arguments must be an object".to_string())?;
            let skill_name = obj
                .get("skill_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing skill_name".to_string())?;
            let skill = self
                .skills
                .iter()
                .find(|s| s.name == skill_name)
                .ok_or_else(|| format!("unknown skill: {}", skill_name))?;
            Ok(strip_skill_frontmatter(&skill.content).to_string())
        } else {
            self.inner.execute(name, args)
        }
    }
}

impl GatewayState {
    /// References to configured provider clients for [`ProviderClients::as_dyn`] dispatch.
    fn provider_clients(&self) -> ProviderClients<'_> {
        ProviderClients {
            ollama: &self.ollama_client,
            lms: &self.lms_client,
            vllm: &self.vllm_client,
            nim: &self.nim_client,
            openai: &self.openai_client,
            hf: &self.hf_client,
        }
    }

    /// Register an in-process channel task to be awaited during graceful shutdown.
    #[allow(dead_code)]
    pub async fn register_channel_task(&self, handle: JoinHandle<()>) {
        self.channel_tasks.write().await.push(handle);
    }

    /// Combined tool list and executor (built at startup; list matches status/tools panel, including delegate_task when workers are configured).
    pub fn tools_and_executor(
        &self,
    ) -> (
        Option<Vec<ToolDefinition>>,
        Option<&dyn agent::ToolExecutor>,
    ) {
        let exec = self.tool_executor.as_deref();
        (self.tools_list.clone(), exec)
    }
}

/// Strip YAML frontmatter (`---` ... `---` blocks) from skill content so we don't duplicate it in the system message.
/// Removes consecutive frontmatter blocks (e.g. duplicated `---` blocks at the start of a SKILL.md).
fn strip_skill_frontmatter(content: &str) -> &str {
    let rest = content.trim_start();
    let rest = rest
        .strip_prefix("---")
        .map(|s| s.trim_start())
        .unwrap_or(rest);
    if let Some(i) = rest.find("\n---") {
        let after = rest.get(i + 4..).unwrap_or(rest).trim_start();
        if after.starts_with("---") {
            return strip_skill_frontmatter(after);
        }
        after
    } else {
        rest
    }
}

/// Build system context string from loaded skills (full SKILL.md per skill). Used when context mode is Full.
/// Format per spec: intro line, then for each skill: bullet line "- **name:**" + optional description, then skill body.
fn build_skill_context_full(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    out.push_str("## Skills\n\n");
    out.push_str("You have skills. Skills have tools. You can:\n\n");
    out.push_str("- call `read_skill` when you need to use a skill\n\n");
    for s in skills {
        out.push_str("### ");
        out.push_str(&s.name);
        out.push_str("\n\n");
        if !s.description.is_empty() {
            out.push_str(&s.description);
            out.push_str("\n\n");
        }
        out.push_str(strip_skill_frontmatter(&s.content));
        out.push_str("\n\n");
    }
    out
}

/// Build skill bodies only (no overview, no added headers). Used for desktop display in read-on-demand so the panel shows exactly what read_skill returns: each skill's body (frontmatter stripped), concatenated.
fn build_skill_bodies_only(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for s in skills {
        out.push_str(strip_skill_frontmatter(&s.content));
        out.push_str("\n\n");
    }
    out
}

/// Build compact skill list (name + description only). Used when context mode is ReadOnDemand; model uses read_skill to load full docs.
fn build_skill_context_compact(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    out.push_str("## Skills\n\n");
    out.push_str("You have skills. Skills have tools. You can:\n\n");
    out.push_str("- call `read_skill` when you need to use a skill\n\n");
    out.push_str("Available skills (name — description):\n\n");
    for s in skills {
        out.push_str("- `");
        out.push_str(&s.name);
        out.push_str("` — ");
        out.push_str(if s.description.is_empty() {
            "(no description)"
        } else {
            &s.description
        });
        out.push_str("\n\n");
    }
    out
}

/// Per-agent skill runtime object embedded in **`status.agents.entries[].skills`**.
fn skill_runtime_json(skills: &[Skill], mode: SkillContextMode) -> serde_json::Value {
    let skills_context = match mode {
        SkillContextMode::Full => build_skill_context_full(skills),
        SkillContextMode::ReadOnDemand => build_skill_context_compact(skills),
    };
    let skills_context_full = build_skill_context_full(skills);
    let skills_context_bodies = match mode {
        SkillContextMode::ReadOnDemand => build_skill_bodies_only(skills),
        SkillContextMode::Full => String::new(),
    };
    let enabled_skills: Vec<String> = skills.iter().map(|s| s.name.clone()).collect();
    let context_mode_wire = match mode {
        SkillContextMode::Full => "full",
        SkillContextMode::ReadOnDemand => "readOnDemand",
    };
    json!({
        "enabledSkills": enabled_skills,
        "contextMode": context_mode_wire,
        "skillsContext": skills_context,
        "skillsContextFull": skills_context_full,
        "skillsContextBodies": if skills_context_bodies.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(skills_context_bodies)
        },
    })
}

/// Build static system context from agent-ctx (AGENTS.md), worker roster, and skills, without a date prefix.
/// Uses context_mode to choose full vs compact skill context. The caller is responsible for
/// prepending the current date when building the final system message for a turn.
fn build_system_context_static(
    agent_ctx: Option<&str>,
    skills: &[Skill],
    context_mode: SkillContextMode,
    agents: &config::AgentsConfig,
    skill_catalog: &[SkillEntry],
) -> String {
    let mut out = String::new();
    if let Some(ctx) = agent_ctx {
        let trimmed = ctx.trim();
        if !trimmed.is_empty() {
            out.push_str(trimmed);
            out.push_str("\n\n");
        }
    }
    let workers_ctx = build_workers_context(agents, skill_catalog);
    if !workers_ctx.trim().is_empty() {
        out.push_str(&workers_ctx);
        out.push_str("\n");
    }
    let skills_ctx = match context_mode {
        SkillContextMode::Full => build_skill_context_full(skills),
        SkillContextMode::ReadOnDemand => build_skill_context_compact(skills),
    };
    if !skills_ctx.trim().is_empty() {
        out.push_str(&skills_ctx);
    }
    out
}

/// Worker static context: **`agents/<workerId>/AGENTS.md`** and skills only (no orchestrator roster).
fn build_worker_system_context_static(
    agent_ctx: Option<&str>,
    skills: &[Skill],
    context_mode: SkillContextMode,
) -> String {
    let mut out = String::new();
    if let Some(ctx) = agent_ctx {
        let trimmed = ctx.trim();
        if !trimmed.is_empty() {
            out.push_str(trimmed);
            out.push_str("\n\n");
        }
    }
    let skills_ctx = match context_mode {
        SkillContextMode::Full => build_skill_context_full(skills),
        SkillContextMode::ReadOnDemand => build_skill_context_compact(skills),
    };
    if !skills_ctx.trim().is_empty() {
        out.push_str(&skills_ctx);
    }
    out
}

struct BuiltSkillRuntime {
    skills: Vec<Skill>,
    tools_list: Option<Vec<ToolDefinition>>,
    tool_executor: Option<Arc<dyn agent::ToolExecutor>>,
}

fn build_skill_runtime_for_entries(
    skill_entries: Vec<SkillEntry>,
    context_mode: SkillContextMode,
    sandbox: Option<crate::exec::WriteSandbox>,
) -> BuiltSkillRuntime {
    let skills: Vec<Skill> = skill_entries.iter().map(Skill::from).collect();
    let descriptors: Vec<(String, crate::skills::ToolDescriptor)> = skill_entries
        .iter()
        .filter_map(|e| {
            e.tool_descriptor
                .as_ref()
                .map(|d| (e.name.clone(), d.clone()))
        })
        .collect();
    let skill_dirs: Vec<(String, std::path::PathBuf)> = skill_entries
        .iter()
        .filter_map(|e| {
            e.tool_descriptor
                .as_ref()
                .map(|_| (e.name.clone(), e.path.clone()))
        })
        .collect();
    let generic_executor =
        GenericToolExecutor::from_descriptors(&descriptors, &skill_dirs, sandbox);
    let mut skill_layer_tools: Vec<ToolDefinition> = Vec::new();
    if context_mode == SkillContextMode::ReadOnDemand && !skills.is_empty() {
        skill_layer_tools.push(read_skill_tool_definition());
    }
    for (_, desc) in &descriptors {
        skill_layer_tools.extend(desc.to_tool_definitions());
    }
    let has_skill_tools = !skill_layer_tools.is_empty();
    let tool_executor: Option<Arc<dyn agent::ToolExecutor>> = if has_skill_tools {
        if context_mode == SkillContextMode::ReadOnDemand && !skills.is_empty() {
            Some(Arc::new(ReadOnDemandExecutor {
                skills: Arc::new(skills.clone()),
                inner: generic_executor,
            }))
        } else {
            Some(Arc::new(generic_executor))
        }
    } else {
        None
    };
    let tools_list = if skill_layer_tools.is_empty() {
        None
    } else {
        Some(skill_layer_tools)
    };
    BuiltSkillRuntime {
        skills,
        tools_list,
        tool_executor,
    }
}

/// Build full system context for a turn: date line, capability hints, then cached static context.
/// Pass **`workers_enabled: None`** for worker agent strings so **`WORKERS_ENABLED`** is omitted.
fn build_system_context_for_today(
    static_ctx: &str,
    workers_enabled: Option<bool>,
    skills_enabled: bool,
) -> String {
    system_context_with_today(static_ctx, workers_enabled, skills_enabled)
}

/// Tool definition for read_skill (used only when context mode is ReadOnDemand).
fn read_skill_tool_definition() -> ToolDefinition {
    ToolDefinition {
        typ: "function".to_string(),
        function: crate::providers::ToolFunctionDefinition {
            name: "read_skill".to_string(),
            description: Some("read a skill by name".to_string()),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["skill_name"],
                "properties": {
                    "skill_name": {
                        "type": "string",
                        "description": "the name of the skill"
                    }
                }
            }),
        },
    }
}

/// Reply text to send to the channel. Matches OpenClaw: send the model's content only; when empty (e.g. tool-calls-only or silent), no placeholder — caller may skip sending.
fn channel_reply_text(result: &agent::AgentTurnResult) -> Option<String> {
    let text = result.content.trim();
    if text.is_empty() {
        None
    } else {
        Some(result.content.clone())
    }
}

/// Message that starts a new session (clear history) when sent via Telegram or other channels. Case-insensitive.
const NEW_SESSION_TRIGGER: &str = "/new";

/// Broadcast a session.message event over WebSocket to connected clients.
fn broadcast_session_message(
    state: &GatewayState,
    session_id: &str,
    role: &str,
    content: &str,
    tool_calls: Option<&[crate::providers::ToolCall]>,
    tool_results: Option<&[String]>,
    channel_id: Option<&str>,
    conversation_id: Option<&str>,
) {
    let payload = if let Some(calls) = tool_calls {
        let tool_calls_value = serde_json::to_value(calls).unwrap_or_else(|_| json!([]));
        let tool_results_value = tool_results
            .map(|rs| serde_json::to_value(rs).unwrap_or_else(|_| json!([])))
            .unwrap_or_else(|| json!([]));
        json!({
            "sessionId": session_id,
            "role": role,
            "content": content,
            "channelId": channel_id,
            "conversationId": conversation_id,
            "toolCalls": tool_calls_value,
            "toolResults": tool_results_value,
        })
    } else {
        json!({
            "sessionId": session_id,
            "role": role,
            "content": content,
            "channelId": channel_id,
            "conversationId": conversation_id,
        })
    };
    let event = json!({
        "type": "event",
        "event": "session.message",
        "payload": payload,
    });
    if let Ok(text) = serde_json::to_string(&event) {
        let _ = state.event_tx.send(text);
    }
}

/// Process one inbound channel message: get or create session, bind, append user message, run agent, send reply.
/// If the message is the new-session trigger (e.g. /new), rebind the conversation to a fresh session and confirm.
async fn process_inbound_message(state: GatewayState, msg: InboundMessage) {
    let trimmed = msg.text.trim();
    if trimmed.eq_ignore_ascii_case(NEW_SESSION_TRIGGER) {
        let old_id = state
            .bindings
            .get_session_id(&msg.channel_id, &msg.conversation_id)
            .await;
        let new_id = state.session_store.create().await;
        state
            .bindings
            .bind(&msg.channel_id, &msg.conversation_id, &new_id)
            .await;
        if let Some(id) = old_id {
            state.session_store.remove(&id).await;
        }
        if let Some(handle) = state.channel_registry.get(&msg.channel_id).await {
            let _ = handle
                .send_message(
                    &msg.conversation_id,
                    "session restarted. next message will start with a clean history.",
                )
                .await;
        }
        return;
    }

    let session_id = state
        .bindings
        .get_session_id(&msg.channel_id, &msg.conversation_id)
        .await;
    let session_id = match session_id {
        Some(id) => id,
        None => {
            let id = state.session_store.create().await;
            state
                .bindings
                .bind(&msg.channel_id, &msg.conversation_id, &id)
                .await;
            id
        }
    };
    if state
        .session_store
        .append_message(&session_id, "user", &msg.text)
        .await
        .is_err()
    {
        log::warn!("inbound: failed to append message");
        return;
    }
    broadcast_session_message(
        &state,
        &session_id,
        "user",
        &msg.text,
        None,
        None,
        Some(&msg.channel_id),
        Some(&msg.conversation_id),
    );
    let provider_choice = resolve_provider_choice(&state.config.agents);
    let model_name = resolve_model(
        state.config.agents.default_model.as_deref(),
        None,
        provider_choice,
    );
    let has_workers = !state.worker_delegate_runtimes.is_empty();
    let orch_skills_enabled = !state.skills.is_empty();
    let system_context = build_system_context_for_today(
        &state.system_context_static,
        Some(has_workers),
        orch_skills_enabled,
    );
    let (tools, tool_executor) = state.tools_and_executor();
    let tools = merge_delegate_task(tools, has_workers);
    let worker_tools = worker_tool_list(tools.as_ref());
    let delegate = Some(DelegateContext {
        clients: state.provider_clients(),
        agents: &state.config.agents,
        orchestrator_system_context: if system_context.trim().is_empty() {
            None
        } else {
            Some(system_context.as_str())
        },
        orchestrator_worker_tools: worker_tools,
        orchestrator_tool_executor: tool_executor,
        worker_runtimes: Some(state.worker_delegate_runtimes.as_ref()),
        observability: Some(DelegateObservability {
            event_tx: state.event_tx.clone(),
            session_id: Some(session_id.clone()),
        }),
        session_store: Some(&state.session_store),
        session_id: Some(session_id.as_str()),
    });
    let result = agent::run_turn_dyn(
        &state.session_store,
        &session_id,
        state.provider_clients().as_dyn(provider_choice),
        &model_name,
        Some(&system_context),
        state.config.agents.max_session_messages,
        tools,
        tool_executor,
        delegate,
        None,
    )
    .await;
    let result = match result {
        Ok(r) => r,
        Err(e) => {
            log::warn!("inbound: agent turn failed: {}", e);
            let fallback = format!(
                "something went wrong: {}. check the gateway logs for details.",
                e
            );
            if let Some(handle) = state.channel_registry.get(&msg.channel_id).await {
                let _ = handle.send_message(&msg.conversation_id, &fallback).await;
            }
            return;
        }
    };
    if let Some(reply) = channel_reply_text(&result) {
        broadcast_session_message(
            &state,
            &session_id,
            "assistant",
            &reply,
            if result.tool_calls.is_empty() {
                None
            } else {
                Some(&result.tool_calls[..])
            },
            if result.tool_calls.is_empty() {
                None
            } else {
                Some(&result.tool_results[..])
            },
            Some(&msg.channel_id),
            Some(&msg.conversation_id),
        );
        if let Some(handle) = state.channel_registry.get(&msg.channel_id).await {
            if handle
                .send_message(&msg.conversation_id, &reply)
                .await
                .is_err()
            {
                log::warn!("inbound: send_message failed");
            }
        }
    }
}

/// Run the gateway server; binds to config.gateway.bind:config.gateway.port.
/// When bind is not loopback, a gateway token must be configured or startup fails.
/// Blocks until shutdown (e.g. Ctrl+C).
/// Requires `chai init` so the profile config and shared `~/.chai/skills` exist.
pub async fn run_gateway(config: Config, paths: ChaiPaths) -> Result<()> {
    init::require_initialized(&paths)?;
    let bind = config.gateway.bind.trim();
    if !config::is_loopback_bind(bind) {
        let token = config::resolve_gateway_token(&config);
        if token.is_none() || config.gateway.auth.mode != config::GatewayAuthMode::Token {
            anyhow::bail!(
                "refusing to bind gateway to {} without auth (set gateway.auth.mode to \"token\" and gateway.auth.token or CHAI_GATEWAY_TOKEN)",
                bind
            );
        }
    }

    let required_token = require_connect_token(&config);
    let paired_path = paths.paired_json();
    let pairing_store = Arc::new(PairingStore::load(paired_path).await);
    let (event_tx, _) = broadcast::channel(64);
    let channel_tasks = Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let ollama_models = Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let lms_base_url = Some(resolve_lms_base_url(&config));
    let lms_client = LmsClient::new(lms_base_url);
    let lms_models = Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let nim_api_key = resolve_nim_api_key(&config);
    let nim_client = NimClient::new(nim_api_key.clone());
    let nim_models = Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let vllm_base = Some(resolve_vllm_base_url(&config));
    let vllm_api_key = resolve_vllm_api_key(&config);
    let vllm_client = VllmClient::new(vllm_base, vllm_api_key.clone());
    let vllm_models = Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let openai_base = Some(resolve_openai_base_url(&config));
    let openai_api_key = resolve_openai_api_key(&config);
    let openai_client = OpenAiClient::new(openai_base, openai_api_key.clone());
    let openai_models = Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let hf_base = Some(resolve_hf_base_url(&config));
    let hf_api_key = resolve_hf_api_key(&config);
    let hf_client = HfClient::new(hf_base, hf_api_key.clone());
    let hf_models = Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let (inbound_tx, mut inbound_rx) = mpsc::channel::<InboundMessage>(64);

    let orch_context_dir = config::orchestrator_context_dir(&config, &paths.profile_dir);
    let skills_dir = config::default_skills_dir(&paths.chai_home);
    let all_entries: Vec<SkillEntry> = match load_skills(skills_dir.as_path()) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!("loading skills failed: {}", e);
            Vec::new()
        }
    };
    log::info!("discovered {} skill package(s) on disk", all_entries.len());

    // Verify skill versions against lockfile
    {
        let mut all_enabled: Vec<&str> = config::orchestrator_skills_enabled_list(&config.agents)
            .iter()
            .map(|s| s.as_str())
            .collect();
        if let Some(workers) = &config.agents.workers {
            for w in workers {
                for name in config::worker_skills_enabled_list(w) {
                    if !all_enabled.contains(&name.as_str()) {
                        all_enabled.push(name.as_str());
                    }
                }
            }
        }
        crate::skills::lockfile::verify_at_startup(
            &all_entries,
            &all_enabled,
            &paths.profile_dir,
            config.skill_lock_mode,
        )?;
    }

    let orch_names = config::orchestrator_skills_enabled_list(&config.agents);
    let orchestrator_entries: Vec<SkillEntry> = all_entries
        .iter()
        .filter(|e| orch_names.iter().any(|n| n == &e.name))
        .cloned()
        .collect();
    log::info!(
        "orchestrator: {} skill package(s) enabled",
        orchestrator_entries.len()
    );
    validate_skill_composition(
        "orchestrator",
        &orchestrator_entries,
        config.agents.default_model.as_deref(),
    );
    let orch_ctx_mode = orchestrator_context_mode(&config.agents);
    if orch_ctx_mode == SkillContextMode::ReadOnDemand {
        log::info!(
            "orchestrator skill context mode: readOnDemand (compact list + read_skill tool)"
        );
    }
    let sandbox = crate::exec::WriteSandbox::new(&paths.sandbox_dir());
    let sandbox_opt = if sandbox.has_roots() {
        log::info!(
            "write sandbox: {} writable root(s) from {}",
            sandbox.roots().len(),
            paths.sandbox_dir().display()
        );
        Some(sandbox)
    } else {
        log::debug!(
            "write sandbox: no sandbox directory at {}",
            paths.sandbox_dir().display()
        );
        None
    };
    let orch_built =
        build_skill_runtime_for_entries(orchestrator_entries, orch_ctx_mode, sandbox_opt.clone());
    let skills = orch_built.skills.clone();
    let agent_ctx = agent_ctx::load_agent_ctx(Some(orch_context_dir.as_path()));
    let system_context_static = build_system_context_static(
        agent_ctx.as_deref(),
        &skills,
        orch_ctx_mode,
        &config.agents,
        &all_entries,
    );

    let has_workers = config
        .agents
        .workers
        .as_ref()
        .map_or(false, |w| !w.is_empty());
    let tools_list = merge_delegate_task(orch_built.tools_list.clone(), has_workers);

    let tool_executor = orch_built.tool_executor.clone();

    let mut worker_map: HashMap<String, WorkerDelegateRuntime> = HashMap::new();
    if let Some(workers) = &config.agents.workers {
        for w in workers {
            let w_dir = config::worker_context_dir(w, &paths.profile_dir);
            let w_agent_ctx = agent_ctx::load_agent_ctx(w_dir.as_deref());
            let w_names = config::worker_skills_enabled_list(w);
            let w_entries: Vec<SkillEntry> = all_entries
                .iter()
                .filter(|e| w_names.iter().any(|n| n == &e.name))
                .cloned()
                .collect();
            let w_label = format!("worker:{}", w.id);
            validate_skill_composition(&w_label, &w_entries, w.default_model.as_deref());
            let w_ctx_mode = worker_context_mode(w);
            let w_built =
                build_skill_runtime_for_entries(w_entries, w_ctx_mode, sandbox_opt.clone());
            let w_static = build_worker_system_context_static(
                w_agent_ctx.as_deref(),
                &w_built.skills,
                w_ctx_mode,
            );
            worker_map.insert(
                w.id.clone(),
                WorkerDelegateRuntime {
                    system_context_static: w_static,
                    context_directory: w_dir.clone(),
                    skills: Arc::new(w_built.skills),
                    tools_list: w_built.tools_list,
                    tool_executor: w_built.tool_executor,
                    context_mode: w_ctx_mode,
                },
            );
        }
    }
    let worker_delegate_runtimes = Arc::new(worker_map);

    #[cfg_attr(not(feature = "matrix"), allow(unused_mut))]
    let mut state = GatewayState {
        config: Arc::new(config.clone()),
        system_context_static,
        required_token,
        event_tx: event_tx.clone(),
        channel_tasks: channel_tasks.clone(),
        inbound_tx: inbound_tx.clone(),
        session_store: Arc::new(SessionStore::new()),
        channel_registry: Arc::new(ChannelRegistry::new()),
        bindings: Arc::new(SessionBindingStore::new()),
        ollama_client: OllamaClient::new(resolve_ollama_base_url(&config)),
        ollama_models: ollama_models.clone(),
        lms_client,
        lms_models: lms_models.clone(),
        nim_client,
        nim_models: nim_models.clone(),
        vllm_client,
        vllm_models: vllm_models.clone(),
        openai_client,
        openai_models: openai_models.clone(),
        hf_client,
        hf_models: hf_models.clone(),
        skills: Arc::new(skills),
        tools_list,
        tool_executor,
        pairing_store,
        #[cfg(feature = "matrix")]
        matrix_channel: None,
        worker_delegate_runtimes,
        skills_discovery_root: skills_dir,
        skill_packages_discovered: all_entries.len(),
        orchestrator_context_dir: orch_context_dir,
    };

    if config::provider_discovery_enabled(&config.agents, "ollama") {
        let ollama = state.ollama_client.clone();
        let models = state.ollama_models.clone();
        tokio::spawn(async move {
            match ollama.list_models().await {
                Ok(list) => {
                    *models.write().await = list;
                    log::info!("ollama model discovery completed");
                }
                Err(e) => {
                    log::debug!("ollama model discovery failed: {}", e);
                }
            }
        });
    } else {
        log::debug!("ollama model discovery skipped (not in enabledProviders)");
    }
    if config::provider_discovery_enabled(&config.agents, "lms") {
        let lms = state.lms_client.clone();
        let models = state.lms_models.clone();
        tokio::spawn(async move {
            match lms.list_models().await {
                Ok(list) => {
                    *models.write().await = list;
                    log::info!("lms model discovery completed");
                }
                Err(e) => {
                    log::debug!("lms model discovery failed: {}", e);
                }
            }
        });
    } else {
        log::debug!("lms model discovery skipped (not in enabledProviders)");
    }
    if config::provider_discovery_enabled(&config.agents, "vllm") {
        let vllm = state.vllm_client.clone();
        let models = state.vllm_models.clone();
        tokio::spawn(async move {
            match vllm.list_models().await {
                Ok(list) => {
                    *models.write().await = list;
                    log::info!("vllm model discovery completed");
                }
                Err(e) => {
                    log::debug!("vllm model discovery failed: {}", e);
                }
            }
        });
    } else {
        log::debug!("vllm model discovery skipped (not in enabledProviders)");
    }
    if config::provider_discovery_enabled(&config.agents, "nim") {
        let models = nim_models.clone();
        let cfg_for_nim = config.clone();
        tokio::spawn(async move {
            *models.write().await = NimClient::gateway_model_list(&cfg_for_nim);
            log::info!("nim model list loaded (static catalog plus optional extraModels)");
        });
    }
    if config::provider_discovery_enabled(&config.agents, "openai") {
        let openai = state.openai_client.clone();
        let models = state.openai_models.clone();
        tokio::spawn(async move {
            match openai.list_models().await {
                Ok(list) => {
                    *models.write().await = list;
                    log::info!("openai model discovery completed");
                }
                Err(e) => {
                    log::debug!("openai model discovery failed: {}", e);
                }
            }
        });
    } else {
        log::debug!("openai model discovery skipped (not in enabledProviders)");
    }
    if config::provider_discovery_enabled(&config.agents, "hf") {
        let hf = state.hf_client.clone();
        let models = state.hf_models.clone();
        tokio::spawn(async move {
            match hf.list_models().await {
                Ok(list) => {
                    *models.write().await = list;
                    log::info!("huggingface model discovery completed");
                }
                Err(e) => {
                    log::debug!("huggingface model discovery failed: {}", e);
                }
            }
        });
    } else {
        log::debug!("huggingface model discovery skipped (not in enabledProviders)");
    }
    if resolve_provider_choice(&config.agents) == ProviderChoice::Nim {
        log::warn!(
            "NVIDIA NIM hosted API is enabled; this is not a privacy-preserving option. Requests and data are sent to NVIDIA servers. Free tier is rate-limited (~40 requests/min)."
        );
        if nim_api_key.is_none() || nim_api_key.as_ref().map(|k| k.is_empty()).unwrap_or(true) {
            log::warn!("NIM provider selected but no API key set (providers.nim.apiKey or NVIDIA_API_KEY). Requests will fail until a key is configured.");
        }
    }
    if resolve_provider_choice(&config.agents) == ProviderChoice::OpenAi {
        log::warn!(
            "OpenAI API is enabled; requests and data are sent to OpenAI (not a local-first option)."
        );
        if openai_api_key.is_none()
            || openai_api_key
                .as_ref()
                .map(|k| k.is_empty())
                .unwrap_or(true)
        {
            log::warn!("openai provider selected but no API key set (providers.openai.apiKey or OPENAI_API_KEY). Requests will fail until a key is configured.");
        }
    }
    if resolve_provider_choice(&config.agents) == ProviderChoice::Hf {
        log::info!(
            "huggingface provider selected; set providers.hf.baseUrl to your OpenAI-compatible endpoint (Inference Endpoints or TGI) including /v1."
        );
        if resolve_hf_base_url(&config) == "http://127.0.0.1:8080/v1" {
            log::warn!("hf provider uses default base URL http://127.0.0.1:8080/v1; set providers.hf.baseUrl to your deployment unless testing locally.");
        }
    }

    {
        let state_inbound = state.clone();
        tokio::spawn(async move {
            while let Some(msg) = inbound_rx.recv().await {
                process_inbound_message(state_inbound.clone(), msg).await;
            }
        });
    }

    let telegram_token = config::resolve_telegram_token(&config);
    let webhook_url = config.channels.telegram.webhook_url.clone();
    let telegram_webhook_for_shutdown: Option<Arc<TelegramChannel>> =
        if let Some(token) = telegram_token {
            let telegram = Arc::new(TelegramChannel::new(
                Some(token),
                if webhook_url.is_some() {
                    TelegramTransport::Webhook
                } else {
                    TelegramTransport::LongPoll
                },
            ));
            if let Some(ref url) = webhook_url {
                let webhook_secret = resolve_telegram_webhook_secret(&config);
                let secret = webhook_secret.as_deref();
                if let Err(e) = telegram.set_webhook(url, secret).await {
                    log::warn!("telegram set_webhook failed: {}", e);
                } else {
                    log::info!("telegram channel registered (webhook mode): {}", url);
                }
                state
                    .channel_registry
                    .register(telegram.id().to_string(), telegram.clone())
                    .await;
                Some(telegram)
            } else {
                let handle = telegram.clone().start_inbound(inbound_tx.clone());
                state.channel_tasks.write().await.push(handle);
                state
                    .channel_registry
                    .register(telegram.id().to_string(), telegram)
                    .await;
                log::info!("telegram channel registered and getUpdates loop started");
                None
            }
        } else {
            None
        };

    #[cfg(feature = "matrix")]
    {
        if let Some(matrix) = connect_matrix_client(&config, paths.profile_dir.as_path()).await {
            let matrix = Arc::new(matrix);
            state.matrix_channel = Some(matrix.clone());
            let handle = matrix.clone().start_inbound(inbound_tx.clone());
            state.channel_tasks.write().await.push(handle);
            state
                .channel_registry
                .register(matrix.id().to_string(), matrix)
                .await;
            log::info!("matrix channel registered and sync loop started");
        }
    }

    if let Some(sig_cfg) = resolve_signal_daemon_config(&config) {
        let signal = Arc::new(SignalChannel::new(sig_cfg));
        let handle = signal.clone().start_inbound(inbound_tx.clone());
        state.channel_tasks.write().await.push(handle);
        state
            .channel_registry
            .register(signal.id().to_string(), signal)
            .await;
        log::info!("signal channel registered and SSE events loop started");
    }

    let channel_registry = state.channel_registry.clone();
    let app = Router::new()
        .route("/", get(health_http))
        .route("/ws", get(ws_handler))
        .route("/telegram/webhook", post(telegram_webhook));
    #[cfg(feature = "matrix")]
    let app = app
        .route(
            "/matrix/verification/pending",
            get(matrix_routes::matrix_verification_pending),
        )
        .route(
            "/matrix/verification/accept",
            post(matrix_routes::matrix_verification_accept),
        )
        .route(
            "/matrix/verification/start-sas",
            post(matrix_routes::matrix_verification_start_sas),
        )
        .route(
            "/matrix/verification/sas",
            get(matrix_routes::matrix_verification_sas),
        )
        .route(
            "/matrix/verification/confirm",
            post(matrix_routes::matrix_verification_confirm),
        )
        .route(
            "/matrix/verification/mismatch",
            post(matrix_routes::matrix_verification_mismatch),
        )
        .route(
            "/matrix/verification/cancel",
            post(matrix_routes::matrix_verification_cancel),
        );
    let app = app.with_state(state);

    let bind_addr = format!("{}:{}", bind, config.gateway.port);
    let _gateway_lock = profile::acquire_gateway_lock(&paths.chai_home, &paths.profile_name)
        .context("acquire gateway lock")?;
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("binding to {}", bind_addr))?;
    log::info!("gateway listening on {}", bind_addr);

    let serve_result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(
            event_tx,
            channel_registry,
            channel_tasks,
            telegram_webhook_for_shutdown,
        ))
        .await;
    drop(_gateway_lock);
    serve_result.context("gateway server exited")?;
    log::info!("gateway stopped");
    Ok(())
}

/// Future that completes when the process should shut down (SIGINT or SIGTERM).
/// Broadcasts a shutdown event to WebSocket clients, stops channel connectors, removes Telegram webhook if used, then awaits in-process channel tasks.
async fn shutdown_signal(
    event_tx: broadcast::Sender<String>,
    channel_registry: Arc<ChannelRegistry>,
    channel_tasks: Arc<tokio::sync::RwLock<Vec<JoinHandle<()>>>>,
    telegram_webhook: Option<Arc<TelegramChannel>>,
) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    log::info!("shutdown signal received, broadcasting shutdown and draining connections");

    let _ = event_tx.send(SHUTDOWN_EVENT_JSON.to_string());

    for id in channel_registry.ids().await {
        if let Some(handle) = channel_registry.get(&id).await {
            handle.stop();
        }
    }

    if let Some(t) = telegram_webhook {
        if let Err(e) = t.delete_webhook().await {
            log::debug!("telegram delete_webhook on shutdown: {}", e);
        }
    }

    let handles = {
        let mut g = channel_tasks.write().await;
        std::mem::take(&mut *g)
    };
    for h in handles {
        let _ = h.await;
    }
    log::info!("channel tasks finished");
}

/// POST /telegram/webhook — receives Telegram update JSON; verifies optional secret, pushes InboundMessage.
async fn telegram_webhook(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    if let Some(ref expected) = config::resolve_telegram_webhook_secret(&state.config) {
        let provided = headers
            .get("X-Telegram-Bot-Api-Secret-Token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if provided != expected.as_str() {
            return StatusCode::FORBIDDEN;
        }
    }
    let update: TelegramUpdate = match serde_json::from_slice(&body) {
        Ok(u) => u,
        Err(_) => return StatusCode::BAD_REQUEST,
    };
    let Some(ref msg) = update.message else {
        return StatusCode::OK;
    };
    let Some(ref text) = msg.text else {
        return StatusCode::OK;
    };
    let inbound = InboundMessage {
        channel_id: "telegram".to_string(),
        conversation_id: msg.chat.id.to_string(),
        text: text.clone(),
    };
    if state.inbound_tx.send(inbound).await.is_err() {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    StatusCode::OK
}

/// GET / returns a simple health JSON (for probes).
async fn health_http(State(state): State<GatewayState>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "running",
        "protocol": PROTOCOL_VERSION,
        "port": state.config.gateway.port,
    }))
}

fn non_empty_cfg_opt(s: &Option<String>) -> bool {
    s.as_ref().map(|x| !x.trim().is_empty()).unwrap_or(false)
}

/// True when Matrix homeserver and credentials are present (env or file); does not imply the client connected.
fn matrix_channel_configured(cfg: &Config) -> bool {
    let homeserver = std::env::var("MATRIX_HOMESERVER")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            cfg.channels
                .matrix
                .homeserver
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    if homeserver.is_none() {
        return false;
    }
    let token = std::env::var("MATRIX_ACCESS_TOKEN")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            cfg.channels
                .matrix
                .access_token
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    if token.is_some() {
        return true;
    }
    let user = std::env::var("MATRIX_USER")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            cfg.channels
                .matrix
                .user
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    let password = std::env::var("MATRIX_PASSWORD")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            cfg.channels
                .matrix
                .password
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    user.is_some() && password.is_some()
}

fn merge_channel_runtime_detail(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    details: &std::collections::HashMap<String, serde_json::Value>,
    channel_id: &str,
) {
    let Some(v) = details.get(channel_id) else {
        return;
    };
    let Some(sub) = v.as_object() else {
        return;
    };
    for (k, val) in sub {
        obj.insert(k.clone(), val.clone());
    }
}

/// GET /ws upgrades to WebSocket. First frame must be connect; we reply with hello-ok.
async fn ws_handler(State(state): State<GatewayState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: GatewayState) {
    let mut sent_hello = false;
    let mut event_rx = state.event_tx.subscribe();

    let nonce = uuid::Uuid::new_v4().to_string();
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let challenge_json = connect_challenge_event(&nonce, ts_ms);
    if socket.send(Message::Text(challenge_json)).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            biased;

            event = event_rx.recv() => {
                match event {
                    Ok(text) => {
                        let is_shutdown = text == SHUTDOWN_EVENT_JSON;
                        let _ = socket.send(Message::Text(text)).await;
                        if is_shutdown {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::debug!("ws client lagged {} broadcast messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            msg = socket.recv() => {
                let Some(Ok(msg)) = msg else { break };
                let Message::Text(text) = msg else { continue };
                let Ok(req): Result<WsRequest, _> = serde_json::from_str(&text) else { continue };

                if req.typ != "req" {
                    continue;
                }

                match req.method.as_str() {
            "connect" => {
                let params: ConnectParams = match serde_json::from_value(req.params.clone()) {
                    Ok(p) => p,
                    Err(_) => {
                        let res = WsResponse::err(&req.id, "invalid connect params");
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                        continue;
                    }
                };
                let auth_for_hello: Option<HelloAuth> = if let Some(ref token) = params.auth.device_token {
                    match state.pairing_store.get_by_token(token).await {
                        Some(entry) => Some(HelloAuth {
                            device_token: entry.device_token,
                            role: entry.role,
                            scopes: entry.scopes,
                        }),
                        None => {
                            let res = WsResponse::err(&req.id, "invalid device token");
                            let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                            continue;
                        }
                    }
                } else if let Some(ref device) = params.device {
                    if let Err(e) = verify_device_signature(device, &params, &nonce) {
                        log::debug!("device signature verification failed: {}", e);
                        let res = WsResponse::err(&req.id, e);
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                        continue;
                    }
                    if let Some(entry) = state.pairing_store.get_by_device_id(&device.id).await {
                        Some(HelloAuth {
                            device_token: entry.device_token,
                            role: entry.role,
                            scopes: entry.scopes,
                        })
                    } else {
                        let token_ok = state.required_token.as_ref().map_or(true, |required| {
                            params.auth.token.as_deref().map_or(false, |t| t.trim() == required)
                        });
                        if !token_ok {
                            let res = WsResponse::err(
                                &req.id,
                                "pairing required: provide gateway token to approve this device",
                            );
                            let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                            continue;
                        }
                        let new_token = uuid::Uuid::new_v4().to_string();
                        if state
                            .pairing_store
                            .add_or_update(
                                device.id.clone(),
                                params.role.clone(),
                                params.scopes.clone(),
                                new_token.clone(),
                            )
                            .await
                            .is_err()
                        {
                            log::warn!("failed to persist pairing store");
                        }
                        Some(HelloAuth {
                            device_token: new_token,
                            role: params.role.clone(),
                            scopes: params.scopes.clone(),
                        })
                    }
                } else {
                    if let Some(ref required) = state.required_token {
                        let provided = params.auth.token.as_deref().unwrap_or("").trim();
                        if provided.is_empty() {
                            let res = WsResponse::err(
                                &req.id,
                                "unauthorized: gateway token missing (set CHAI_GATEWAY_TOKEN or gateway.auth.token)",
                            );
                            let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                            continue;
                        }
                        if provided != required {
                            let res = WsResponse::err(&req.id, "unauthorized: gateway token mismatch");
                            let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                            continue;
                        }
                    }
                    None
                };
                let protocol = params.max_protocol.unwrap_or(PROTOCOL_VERSION).min(PROTOCOL_VERSION);
                let hello = HelloOk {
                    typ: "hello-ok".to_string(),
                    protocol,
                    policy: Some(crate::gateway::protocol::HelloPolicy {
                        tick_interval_ms: Some(15_000),
                    }),
                    auth: auth_for_hello,
                };
                let res = WsResponse::ok(&req.id, serde_json::to_value(&hello).unwrap_or(json!({})));
                if socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await.is_ok() {
                    sent_hello = true;
                }
            }
            "health" => {
                let payload = json!({
                    "status": "running",
                    "protocol": PROTOCOL_VERSION,
                });
                let res = WsResponse::ok(&req.id, payload);
                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
            }
            "status" => {
                let auth_mode = if state.required_token.is_some() {
                    "token"
                } else {
                    "none"
                };
                let provider_choice = resolve_provider_choice(&state.config.agents);
                let default_model = resolve_model(
                    state.config.agents.default_model.as_deref(),
                    None,
                    provider_choice,
                );
                let mut ollama_models = state.ollama_models.read().await.clone();
                let mut lms_models = state.lms_models.read().await.clone();
                let mut vllm_models = state.vllm_models.read().await.clone();
                let mut nim_models = state.nim_models.read().await.clone();
                let mut openai_models = state.openai_models.read().await.clone();
                let mut hf_models = state.hf_models.read().await.clone();
                ollama_models.sort_by(|a, b| a.name.cmp(&b.name));
                lms_models.sort_by(|a, b| a.name.cmp(&b.name));
                vllm_models.sort_by(|a, b| a.name.cmp(&b.name));
                nim_models.sort_by(|a, b| a.name.cmp(&b.name));
                openai_models.sort_by(|a, b| a.name.cmp(&b.name));
                hf_models.sort_by(|a, b| a.name.cmp(&b.name));
                let has_workers_status = !state.worker_delegate_runtimes.is_empty();
                let orch_skills_enabled = !state.skills.is_empty();
                let system_context = build_system_context_for_today(
                    &state.system_context_static,
                    Some(has_workers_status),
                    orch_skills_enabled,
                );
                let orch_mode = orchestrator_context_mode(&state.config.agents);
                let tools_string = state
                    .tools_list
                    .as_ref()
                    .and_then(|tools| {
                        if tools.is_empty() {
                            None
                        } else {
                            serde_json::to_string_pretty(tools).ok()
                        }
                    });
                let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                let orchestration_catalog = build_orchestration_catalog(
                    &state.config.agents,
                    &ollama_models,
                    &lms_models,
                    &vllm_models,
                    &nim_models,
                    &openai_models,
                    &hf_models,
                );
                let orchestrator_id = state
                    .config
                    .agents
                    .orchestrator_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("orchestrator");
                let worker_defaults: HashMap<String, (String, String)> = state
                    .config
                    .agents
                    .workers
                    .as_ref()
                    .map(|ws| {
                        ws.iter()
                            .filter_map(|w| {
                                let id = w.id.trim();
                                if id.is_empty() {
                                    return None;
                                }
                                let pair = effective_worker_defaults(&state.config.agents, w);
                                Some((id.to_string(), pair))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let discovery_ids = config::discovery_enabled_provider_ids(&state.config.agents);
                let reg_ids = state.channel_registry.ids().await;
                let active: HashSet<&str> = reg_ids.iter().map(|s| s.as_str()).collect();
                let cfg_ref = state.config.as_ref();
                let telegram_configured = config::resolve_telegram_token(cfg_ref).is_some()
                    || non_empty_cfg_opt(&cfg_ref.channels.telegram.webhook_url);
                #[cfg(feature = "matrix")]
                let matrix_active = active.contains("matrix");
                #[cfg(not(feature = "matrix"))]
                let matrix_active = false;
                let matrix_configured = matrix_channel_configured(cfg_ref);
                let signal_configured = resolve_signal_daemon_config(cfg_ref).is_some();
                let channel_runtime = state.channel_registry.channel_status_details().await;
                let mut telegram_ch = serde_json::Map::new();
                telegram_ch.insert("active".into(), json!(active.contains("telegram")));
                telegram_ch.insert("configured".into(), json!(telegram_configured));
                merge_channel_runtime_detail(&mut telegram_ch, &channel_runtime, "telegram");
                let mut matrix_ch = serde_json::Map::new();
                matrix_ch.insert("active".into(), json!(matrix_active));
                matrix_ch.insert("configured".into(), json!(matrix_configured));
                merge_channel_runtime_detail(&mut matrix_ch, &channel_runtime, "matrix");
                let mut signal_ch = serde_json::Map::new();
                signal_ch.insert("active".into(), json!(active.contains("signal")));
                signal_ch.insert("configured".into(), json!(signal_configured));
                merge_channel_runtime_detail(&mut signal_ch, &channel_runtime, "signal");
                let channels_block = json!({
                    "telegram": serde_json::Value::Object(telegram_ch),
                    "matrix": serde_json::Value::Object(matrix_ch),
                    "signal": serde_json::Value::Object(signal_ch),
                });
                let agents_cfg = &state.config.agents;
                let provider_entry =
                    |id: &str, models: &serde_json::Value| -> serde_json::Value {
                        let on = config::provider_discovery_enabled(agents_cfg, id);
                        json!({
                            "discovery": on,
                            "models": if on { models.clone() } else { json!([]) },
                        })
                    };
                let mut providers_map = serde_json::Map::new();
                providers_map.insert(
                    "ollama".into(),
                    provider_entry(
                        "ollama",
                        &serde_json::to_value(&ollama_models).unwrap_or_else(|_| json!([])),
                    ),
                );
                providers_map.insert(
                    "lms".into(),
                    provider_entry(
                        "lms",
                        &serde_json::to_value(&lms_models).unwrap_or_else(|_| json!([])),
                    ),
                );
                providers_map.insert(
                    "vllm".into(),
                    provider_entry(
                        "vllm",
                        &serde_json::to_value(&vllm_models).unwrap_or_else(|_| json!([])),
                    ),
                );
                providers_map.insert(
                    "nim".into(),
                    provider_entry(
                        "nim",
                        &serde_json::to_value(&nim_models).unwrap_or_else(|_| json!([])),
                    ),
                );
                providers_map.insert(
                    "openai".into(),
                    provider_entry(
                        "openai",
                        &serde_json::to_value(&openai_models).unwrap_or_else(|_| json!([])),
                    ),
                );
                providers_map.insert(
                    "hf".into(),
                    provider_entry(
                        "hf",
                        &serde_json::to_value(&hf_models).unwrap_or_else(|_| json!([])),
                    ),
                );
                let mut worker_ids: Vec<String> =
                    state.worker_delegate_runtimes.keys().cloned().collect();
                worker_ids.sort();

                let orch_entry = json!({
                    "id": orchestrator_id,
                    "role": "orchestrator",
                    "contextDirectory": state.orchestrator_context_dir.display().to_string(),
                    "defaultProvider": provider_id(provider_choice),
                    "defaultModel": default_model,
                    "enabledProviders": serde_json::to_value(&discovery_ids).unwrap_or_else(|_| json!([])),
                    "systemContext": serde_json::to_value(&system_context).unwrap_or_else(|_| json!("")),
                    "tools": serde_json::to_value(&tools_string).unwrap_or_else(|_| serde_json::Value::Null),
                    "skills": skill_runtime_json(state.skills.as_ref(), orch_mode),
                });

                let mut entries: Vec<serde_json::Value> = vec![orch_entry];
                for wid in &worker_ids {
                    if let Some(rt) = state.worker_delegate_runtimes.get(wid) {
                        let w_skills_enabled = !rt.skills.is_empty();
                        let w_system_context = build_system_context_for_today(
                            &rt.system_context_static,
                            None,
                            w_skills_enabled,
                        );
                        let w_tools_string = rt.tools_list.as_ref().and_then(|tools| {
                            if tools.is_empty() {
                                None
                            } else {
                                serde_json::to_string_pretty(tools).ok()
                            }
                        });
                        let (w_prov, w_model) = worker_defaults
                            .get(wid)
                            .cloned()
                            .unwrap_or_default();
                        let ctx_dir = rt
                            .context_directory
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                        entries.push(json!({
                            "id": wid,
                            "role": "worker",
                            "contextDirectory": ctx_dir,
                            "defaultProvider": w_prov,
                            "defaultModel": w_model,
                            "enabledProviders": serde_json::Value::Null,
                            "systemContext": serde_json::to_value(&w_system_context).unwrap_or_else(|_| json!("")),
                            "tools": w_tools_string
                                .as_ref()
                                .map(|s| serde_json::Value::String(s.clone()))
                                .unwrap_or(serde_json::Value::Null),
                            "skills": skill_runtime_json(rt.skills.as_ref(), rt.context_mode),
                        }));
                    }
                }

                let agents_block = json!({
                    "orchestrationCatalog": serde_json::to_value(&orchestration_catalog).unwrap_or_else(|_| json!([])),
                    "entries": entries,
                });
                let clock_block = json!({
                    "date": today,
                });
                let skill_packages_block = json!({
                    "discoveryRoot": state.skills_discovery_root.display().to_string(),
                    "packagesDiscovered": state.skill_packages_discovered,
                });
                let gateway_block = json!({
                    "status": "running",
                    "protocol": PROTOCOL_VERSION,
                    "port": state.config.gateway.port,
                    "bind": state.config.gateway.bind,
                    "auth": auth_mode,
                });
                // Key order matches `.agents/spec/GATEWAY_STATUS.md` and config cross-check:
                // clock (extra), then gateway → channels → providers → agents like config, then skillPackages (extra).
                let mut pl = serde_json::Map::new();
                pl.insert("clock".into(), clock_block);
                pl.insert("gateway".into(), gateway_block);
                pl.insert("channels".into(), channels_block);
                pl.insert(
                    "providers".into(),
                    serde_json::Value::Object(providers_map),
                );
                pl.insert("agents".into(), agents_block);
                pl.insert("skillPackages".into(), skill_packages_block);
                let payload = serde_json::Value::Object(pl);
                let res = WsResponse::ok(&req.id, payload);
                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
            }
            "send" => {
                let params: SendParams = match serde_json::from_value(req.params.clone()) {
                    Ok(p) => p,
                    Err(_) => {
                        let res = WsResponse::err(&req.id, "invalid send params");
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                        continue;
                    }
                };
                let channel = state.channel_registry.get(&params.channel_id).await;
                match channel {
                    None => {
                        let res = WsResponse::err(&req.id, "channel not found");
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                    }
                    Some(handle) => {
                        match handle.send_message(&params.conversation_id, &params.message).await {
                            Ok(()) => {
                                let res = WsResponse::ok(&req.id, json!({ "sent": true }));
                                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                            }
                            Err(e) => {
                                let res = WsResponse::err(&req.id, e);
                                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                            }
                        }
                    }
                }
            }
            "agent" => {
                let params: AgentParams = match serde_json::from_value(req.params.clone()) {
                    Ok(p) => p,
                    Err(_) => {
                        let res = WsResponse::err(&req.id, "invalid agent params");
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                        continue;
                    }
                };
                let session_id = if let Some(ref id) = params.session_id {
                    state.session_store.get_or_create(id.clone()).await
                } else {
                    state.session_store.create().await
                };
                let user_message = params.message.clone();
                if let Err(e) = state
                    .session_store
                    .append_message(&session_id, "user", &params.message)
                    .await
                {
                    let res = WsResponse::err(&req.id, e);
                    let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                    continue;
                }
                broadcast_session_message(
                    &state,
                    &session_id,
                    "user",
                    &user_message,
                    None,
                    None,
                    None,
                    None,
                );
                // Use request provider override when valid (see canonical_provider), else config default.
                let provider_choice = params
                    .provider
                    .as_deref()
                    .and_then(config::canonical_provider)
                    .map(provider_choice_from_canonical)
                    .unwrap_or_else(|| resolve_provider_choice(&state.config.agents));
                let model_name = resolve_model(
                    state.config.agents.default_model.as_deref(),
                    params.model.as_deref(),
                    provider_choice,
                );
                let has_workers = !state.worker_delegate_runtimes.is_empty();
                let orch_skills_enabled = !state.skills.is_empty();
                let system_context = build_system_context_for_today(
                    &state.system_context_static,
                    Some(has_workers),
                    orch_skills_enabled,
                );
                let (tools, tool_executor) = state.tools_and_executor();
                let tools = merge_delegate_task(tools, has_workers);
                let worker_tools = worker_tool_list(tools.as_ref());
                let delegate = Some(DelegateContext {
                    clients: state.provider_clients(),
                    agents: &state.config.agents,
                    orchestrator_system_context: if system_context.trim().is_empty() {
                        None
                    } else {
                        Some(system_context.as_str())
                    },
                    orchestrator_worker_tools: worker_tools,
                    orchestrator_tool_executor: tool_executor,
                    worker_runtimes: Some(state.worker_delegate_runtimes.as_ref()),
                    observability: Some(DelegateObservability {
                        event_tx: state.event_tx.clone(),
                        session_id: Some(session_id.clone()),
                    }),
                    session_store: Some(&state.session_store),
                    session_id: Some(session_id.as_str()),
                });
                let run_result = agent::run_turn_dyn(
                    &state.session_store,
                    &session_id,
                    state.provider_clients().as_dyn(provider_choice),
                    &model_name,
                    Some(&system_context),
                    state.config.agents.max_session_messages,
                    tools,
                    tool_executor,
                    delegate,
                    None,
                )
                .await;
                match run_result
                {
                    Ok(result) => {
                        let binding = state.bindings.get_channel_binding(&session_id).await;
                        let (channel_id, conv_id) = match binding {
                            Some((cid, conv)) => (Some(cid), Some(conv)),
                            None => (None, None),
                        };
                        broadcast_session_message(
                            &state,
                            &session_id,
                            "assistant",
                            &result.content,
                            if result.tool_calls.is_empty() {
                                None
                            } else {
                                Some(&result.tool_calls[..])
                            },
                            if result.tool_calls.is_empty() {
                                None
                            } else {
                                Some(&result.tool_results[..])
                            },
                            channel_id.as_deref(),
                            conv_id.as_deref(),
                        );
                        if let Some(reply) = channel_reply_text(&result) {
                            if let Some((channel_id, conv_id)) =
                                state.bindings.get_channel_binding(&session_id).await
                            {
                                if let Some(handle) = state.channel_registry.get(&channel_id).await {
                                    let _ = handle.send_message(&conv_id, &reply).await;
                                }
                            }
                        }
                        let tool_calls_payload = serde_json::to_value(&result.tool_calls)
                            .unwrap_or_else(|_| json!([]));
                        let tool_results_payload = serde_json::to_value(&result.tool_results)
                            .unwrap_or_else(|_| json!([]));
                        log::debug!(
                            "agent turn: {} tool call(s), session_id: {}",
                            result.tool_calls.len(),
                            session_id
                        );
                        let payload = json!({
                            "reply": result.content,
                            "sessionId": session_id,
                            "toolCalls": tool_calls_payload,
                            "toolResults": tool_results_payload
                        });
                        let res = WsResponse::ok(&req.id, payload);
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                    }
                    Err(e) => {
                        let res = WsResponse::err(&req.id, e.to_string());
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                    }
                }
            }
            _ => {
                let res = WsResponse::err(&req.id, format!("unknown method: {}", req.method));
                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
            }
        }
            }
        }
    }

    if !sent_hello {
        log::debug!("ws client disconnected before sending connect");
    }
}
