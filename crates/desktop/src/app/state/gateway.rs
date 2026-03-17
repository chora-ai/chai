use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use super::super::{
    AgentReply, ChaiApp, GatewayStatusDetails, PROBE_INTERVAL_FRAMES, STATUS_INTERVAL_FRAMES,
};

impl ChaiApp {
    /// Poll for probe result and optionally start a new probe. Call each frame.
    pub(crate) fn poll_gateway_probe(&mut self) {
        if let Some(rx) = &self.probe_receiver {
            if let Ok(ok) = rx.try_recv() {
                self.gateway_probe_completed = true;
                self.gateway_responds = ok;
                if !ok {
                    self.gateway_status = None;
                }
                self.probe_receiver = None;
            }
        }
        self.frames_since_probe = self.frames_since_probe.saturating_add(1);
        if self.probe_receiver.is_none() && self.frames_since_probe >= PROBE_INTERVAL_FRAMES {
            self.frames_since_probe = 0;
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let (config, _) = lib::config::load_config(None)
                    .unwrap_or((lib::config::Config::default(), PathBuf::new()));
                let addr_str = format!(
                    "{}:{}",
                    config.gateway.bind.trim(),
                    config.gateway.port
                );
                let ok = addr_str
                    .parse::<SocketAddr>()
                    .ok()
                    .and_then(|addr| {
                        std::net::TcpStream::connect_timeout(
                            &addr,
                            Duration::from_millis(800),
                        )
                        .ok()
                    })
                    .is_some();
                let _ = tx.send(ok);
            });
            self.probe_receiver = Some(rx);
        }
    }

    /// When gateway status is received, ensure current model is in the available list for the effective backend; if not, switch to gateway default or first available.
    pub(crate) fn reconcile_model_with_status(&mut self) {
        let Some(ref details) = self.gateway_status else {
            return;
        };
        let backend = self
            .current_backend
            .as_deref()
            .or(details.default_backend.as_deref())
            .map(|b| if b == "lm_studio" { "lmstudio" } else { b })
            .unwrap_or("ollama");
        let models: &[String] = if backend == "lmstudio" {
            &details.lm_studio_models
        } else if backend == "nim" {
            &details.nim_models
        } else {
            &details.ollama_models
        };
        if models.is_empty() {
            return;
        }
        let effective = self
            .current_model
            .as_deref()
            .or(details.default_model.as_deref())
            .or(self.default_model.as_deref());
        let in_list = effective
            .map(|m| models.iter().any(|x| x == m))
            .unwrap_or(false);
        if !in_list {
            self.current_model = details
                .default_model
                .clone()
                .filter(|m| models.contains(m))
                .or_else(|| models.first().cloned());
        }
    }

    /// Request that the next status poll performs an immediate fetch (e.g. after switching backend so the model list is up to date).
    pub(crate) fn request_status_refetch(&mut self) {
        self.frames_since_status = STATUS_INTERVAL_FRAMES;
    }

    /// Poll for status fetch result and optionally start a new fetch when gateway is running. Call each frame.
    /// When the gateway has just come back up (responding but no status yet), fetch immediately so the context layout updates without delay.
    pub(crate) fn poll_status_fetch(&mut self) {
        if let Some(rx) = &self.status_receiver {
            if let Ok(result) = rx.try_recv() {
                self.gateway_status = result.ok();
                self.reconcile_model_with_status();
                self.status_receiver = None;
            }
        }
        if !self.gateway_responds || self.status_receiver.is_some() {
            return;
        }
        let need_immediate = self.gateway_status.is_none();
        self.frames_since_status = self.frames_since_status.saturating_add(1);
        if need_immediate || self.frames_since_status >= STATUS_INTERVAL_FRAMES {
            self.frames_since_status = 0;
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let result = fetch_gateway_status();
                let _ = tx.send(result);
            });
            self.status_receiver = Some(rx);
        }
    }
}

/// Fetch gateway status via WebSocket (connect + status). Runs in a thread; use blocking.
pub(crate) fn fetch_gateway_status() -> Result<GatewayStatusDetails, String> {
    let (config, _) = lib::config::load_config(None).map_err(|e| e.to_string())?;
    let bind = config.gateway.bind.trim();
    let port = config.gateway.port;
    let token = lib::config::resolve_gateway_token(&config);
    let ws_url = format!("ws://{}:{}/ws", bind, port);

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .map_err(|e| e.to_string())?;

        let first = ws
            .next()
            .await
            .ok_or("no first frame")?
            .map_err(|e| e.to_string())?;
        let Message::Text(challenge_text) = first else {
            return Err("expected text challenge frame".to_string());
        };
        let challenge: serde_json::Value =
            serde_json::from_str(&challenge_text).map_err(|e| e.to_string())?;
        let nonce = challenge
            .get("payload")
            .and_then(|p| p.get("nonce").and_then(|n| n.as_str()))
            .ok_or("expected connect.challenge event with nonce")?
            .to_string();

        let connect_params = if let Some(device_token) = lib::device::load_device_token() {
            serde_json::json!({ "auth": { "deviceToken": device_token } })
        } else {
            let identity = lib::device::DeviceIdentity::load(lib::device::default_device_path().as_path())
                .or_else(|| {
                    let id = lib::device::DeviceIdentity::generate().ok()?;
                    let _ = id.save(&lib::device::default_device_path());
                    Some(id)
                })
                .ok_or("failed to load or create device identity")?;
            let signed_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let token_str = token.as_deref().unwrap_or("");
            let scopes: Vec<String> = vec!["operator.read".into(), "operator.write".into()];
            let payload_str = lib::device::build_connect_payload(
                &identity.device_id,
                "chai-desktop",
                "operator",
                "operator",
                &scopes,
                signed_at,
                token_str,
                &nonce,
            );
            let signature = identity.sign(&payload_str).map_err(|e| e.to_string())?;
            let mut params = serde_json::json!({
                "client": { "id": "chai-desktop", "mode": "operator" },
                "role": "operator",
                "scopes": scopes,
                "device": {
                    "id": identity.device_id,
                    "publicKey": identity.public_key,
                    "signature": signature,
                    "signedAt": signed_at,
                    "nonce": nonce
                }
            });
            if let Some(ref t) = token {
                params["auth"] = serde_json::json!({ "token": t });
            } else {
                params["auth"] = serde_json::json!({});
            }
            params
        };

        let connect_req = serde_json::json!({
            "type": "req",
            "id": "1",
            "method": "connect",
            "params": connect_params
        });
        ws.send(Message::Text(connect_req.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        let mut details = GatewayStatusDetails::default();
        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") {
                continue;
            }
            if res.get("id").and_then(|v| v.as_str()) == Some("1") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("connect failed");
                    return Err(err.to_string());
                }
                if let Some(auth) = res.get("payload").and_then(|p| p.get("auth")) {
                    if let Some(dt) = auth.get("deviceToken").and_then(|v| v.as_str()) {
                        let _ = lib::device::save_device_token(dt);
                    }
                }
                break;
            }
        }

        let status_req = serde_json::json!({
            "type": "req",
            "id": "2",
            "method": "status",
            "params": {}
        });
        ws.send(Message::Text(status_req.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") {
                continue;
            }
            if res.get("id").and_then(|v| v.as_str()) == Some("2") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("status failed");
                    return Err(err.to_string());
                }
                let payload = res.get("payload").ok_or("missing payload")?;
                details.protocol = payload.get("protocol").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                details.port = payload.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
                details.bind = payload
                    .get("bind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                details.auth = payload
                    .get("auth")
                    .and_then(|v| v.as_str())
                    .unwrap_or("none")
                    .to_string();
                details.default_backend = payload.get("defaultBackend").and_then(|v| v.as_str()).map(String::from);
                details.default_model = payload.get("defaultModel").and_then(|v| v.as_str()).map(String::from);
                details.ollama_models = payload
                    .get("ollamaModels")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                details.lm_studio_models = payload
                    .get("lmStudioModels")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                details.nim_models = payload
                    .get("nimModels")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                details.agent_context = payload.get("agentContext").and_then(|v| v.as_str()).map(String::from);
                details.system_context = payload.get("systemContext").and_then(|v| v.as_str()).map(String::from);
                details.date = payload.get("date").and_then(|v| v.as_str()).map(String::from);
                details.skills_context = payload.get("skillsContext").and_then(|v| v.as_str()).map(String::from);
                details.skills_context_full = payload.get("skillsContextFull").and_then(|v| v.as_str()).map(String::from);
                details.skills_context_bodies = payload.get("skillsContextBodies").and_then(|v| v.as_str()).map(String::from);
                details.context_mode = payload.get("contextMode").and_then(|v| v.as_str()).map(String::from);
                details.tools = payload.get("tools").and_then(|v| v.as_str()).map(String::from);
                return Ok(details);
            }
        }
        Err("no status response".to_string())
    })
}

/// Resolve the chai CLI binary: same directory as this executable, or "chai" from PATH.
pub(crate) fn resolve_chai_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = if cfg!(windows) { "chai.exe" } else { "chai" };
    let candidate = dir.join(name);
    if candidate.exists() {
        return Some(candidate);
    }
    // Fallback: assume "chai" is on PATH
    Some(PathBuf::from("chai"))
}

/// Run one agent turn against the gateway: connect, send message, return reply and session id.
pub(crate) fn run_agent_turn(
    session_id: Option<String>,
    message: String,
    backend: Option<String>,
    model: Option<String>,
) -> Result<AgentReply, String> {
    let (config, _) = lib::config::load_config(None).map_err(|e| e.to_string())?;
    let bind = config.gateway.bind.trim();
    let port = config.gateway.port;
    let token = lib::config::resolve_gateway_token(&config);
    let ws_url = format!("ws://{}:{}/ws", bind, port);

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .map_err(|e| e.to_string())?;

        let first = ws
            .next()
            .await
            .ok_or("no first frame")?
            .map_err(|e| e.to_string())?;
        let Message::Text(challenge_text) = first else {
            return Err("expected text challenge frame".to_string());
        };
        let challenge: serde_json::Value =
            serde_json::from_str(&challenge_text).map_err(|e| e.to_string())?;
        let nonce = challenge
            .get("payload")
            .and_then(|p| p.get("nonce").and_then(|n| n.as_str()))
            .ok_or("expected connect.challenge event with nonce")?
            .to_string();

        let connect_params = if let Some(device_token) = lib::device::load_device_token() {
            serde_json::json!({ "auth": { "deviceToken": device_token } })
        } else {
            let identity = lib::device::DeviceIdentity::load(
                lib::device::default_device_path().as_path(),
            )
            .or_else(|| {
                let id = lib::device::DeviceIdentity::generate().ok()?;
                let _ = id.save(&lib::device::default_device_path());
                Some(id)
            })
            .ok_or("failed to load or create device identity")?;
            let signed_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let token_str = token.as_deref().unwrap_or("");
            let scopes: Vec<String> = vec!["operator.read".into(), "operator.write".into()];
            let payload_str = lib::device::build_connect_payload(
                &identity.device_id,
                "chai-desktop",
                "operator",
                "operator",
                &scopes,
                signed_at,
                token_str,
                &nonce,
            );
            let signature = identity.sign(&payload_str).map_err(|e| e.to_string())?;
            let mut params = serde_json::json!({
                "client": { "id": "chai-desktop", "mode": "operator" },
                "role": "operator",
                "scopes": scopes,
                "device": {
                    "id": identity.device_id,
                    "publicKey": identity.public_key,
                    "signature": signature,
                    "signedAt": signed_at,
                    "nonce": nonce
                }
            });
            if let Some(ref t) = token {
                params["auth"] = serde_json::json!({ "token": t });
            } else {
                params["auth"] = serde_json::json!({});
            }
            params
        };

        let connect_req = serde_json::json!({
            "type": "req",
            "id": "1",
            "method": "connect",
            "params": connect_params
        });
        ws.send(Message::Text(connect_req.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") {
                continue;
            }
            if res.get("id").and_then(|v| v.as_str()) == Some("1") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("connect failed");
                    return Err(err.to_string());
                }
                if let Some(auth) = res.get("payload").and_then(|p| p.get("auth")) {
                    if let Some(dt) = auth.get("deviceToken").and_then(|v| v.as_str()) {
                        let _ = lib::device::save_device_token(dt);
                    }
                }
                break;
            }
        }

        let mut agent_params = serde_json::json!({
            "message": message,
        });
        if let Some(id) = session_id {
            agent_params["sessionId"] = serde_json::Value::String(id);
        }
        if let Some(b) = &backend {
            agent_params["backend"] = serde_json::Value::String(b.clone());
        }
        if let Some(m) = &model {
            agent_params["model"] = serde_json::Value::String(m.clone());
        }

        let agent_req = serde_json::json!({
            "type": "req",
            "id": "2",
            "method": "agent",
            "params": agent_params
        });
        ws.send(Message::Text(agent_req.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") {
                continue;
            }
            if res.get("id").and_then(|v| v.as_str()) == Some("2") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("agent failed");
                    return Err(err.to_string());
                }
                let payload = res.get("payload").ok_or("missing payload")?;
                let session_id = payload
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .ok_or("missing sessionId in agent response")?
                    .to_string();
                let reply = payload
                    .get("reply")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let tool_calls = payload
                    .get("toolCalls")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.clone())
                    .unwrap_or_default();
                return Ok(AgentReply {
                    session_id,
                    reply,
                    tool_calls,
                });
            }
        }
        Err("no agent response".to_string())
    })
}

