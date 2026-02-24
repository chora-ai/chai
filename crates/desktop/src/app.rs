//! Chai Desktop — egui app state and UI.

use eframe::egui;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Child;
use std::sync::mpsc;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

/// Fetch gateway status via WebSocket (connect + status). Runs in a thread; use blocking.
fn fetch_gateway_status() -> Result<GatewayStatusDetails, String> {
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
                details.ollama_models = payload
                    .get("ollamaModels")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                return Ok(details);
            }
        }
        Err("no status response".to_string())
    })
}

/// Resolve the chai CLI binary: same directory as this executable, or "chai" from PATH.
fn resolve_chai_binary() -> Option<PathBuf> {
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

/// Frames between gateway probes (probe at ~1 Hz if 60 fps).
const PROBE_INTERVAL_FRAMES: u32 = 60;

/// Frames between WebSocket status fetches when gateway is running (~0.5 Hz).
const STATUS_INTERVAL_FRAMES: u32 = 120;

/// Live gateway details from WebSocket `status` method.
#[derive(Clone, Default)]
pub struct GatewayStatusDetails {
    pub protocol: u32,
    pub port: u16,
    pub bind: String,
    pub auth: String,
    /// Ollama model names from gateway discovery (empty if Ollama unreachable).
    pub ollama_models: Vec<String>,
}

pub struct ChaiApp {
    /// When Some, the gateway subprocess is running. Cleared when process exits or we stop it.
    gateway_process: Option<Child>,
    /// Last error from start gateway (e.g. spawn failed).
    gateway_error: Option<String>,
    /// True if the configured gateway address:port accepted a TCP connection (we or someone else).
    gateway_responds: bool,
    /// When Some, a probe is in flight; we read the result here.
    probe_receiver: Option<mpsc::Receiver<bool>>,
    /// Frames since we last started a probe.
    frames_since_probe: u32,
    /// When Some, a status fetch is in flight; we read the result here.
    status_receiver: Option<mpsc::Receiver<Result<GatewayStatusDetails, String>>>,
    /// Frames since we last started a status fetch.
    frames_since_status: u32,
    /// Last successful gateway status (protocol, port, bind, auth). Cleared when gateway stops responding.
    gateway_status: Option<GatewayStatusDetails>,
}

impl Default for ChaiApp {
    fn default() -> Self {
        Self {
            gateway_process: None,
            gateway_error: None,
            gateway_responds: false,
            probe_receiver: None,
            frames_since_probe: 0,
            status_receiver: None,
            frames_since_status: 0,
            gateway_status: None,
        }
    }
}

impl ChaiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    /// Poll for probe result and optionally start a new probe. Call each frame.
    fn poll_gateway_probe(&mut self) {
        if let Some(rx) = &self.probe_receiver {
            if let Ok(ok) = rx.try_recv() {
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
                let (config, _) = lib::config::load_config(None).unwrap_or((lib::config::Config::default(), PathBuf::new()));
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

    /// Poll for status fetch result and optionally start a new fetch when gateway is running. Call each frame.
    fn poll_status_fetch(&mut self) {
        if let Some(rx) = &self.status_receiver {
            if let Ok(result) = rx.try_recv() {
                self.gateway_status = result.ok();
                self.status_receiver = None;
            }
        }
        if !self.gateway_responds || self.status_receiver.is_some() {
            return;
        }
        self.frames_since_status = self.frames_since_status.saturating_add(1);
        if self.frames_since_status >= STATUS_INTERVAL_FRAMES {
            self.frames_since_status = 0;
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let result = fetch_gateway_status();
                let _ = tx.send(result);
            });
            self.status_receiver = Some(rx);
        }
    }

    /// True if we started the gateway and it is still running (we can stop it).
    fn gateway_owned(&mut self) -> bool {
        if let Some(ref mut child) = self.gateway_process {
            if child.try_wait().ok().flatten().is_some() {
                self.gateway_process = None;
                return false;
            }
            return true;
        }
        false
    }

    fn start_gateway(&mut self) {
        self.gateway_error = None;
        let (config, _) = match lib::config::load_config(None) {
            Ok(pair) => pair,
            Err(e) => {
                self.gateway_error = Some(format!("failed to load config: {}", e));
                return;
            }
        };
        let port = config.gateway.port;
        let binary = match resolve_chai_binary() {
            Some(p) => p,
            None => {
                self.gateway_error = Some("could not find chai binary".to_string());
                return;
            }
        };
        let child = std::process::Command::new(&binary)
            .args(["gateway", "--port", &port.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        match child {
            Ok(c) => {
                self.gateway_process = Some(c);
            }
            Err(e) => {
                self.gateway_error = Some(format!("failed to start gateway: {}", e));
            }
        }
    }

    fn stop_gateway(&mut self) {
        if let Some(mut child) = self.gateway_process.take() {
            let _ = child.kill();
        }
        self.gateway_error = None;
    }
}

impl eframe::App for ChaiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Chai");
            ui.add_space(8.0);
            ui.label("Multi-agent management system — local models, strong privacy.");
            ui.add_space(16.0);

            self.poll_gateway_probe();
            self.poll_status_fetch();
            let owned = self.gateway_owned();
            let running = owned || self.gateway_responds;

            ui.horizontal(|ui| {
                ui.label("Gateway:");
                if running {
                    ui.label("running");
                    if owned && ui.button("Stop gateway").clicked() {
                        self.stop_gateway();
                    }
                } else {
                    ui.label("stopped");
                    if ui.button("Start gateway").clicked() {
                        self.start_gateway();
                    }
                }
            });

            if running {
                if let Some(ref s) = self.gateway_status {
                    ui.label(format!(
                        "protocol {} · port {} · {} · auth: {}",
                        s.protocol, s.port, s.bind, s.auth
                    ));
                    if !s.ollama_models.is_empty() {
                        ui.label(format!("Ollama: {} model(s) — {}", s.ollama_models.len(), s.ollama_models.join(", ")));
                    }
                } else if self.status_receiver.is_some() {
                    ui.label("fetching status…");
                }
            }

            if let Some(ref err) = self.gateway_error {
                ui.add_space(8.0);
                ui.colored_label(egui::Color32::RED, err);
            }
        });
    }
}
