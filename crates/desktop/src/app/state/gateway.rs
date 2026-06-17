use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use super::super::{
    AgentReply, AgentSkillsRuntime, ChaiApp, GatewayStatusDetails, PROBE_INTERVAL_FRAMES,
    STATUS_INTERVAL_FRAMES,
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
            let profile_override = self.effective_profile_override().map(String::from);
            std::thread::spawn(move || {
                let Ok((config, _paths)) = lib::config::load_config(profile_override.as_deref()) else {
                    let _ = tx.send(false);
                    return;
                };
                let addr_str = format!("{}:{}", config.gateway.bind.trim(), config.gateway.port);
                let ok = addr_str
                    .parse::<SocketAddr>()
                    .ok()
                    .and_then(|addr| {
                        std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(800)).ok()
                    })
                    .is_some();
                let _ = tx.send(ok);
            });
            self.probe_receiver = Some(rx);
        }
    }

    /// When gateway status is received, ensure current model is in the available list for the effective provider; if not, switch to gateway default or first available.
    pub(crate) fn reconcile_model_with_status(&mut self) {
        if self.gateway_status.is_none() {
            return;
        }
        let enabled = self.enabled_providers();
        let Some(ref details) = self.gateway_status else {
            return;
        };
        let provider = self
            .current_provider
            .as_deref()
            .or(details.default_provider.as_deref())
            .or_else(|| details.provider_info.keys().next().map(|s| s.as_str()))
            .or_else(|| enabled.first().map(|s| s.as_str()))
            .unwrap_or("ollama");
        let models: &[String] = details
            .provider_info
            .get(provider)
            .map(|info| info.models.as_slice())
            .unwrap_or(&[]);
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

    /// Request that the next status poll performs an immediate fetch (e.g. after switching provider so the model list is up to date).
    pub(crate) fn request_status_refetch(&mut self) {
        self.frames_since_status = STATUS_INTERVAL_FRAMES;
    }

    /// Poll for status fetch result and optionally start a new fetch when gateway is running.
    /// When the gateway has just come back up (responding but no status yet), fetch immediately so the context layout updates without delay.
    /// When the previous fetch failed (gateway_status is None but a fetch already completed this
    /// session), wait for the normal STATUS_INTERVAL_FRAMES cadence instead of retrying
    /// every frame to avoid a tight reconnect loop.
    pub(crate) fn poll_status_fetch(&mut self) {
        if let Some(rx) = &self.status_receiver {
            if let Ok(result) = rx.try_recv() {
                self.gateway_status = result.ok();
                self.reconcile_dashboard_agent_selection();
                self.reconcile_model_with_status();
                self.status_receiver = None;
                if self.gateway_status.is_none() {
                    // Previous fetch failed — reset the frame counter so the next
                    // attempt waits for the full interval rather than retrying
                    // immediately (which would create a tight loop of WS connects).
                    self.frames_since_status = 0;
                    self.status_fetch_ever_failed = true;
                } else {
                    self.status_fetch_ever_failed = false;
                }
            }
        }
        if !self.gateway_responds || self.status_receiver.is_some() {
            return;
        }
        // Only fetch immediately on the very first detection (gateway_status has never
        // been set AND no previous fetch has failed). Once a fetch has failed, let the
        // normal interval cadence apply to avoid a tight retry loop of WebSocket connects.
        let need_immediate = self.gateway_status.is_none() && !self.status_fetch_ever_failed;
        self.frames_since_status = self.frames_since_status.saturating_add(1);
        if need_immediate || self.frames_since_status >= STATUS_INTERVAL_FRAMES {
            self.frames_since_status = 0;
            let (tx, rx) = mpsc::channel();
            let profile_override = self.effective_profile_override().map(String::from);
            std::thread::spawn(move || {
                let result = fetch_gateway_status(profile_override.as_deref());
                let _ = tx.send(result);
            });
            self.status_receiver = Some(rx);
        }
    }

    /// Poll for gateway log fetch result and optionally start a new fetch. Call each frame.
    ///
    /// Only fetches logs from the gateway when the gateway is **external** (not owned
    /// by the desktop). When the desktop spawns the gateway itself, it already captures
    /// the gateway's stderr/stdout directly, so the WS method is unnecessary.
    ///
    /// Uses `gateway_logs_cursor` (a sequence number) to skip lines already ingested,
    /// avoiding duplicates.
    pub(crate) fn poll_gateway_logs_fetch(&mut self, owned: bool) {
        if let Some(rx) = &self.gateway_logs_receiver {
            if let Ok(result) = rx.try_recv() {
                if let Ok((lines, max_seq)) = result {
                    for line in lines {
                        crate::app::state::logs::push_gateway_log_line(line);
                    }
                    self.gateway_logs_cursor = max_seq;
                }
                self.gateway_logs_receiver = None;
            }
        }
        // Only fetch logs from external gateways.
        if owned || !self.gateway_responds || self.gateway_logs_receiver.is_some() {
            // When the gateway is owned or not responding, reset the cursor,
            // frame counter, and any in-flight receiver so the next external
            // gateway starts fresh.
            if owned || !self.gateway_responds {
                self.gateway_logs_cursor = 0;
                self.frames_since_gateway_logs = 0;
                self.gateway_logs_receiver = None;
            }
            return;
        }
        self.frames_since_gateway_logs = self.frames_since_gateway_logs.saturating_add(1);
        if self.frames_since_gateway_logs >= STATUS_INTERVAL_FRAMES {
            self.frames_since_gateway_logs = 0;
            let (tx, rx) = mpsc::channel();
            let profile_override = self.effective_profile_override().map(String::from);
            let after_seq = self.gateway_logs_cursor;
            std::thread::spawn(move || {
                let result = fetch_gateway_logs(profile_override.as_deref(), after_seq);
                let _ = tx.send(result);
            });
            self.gateway_logs_receiver = Some(rx);
        }
    }
}

/// Parse the `providers` block from gateway status into per-provider info.
/// The gateway now sends each provider as `{ "endpointType": "...", "discovery": bool, "models": [string, ...] }`
/// keyed by provider id, instead of the old fixed-field format with `{"name": "..."}` model objects.
fn parse_providers_block(providers: Option<&serde_json::Value>) -> std::collections::HashMap<String, super::super::ProviderStatusInfo> {
    let Some(obj) = providers.and_then(|p| p.as_object()) else {
        return std::collections::HashMap::new();
    };
    let mut map = std::collections::HashMap::new();
    for (pid, val) in obj {
        let endpoint_type = val.get("endpointType")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let discovery = val.get("discovery")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        // Models are now flat strings (not {"name": "..."} objects).
        let models = val.get("models")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        map.insert(pid.clone(), super::super::ProviderStatusInfo {
            endpoint_type,
            discovery,
            models,
        });
    }
    map
}

/// Fetch gateway status via WebSocket (connect + status). Runs in a thread; use blocking.
pub(crate) fn fetch_gateway_status(profile_override: Option<&str>) -> Result<GatewayStatusDetails, String> {
    let (config, paths) = lib::config::load_config(profile_override).map_err(|e| e.to_string())?;
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

        let connect_params = build_connect_params(&paths, token.as_deref(), &nonce)?;

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
                    // If the device token was rejected, delete it and retry with
                    // device identity + signature so the next attempt doesn't loop
                    // on the same stale token.
                    if err == "invalid device token" {
                        let _ = std::fs::remove_file(paths.device_token_path());
                    }
                    return Err(err.to_string());
                }
                if let Some(auth) = res.get("payload").and_then(|p| p.get("auth")) {
                    if let Some(dt) = auth.get("deviceToken").and_then(|v| v.as_str()) {
                        let _ = lib::device::save_device_token_to(&paths.device_token_path(), dt);
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
                details.status_response_json = serde_json::to_string_pretty(&res).ok();
                let payload = res.get("payload").ok_or("missing payload")?;
                let gateway = payload.get("gateway");
                let agents_pl = payload.get("agents");
                let providers_pl = payload.get("providers");
                details.channels_block = payload.get("channels").cloned();
                details.providers_block = payload.get("providers").cloned();
                if let Some(sp) = payload.get("skillPackages") {
                    details.skill_packages_discovery_root = sp
                        .get("discoveryRoot")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    details.skill_packages_discovered =
                        sp.get("packagesDiscovered").and_then(|v| v.as_u64());
                }
                details.protocol = gateway
                    .and_then(|g| g.get("protocol"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                details.port = gateway
                    .and_then(|g| g.get("port"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u16;
                details.bind = gateway
                    .and_then(|g| g.get("bind"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                details.auth = gateway
                    .and_then(|g| g.get("auth"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("none")
                    .to_string();
                details.status = gateway
                    .and_then(|g| g.get("status"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if let Some(entries) = agents_pl
                    .and_then(|a| a.get("entries"))
                    .and_then(|e| e.as_array())
                {
                    for entry in entries {
                        let Some(id) = entry
                            .get("id")
                            .and_then(|v| v.as_str())
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        else {
                            continue;
                        };
                        let id = id.to_string();
                        let role = entry.get("role").and_then(|v| v.as_str()).unwrap_or("");

                        if let Some(s) = entry.get("systemContext").and_then(|v| v.as_str()) {
                            details
                                .agent_system_contexts
                                .insert(id.clone(), s.to_string());
                        }

                        if let Some(t) = entry
                            .get("tools")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                        {
                            details.agent_tools.insert(id.clone(), t.to_string());
                        }

                        // Parse per-agent skills runtime data from `entry.skills`.
                        let sk = entry.get("skills");
                        let mut agent_rt = AgentSkillsRuntime::default();
                        if let Some(sk_obj) = sk.and_then(|v| v.as_object()) {
                            agent_rt.enabled_skills = sk_obj
                                .get("enabledSkills")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default();
                            agent_rt.context_mode = sk_obj
                                .get("contextMode")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            agent_rt.skills_context = sk_obj
                                .get("skillsContext")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            agent_rt.skills_context_full = sk_obj
                                .get("skillsContextFull")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            agent_rt.skills_context_bodies = sk_obj
                                .get("skillsContextBodies")
                                .and_then(|v| v.as_object())
                                .map(|obj| {
                                    obj.iter()
                                        .filter_map(|(k, v)| {
                                            v.as_str().map(|s| (k.clone(), s.to_string()))
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                        }
                        details.agent_skills.insert(id.clone(), agent_rt);

                        // Backfill agent_context_modes from per-agent runtime data.
                        if let Some(mode) = details
                            .agent_skills
                            .get(&id)
                            .and_then(|rt| rt.context_mode.as_deref())
                        {
                            details
                                .agent_context_modes
                                .insert(id.clone(), mode.to_string());
                        }

                        match role {
                            "orchestrator" => {
                                details.orchestrator_id = Some(id.clone());
                                details.orchestrator_context_dir = entry
                                    .get("contextDirectory")
                                    .and_then(|v| v.as_str())
                                    .map(String::from);
                                details.default_provider = entry
                                    .get("defaultProvider")
                                    .and_then(|v| v.as_str())
                                    .map(String::from);
                                details.default_model = entry
                                    .get("defaultModel")
                                    .and_then(|v| v.as_str())
                                    .map(String::from);
                                details.enabled_providers = entry
                                    .get("enabledProviders")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| {
                                                v.as_str().map(|s| s.trim().to_string())
                                            })
                                            .filter(|s| !s.is_empty())
                                            .collect()
                                    });
                                details.system_context = entry
                                    .get("systemContext")
                                    .and_then(|v| v.as_str())
                                    .map(String::from);
                                // Top-level orchestrator shortcuts (mirror from agent_skills).
                                if let Some(rt) = details.agent_skills.get(&id) {
                                    details.skills_context = rt.skills_context.clone();
                                    details.skills_context_full = rt.skills_context_full.clone();
                                    details.skills_context_bodies = rt.skills_context_bodies.clone();
                                    details.context_mode = rt.context_mode.clone();
                                }
                                details.tools = entry
                                    .get("tools")
                                    .and_then(|v| v.as_str())
                                    .map(String::from);
                            }
                            "worker" => {
                                details.workers.push(crate::app::types::StatusWorkerRow {
                                    id: id.clone(),
                                    default_provider: entry
                                        .get("defaultProvider")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    default_model: entry
                                        .get("defaultModel")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                });
                            }
                            _ => {}
                        }
                    }
                }
                details.provider_info = parse_providers_block(providers_pl);
                details.orchestration_catalog = agents_pl
                    .and_then(|a| a.get("orchestrationCatalog"))
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|o| {
                                let provider = o
                                    .get("provider")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .trim()
                                    .to_string();
                                let model = o
                                    .get("model")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .trim()
                                    .to_string();
                                if provider.is_empty() || model.is_empty() {
                                    return None;
                                }
                                Some(crate::app::types::OrchestrationCatalogRow {
                                    provider,
                                    model,
                                    discovered: o
                                        .get("discovered")
                                        .and_then(|v| v.as_bool())
                                        .unwrap_or(false),
                                })
                            })
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
/// Build WebSocket connect params using device token (if available) or device identity + signature.
/// Shared by `fetch_gateway_status`, `run_agent_turn`, and `run_session_events_loop`.
pub(crate) fn build_connect_params(
    paths: &lib::profile::ChaiPaths,
    gateway_token: Option<&str>,
    nonce: &str,
) -> Result<serde_json::Value, String> {
    if let Some(device_token) = lib::device::load_device_token_from(&paths.device_token_path()) {
        Ok(serde_json::json!({ "auth": { "deviceToken": device_token } }))
    } else {
        let identity = lib::device::DeviceIdentity::load(paths.device_json().as_path())
            .or_else(|| {
                let id = lib::device::DeviceIdentity::generate().ok()?;
                let _ = id.save(&paths.device_json());
                Some(id)
            })
            .ok_or("failed to load or create device identity")?;
        let signed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let token_str = gateway_token.unwrap_or("");
        let scopes: Vec<String> = vec!["operator.read".into(), "operator.write".into()];
        let payload_str = lib::device::build_connect_payload(
            &identity.device_id,
            "chai-desktop",
            "operator",
            "operator",
            &scopes,
            signed_at,
            token_str,
            nonce,
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
        if let Some(t) = gateway_token {
            params["auth"] = serde_json::json!({ "token": t });
        } else {
            params["auth"] = serde_json::json!({});
        }
        Ok(params)
    }
}

/// Fetch new gateway log lines via the `logs` WebSocket method.
///
/// Connects to the gateway, authenticates, sends a `logs` request with
/// `afterSeq`, and returns the new lines plus the max sequence number.
/// Used by the desktop to display gateway logs when connected to an
/// external (non-owned) gateway.
///
/// Returns `(new_lines, max_seq)`.
pub(crate) fn fetch_gateway_logs(profile_override: Option<&str>, after_seq: u64) -> Result<(Vec<String>, u64), String> {
    let (config, paths) = lib::config::load_config(profile_override).map_err(|e| e.to_string())?;
    let bind = config.gateway.bind.trim();
    let port = config.gateway.port;
    let token = lib::config::resolve_gateway_token(&config);
    let ws_url = format!("ws://{}:{}/ws", bind, port);

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .map_err(|e| e.to_string())?;

        // Read challenge.
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

        let connect_params = build_connect_params(&paths, token.as_deref(), &nonce)?;

        // Connect.
        let connect_req = serde_json::json!({
            "type": "req",
            "id": "1",
            "method": "connect",
            "params": connect_params
        });
        ws.send(Message::Text(connect_req.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        // Wait for hello-ok.
        let hello = ws
            .next()
            .await
            .ok_or("no hello-ok frame")?
            .map_err(|e| e.to_string())?;
        let Message::Text(hello_text) = hello else {
            return Err("expected text hello-ok frame".to_string());
        };
        let hello_val: serde_json::Value =
            serde_json::from_str(&hello_text).map_err(|e| e.to_string())?;
        if !hello_val
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            let err = hello_val
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("hello-ok not ok");
            if err == "invalid device token" {
                let _ = std::fs::remove_file(paths.device_token_path());
            }
            return Err(err.to_string());
        }
        if let Some(auth) = hello_val.get("payload").and_then(|p| p.get("auth")) {
            if let Some(dt) = auth.get("deviceToken").and_then(|v| v.as_str()) {
                let _ = lib::device::save_device_token_to(&paths.device_token_path(), dt);
            }
        }

        // Send logs request.
        let logs_req = serde_json::json!({
            "type": "req",
            "id": "2",
            "method": "logs",
            "params": { "afterSeq": after_seq }
        });
        ws.send(Message::Text(logs_req.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        // Read logs response.
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
                        .unwrap_or("logs request failed");
                    return Err(err.to_string());
                }
                let payload = res.get("payload").ok_or("missing payload")?;
                let max_seq = payload
                    .get("maxSeq")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let lines_arr = payload
                    .get("lines")
                    .and_then(|v| v.as_array())
                    .ok_or("missing lines array")?;
                let log_lines: Vec<String> = lines_arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                return Ok((log_lines, max_seq));
            }
        }
        Err("no logs response".to_string())
    })
}

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
    profile_override: Option<&str>,
    session_id: Option<String>,
    message: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<AgentReply, String> {
    let (config, paths) = lib::config::load_config(profile_override).map_err(|e| e.to_string())?;
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

        let connect_params = build_connect_params(&paths, token.as_deref(), &nonce)?;

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
                    if err == "invalid device token" {
                        let _ = std::fs::remove_file(paths.device_token_path());
                    }
                    return Err(err.to_string());
                }
                if let Some(auth) = res.get("payload").and_then(|p| p.get("auth")) {
                    if let Some(dt) = auth.get("deviceToken").and_then(|v| v.as_str()) {
                        let _ = lib::device::save_device_token_to(&paths.device_token_path(), dt);
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
        if let Some(b) = &provider {
            agent_params["provider"] = serde_json::Value::String(b.clone());
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
            let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
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
                let tool_results = payload
                    .get("toolResults")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let loop_limit_reached = payload
                    .get("loopLimitReached")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let pending_tool_calls = payload
                    .get("pendingToolCalls")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.clone())
                    .unwrap_or_default();
                let stopped = payload
                    .get("stopped")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                return Ok(AgentReply {
                    session_id,
                    reply,
                    tool_calls,
                    tool_results,
                    loop_limit_reached,
                    pending_tool_calls,
                    stopped,
                });
            }
        }
        Err("no agent response".to_string())
    })
}

/// Send a stop signal to the gateway for the specified session.
/// The agent turn will pause after the current iteration completes.
/// This is idempotent — stopping an idle session is a no-op.
pub(crate) fn send_stop(profile_override: Option<&str>, session_id: &str) -> Result<bool, String> {
    let (config, paths) = lib::config::load_config(profile_override).map_err(|e| e.to_string())?;
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

        let connect_params = build_connect_params(&paths, token.as_deref(), &nonce)?;

        let connect_req = serde_json::json!({
            "type": "req",
            "id": "1",
            "method": "connect",
            "params": connect_params
        });
        ws.send(Message::Text(connect_req.to_string()))
            .await
            .map_err(|e| e.to_string())?;

        // Wait for hello-ok
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
                    if err == "invalid device token" {
                        let _ = std::fs::remove_file(paths.device_token_path());
                    }
                    return Err(err.to_string());
                }
                if let Some(auth) = res.get("payload").and_then(|p| p.get("auth")) {
                    if let Some(dt) = auth.get("deviceToken").and_then(|v| v.as_str()) {
                        let _ = lib::device::save_device_token_to(&paths.device_token_path(), dt);
                    }
                }
                break;
            }
        }

        let stop_req = serde_json::json!({
            "type": "req",
            "id": "2",
            "method": "stop",
            "params": { "sessionId": session_id }
        });
        ws.send(Message::Text(stop_req.to_string()))
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
                        .unwrap_or("stop failed");
                    return Err(err.to_string());
                }
                let stopped = res
                    .get("payload")
                    .and_then(|p| p.get("stopped"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                return Ok(stopped);
            }
        }
        Err("no stop response".to_string())
    })
}
