//! Gateway HTTP + WebSocket server (single port).

use crate::agent;
use crate::agent_ctx;
#[cfg(feature = "matrix")]
use crate::channels::{connect_matrix_client, MatrixChannel};
#[cfg(feature = "signal")]
use crate::channels::{resolve_signal_daemon_config, SignalChannel};
#[cfg(not(feature = "signal"))]
use crate::channels::resolve_signal_daemon_config;
use crate::channels::{
    ChannelHandle, ChannelRegistry, InboundMessage, TelegramChannel, TelegramTransport,
    TelegramUpdate,
};
use crate::config::{
    self, matrix_channel_configured, orchestrator_context_mode, resolve_telegram_webhook_secret,
    sessions_dir, worker_context_mode, Config, SkillContextMode,
};
#[cfg(feature = "matrix")]
use crate::gateway::matrix_routes;
use crate::gateway::pairing::PairingStore;
use crate::gateway::protocol::{
    AgentDetailParams, AgentParams, ConnectDevice, ConnectParams, HelloAuth, HelloOk,
    SendParams, SessionsDeleteParams, SessionsHistoryParams, StopParams, WsRequest, WsResponse,
};
use crate::init;
use crate::orchestration::{
    build_workers_context, effective_worker_defaults,
    merge_delegate_task, resolve_model,
    resolve_provider_choice, worker_tool_list, DelegateContext,
    DelegateObservability, ProviderChoice, ProviderClients, WorkerDelegateRuntime,
};
use crate::profile::{self, ChaiPaths};
use crate::providers::{
    build_provider_client,
    ToolDefinition,
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
        Query, State,
    },
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
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
    /// System context built from orchestrator **`AGENT.md`**, worker roster, and skills.
    pub system_context: String,
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
    /// Per-provider runtime state (client + discovered model list), keyed by provider id.
    pub provider_states: Arc<HashMap<String, ProviderRuntimeState>>,
    /// Built provider clients for dispatch (indexed by provider id).
    pub provider_clients: ProviderClients,
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
    /// Count of skill packages found on disk before orchestrator filtering.
    pub skills_packages_discovered: usize,
    /// Lock mode from `config.skills.lockMode`.
    pub skills_lock_mode: config::SkillLockMode,
    /// Lockfile generation number (from `skills.lock`), or `None` when no lockfile exists.
    pub skills_lock_generation: Option<u64>,
    /// Number of skills pinned in the lockfile (0 when no lockfile exists).
    pub skills_locked_count: usize,
    /// Number of writable roots in the sandbox (0 when sandbox is missing).
    pub sandbox_roots_count: usize,
    /// Per-session stop flags. When set, the agent loop breaks after the current iteration.
    /// The flag is cleared at the start of each new turn.
    pub session_stop_flags: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
}

/// Per-provider runtime state: discovered model name list.
/// (Provider clients are stored separately in `GatewayState::provider_clients`.)
#[derive(Clone, Default)]
pub struct ProviderRuntimeState {
    /// Discovered model names (populated at startup or soon after). Empty if unreachable.
    pub models: Arc<tokio::sync::RwLock<Vec<String>>>,
}

/// Executor that handles read_skill (lookup by name, return SKILL.md content) and delegates all other tools to the generic executor. Used when context mode is ReadOnDemand.
struct ReadOnDemandExecutor {
    skills: Arc<Vec<Skill>>,
    inner: GenericToolExecutor,
}

impl agent::ToolExecutor for ReadOnDemandExecutor {
    fn execute(&self, name: &str, args: &serde_json::Value, session_id: Option<&str>) -> Result<String, String> {
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
            self.inner.execute(name, args, session_id)
        }
    }
}

impl GatewayState {
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
    out.push_str("You have skills. Skills have tools.\n\n");
    for s in skills {
        out.push_str("### ");
        out.push_str(&s.name);
        out.push_str("\n\n");
        if !s.description.is_empty() {
            out.push_str(&s.description);
            out.push_str("\n\n");
        }
        out.push_str("--- SKILL.md (BOF) ---");
        out.push_str("\n\n");
        out.push_str(strip_skill_frontmatter(&s.content));
        out.push_str("\n");
        out.push_str("--- SKILL.md (EOF) ---");
        out.push_str("\n\n");
    }
    out
}

/// Build per-skill body map (name → frontmatter-stripped body). Used for desktop display in
/// read-on-demand so the panel can render each skill in its own box with a name header.
fn build_skill_bodies_map(skills: &[Skill]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for s in skills {
        let body = strip_skill_frontmatter(&s.content).to_string();
        map.insert(s.name.clone(), body);
    }
    map
}

/// Build compact skill list (name + description only). Used when context mode is ReadOnDemand; model uses read_skill to load full docs.
fn build_skill_context_compact(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    out.push_str("## Skills\n\n");
    out.push_str("You have skills. Skills have tools.\n\n");
    out.push_str("You can call `read_skill` to read about a skill.\n\n");
    out.push_str("Available skills:\n\n");
    for s in skills {
        out.push_str("- `");
        out.push_str(&s.name);
        out.push_str("` — ");
        out.push_str(if s.description.is_empty() {
            "(no description)"
        } else {
            &s.description
        });
        out.push_str("\n");
    }
    out
}

/// Per-agent **`skillsContext`** value for the status payload.
///
/// Returns a map of skill name → frontmatter-stripped `SKILL.md` body,
/// or **`null`** when no skills are loaded.
fn skills_context_json(skills: &[Skill]) -> serde_json::Value {
    let map = build_skill_bodies_map(skills);
    if map.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect(),
        )
    }
}
/// Build system context from agent-ctx (AGENT.md), worker roster, and skills.
/// Uses context_mode to choose full vs compact skill context.
fn build_system_context(
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

/// Worker system context: **`agents/<workerId>/AGENT.md`** and skills only (no orchestrator roster).
fn build_worker_system_context(
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

/// Convert a `SessionMessage` to a JSON value with camelCase keys for the wire protocol.
/// Uses manual construction (not `serde_json::to_value`) because `SessionMessage` serializes
/// with snake_case keys for on-disk storage, but the WebSocket protocol uses camelCase.
fn session_message_to_json(m: &crate::session::SessionMessage) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert("role".to_string(), json!(m.role));
    obj.insert("content".to_string(), json!(m.content));
    if let Some(ref calls) = m.tool_calls {
        obj.insert("toolCalls".to_string(), serde_json::to_value(calls).unwrap_or_else(|_| json!([])));
    }
    if let Some(ref name) = m.tool_name {
        obj.insert("toolName".to_string(), json!(name));
    }
    serde_json::Value::Object(obj)
}

/// Broadcast a `gateway.config.changed` event to connected WebSocket clients.
/// Called when model discovery updates the provider model list so the desktop
/// can refresh its status display immediately instead of waiting for the next poll.
fn broadcast_config_changed(event_tx: &tokio::sync::broadcast::Sender<String>) {
    let event = json!({
        "type": "event",
        "event": crate::orchestration::delegate::EVENT_CONFIG_CHANGED,
        "payload": {},
    });
    if let Ok(text) = serde_json::to_string(&event) {
        let _ = event_tx.send(text);
    }
}

/// Process one inbound channel message: get or create session, bind, append user message, run agent, send reply.
/// If the message is the new-session trigger (e.g. /new), rebind the conversation to a fresh session and confirm.
async fn process_inbound_message(state: GatewayState, msg: InboundMessage) {
    log::info!(
        "inbound: channel={}, conversation={}, text_len={}",
        msg.channel_id,
        msg.conversation_id,
        msg.text.len()
    );
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
            state.bindings.remove_binding(&id).await;
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
        Some(id) => {
            // Ensure the session is loaded (lazy-load from disk if needed).
            if state.session_store.get(&id).await.is_some() {
                id
            } else {
                // Session was deleted or corrupt — create a new one and rebind.
                let new_id = state.session_store.create().await;
                state
                    .bindings
                    .bind(&msg.channel_id, &msg.conversation_id, &new_id)
                    .await;
                new_id
            }
        }
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
    let provider_choice = resolve_provider_choice(&state.config.providers, &state.config.agents);
    let model_name = resolve_model(
        &state.config.providers,
        state.config.agents.default_model.as_deref(),
        None,
        &provider_choice,
    );
    let has_workers = !state.worker_delegate_runtimes.is_empty();
    let system_context = &state.system_context;
    let (tools, tool_executor) = state.tools_and_executor();
    let tools = merge_delegate_task(tools, has_workers);
    let worker_tools = worker_tool_list(tools.as_ref());
    // Get or create the stop flag for this session before building DelegateContext
    // so the worker turn can also be stopped (used by channel stop if needed).
    let stop_flag = {
        let mut flags = state.session_stop_flags.write().await;
        flags
            .entry(session_id.clone())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone()
    };
    let delegate = Some(DelegateContext {
        clients: &state.provider_clients,
        providers: &state.config.providers,
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
            source: Some("orchestrator".to_string()),
            tool_index_offset: 0,
            emitted_tool_calls: AtomicUsize::new(0),
        }),
        session_store: Some(&state.session_store),
        session_id: Some(session_id.as_str()),
        stop_flag: Some(stop_flag.clone()),
        tool_index_offset: 0,
    });
    let provider_dyn = state.provider_clients.get(&provider_choice)
        .ok_or_else(|| format!("no client for provider '{}'", provider_choice))
        .expect("provider client should exist");
    let result = agent::run_turn_dyn(
        &state.session_store,
        &session_id,
        provider_dyn,
        &model_name,
        Some(system_context),
        state.config.agents.max_tool_loops_per_turn,
        tools,
        tool_executor,
        delegate,
        None,
        Some(stop_flag),
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
    let (event_tx, _) = broadcast::channel(256);
    let channel_tasks = Arc::new(tokio::sync::RwLock::new(Vec::new()));

    // Build provider clients and runtime state dynamically from the providers array.
    let mut provider_states: HashMap<String, ProviderRuntimeState> = HashMap::new();
    let mut provider_clients = ProviderClients::default();
    for def in &config.providers.entries {
        let client = match build_provider_client(def, &config.providers) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("skipping provider '{}': {}", def.id, e);
                continue;
            }
        };
        provider_clients.insert(def.id.clone(), client.clone());
        provider_states.insert(
            def.id.clone(),
            ProviderRuntimeState {
                models: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            },
        );
    }

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
        let mut all_enabled: Vec<&str> = config::orchestrator_enabled_skills_list(&config.agents)
            .iter()
            .map(|s| s.as_str())
            .collect();
        if let Some(workers) = &config.agents.workers {
            for w in workers {
                for name in config::worker_enabled_skills_list(w) {
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
            config.skills.lock_mode,
        )?;
    }

    // Read lockfile metadata for the status payload.
    let (skills_lock_generation, skills_locked_count) =
        match crate::skills::lockfile::read_lock(&paths.profile_dir) {
            Ok(Some(lock)) => (Some(lock.generation), lock.skills.len()),
            Ok(None) => (None, 0),
            Err(e) => {
                log::debug!("could not read skills.lock for status: {}", e);
                (None, 0)
            }
        };

    let orch_names = config::orchestrator_enabled_skills_list(&config.agents);
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
        match config.sandbox.mode {
            config::SandboxMode::Strict => {
                anyhow::bail!(
                    "sandbox directory not found at {}; set sandbox.mode to \"current\" (CWD as writable root) or \"unsafe\" (no sandbox) to start without a sandbox directory",
                    paths.sandbox_dir().display()
                );
            }
            config::SandboxMode::Current => {
                let cwd_sandbox = crate::exec::WriteSandbox::from_cwd();
                let cwd_roots = cwd_sandbox.roots().len();
                if cwd_sandbox.has_roots() {
                    log::warn!(
                        "no sandbox directory at {}; falling back to CWD sandbox ({} writable root at {})",
                        paths.sandbox_dir().display(),
                        cwd_roots,
                        cwd_sandbox.roots()[0].display()
                    );
                    Some(cwd_sandbox)
                } else {
                    anyhow::bail!(
                        "no sandbox directory at {} and CWD could not be resolved; set sandbox.mode to \"unsafe\" to start without a sandbox",
                        paths.sandbox_dir().display()
                    );
                }
            }
            config::SandboxMode::Unsafe => {
                log::warn!(
                    "no sandbox directory at {}; CWD confinement and path validation are disabled (sandbox.mode is \"unsafe\")",
                    paths.sandbox_dir().display()
                );
                None
            }
        }
    };
    // Recount roots after potential CWD fallback.
    let sandbox_roots_count = sandbox_opt
        .as_ref()
        .map(|s| s.roots().len())
        .unwrap_or(0);
    let orch_built =
        build_skill_runtime_for_entries(orchestrator_entries, orch_ctx_mode, sandbox_opt.clone());
    let skills = orch_built.skills.clone();
    let agent_ctx = agent_ctx::load_agent_ctx(Some(orch_context_dir.as_path()));
    let system_context = build_system_context(
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
            let w_names = config::worker_enabled_skills_list(w);
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
            let w_context = build_worker_system_context(
                w_agent_ctx.as_deref(),
                &w_built.skills,
                w_ctx_mode,
            );
            worker_map.insert(
                w.id.clone(),
                WorkerDelegateRuntime {
                    system_context: w_context,
                    skills: Arc::new(w_built.skills),
                    tools_list: w_built.tools_list,
                    tool_executor: w_built.tool_executor,
                    context_mode: w_ctx_mode,
                },
            );
        }
    }
    let worker_delegate_runtimes = Arc::new(worker_map);

    // Build persistent session and binding stores.
    let orch_id = config
        .agents
        .orchestrator_id
        .as_deref()
        .unwrap_or("orchestrator")
        .trim();
    let orch_id = if orch_id.is_empty() {
        "orchestrator"
    } else {
        orch_id
    };
    let sessions_path = sessions_dir(&paths.profile_dir, orch_id);
    let session_store = Arc::new(SessionStore::with_data_dir(sessions_path.clone()));
    let binding_store = Arc::new(SessionBindingStore::with_data_dir(sessions_path.clone()));

    #[cfg_attr(not(feature = "matrix"), allow(unused_mut))]
    let mut state = GatewayState {
        config: Arc::new(config.clone()),
        system_context,
        required_token,
        event_tx: event_tx.clone(),
        channel_tasks: channel_tasks.clone(),
        inbound_tx: inbound_tx.clone(),
        session_store: session_store.clone(),
        channel_registry: Arc::new(ChannelRegistry::new()),
        bindings: binding_store.clone(),
        provider_states: Arc::new(provider_states),
        provider_clients,
        skills: Arc::new(skills),
        tools_list,
        tool_executor,
        pairing_store,
        #[cfg(feature = "matrix")]
        matrix_channel: None,
        worker_delegate_runtimes,
        skills_packages_discovered: all_entries.len(),
        skills_lock_mode: config.skills.lock_mode,
        skills_lock_generation,
        skills_locked_count,
        sandbox_roots_count,
        session_stop_flags: Arc::new(RwLock::new(HashMap::new())),
    };

    // Scan persisted sessions on startup (populates disk index for lazy loading).
    {
        let summaries = session_store.scan().await;
        if !summaries.is_empty() {
            log::info!(
                "loaded {} persisted session(s) from {}",
                summaries.len(),
                sessions_path.display(),
            );
        }
    }
    // Spawn model discovery tasks for each configured provider.
    {
        let states = state.provider_states.clone();
        let event_tx = state.event_tx.clone();
        let providers = &config.providers;
        let agents = &config.agents;
        for def in &providers.entries {
            let provider_id = def.id.clone();
            let discovery_on = config::provider_discovery_enabled(providers, agents, &provider_id);
            if !discovery_on {
                log::debug!("{} model discovery skipped (not in enabledProviders)", provider_id);
                continue;
            }

            let runtime = match states.get(&provider_id) {
                Some(r) => r.clone(),
                None => continue,
            };
            let model_discovery = def.model_discovery;
            let static_models = def.static_models.clone();
            let tx = event_tx.clone();

            match model_discovery {
                config::ModelDiscovery::Auto => {
                    match def.endpoint_type {
                        config::EndpointType::Ollama => {
                            let ollama = crate::providers::OllamaClient::new(
                                crate::config::resolve_provider_base_url(providers, &provider_id),
                            );
                            let models = runtime.models.clone();
                            tokio::spawn(async move {
                                match ollama.list_models().await {
                                    Ok(list) => {
                                        let names: Vec<String> = list.into_iter().map(|m| m.name).collect();
                                        *models.write().await = names;
                                        log::info!("{} model discovery completed", provider_id);
                                        broadcast_config_changed(&tx);
                                    }
                                    Err(e) => {
                                        log::debug!("{} model discovery failed: {}", provider_id, e);
                                    }
                                }
                            });
                        }
                        config::EndpointType::OpenaiCompat => {
                            let compat = crate::providers::OpenAiCompatClient::new_adapter(
                                crate::config::resolve_provider_base_url(providers, &provider_id)
                                    .unwrap_or_default(),
                                crate::config::resolve_provider_api_key(providers, &provider_id),
                            );
                            let models = runtime.models.clone();
                            tokio::spawn(async move {
                                match compat.list_models_openai().await {
                                    Ok(list) => {
                                        *models.write().await = list;
                                        log::info!("{} model discovery completed", provider_id);
                                        broadcast_config_changed(&tx);
                                    }
                                    Err(e) => {
                                        log::debug!("{} model discovery failed: {}", provider_id, e);
                                    }
                                }
                            });
                        }
                    }
                }
                config::ModelDiscovery::Lmstudio => {
                    let compat = crate::providers::OpenAiCompatClient::new_adapter(
                        crate::config::resolve_provider_base_url(providers, &provider_id)
                            .unwrap_or_default(),
                        crate::config::resolve_provider_api_key(providers, &provider_id),
                    );
                    let models = runtime.models.clone();
                    tokio::spawn(async move {
                        match compat.list_models_lmstudio().await {
                            Ok(list) => {
                                *models.write().await = list;
                                log::info!("{} model discovery completed (lmstudio)", provider_id);
                                broadcast_config_changed(&tx);
                            }
                            Err(e) => {
                                log::debug!("{} model discovery failed (lmstudio): {}", provider_id, e);
                            }
                        }
                    });
                }
                config::ModelDiscovery::Static => {
                    let models = runtime.models.clone();
                    tokio::spawn(async move {
                        let mut names = static_models;
                        names.sort();
                        *models.write().await = names;
                        log::info!("{} model list loaded (static from config)", provider_id);
                        broadcast_config_changed(&tx);
                    });
                }
            }
        }
    }

    // Startup warnings for non-local providers in the default provider selection.
    {
        let default_choice = resolve_provider_choice(&config.providers, &config.agents);
        if let Some(def) = config.providers.get(default_choice.as_str()) {
            match def.endpoint_type {
                config::EndpointType::OpenaiCompat => {
                    let base = crate::config::resolve_provider_base_url(&config.providers, default_choice.as_str());
                    if base.as_ref().map(|u| u.contains("localhost")).unwrap_or(false) {
                        log::warn!(
                            "a resolver or hosts file could change what \"localhost\" points to; 127.0.0.1 is recommended"
                        );
                    }
                    if base.as_ref().map(|u| !(u.contains("localhost") || u.contains("127.0.0.1"))).unwrap_or(false) {                        log::warn!(
                            "a non-local provider is enabled; requests and data will be sent to a non-local API"
                        );
                        let api_key = crate::config::resolve_provider_api_key(&config.providers, &default_choice.as_str());
                        if api_key.as_ref().map(|k| k.is_empty()).unwrap_or(true) {
                            log::warn!("{} provider selected but no API key set. Requests will fail until a key is configured.", default_choice);
                        }
                    }
                }
                _ => {}
            }
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

    // Channel startup summary.
    {
        let mut channel_summary: Vec<String> = Vec::new();
        if config::resolve_telegram_token(&config).is_some() {
            let mode = if config.channels.telegram.webhook_url.is_some() {
                "webhook"
            } else {
                "long-poll"
            };
            channel_summary.push(format!("telegram ({})", mode));
        }
        #[cfg(feature = "matrix")]
        if matrix_channel_configured(&config) {
            channel_summary.push("matrix (experimental)".to_string());
        }
        #[cfg(not(feature = "matrix"))]
        if matrix_channel_configured(&config) {
            channel_summary.push("matrix (not built)".to_string());
        }
        #[cfg(feature = "signal")]
        if resolve_signal_daemon_config(&config).is_some() {
            channel_summary.push("signal (experimental)".to_string());
        }
        #[cfg(not(feature = "signal"))]
        if resolve_signal_daemon_config(&config).is_some() {
            channel_summary.push("signal (not built)".to_string());
        }
        if channel_summary.is_empty() {
            log::info!("channels: none configured (CLI and desktop chat always available)");
        } else {
            log::info!(
                "channels: {}",
                channel_summary.join(", ")
            );
        }
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

    #[cfg(feature = "signal")]
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
        .route("/logs", get(logs_http))
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

/// Query parameters for the `/logs` HTTP endpoint.
#[derive(Deserialize)]
struct LogsQuery {
    /// Return lines with sequence numbers greater than this value.
    #[serde(default)]
    after_seq: u64,
    /// Maximum number of lines to return (default 200, max 1000).
    #[serde(default = "default_log_lines")]
    lines: usize,
}

fn default_log_lines() -> usize {
    200
}

/// GET /logs?afterSeq=N&lines=M returns log lines from the gateway's ring buffer.
async fn logs_http(Query(params): Query<LogsQuery>) -> Json<serde_json::Value> {
    let lines_limit = params.lines.min(1000);
    let (lines, max_seq) = crate::logging::log_lines_after(params.after_seq);
    let lines: Vec<String> = lines.into_iter().take(lines_limit).collect();
    Json(json!({
        "lines": lines,
        "maxSeq": max_seq,
    }))
}
fn non_empty_cfg_opt(s: &Option<String>) -> bool {
    s.as_ref().map(|x| !x.trim().is_empty()).unwrap_or(false)
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
    let mut connect_attempted = false;
    let mut event_rx = state.event_tx.subscribe();

    let nonce = uuid::Uuid::new_v4().to_string();
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let challenge_json = connect_challenge_event(&nonce, ts_ms);
    if socket.send(Message::Text(challenge_json.into())).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            biased;

            event = event_rx.recv() => {
                match event {
                    Ok(text) => {
                        let is_shutdown = text == SHUTDOWN_EVENT_JSON;
                        let _ = socket.send(Message::Text(text.into())).await;
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
                connect_attempted = true;
                let params: ConnectParams = match serde_json::from_value(req.params.clone()) {
                    Ok(p) => p,
                    Err(_) => {
                        let res = WsResponse::err(&req.id, "invalid connect params");
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
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
                            let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                            continue;
                        }
                    }
                } else if let Some(ref device) = params.device {
                    if let Err(e) = verify_device_signature(device, &params, &nonce) {
                        log::debug!("device signature verification failed: {}", e);
                        let res = WsResponse::err(&req.id, e);
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
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
                            let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
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
                            let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                            continue;
                        }
                        if provided != required {
                            let res = WsResponse::err(&req.id, "unauthorized: gateway token mismatch");
                            let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
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
                if socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await.is_ok() {
                    sent_hello = true;
                }
            }
            "health" => {
                let payload = json!({
                    "status": "running",
                    "protocol": PROTOCOL_VERSION,
                });
                let res = WsResponse::ok(&req.id, payload);
                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
            }
            "status" => {
                let auth_mode = if state.required_token.is_some() {
                    "token"
                } else {
                    "none"
                };
                let provider_choice = resolve_provider_choice(&state.config.providers, &state.config.agents);
                let default_model = resolve_model(
                    &state.config.providers,
                    state.config.agents.default_model.as_deref(),
                    None,
                    &provider_choice,
                );

                let orch_mode = orchestrator_context_mode(&state.config.agents);
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
                                let pair = effective_worker_defaults(&state.config.providers, &state.config.agents, w);
                                Some((id.to_string(), pair))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let discovery_ids = config::discovery_enabled_provider_ids(&state.config.providers, &state.config.agents);
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
                #[cfg(feature = "signal")]
                let signal_configured = resolve_signal_daemon_config(cfg_ref).is_some();
                #[cfg(not(feature = "signal"))]
                let signal_configured = false;
                let channel_runtime = state.channel_registry.channel_status_details().await;
                let mut telegram_ch = serde_json::Map::new();
                telegram_ch.insert("active".into(), json!(active.contains("telegram")));
                telegram_ch.insert("configured".into(), json!(telegram_configured));
                merge_channel_runtime_detail(&mut telegram_ch, &channel_runtime, "telegram");
                let mut matrix_ch = serde_json::Map::new();
                matrix_ch.insert("active".into(), json!(matrix_active));
                matrix_ch.insert("configured".into(), json!(matrix_configured));
                merge_channel_runtime_detail(&mut matrix_ch, &channel_runtime, "matrix");
                #[cfg(feature = "signal")]
                let signal_active = active.contains("signal");
                #[cfg(not(feature = "signal"))]
                let signal_active = false;
                let mut signal_ch = serde_json::Map::new();
                signal_ch.insert("active".into(), json!(signal_active));
                signal_ch.insert("configured".into(), json!(signal_configured));
                merge_channel_runtime_detail(&mut signal_ch, &channel_runtime, "signal");
                let channels_block = json!({
                    "telegram": serde_json::Value::Object(telegram_ch),
                    "matrix": serde_json::Value::Object(matrix_ch),
                    "signal": serde_json::Value::Object(signal_ch),
                });

                // Build providers map dynamically from configured providers.
                let mut providers_map = serde_json::Map::new();
                for (pid, runtime) in state.provider_states.iter() {
                    let models = runtime.models.read().await.clone();
                    let def = state.config.providers.get(pid);
                    let endpoint_type = def
                        .map(|d| d.endpoint_type.as_str())
                        .unwrap_or("unknown");
                    let model_discovery = def
                        .map(|d| d.model_discovery.as_str())
                        .unwrap_or("auto");
                    providers_map.insert(
                        pid.clone(),
                        json!({
                            "endpointType": endpoint_type,
                            "modelDiscovery": model_discovery,
                            "models": models,
                        }),
                    );
                }

                let mut worker_ids: Vec<String> =
                    state.worker_delegate_runtimes.keys().cloned().collect();
                worker_ids.sort();

                let orch_enabled_skills: Vec<String> = state.skills.iter().map(|s| s.name.clone()).collect();
                let orch_context_mode_wire = match orch_mode {
                    SkillContextMode::Full => "full",
                    SkillContextMode::ReadOnDemand => "readOnDemand",
                };
                let orch_entry = json!({
                    "id": orchestrator_id,
                    "role": "orchestrator",
                    "defaultProvider": provider_choice.as_str(),
                    "defaultModel": default_model,
                    "enabledProviders": serde_json::to_value(&discovery_ids).unwrap_or_else(|_| json!([])),
                    "enabledSkills": orch_enabled_skills,
                    "contextMode": orch_context_mode_wire,
                    "maxToolLoopsPerTurn": state.config.agents.max_tool_loops_per_turn,
                    "maxDelegationsPerTurn": state.config.agents.max_delegations_per_turn,
                    "maxDelegationsPerSession": state.config.agents.max_delegations_per_session,
                    "maxDelegationsPerWorker": serde_json::to_value(&state.config.agents.max_delegations_per_worker).unwrap_or_else(|_| serde_json::Value::Null),
                });

                let mut entries: Vec<serde_json::Value> = vec![orch_entry];
                for wid in &worker_ids {
                    if let Some(rt) = state.worker_delegate_runtimes.get(wid) {
                        let (w_prov, w_model) = worker_defaults
                            .get(wid)
                            .cloned()
                            .unwrap_or_default();
                        let w_enabled_skills: Vec<String> = rt.skills.iter().map(|s| s.name.clone()).collect();
                        let w_context_mode_wire = match rt.context_mode {
                            SkillContextMode::Full => "full",
                            SkillContextMode::ReadOnDemand => "readOnDemand",
                        };
                        entries.push(json!({
                            "id": wid,
                            "role": "worker",
                            "defaultProvider": w_prov,
                            "defaultModel": w_model,
                            "enabledProviders": serde_json::Value::Null,
                            "enabledSkills": w_enabled_skills,
                            "contextMode": w_context_mode_wire,
                            "maxToolLoopsPerTurn": serde_json::Value::Null,
                            "maxDelegationsPerTurn": serde_json::Value::Null,
                            "maxDelegationsPerSession": serde_json::Value::Null,
                            "maxDelegationsPerWorker": serde_json::Value::Null,
                        }));
                    }
                }

                let agents_block = serde_json::Value::Array(entries);
                let skills_block = json!({
                    "packagesDiscovered": state.skills_packages_discovered,
                    "lockMode": match state.skills_lock_mode {
                        config::SkillLockMode::Strict => "strict",
                        config::SkillLockMode::Warn => "warn",
                    },
                    "lockGeneration": state.skills_lock_generation,
                    "lockedSkills": state.skills_locked_count,
                });
                let gateway_block = json!({
                    "status": "running",
                    "protocol": PROTOCOL_VERSION,
                    "port": state.config.gateway.port,
                    "bind": state.config.gateway.bind,
                    "auth": auth_mode,
                });
                let sandbox_block = json!({
                    "mode": state.config.sandbox.mode.as_str(),
                    "roots": state.sandbox_roots_count,
                });
                // Key order matches `base/spec/GATEWAY_STATUS.md` and config cross-check:
                // gateway → channels → providers → sandbox → agents → skills.
                let mut pl = serde_json::Map::new();
                pl.insert("gateway".into(), gateway_block);
                pl.insert("channels".into(), channels_block);
                pl.insert(
                    "providers".into(),
                    serde_json::Value::Object(providers_map),
                );
                pl.insert("sandbox".into(), sandbox_block);
                pl.insert("agents".into(), agents_block);
                pl.insert("skills".into(), skills_block);
                let payload = serde_json::Value::Object(pl);
                let res = WsResponse::ok(&req.id, payload);
                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
            }
            "agentDetail" => {
                let params: AgentDetailParams = match serde_json::from_value(req.params.clone()) {
                    Ok(p) => p,
                    Err(_) => {
                        let res = WsResponse::err(&req.id, "invalid agentDetail params");
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                        continue;
                    }
                };
                let agent_id = params.agent_id.trim();
                if agent_id.is_empty() {
                    let res = WsResponse::err(&req.id, "missing agentId");
                    let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                    continue;
                }
                let orchestrator_id = state
                    .config
                    .agents
                    .orchestrator_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("orchestrator");
                if agent_id == orchestrator_id {
                    let system_context = &state.system_context;
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
                    let payload = json!({
                        "id": orchestrator_id,
                        "role": "orchestrator",
                        "systemContext": system_context,
                        "tools": tools_string,
                        "skillsContext": skills_context_json(state.skills.as_ref()),
                    });
                    let res = WsResponse::ok(&req.id, payload);
                    let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                } else if let Some(rt) = state.worker_delegate_runtimes.get(agent_id) {
                    let w_tools_string = rt.tools_list.as_ref().and_then(|tools| {
                        if tools.is_empty() {
                            None
                        } else {
                            serde_json::to_string_pretty(tools).ok()
                        }
                    });
                    let payload = json!({
                        "id": agent_id,
                        "role": "worker",
                        "systemContext": rt.system_context,
                        "tools": w_tools_string,
                        "skillsContext": skills_context_json(rt.skills.as_ref()),
                    });
                    let res = WsResponse::ok(&req.id, payload);
                    let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                } else {
                    let res = WsResponse::err(&req.id, "unknown agent id");
                    let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                }
            }
            "send" => {
                let params: SendParams = match serde_json::from_value(req.params.clone()) {
                    Ok(p) => p,
                    Err(_) => {
                        let res = WsResponse::err(&req.id, "invalid send params");
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                        continue;
                    }
                };
                let channel = state.channel_registry.get(&params.channel_id).await;
                match channel {
                    None => {
                        let res = WsResponse::err(&req.id, "channel not found");
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                    }
                    Some(handle) => {
                        match handle.send_message(&params.conversation_id, &params.message).await {
                            Ok(()) => {
                                let res = WsResponse::ok(&req.id, json!({ "sent": true }));
                                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                            }
                            Err(e) => {
                                let res = WsResponse::err(&req.id, e);
                                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
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
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
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
                    let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
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
                // Use request provider override when valid, else config default.
                let provider_choice = params
                    .provider
                    .as_deref()
                    .and_then(|s| config::canonical_provider_id(&state.config.providers, s))
                    .map(ProviderChoice::new)
                    .unwrap_or_else(|| resolve_provider_choice(&state.config.providers, &state.config.agents));
                let model_name = resolve_model(
                    &state.config.providers,
                    state.config.agents.default_model.as_deref(),
                    params.model.as_deref(),
                    &provider_choice,
                );
                let has_workers = !state.worker_delegate_runtimes.is_empty();
                let system_context = &state.system_context;
                let (tools, tool_executor) = state.tools_and_executor();
                let tools = merge_delegate_task(tools, has_workers);
                let worker_tools = worker_tool_list(tools.as_ref());
                // Get or create the stop flag for this session. The flag is cleared
                // at the start of each new turn inside execute_turn_main. The same
                // flag is passed to the worker via DelegateContext so that pressing
                // stop also interrupts a running worker turn.
                let stop_flag = {
                    let mut flags = state.session_stop_flags.write().await;
                    flags
                        .entry(session_id.clone())
                        .or_insert_with(|| Arc::new(AtomicBool::new(false)))
                        .clone()
                };
                let delegate = Some(DelegateContext {
                    clients: &state.provider_clients,
                    providers: &state.config.providers,
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
                        source: Some("orchestrator".to_string()),
                        tool_index_offset: 0,
                        emitted_tool_calls: AtomicUsize::new(0),
                    }),
                    session_store: Some(&state.session_store),
                    session_id: Some(session_id.as_str()),
                    stop_flag: Some(stop_flag.clone()),
                    tool_index_offset: 0,
                });
                let provider_dyn = state.provider_clients.get(&provider_choice)
                    .ok_or_else(|| format!("no client for provider '{}'", provider_choice))
                    .expect("provider client should exist");
                let system_context_opt = if system_context.trim().is_empty() {
                    None
                } else {
                    Some(system_context.as_str())
                };
                let run_result = agent::run_turn_dyn(
                    &state.session_store,
                    &session_id,
                    provider_dyn,
                    &model_name,
                    system_context_opt,
                    state.config.agents.max_tool_loops_per_turn,
                    tools,
                    tool_executor,
                    delegate,
                    None,
                    Some(stop_flag),
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
                        let mut payload = json!({
                            "reply": result.content,
                            "sessionId": session_id,
                            "toolCalls": tool_calls_payload,
                            "toolResults": tool_results_payload,
                            "loopLimitReached": result.loop_limit_reached,
                            "stopped": result.stopped,
                        });
                        if result.loop_limit_reached && !result.pending_tool_calls.is_empty() {
                            let pending = serde_json::to_value(&result.pending_tool_calls)
                                .unwrap_or_else(|_| json!([]));
                            payload["pendingToolCalls"] = pending;
                        }
                        let res = WsResponse::ok(&req.id, payload);
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                    }
                    Err(e) => {
                        let res = WsResponse::err(&req.id, e.to_string());
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                    }
                }
            }
            "stop" => {
                let params: StopParams = match serde_json::from_value(req.params.clone()) {
                    Ok(p) => p,
                    Err(_) => {
                        let res = WsResponse::err(&req.id, "invalid stop params");
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                        continue;
                    }
                };
                let flags = state.session_stop_flags.read().await;
                if let Some(flag) = flags.get(&params.session_id) {
                    flag.store(true, Ordering::SeqCst);
                    log::info!("stop: set stop flag for session {}", params.session_id);
                } else {
                    log::debug!("stop: no active turn for session {}, ignoring", params.session_id);
                }
                let res = WsResponse::ok(&req.id, json!({ "stopped": true }));
                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
            }
            "logs" => {
                let after_seq = req.params.get("afterSeq").and_then(|v| v.as_u64()).unwrap_or(0);
                let lines_limit = req.params.get("lines").and_then(|v| v.as_u64()).unwrap_or(200) as usize;
                let (lines, max_seq) = crate::logging::log_lines_after(after_seq);
                let lines: Vec<String> = lines.into_iter().take(lines_limit).collect();
                let res = WsResponse::ok(&req.id, json!({
                    "lines": lines,
                    "maxSeq": max_seq,
                }));
                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
            }
            "sessions.list" => {
                let summaries = state.session_store.scan().await;
                let mut entries = Vec::new();
                for s in &summaries {
                    let mut entry = json!({
                        "id": s.id,
                        "createdAt": s.created_at,
                        "updatedAt": s.updated_at,
                        "messageCount": s.message_count,
                    });
                    if let Some((ch, conv)) = state.bindings.get_channel_binding(&s.id).await {
                        entry.as_object_mut().unwrap().insert(
                            "channelBinding".to_string(),
                            json!({ "channelId": ch, "conversationId": conv }),
                        );
                    }
                    entries.push(entry);
                }
                // Sort by updatedAt descending (most recently updated first).
                entries.sort_by(|a, b| {
                    let ua = a.get("updatedAt").and_then(|v| v.as_str()).unwrap_or("");
                    let ub = b.get("updatedAt").and_then(|v| v.as_str()).unwrap_or("");
                    ub.cmp(ua)
                });
                let res = WsResponse::ok(&req.id, json!({ "sessions": entries }));
                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
            }
            "sessions.history" => {
                let params: SessionsHistoryParams = match serde_json::from_value(req.params.clone()) {
                    Ok(p) => p,
                    Err(_) => {
                        let res = WsResponse::err(&req.id, "invalid sessions.history params");
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                        continue;
                    }
                };
                match state.session_store.get(&params.session_id).await {
                    Some(session) => {
                        let offset = params.offset.unwrap_or(0);
                        let messages: Vec<serde_json::Value> = if let Some(limit) = params.limit {
                            session.messages.iter()
                                .skip(offset)
                                .take(limit)
                                .map(session_message_to_json)
                                .collect()
                        } else {
                            session.messages.iter()
                                .skip(offset)
                                .map(session_message_to_json)
                                .collect()
                        };
                        let res = WsResponse::ok(&req.id, json!({
                            "id": session.id,
                            "messages": messages,
                            "createdAt": session.created_at,
                            "updatedAt": session.updated_at,
                        }));
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                    }
                    None => {
                        let res = WsResponse::err(&req.id, "session not found");
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                    }
                }
            }
            "sessions.delete" => {
                let params: SessionsDeleteParams = match serde_json::from_value(req.params.clone()) {
                    Ok(p) => p,
                    Err(_) => {
                        let res = WsResponse::err(&req.id, "invalid sessions.delete params");
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                        continue;
                    }
                };
                match state.session_store.remove(&params.session_id).await {
                    Some(_) => {
                        state.bindings.remove_binding(&params.session_id).await;
                        // Broadcast session.deleted event so clients can update without polling.
                        let event = json!({
                            "type": "event",
                            "event": "session.deleted",
                            "payload": { "sessionId": params.session_id },
                        });
                        if let Ok(text) = serde_json::to_string(&event) {
                            let _ = state.event_tx.send(text);
                        }
                        let res = WsResponse::ok(&req.id, json!({ "deleted": true }));
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                    }
                    None => {
                        let res = WsResponse::err(&req.id, "session not found");
                        let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
                    }
                }
            }
            "sessions.delete_all" => {
                let count = state.session_store.remove_all().await;
                state.bindings.remove_all().await;
                // Broadcast sessions.cleared event so clients can update without polling.
                let event = json!({
                    "type": "event",
                    "event": "sessions.cleared",
                    "payload": {},
                });
                if let Ok(text) = serde_json::to_string(&event) {
                    let _ = state.event_tx.send(text);
                }
                let res = WsResponse::ok(&req.id, json!({ "deletedCount": count }));
                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
            }
            _ => {
                let res = WsResponse::err(&req.id, format!("unknown method: {}", req.method));
                let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default().into())).await;
            }
        }
            }
        }
    }

    if !sent_hello {
        if connect_attempted {
            log::debug!("ws client connect rejected, client disconnected");
        } else {
            log::debug!("ws client disconnected before sending connect");
        }
    }
}
