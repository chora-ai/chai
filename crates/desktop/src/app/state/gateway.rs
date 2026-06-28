use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use super::super::{
    AgentReply, AgentSkillsRuntime, ChaiApp, GatewayStatusDetails, StatusViewMode, PROBE_INTERVAL_FRAMES,
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
                    self.invalidate_agent_detail_cache();
                }
                self.probe_receiver = None;
            }
        }
        self.frames_since_probe = self.frames_since_probe.saturating_add(1);
        if self.probe_receiver.is_none() && self.frames_since_probe >= PROBE_INTERVAL_FRAMES {
            self.frames_since_probe = 0;
            let (tx, rx) = mpsc::channel();
            let profile_override = self.cached_profile_override.clone();
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
        let active_orch_id = self.active_orchestrator_id.as_deref();
        let provider = self
            .current_provider
            .as_deref()
            .or_else(|| details.default_provider_for(active_orch_id))
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
            .or_else(|| details.default_model_for(active_orch_id))
            .or(self.default_model.as_deref());
        let in_list = effective
            .map(|m| models.iter().any(|x| x == m))
            .unwrap_or(false);
        if !in_list {
            self.current_model = details
                .default_model_for(active_orch_id)
                .map(String::from)
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
                let prev_status = self.gateway_status.take();
                self.gateway_status = result.ok();
                self.reconcile_dashboard_agent_selection();
                self.reconcile_model_with_status();

                // Only invalidate the agent detail cache when something that
                // affects agent detail data actually changed. Unconditional
                // invalidation on every status poll caused the Agent and Tools
                // screens to flicker "Loading agent detail..." every ~2 seconds.
                let should_invalidate = match (&prev_status, &self.gateway_status) {
                    (Some(prev), Some(new)) => {
                        // Agent roster changed (ids added or removed).
                        let prev_keys: std::collections::HashSet<_> =
                            prev.agent_skills.keys().collect();
                        let new_keys: std::collections::HashSet<_> =
                            new.agent_skills.keys().collect();
                        if prev_keys != new_keys {
                            true
                        } else if prev.skills_lock_generation != new.skills_lock_generation {
                            // Skill lock generation changed (packages re-resolved).
                            true
                        } else {
                            // Any cached agent's context mode changed.
                            self.agent_detail_cache.keys().any(|id| {
                                prev.agent_skills
                                    .get(id)
                                    .and_then(|r| r.context_mode.as_deref())
                                    != new
                                        .agent_skills
                                        .get(id)
                                        .and_then(|r| r.context_mode.as_deref())
                            })
                        }
                    }
                    (None, Some(_)) => true, // First status after gateway starts.
                    _ => false,              // Gateway went down — already handled by probe.
                };

                if should_invalidate {
                    self.invalidate_agent_detail_cache();
                }
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
            let profile_override = self.cached_profile_override.clone();
            let needs_raw_json = self.status_view_mode == StatusViewMode::RawJson;
            std::thread::spawn(move || {
                let result = fetch_gateway_status(profile_override.as_deref(), needs_raw_json);
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
            let profile_override = self.cached_profile_override.clone();
            let after_seq = self.gateway_logs_cursor;
            std::thread::spawn(move || {
                let result = fetch_gateway_logs(profile_override.as_deref(), after_seq);
                let _ = tx.send(result);
            });
            self.gateway_logs_receiver = Some(rx);
        }
    }

    /// Poll for in-flight `agentDetail` fetch result and optionally start a new fetch.
    /// Called each frame. Fetches agent detail on-demand when the Agent or Tools
    /// screen is active and the selected agent's detail is not yet cached (or the
    /// user switched to a different agent).
    pub(crate) fn poll_agent_detail(&mut self) {
        // Check for in-flight result.
        if let Some((ref in_flight_id, ref rx)) = self.agent_detail_receiver {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(detail) => {
                        self.agent_detail_cache.insert(detail.agent_id.clone(), detail);
                        self.agent_detail_fetch_error = None;
                    }
                    Err(e) => {
                        if !self.agent_detail_cache.contains_key(in_flight_id) {
                            // Only store the error when there is no cached data
                            // to fall back on for this agent.
                            self.agent_detail_fetch_error = Some((in_flight_id.clone(), e));
                        }
                    }
                }
                self.agent_detail_receiver = None;
            }
        }

        // Don't fetch if gateway isn't running or a fetch is already in flight.
        if !self.gateway_responds || self.agent_detail_receiver.is_some() {
            return;
        }

        // Determine the selected agent id from the dashboard picker.
        let selected_id = self.dashboard_agent_id.clone().or_else(|| {
            self.gateway_status.as_ref().and_then(|gs| gs.orchestrator_id().map(String::from))
        });

        let Some(ref target_id) = selected_id else {
            return;
        };

        // Only fetch if this agent isn't cached yet.
        if self.agent_detail_cache.contains_key(target_id) {
            return;
        }

        self.agent_detail_requested_id = Some(target_id.clone());
        let (tx, rx) = mpsc::channel();
        let profile_override = self.cached_profile_override.clone();
        let agent_id = target_id.clone();
        let agent_id_clone = agent_id.clone();
        std::thread::spawn(move || {
            let result = fetch_agent_detail(profile_override.as_deref(), &agent_id_clone);
            let _ = tx.send(result);
        });
        self.agent_detail_receiver = Some((agent_id, rx));
    }

    /// Invalidate the agent detail cache. Called when a status refresh detects a
    /// material change (agent roster, skill lock generation, or context mode), or
    /// when the gateway stops.
    pub(crate) fn invalidate_agent_detail_cache(&mut self) {
        self.agent_detail_cache.clear();
        self.agent_detail_fetch_error = None;
        self.agent_detail_receiver = None;
        self.agent_detail_requested_id = None;
    }
}

/// Parse the `providers` block from gateway status into per-provider info.
/// The gateway now sends each provider as `{ "endpointType": "...", "modelDiscovery": "...", "models": [string, ...] }`
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
        let model_discovery = val.get("modelDiscovery")
            .and_then(|v| v.as_str())
            .unwrap_or("auto")
            .to_string();
        // Models are flat strings (not {"name": "..."} objects).
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
            model_discovery,
            models,
        });
    }
    map
}

/// Fetch gateway status via WebSocket (connect + status). Runs in a thread; use blocking.
pub(crate) fn fetch_gateway_status(profile_override: Option<&str>, needs_raw_json: bool) -> Result<GatewayStatusDetails, String> {
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
        ws.send(Message::Text(connect_req.to_string().into()))
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
        ws.send(Message::Text(status_req.to_string().into()))
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
                details.status_response_json = if needs_raw_json { serde_json::to_string_pretty(&res).ok() } else { None };
                let payload = res.get("payload").ok_or("missing payload")?;
                let gateway = payload.get("gateway");
                let agents_pl = payload.get("agents");
                let providers_pl = payload.get("providers");
                details.channels_block = payload.get("channels").cloned();
                if let Some(sp) = payload.get("skills") {
                    details.skills_packages_discovered =
                        sp.get("packagesDiscovered").and_then(|v| v.as_u64());
                    details.skills_lock_mode = sp
                        .get("lockMode")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    details.skills_lock_generation =
                        sp.get("lockGeneration").and_then(|v| v.as_u64());
                    details.skills_locked_count =
                        sp.get("lockedSkills").and_then(|v| v.as_u64());
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
                details.sandbox_mode = payload
                    .get("sandbox")
                    .and_then(|s| s.get("mode"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("strict")
                    .to_string();
                details.sandbox_roots = payload
                    .get("sandbox")
                    .and_then(|s| s.get("roots"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if let Some(entries) = agents_pl.and_then(|a| a.as_array())
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

                        // Parse per-agent skill runtime data (lightweight fields only;
                        // systemContext, tools, skillsContext are fetched on-demand via agentDetail).
                        let mut agent_rt = AgentSkillsRuntime::default();
                        agent_rt.enabled_skills = entry
                            .get("enabledSkills")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        agent_rt.context_mode = entry
                            .get("contextMode")
                            .and_then(|v| v.as_str())
                            .map(String::from);
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
                                let orch_enabled_providers = entry
                                    .get("enabledProviders")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| {
                                                v.as_str().map(|s| s.trim().to_string())
                                            })
                                            .filter(|s| !s.is_empty())
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                let orch_enabled_skills = details
                                    .agent_skills
                                    .get(&id)
                                    .map(|rt| rt.enabled_skills.clone())
                                    .unwrap_or_default();
                                let orch_enabled_workers = entry
                                    .get("enabledWorkers")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    });
                                let orch_context_mode = details
                                    .agent_context_modes
                                    .get(&id)
                                    .cloned();
                                let orch_max_tool_loops = entry
                                    .get("maxToolLoopsPerTurn")
                                    .and_then(|v| v.as_u64())
                                    .map(|n| n as u32);
                                let orch_max_del_per_turn = entry
                                    .get("maxDelegationsPerTurn")
                                    .and_then(|v| v.as_u64())
                                    .map(|n| n as usize);
                                let orch_max_del_per_session = entry
                                    .get("maxDelegationsPerSession")
                                    .and_then(|v| v.as_u64())
                                    .map(|n| n as usize);
                                let orch_max_del_per_worker = entry
                                    .get("maxDelegationsPerWorker")
                                    .and_then(|v| v.as_object())
                                    .map(|obj| {
                                        obj.iter()
                                            .filter_map(|(k, v)| {
                                                v.as_u64().map(|n| (k.clone(), n as usize))
                                            })
                                            .collect()
                                    });
                                details.orchestrators.push(crate::app::types::StatusOrchestratorRow {
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
                                    enabled_providers: orch_enabled_providers,
                                    enabled_skills: orch_enabled_skills,
                                    enabled_workers: orch_enabled_workers,
                                    context_mode: orch_context_mode,
                                    max_tool_loops_per_turn: orch_max_tool_loops,
                                    max_delegations_per_turn: orch_max_del_per_turn,
                                    max_delegations_per_session: orch_max_del_per_session,
                                    max_delegations_per_worker: orch_max_del_per_worker,
                                });
                            }
                            "worker" => {
                                let w_skills = entry
                                    .get("enabledSkills")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                let w_ctx_mode = entry
                                    .get("contextMode")
                                    .and_then(|v| v.as_str())
                                    .map(String::from);
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
                                    enabled_skills: w_skills,
                                    context_mode: w_ctx_mode,
                                });
                            }
                            _ => {}
                        }
                    }
                }
                details.provider_info = parse_providers_block(providers_pl);
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
        ws.send(Message::Text(connect_req.to_string().into()))
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
        ws.send(Message::Text(logs_req.to_string().into()))
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

/// Fetch per-agent detail via the `agentDetail` WebSocket method.
/// Returns the heavy fields (systemContext, tools, skillsContext) for a single agent,
/// fetched on-demand when the Agent or Tools screen is active.
pub(crate) fn fetch_agent_detail(
    profile_override: Option<&str>,
    agent_id: &str,
) -> Result<crate::app::types::AgentDetail, String> {
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
        ws.send(Message::Text(connect_req.to_string().into()))
            .await
            .map_err(|e| e.to_string())?;

        // Wait for connect response.
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

        // Send agentDetail request.
        let detail_req = serde_json::json!({
            "type": "req",
            "id": "2",
            "method": "agentDetail",
            "params": { "agentId": agent_id }
        });
        ws.send(Message::Text(detail_req.to_string().into()))
            .await
            .map_err(|e| e.to_string())?;

        // Read agentDetail response.
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
                        .unwrap_or("agentDetail failed");
                    return Err(err.to_string());
                }
                let payload = res.get("payload").ok_or("missing payload")?;
                let mut detail = crate::app::types::AgentDetail::default();
                detail.agent_id = agent_id.to_string();
                detail.role = payload
                    .get("role")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                detail.system_context = payload
                    .get("systemContext")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                detail.tools = payload
                    .get("tools")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from);
                detail.skills_context = payload
                    .get("skillsContext")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default();
                return Ok(detail);
            }
        }
        Err("no agentDetail response".to_string())
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
    orchestrator_id: Option<String>,
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
        ws.send(Message::Text(connect_req.to_string().into()))
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
        if let Some(id) = &orchestrator_id {
            agent_params["orchestratorId"] = serde_json::Value::String(id.clone());
        }
        let agent_req = serde_json::json!({
            "type": "req",
            "id": "2",
            "method": "agent",
            "params": agent_params
        });
        ws.send(Message::Text(agent_req.to_string().into()))
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
        ws.send(Message::Text(connect_req.to_string().into()))
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
        ws.send(Message::Text(stop_req.to_string().into()))
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

/// Fetch the session list from the gateway via `sessions.list` WebSocket method.
/// Returns summary metadata for all sessions (id, timestamps, message count, channel binding).
/// When `orchestrator_id` is `Some`, scopes the request to that orchestrator's session store.
pub(crate) fn fetch_sessions_list(profile_override: Option<&str>, orchestrator_id: Option<&str>) -> Result<Vec<crate::app::SessionSummary>, String> {
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
        let first = ws.next().await.ok_or("no first frame")?.map_err(|e| e.to_string())?;
        let Message::Text(challenge_text) = first else {
            return Err("expected text challenge frame".to_string());
        };
        let challenge: serde_json::Value = serde_json::from_str(&challenge_text).map_err(|e| e.to_string())?;
        let nonce = challenge
            .get("payload").and_then(|p| p.get("nonce").and_then(|n| n.as_str()))
            .ok_or("expected connect.challenge event with nonce")?
            .to_string();
        let connect_params = build_connect_params(&paths, token.as_deref(), &nonce)?;
        let connect_req = serde_json::json!({ "type": "req", "id": "1", "method": "connect", "params": connect_params });
        ws.send(Message::Text(connect_req.to_string().into())).await.map_err(|e| e.to_string())?;
        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") { continue; }
            if res.get("id").and_then(|v| v.as_str()) == Some("1") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res.get("error").and_then(|v| v.as_str()).unwrap_or("connect failed");
                    if err == "invalid device token" { let _ = std::fs::remove_file(paths.device_token_path()); }
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
        let mut params = serde_json::json!({});
        if let Some(id) = orchestrator_id {
            params["orchestratorId"] = serde_json::Value::String(id.to_string());
        }
        let req = serde_json::json!({ "type": "req", "id": "2", "method": "sessions.list", "params": params });
        ws.send(Message::Text(req.to_string().into())).await.map_err(|e| e.to_string())?;
        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") { continue; }
            if res.get("id").and_then(|v| v.as_str()) == Some("2") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    return Err(res.get("error").and_then(|v| v.as_str()).unwrap_or("sessions.list failed").to_string());
                }
                let payload = res.get("payload").ok_or("missing payload")?;
                let sessions_arr = payload.get("sessions").and_then(|v| v.as_array()).ok_or("missing sessions array")?;
                let mut summaries = Vec::new();
                for entry in sessions_arr {
                    let id = entry.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let created_at = entry.get("createdAt").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let updated_at = entry.get("updatedAt").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let message_count = entry.get("messageCount").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let channel_binding = entry.get("channelBinding").and_then(|b| {
                        let cid = b.get("channelId").and_then(|v| v.as_str()).unwrap_or("");
                        let conv = b.get("conversationId").and_then(|v| v.as_str()).unwrap_or("");
                        if cid.is_empty() && conv.is_empty() { None } else {
                            Some(crate::app::ChannelBinding { channel_id: cid.to_string(), conversation_id: conv.to_string() })
                        }
                    });
                    summaries.push(crate::app::SessionSummary { id, created_at, updated_at, message_count, channel_binding });
                }
                return Ok(summaries);
            }
        }
        Err("no sessions.list response".to_string())
    })
}

/// Fetch session history from the gateway via `sessions.history` WebSocket method.
pub(crate) fn fetch_sessions_history(profile_override: Option<&str>, session_id: &str) -> Result<crate::app::SessionHistory, String> {
    let (config, paths) = lib::config::load_config(profile_override).map_err(|e| e.to_string())?;
    let bind = config.gateway.bind.trim();
    let port = config.gateway.port;
    let token = lib::config::resolve_gateway_token(&config);
    let ws_url = format!("ws://{}:{}/ws", bind, port);

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.map_err(|e| e.to_string())?;
        let first = ws.next().await.ok_or("no first frame")?.map_err(|e| e.to_string())?;
        let Message::Text(challenge_text) = first else { return Err("expected text challenge frame".to_string()); };
        let challenge: serde_json::Value = serde_json::from_str(&challenge_text).map_err(|e| e.to_string())?;
        let nonce = challenge.get("payload").and_then(|p| p.get("nonce").and_then(|n| n.as_str())).ok_or("expected connect.challenge event with nonce")?.to_string();
        let connect_params = build_connect_params(&paths, token.as_deref(), &nonce)?;
        let connect_req = serde_json::json!({ "type": "req", "id": "1", "method": "connect", "params": connect_params });
        ws.send(Message::Text(connect_req.to_string().into())).await.map_err(|e| e.to_string())?;
        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") { continue; }
            if res.get("id").and_then(|v| v.as_str()) == Some("1") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res.get("error").and_then(|v| v.as_str()).unwrap_or("connect failed");
                    if err == "invalid device token" { let _ = std::fs::remove_file(paths.device_token_path()); }
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
        let req = serde_json::json!({ "type": "req", "id": "2", "method": "sessions.history", "params": { "sessionId": session_id } });
        ws.send(Message::Text(req.to_string().into())).await.map_err(|e| e.to_string())?;
        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") { continue; }
            if res.get("id").and_then(|v| v.as_str()) == Some("2") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    return Err(res.get("error").and_then(|v| v.as_str()).unwrap_or("sessions.history failed").to_string());
                }
                let payload = res.get("payload").ok_or("missing payload")?;
                let id = payload.get("id").and_then(|v| v.as_str()).unwrap_or(session_id).to_string();
                let created_at = payload.get("createdAt").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let updated_at = payload.get("updatedAt").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let messages_arr = payload.get("messages").and_then(|v| v.as_array()).ok_or("missing messages array")?;
                let mut messages = Vec::new();
                // The gateway stores tool calls and results differently from the live
                // event stream. In the session file:
                //   - assistant messages carry toolCalls[{function:{name, arguments, index}}]
                //   - tool results are separate "tool" messages with toolName and content
                // We need to reconstruct the desktop timeline: individual tool_call
                // entries with matched tool_result fields, followed by the assistant text.
                //
                // First pass: collect "tool" messages in order so we can match them
                // sequentially to their corresponding tool calls.
                let mut tool_result_entries: Vec<(String, String)> = Vec::new();
                for m in messages_arr {
                    let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("");
                    if role == "tool" {
                        let tool_name = m.get("toolName").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let content = m.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        tool_result_entries.push((tool_name, content));
                    }
                }
                // Second pass: build the chat timeline.
                // Use a cursor into tool_result_entries to match results to tool calls
                // in sequential order (the gateway stores them in the same order as the
                // tool calls that triggered them).
                let mut tool_result_cursor: usize = 0;
                for m in messages_arr {
                    let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let content = m.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    match role.as_str() {
                        "user" => { messages.push(crate::app::ChatMessage::user(content)); }
                        "system" => { messages.push(crate::app::ChatMessage::system(content)); }
	                        "assistant" => {
	                            let tool_calls = m.get("toolCalls").and_then(|v| v.as_array());

	                            if let Some(calls) = tool_calls {
	                                if !calls.is_empty() {
	                                    // Emit the assistant progress text before tool_call
	                                    // entries, matching the live event stream order where
	                                    // session.assistant_progress arrives before
	                                    // session.tool_call. Skip empty assistant messages —
	                                    // the live event stream drops session.message events
	                                    // with empty content, so including them in history
	                                    // would cause extra spacing (the render function skips
	                                    // them but the layout still adds inter-message spacing).
	                                    if !content.trim().is_empty() {
	                                        messages.push(crate::app::ChatMessage {
	                                            role: "assistant_progress".to_string(),
	                                            content: content.clone(),
	                                            tool_calls: None,
	                                            tool_results: None,
	                                            delegation_event: None,
	                                            tool_name: None,
	                                            tool_args: None,
	                                            tool_result: None,
	                                            tool_index: None,
	                                            source: None,
	                                            pending_tool_calls: None,
	                                        });
	                                    }
	                                    // Emit individual tool_call entries for each tool call,
	                                    // matching the live event stream structure. Try to match
	                                    // stored "tool" results to these tool_call entries by
	                                    // advancing the cursor sequentially.
	                                    for call in calls.iter() {
	                                        let tool_name = call.get("function")
	                                            .and_then(|f| f.get("name"))
	                                            .and_then(|n| n.as_str())
	                                            .unwrap_or("unknown")
	                                            .to_string();
	                                        let tool_args = call.get("function")
	                                            .and_then(|f| f.get("arguments"))
	                                            .cloned();
	                                        let tool_index = call.get("function")
	                                            .and_then(|f| f.get("index"))
	                                            .and_then(|v| v.as_u64())
	                                            .map(|v| v as usize);

	                                        // Match the tool result by advancing the cursor.
	                                        // The gateway stores tool results in the same order
	                                        // as the tool calls that triggered them.
	                                        let tool_result = if tool_result_cursor < tool_result_entries.len() {
	                                            let (ref name, ref result) = tool_result_entries[tool_result_cursor];
	                                            if name == &tool_name {
	                                                tool_result_cursor += 1;
	                                                Some(result.clone())
	                                            } else {
	                                                // Name mismatch — still try advancing in case
	                                                // there are unmatched entries.
	                                                None
	                                            }
	                                        } else {
	                                            None
	                                        };

	                                        messages.push(crate::app::ChatMessage {
	                                            role: "tool_call".to_string(),
	                                            content: String::new(),
	                                            tool_calls: None,
	                                            tool_results: None,
	                                            delegation_event: None,
	                                            tool_name: Some(tool_name),
	                                            tool_args,
	                                            tool_result,
	                                            tool_index,
	                                            source: None,
	                                            pending_tool_calls: None,
	                                        });
	                                    }
	                                    continue;
	                                }
                            }
                            // No tool calls — emit the assistant message as-is.
                            let tool_calls_val = tool_calls.map(|arr| arr.clone());
                            let tool_results = m.get("toolResults").and_then(|v| v.as_array()).map(|arr| {
                                arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect::<Vec<_>>()
                            });
                            let tool_results = tool_results.filter(|v| !v.is_empty());
                            messages.push(crate::app::ChatMessage::assistant(content, tool_calls_val, tool_results));
                        }
                        // "tool" messages are handled above by matching them to their
                        // tool_call entries. Skip them here to avoid duplicate entries.
                        "tool" => {}
                        _ => {}
                    }
                }
                return Ok(crate::app::SessionHistory { id, messages, created_at, updated_at });
            }
        }
        Err("no sessions.history response".to_string())
    })
}

/// Delete a session via the `sessions.delete` WebSocket method.
pub(crate) fn fetch_sessions_delete(profile_override: Option<&str>, session_id: &str) -> Result<bool, String> {
    let (config, paths) = lib::config::load_config(profile_override).map_err(|e| e.to_string())?;
    let bind = config.gateway.bind.trim();
    let port = config.gateway.port;
    let token = lib::config::resolve_gateway_token(&config);
    let ws_url = format!("ws://{}:{}/ws", bind, port);

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.map_err(|e| e.to_string())?;
        let first = ws.next().await.ok_or("no first frame")?.map_err(|e| e.to_string())?;
        let Message::Text(challenge_text) = first else { return Err("expected text challenge frame".to_string()); };
        let challenge: serde_json::Value = serde_json::from_str(&challenge_text).map_err(|e| e.to_string())?;
        let nonce = challenge.get("payload").and_then(|p| p.get("nonce").and_then(|n| n.as_str())).ok_or("expected connect.challenge event with nonce")?.to_string();
        let connect_params = build_connect_params(&paths, token.as_deref(), &nonce)?;
        let connect_req = serde_json::json!({ "type": "req", "id": "1", "method": "connect", "params": connect_params });
        ws.send(Message::Text(connect_req.to_string().into())).await.map_err(|e| e.to_string())?;
        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") { continue; }
            if res.get("id").and_then(|v| v.as_str()) == Some("1") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res.get("error").and_then(|v| v.as_str()).unwrap_or("connect failed");
                    if err == "invalid device token" { let _ = std::fs::remove_file(paths.device_token_path()); }
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
        let req = serde_json::json!({ "type": "req", "id": "2", "method": "sessions.delete", "params": { "sessionId": session_id } });
        ws.send(Message::Text(req.to_string().into())).await.map_err(|e| e.to_string())?;
        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") { continue; }
            if res.get("id").and_then(|v| v.as_str()) == Some("2") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    return Err(res.get("error").and_then(|v| v.as_str()).unwrap_or("sessions.delete failed").to_string());
                }
                return Ok(true);
            }
        }
        Err("no sessions.delete response".to_string())
    })
}

/// Delete all sessions via the `sessions.delete_all` WebSocket method.
/// When `orchestrator_id` is Some, only deletes sessions for that orchestrator.
/// When None, deletes sessions for all orchestrators.
pub(crate) fn fetch_sessions_delete_all(profile_override: Option<&str>, orchestrator_id: Option<&str>) -> Result<usize, String> {
    let (config, paths) = lib::config::load_config(profile_override).map_err(|e| e.to_string())?;
    let bind = config.gateway.bind.trim();
    let port = config.gateway.port;
    let token = lib::config::resolve_gateway_token(&config);
    let ws_url = format!("ws://{}:{}/ws", bind, port);

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.map_err(|e| e.to_string())?;
        let first = ws.next().await.ok_or("no first frame")?.map_err(|e| e.to_string())?;
        let Message::Text(challenge_text) = first else { return Err("expected text challenge frame".to_string()); };
        let challenge: serde_json::Value = serde_json::from_str(&challenge_text).map_err(|e| e.to_string())?;
        let nonce = challenge.get("payload").and_then(|p| p.get("nonce").and_then(|n| n.as_str())).ok_or("expected connect.challenge event with nonce")?.to_string();
        let connect_params = build_connect_params(&paths, token.as_deref(), &nonce)?;
        let connect_req = serde_json::json!({ "type": "req", "id": "1", "method": "connect", "params": connect_params });
        ws.send(Message::Text(connect_req.to_string().into())).await.map_err(|e| e.to_string())?;
        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") { continue; }
            if res.get("id").and_then(|v| v.as_str()) == Some("1") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res.get("error").and_then(|v| v.as_str()).unwrap_or("connect failed");
                    if err == "invalid device token" { let _ = std::fs::remove_file(paths.device_token_path()); }
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
        let mut params = serde_json::json!({});
        if let Some(id) = orchestrator_id {
            params["orchestratorId"] = serde_json::Value::String(id.to_string());
        }
        let req = serde_json::json!({ "type": "req", "id": "2", "method": "sessions.delete_all", "params": params });
        ws.send(Message::Text(req.to_string().into())).await.map_err(|e| e.to_string())?;
        while let Some(msg) = ws.next().await {
            let msg = msg.map_err(|e| e.to_string())?;
            let Message::Text(text) = msg else { continue };
            let res: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") { continue; }
            if res.get("id").and_then(|v| v.as_str()) == Some("2") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    return Err(res.get("error").and_then(|v| v.as_str()).unwrap_or("sessions.delete_all failed").to_string());
                }
                let count = res.get("payload").and_then(|p| p.get("deletedCount")).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                return Ok(count);
            }
        }
        Err("no sessions.delete_all response".to_string())
    })
}
