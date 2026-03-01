//! Gateway HTTP + WebSocket server (single port).

use crate::agent;
use crate::channels::{
    ChannelHandle, ChannelRegistry, InboundMessage, TelegramChannel, TelegramUpdate,
};
use crate::config::{
    self, resolve_lm_studio_base_url, resolve_lm_studio_endpoint_type, Config, SkillContextMode,
};
use crate::agent_ctx;
use crate::init;
use crate::skills::{load_skills, Skill, SkillEntry};
use crate::tools::GenericToolExecutor;
use crate::gateway::pairing::PairingStore;
use crate::gateway::protocol::{
    AgentParams, ConnectDevice, ConnectParams, HelloAuth, HelloOk, SendParams, WsRequest, WsResponse,
};
use crate::llm::{LmStudioClient, LmStudioModel, OllamaClient, OllamaModel, ToolDefinition};
use crate::routing::SessionBindingStore;
use crate::session::SessionStore;
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
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const PROTOCOL_VERSION: u32 = 1;

const DEFAULT_MODEL_FALLBACK: &str = "llama3.2:latest";
const DEFAULT_MODEL_FALLBACK_LMSTUDIO: &str = "gpt-oss-20b";

/// Which LLM backend to use (from agents.defaultBackend).
#[derive(Clone, Copy)]
enum BackendChoice {
    Ollama,
    LmStudio,
}

fn backend_name(choice: BackendChoice) -> &'static str {
    match choice {
        BackendChoice::Ollama => "ollama",
        BackendChoice::LmStudio => "lmstudio",
    }
}

/// Resolve backend from config. Uses agents.defaultBackend ("ollama" | "lmstudio", case-insensitive). Defaults to Ollama when absent or invalid.
fn resolve_backend(agents: &crate::config::AgentsConfig) -> BackendChoice {
    let b = agents
        .default_backend
        .as_deref()
        .unwrap_or("ollama")
        .trim()
        .to_lowercase();
    if b == "lmstudio" || b == "lm_studio" {
        BackendChoice::LmStudio
    } else {
        BackendChoice::Ollama
    }
}

/// Resolve model id from config and optional request param. No prefix stripping—model id is passed as-is to the backend.
/// When no model is set: Ollama uses DEFAULT_MODEL_FALLBACK; LM Studio uses DEFAULT_MODEL_FALLBACK_LMSTUDIO (set defaultModel if your server uses a different id).
fn resolve_model(
    config_model: Option<&str>,
    param_model: Option<&str>,
    backend: BackendChoice,
) -> String {
    let s = param_model
        .or(config_model)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    match (s, backend) {
        (Some(name), _) => name,
        (None, BackendChoice::Ollama) => DEFAULT_MODEL_FALLBACK.to_string(),
        (None, BackendChoice::LmStudio) => DEFAULT_MODEL_FALLBACK_LMSTUDIO.to_string(),
    }
}

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
        device.id,
        client_id,
        client_mode,
        role,
        scopes_str,
        device.signed_at,
        token,
        device.nonce
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
    pk.verify_strict(payload.as_bytes(), &sig).map_err(|_| "device signature verification failed".to_string())?;
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
    /// Optional agent-level context (e.g. AGENTS.md from workspace).
    pub agent_ctx: Option<String>,
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
    pub lm_studio_client: LmStudioClient,
    /// LM Studio models discovered at startup (or soon after). Empty if LM Studio unreachable.
    pub lm_studio_models: Arc<tokio::sync::RwLock<Vec<LmStudioModel>>>,
    /// Loaded skills (name, description, content) for system context. Empty if load failed or no dirs.
    pub skills: Arc<Vec<Skill>>,
    /// Combined tool definitions for the agent (from skills' tools.json only). None when no tools.
    pub tools_list: Option<Vec<ToolDefinition>>,
    /// Generic executor built from skills' tools.json. None when no tools.
    pub tool_executor: Option<Arc<dyn agent::ToolExecutor>>,
    /// Paired devices (deviceId → role, scopes, deviceToken); used for deviceToken auth and issuing new tokens.
    pub pairing_store: Arc<PairingStore>,
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
    /// Register an in-process channel task to be awaited during graceful shutdown.
    #[allow(dead_code)]
    pub async fn register_channel_task(&self, handle: JoinHandle<()>) {
        self.channel_tasks.write().await.push(handle);
    }

    /// Combined tool list and executor (built at startup; includes read_skill when context mode is ReadOnDemand).
    pub fn tools_and_executor(
        &self,
    ) -> (Option<Vec<ToolDefinition>>, Option<&dyn agent::ToolExecutor>) {
        let exec = self.tool_executor.as_deref();
        (self.tools_list.clone(), exec)
    }
}

/// Strip YAML frontmatter (`---` ... `---` blocks) from skill content so we don't duplicate it in the system message.
/// Removes consecutive frontmatter blocks (e.g. duplicated `---` blocks at the start of a SKILL.md).
fn strip_skill_frontmatter(content: &str) -> &str {
    let rest = content.trim_start();
    let rest = rest.strip_prefix("---").map(|s| s.trim_start()).unwrap_or(rest);
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
fn build_skill_context_full(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut out = String::from("You have access to the following tools:\n\n");
    for s in skills {
        out.push_str("- **");
        out.push_str(&s.name);
        out.push_str(":** ");
        if !s.description.is_empty() {
            out.push_str(&s.description);
            out.push_str("\n\n");
        }
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
    let mut out = String::from(
        "You have access to the following tools. Use the read_skill tool to load a skill's full documentation when it clearly applies to the user's request.\n\n",
    );
    out.push_str("## Available tools\n\n");
    for s in skills {
        out.push_str("- **");
        out.push_str(&s.name);
        out.push_str("**: ");
        out.push_str(if s.description.is_empty() {
            "(no description)"
        } else {
            s.description.trim()
        });
        out.push_str("\n");
    }
    out
}

/// Build full system context from agent-ctx (AGENTS.md) and skills. Uses context_mode to choose full vs compact skill context.
/// Prepends the current local date (YYYY-MM-DD) so the model knows "today" for skills like notesmd-cli-daily.
fn build_system_context(
    agent_ctx: Option<&str>,
    skills: &[Skill],
    context_mode: SkillContextMode,
) -> String {
    let mut out = String::new();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    out.push_str("Today's date: ");
    out.push_str(&today);
    out.push_str("\n\n");
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

/// Tool definition for read_skill (used only when context mode is ReadOnDemand).
fn read_skill_tool_definition() -> ToolDefinition {
    ToolDefinition {
        typ: "function".to_string(),
        function: crate::llm::ToolFunctionDefinition {
            name: "read_skill".to_string(),
            description: Some(
                "Load the full documentation (SKILL.md) for a skill. Call when the user's request clearly applies to that skill and you need the full instructions and tool usage details.".to_string(),
            ),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["skill_name"],
                "properties": {
                    "skill_name": {
                        "type": "string",
                        "description": "Name of the skill (e.g. notesmd-cli). Use the exact name from the available skills list."
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
    channel_id: Option<&str>,
    conversation_id: Option<&str>,
) {
    let event = json!({
        "type": "event",
        "event": "session.message",
        "payload": {
            "sessionId": session_id,
            "role": role,
            "content": content,
            "channelId": channel_id,
            "conversationId": conversation_id,
        }
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
                .send_message(&msg.conversation_id, "session restarted. next message will start with a clean history.")
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
        Some(&msg.channel_id),
        Some(&msg.conversation_id),
    );
    let backend_choice = resolve_backend(&state.config.agents);
    let model_name = resolve_model(
        state.config.agents.default_model.as_deref(),
        None,
        backend_choice,
    );
    let system_context = build_system_context(
        state.agent_ctx.as_deref(),
        &state.skills,
        state.config.skills.context_mode,
    );
    let (tools, tool_executor) = state.tools_and_executor();
    let result = match backend_choice {
        BackendChoice::Ollama => {
            agent::run_turn(
                &state.session_store,
                &session_id,
                &state.ollama_client,
                &model_name,
                Some(&system_context),
                tools,
                tool_executor,
                None,
            )
            .await
        }
        BackendChoice::LmStudio => {
            agent::run_turn(
                &state.session_store,
                &session_id,
                &state.lm_studio_client,
                &model_name,
                Some(&system_context),
                tools,
                tool_executor,
                None,
            )
            .await
        }
    };
    let result = match result {
        Ok(r) => r,
        Err(e) => {
            log::warn!("inbound: agent turn failed: {}", e);
            let fallback = format!("something went wrong: {}. check the gateway logs for details.", e);
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
            Some(&msg.channel_id),
            Some(&msg.conversation_id),
        );
        if let Some(handle) = state.channel_registry.get(&msg.channel_id).await {
            if handle.send_message(&msg.conversation_id, &reply).await.is_err() {
                log::warn!("inbound: send_message failed");
            }
        }
    }
}

/// Run the gateway server; binds to config.gateway.bind:config.gateway.port.
/// When bind is not loopback, a gateway token must be configured or startup fails.
/// Blocks until shutdown (e.g. Ctrl+C).
/// `config_path` is the path to the config file (used to resolve the config directory for skills).
/// Requires the configuration directory to be initialized (`chai init`) so the skills directory exists.
pub async fn run_gateway(config: Config, config_path: PathBuf) -> Result<()> {
    init::require_initialized(&config_path, &config)?;
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
    let paired_path = dirs::home_dir()
        .map(|h| h.join(".chai").join("paired.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("paired.json"));
    let pairing_store = Arc::new(PairingStore::load(paired_path).await);
    let (event_tx, _) = broadcast::channel(64);
    let channel_tasks = Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let ollama_models = Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let lm_studio_base_url = Some(resolve_lm_studio_base_url(&config.agents));
    let lm_studio_endpoint_type = resolve_lm_studio_endpoint_type(&config.agents);
    let lm_studio_client = LmStudioClient::new(lm_studio_base_url, lm_studio_endpoint_type);
    let lm_studio_models = Arc::new(tokio::sync::RwLock::new(Vec::new()));
    let (inbound_tx, mut inbound_rx) = mpsc::channel::<InboundMessage>(64);

    let workspace_dir = config::resolve_workspace_dir(&config);
    let skills_dir = config::resolve_skills_dir(&config, &config_path);
    let mut skill_entries: Vec<SkillEntry> = match load_skills(
        Some(skills_dir.as_path()),
        &config.skills.extra_dirs,
    ) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!("loading skills failed: {}", e);
            Vec::new()
        }
    };
    skill_entries.retain(|e| config.skills.enabled.iter().any(|n| n == &e.name));
    log::info!("loaded {} skill(s) for agent context", skill_entries.len());
    if config.skills.context_mode == SkillContextMode::ReadOnDemand {
        log::info!("skill context mode: readOnDemand (compact list + read_skill tool)");
    }
    let skills: Vec<Skill> = skill_entries.iter().map(Skill::from).collect();
    let agent_ctx = agent_ctx::load_agent_ctx(workspace_dir.as_deref());

    // Descriptor-based: skills with tools.json
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
    let generic_executor = GenericToolExecutor::from_descriptors(
        &descriptors,
        &skill_dirs,
        config.skills.allow_scripts,
    );
    let context_mode = config.skills.context_mode;

    // Tool list: descriptor tools; when ReadOnDemand, prepend read_skill
    let mut tools_list: Vec<ToolDefinition> = Vec::new();
    if context_mode == SkillContextMode::ReadOnDemand && !skills.is_empty() {
        tools_list.push(read_skill_tool_definition());
    }
    for (_, desc) in &descriptors {
        tools_list.extend(desc.to_tool_definitions());
    }
    let tools_list = if tools_list.is_empty() {
        None
    } else {
        Some(tools_list)
    };

    // Executor: when ReadOnDemand and we have any tools, wrap generic in ReadOnDemandExecutor; otherwise generic only (or none)
    let tool_executor: Option<Arc<dyn agent::ToolExecutor>> = if tools_list.is_some() {
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

    let state = GatewayState {
        config: Arc::new(config.clone()),
        agent_ctx,
        required_token,
        event_tx: event_tx.clone(),
        channel_tasks: channel_tasks.clone(),
        inbound_tx: inbound_tx.clone(),
        session_store: Arc::new(SessionStore::new()),
        channel_registry: Arc::new(ChannelRegistry::new()),
        bindings: Arc::new(SessionBindingStore::new()),
        ollama_client: OllamaClient::new(None),
        ollama_models: ollama_models.clone(),
        lm_studio_client,
        lm_studio_models: lm_studio_models.clone(),
        skills: Arc::new(skills),
        tools_list,
        tool_executor,
        pairing_store,
    };

    if config::backend_discovery_enabled(&config.agents, "ollama") {
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
        log::debug!("ollama model discovery skipped (not in enabledBackends)");
    }
    if config::backend_discovery_enabled(&config.agents, "lmstudio") {
        let lm_studio = state.lm_studio_client.clone();
        let models = state.lm_studio_models.clone();
        tokio::spawn(async move {
            match lm_studio.list_models().await {
                Ok(list) => {
                    *models.write().await = list;
                    log::info!("lm studio model discovery completed");
                }
                Err(e) => {
                    log::debug!("lm studio model discovery failed: {}", e);
                }
            }
        });
    } else {
        log::debug!("lm studio model discovery skipped (not in enabledBackends)");
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
            let telegram = Arc::new(TelegramChannel::new(Some(token)));
            if let Some(ref url) = webhook_url {
                let secret = config.channels.telegram.webhook_secret.as_deref();
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
                let handle = telegram.clone().start_inbound(inbound_tx);
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

    let channel_registry = state.channel_registry.clone();
    let app = Router::new()
        .route("/", get(health_http))
        .route("/ws", get(ws_handler))
        .route("/telegram/webhook", post(telegram_webhook))
        .with_state(state);

    let bind_addr = format!("{}:{}", bind, config.gateway.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("binding to {}", bind_addr))?;
    log::info!("gateway listening on {}", bind_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(
            event_tx,
            channel_registry,
            channel_tasks,
            telegram_webhook_for_shutdown,
        ))
        .await
        .context("gateway server exited")?;
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
    if let Some(ref expected) = state.config.channels.telegram.webhook_secret {
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
        "runtime": "running",
        "protocol": PROTOCOL_VERSION,
        "port": state.config.gateway.port,
    }))
}

/// GET /ws upgrades to WebSocket. First frame must be connect; we reply with hello-ok.
async fn ws_handler(
    State(state): State<GatewayState>,
    ws: WebSocketUpgrade,
) -> Response {
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
                    "runtime": "running",
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
                let backend_choice = resolve_backend(&state.config.agents);
                let default_model = resolve_model(
                    state.config.agents.default_model.as_deref(),
                    None,
                    backend_choice,
                );
                let ollama_models = state.ollama_models.read().await.clone();
                let lm_studio_models = state.lm_studio_models.read().await.clone();
                let system_context = build_system_context(
                    state.agent_ctx.as_deref(),
                    &state.skills,
                    state.config.skills.context_mode,
                );
                let skills_context = match state.config.skills.context_mode {
                    SkillContextMode::Full => build_skill_context_full(&state.skills),
                    SkillContextMode::ReadOnDemand => build_skill_context_compact(&state.skills),
                };
                let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                let payload = json!({
                    "runtime": "running",
                    "protocol": PROTOCOL_VERSION,
                    "port": state.config.gateway.port,
                    "bind": state.config.gateway.bind,
                    "auth": auth_mode,
                    "defaultBackend": backend_name(backend_choice),
                    "defaultModel": default_model,
                    "ollamaModels": ollama_models,
                    "lmStudioModels": lm_studio_models,
                    "agentContext": state.agent_ctx,
                    "systemContext": system_context,
                    "date": today,
                    "skillsContext": skills_context,
                    "contextMode": state.config.skills.context_mode,
                });
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
                );
                // Use request backend override when valid ("ollama" | "lmstudio"), else config default.
                let backend_choice = params
                    .backend
                    .as_deref()
                    .map(|b| b.trim().to_lowercase())
                    .and_then(|b| {
                        if b == "ollama" {
                            Some(BackendChoice::Ollama)
                        } else if b == "lmstudio" || b == "lm_studio" {
                            Some(BackendChoice::LmStudio)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| resolve_backend(&state.config.agents));
                let model_name = resolve_model(
                    state.config.agents.default_model.as_deref(),
                    params.model.as_deref(),
                    backend_choice,
                );
                let system_context = build_system_context(
                    state.agent_ctx.as_deref(),
                    &state.skills,
                    state.config.skills.context_mode,
                );
                let (tools, tool_executor) = state.tools_and_executor();
                let run_result = match backend_choice {
                    BackendChoice::Ollama => {
                        agent::run_turn(
                            &state.session_store,
                            &session_id,
                            &state.ollama_client,
                            &model_name,
                            Some(&system_context),
                            tools,
                            tool_executor,
                            None,
                        )
                        .await
                    }
                    BackendChoice::LmStudio => {
                        agent::run_turn(
                            &state.session_store,
                            &session_id,
                            &state.lm_studio_client,
                            &model_name,
                            Some(&system_context),
                            tools,
                            tool_executor,
                            None,
                        )
                        .await
                    }
                };
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
                        let payload = json!({
                            "reply": result.content,
                            "sessionId": session_id,
                            "toolCalls": result.tool_calls
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
