//! Gateway HTTP + WebSocket server (single port).

use crate::agent;
use crate::channels::{
    ChannelHandle, ChannelRegistry, InboundMessage, TelegramChannel, TelegramUpdate,
};
use crate::config::{self, Config};
use crate::agent_ctx;
use crate::init;
use crate::exec;
use crate::skills::{load_skills, Skill};
use crate::tools;
use crate::gateway::pairing::PairingStore;
use crate::gateway::protocol::{
    AgentParams, ConnectDevice, ConnectParams, HelloAuth, HelloOk, SendParams, WsRequest, WsResponse,
};
use crate::llm::{OllamaClient, OllamaModel, ToolDefinition};
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

/// JSON event frame sent to WebSocket clients when the gateway is shutting down.
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
    /// Loaded skills (name, description, content) for system context. Empty if load failed or no dirs.
    pub skills: Arc<Vec<Skill>>,
    /// When any Obsidian-related skill is loaded, combined tools and a dispatcher executor (obsidian and/or notesmd-cli). None when neither skill is loaded.
    pub obsidian_executor: Option<Arc<dyn agent::ToolExecutor>>,
    /// Paired devices (deviceId → role, scopes, deviceToken); used for deviceToken auth and issuing new tokens.
    pub pairing_store: Arc<PairingStore>,
}

/// Dispatches tool calls to the obsidian and/or notesmd-cli executors by tool name prefix.
struct ObsidianDispatcher {
    obsidian: Option<Arc<tools::ObsidianToolExecutor>>,
    notesmd_cli: Option<Arc<tools::NotesmdCliToolExecutor>>,
}

impl agent::ToolExecutor for ObsidianDispatcher {
    fn execute(&self, name: &str, args: &serde_json::Value) -> Result<String, String> {
        if name.starts_with("notesmd_cli_") {
            return match &self.notesmd_cli {
                Some(e) => e.execute(name, args),
                None => Err("notesmd-cli skill not loaded".to_string()),
            };
        }
        if name.starts_with("obsidian_") {
            return match &self.obsidian {
                Some(e) => e.execute(name, args),
                None => Err("obsidian skill not loaded".to_string()),
            };
        }
        Err(format!("unknown tool: {}", name))
    }
}

impl GatewayState {
    /// Register an in-process channel task to be awaited during graceful shutdown.
    #[allow(dead_code)]
    pub async fn register_channel_task(&self, handle: JoinHandle<()>) {
        self.channel_tasks.write().await.push(handle);
    }

    /// Build the combined Obsidian tools list and executor reference from loaded skills.
    /// When only one skill is loaded, only that skill's tools are included.
    pub fn obsidian_tools_and_executor(
        &self,
    ) -> (Option<Vec<ToolDefinition>>, Option<&dyn agent::ToolExecutor>) {
        let has_obsidian = self.skills.iter().any(|s| s.name == "obsidian");
        let has_notesmd_cli = self.skills.iter().any(|s| s.name == "notesmd-cli");
        let tools = if has_obsidian || has_notesmd_cli {
            let mut v = Vec::new();
            if has_obsidian {
                v.extend(tools::obsidian_tool_definitions());
            }
            if has_notesmd_cli {
                v.extend(tools::notesmd_cli_tool_definitions());
            }
            Some(v)
        } else {
            None
        };
        let exec = self.obsidian_executor.as_deref();
        (tools, exec)
    }
}

/// Strip YAML frontmatter (first `---` ... `---` block) from skill content so we don't duplicate it in the system message.
fn strip_skill_frontmatter(content: &str) -> &str {
    let rest = content.strip_prefix("---").map(|s| s.trim_start()).unwrap_or(content);
    if let Some(i) = rest.find("\n---") {
        rest.get(i + 4..).unwrap_or(rest).trim_start()
    } else {
        rest
    }
}

/// Build system context string from loaded skills (injected into the agent as a system message).
fn build_skill_context(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut out = String::from("You have access to the following skills. Use them when relevant.\n\n");
    for s in skills {
        out.push_str("## ");
        out.push_str(&s.name);
        out.push_str("\n");
        if !s.description.is_empty() {
            out.push_str(&s.description);
            out.push_str("\n\n");
        }
        out.push_str(strip_skill_frontmatter(&s.content));
        out.push_str("\n\n");
    }
    out
}

/// Build full system context from agent-ctx (AGENTS.md) and skills.
fn build_system_context(agent_ctx: Option<&str>, skills: &[Skill]) -> String {
    let mut out = String::new();
    if let Some(ctx) = agent_ctx {
        let trimmed = ctx.trim();
        if !trimmed.is_empty() {
            out.push_str(trimmed);
            out.push_str("\n\n");
        }
    }
    let skills_ctx = build_skill_context(skills);
    if !skills_ctx.trim().is_empty() {
        out.push_str(&skills_ctx);
    }
    out
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
    let model = state
        .config
        .agents
        .default_model
        .as_deref()
        .unwrap_or("llama3.2:latest");
    let system_context = build_system_context(state.agent_ctx.as_deref(), &state.skills);
    let (tools, tool_executor) = state.obsidian_tools_and_executor();
    let result = match agent::run_turn(
        &state.session_store,
        &session_id,
        &state.ollama_client,
        model,
        Some(&system_context),
        tools,
        tool_executor,
        None,
    )
    .await
    {
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
/// Requires the configuration directory to be initialized (`chai init`) so bundled skills exist.
pub async fn run_gateway(config: Config, config_path: PathBuf) -> Result<()> {
    init::require_initialized(&config_path)?;
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
    let (inbound_tx, mut inbound_rx) = mpsc::channel::<InboundMessage>(64);

    let workspace_dir = config::resolve_workspace_dir(&config);
    let bundled_dir = config::bundled_skills_dir(&config_path);
    let mut skills: Vec<Skill> = match load_skills(
        Some(bundled_dir.as_path()),
        workspace_dir.as_deref(),
        &config.skills.extra_dirs,
    ) {
        Ok(entries) => entries.into_iter().map(Skill::from).collect(),
        Err(e) => {
            log::warn!("loading skills failed: {}", e);
            Vec::new()
        }
    };
    skills.retain(|s| !config.skills.disabled.iter().any(|d| d == &s.name));
    log::info!("loaded {} skill(s) for agent context", skills.len());
    let agent_ctx = agent_ctx::load_agent_ctx(workspace_dir.as_deref());

    let has_obsidian = skills.iter().any(|s| s.name == "obsidian");
    let has_notesmd_cli = skills.iter().any(|s| s.name == "notesmd-cli");
    let obsidian_executor: Option<Arc<dyn agent::ToolExecutor>> =
        if has_obsidian || has_notesmd_cli {
            Some(Arc::new(ObsidianDispatcher {
                obsidian: if has_obsidian {
                    Some(Arc::new(tools::ObsidianToolExecutor {
                        allowlist: exec::obsidian_allowlist(),
                    }))
                } else {
                    None
                },
                notesmd_cli: if has_notesmd_cli {
                    Some(Arc::new(tools::NotesmdCliToolExecutor {
                        allowlist: exec::notesmd_cli_allowlist(),
                    }))
                } else {
                    None
                },
            }))
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
        skills: Arc::new(skills),
        obsidian_executor,
        pairing_store,
    };

    {
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
                        let _ = socket.send(Message::Text(text)).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::debug!("ws client lagged {} broadcast messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
                break;
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
                let models = state.ollama_models.read().await.clone();
                let payload = json!({
                    "runtime": "running",
                    "protocol": PROTOCOL_VERSION,
                    "port": state.config.gateway.port,
                    "bind": state.config.gateway.bind,
                    "auth": auth_mode,
                    "ollamaModels": models,
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
                if let Err(e) = state
                    .session_store
                    .append_message(&session_id, "user", &params.message)
                    .await
                {
                    let res = WsResponse::err(&req.id, e);
                    let _ = socket.send(Message::Text(serde_json::to_string(&res).unwrap_or_default())).await;
                    continue;
                }
                let model = state
                    .config
                    .agents
                    .default_model
                    .as_deref()
                    .unwrap_or("llama3.2:latest");
                let system_context = build_system_context(state.agent_ctx.as_deref(), &state.skills);
                let (tools, tool_executor) = state.obsidian_tools_and_executor();
                match agent::run_turn(
                    &state.session_store,
                    &session_id,
                    &state.ollama_client,
                    model,
                    Some(&system_context),
                    tools,
                    tool_executor,
                    None,
                )
                .await
                {
                    Ok(result) => {
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
