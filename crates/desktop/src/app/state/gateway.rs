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
    /// Poll for probe result and optionally start a new probe for the active profile's gateway.
    /// Call each frame.
    pub(crate) fn poll_gateway_probe(&mut self) {
        // Check for in-flight probe result.
        let probe_rx = self.gw_ref()
            .and_then(|gw| gw.probe_receiver.as_ref())
            .and_then(|rx| rx.try_recv().ok());
        if let Some(ok) = probe_rx {
            let need_invalidate;
            {
                let gw = self.gw();
                gw.probe_completed = true;
                gw.responds = ok;
                if !ok {
                    gw.status = None;
                    need_invalidate = true;
                } else {
                    need_invalidate = false;
                }
                gw.probe_receiver = None;
            }
            if need_invalidate {
                self.invalidate_agent_detail_cache();
            }
        }
        // Increment frame counter.
        {
            let next = self.gw_ref().map_or(0, |gw| gw.frames_since_probe).saturating_add(1);
            self.gw().frames_since_probe = next;
        }
        let (probe_rx_none, frames_since_probe) = match self.gw_ref() {
            Some(gw) => (gw.probe_receiver.is_none(), gw.frames_since_probe),
            None => (true, 0),
        };
        if probe_rx_none && frames_since_probe >= PROBE_INTERVAL_FRAMES {
            self.gw().frames_since_probe = 0;
            let (tx, rx) = mpsc::channel();
            let profile_override = Some(self.profile_active.clone());
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
            self.gw().probe_receiver = Some(rx);
        }
    }

    /// When gateway status is received, ensure current model is in the available list for the effective provider; if not, switch to gateway default or first available.
    pub(crate) fn reconcile_model_with_status(&mut self) {
        // Clone the needed data to avoid holding a borrow while calling gw().
        let (status_data, active_orch_id, current_provider, current_model, default_model) = match self.gw_ref() {
            Some(gw) => (
                gw.status.clone(),
                gw.active_orchestrator_id.clone(),
                gw.current_provider.clone(),
                gw.current_model.clone(),
                gw.default_model.clone(),
            ),
            None => return,
        };
        let Some(ref details) = status_data else {
            return;
        };
        let enabled = self.enabled_providers();
        let provider = current_provider
            .as_deref()
            .or_else(|| details.default_provider_for(active_orch_id.as_deref()))
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
        let effective = current_model
            .as_deref()
            .or_else(|| details.default_model_for(active_orch_id.as_deref()))
            .or(default_model.as_deref());
        let in_list = effective
            .map(|m| models.iter().any(|x| x == m))
            .unwrap_or(false);
        if !in_list {
            self.gw().current_model = details
                .default_model_for(active_orch_id.as_deref())
                .map(String::from)
                .filter(|m| models.contains(m))
                .or_else(|| models.first().cloned());
        }
    }

    /// Request that the next status poll performs an immediate fetch (e.g. after switching provider so the model list is up to date).
    pub(crate) fn request_status_refetch(&mut self) {
        self.gw().frames_since_status = STATUS_INTERVAL_FRAMES;
    }

    /// Poll for status fetch result and optionally start a new fetch when gateway is running.
    pub(crate) fn poll_status_fetch(&mut self) {
        // Check for in-flight result.
        let status_rx = self.gw_ref()
            .and_then(|gw| gw.status_receiver.as_ref())
            .and_then(|rx| rx.try_recv().ok());
        if let Some(result) = status_rx {
            let prev_status = self.gw().status.take();
            self.gw().status = result.ok();
            self.reconcile_dashboard_agent_selection();
            self.reconcile_model_with_status();

            // Clone status so we don't hold an immutable borrow while calling gw().
            let curr_status = self.gw_ref().and_then(|gw| gw.status.clone());
            let cached_agent_ids: Vec<String> = self.gw_ref()
                .map(|gw| gw.agent_detail_cache.keys().cloned().collect())
                .unwrap_or_default();

            // Only invalidate the agent detail cache when something that
            // affects agent detail data actually changed.
            let should_invalidate = match (&prev_status, &curr_status) {
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
                        cached_agent_ids.iter().any(|id| {
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
            self.gw().status_receiver = None;
            let status_is_none = self.gw_ref().map_or(true, |gw| gw.status.is_none());
            if status_is_none {
                self.gw().frames_since_status = 0;
                self.gw().status_fetch_ever_failed = true;
            } else {
                self.gw().status_fetch_ever_failed = false;
            }
        }
        let (responds, status_rx_is_some) = match self.gw_ref() {
            Some(gw) => (gw.responds, gw.status_receiver.is_some()),
            None => (false, false),
        };
        if !responds || status_rx_is_some {
            return;
        }
        // Only fetch immediately on the very first detection (gateway_status has never
        // been set AND no previous fetch has failed). Once a fetch has failed, let the
        // normal interval cadence apply to avoid a tight retry loop of WebSocket connects.
        let (need_immediate, frames_since_status) = match self.gw_ref() {
            Some(gw) => (gw.status.is_none() && !gw.status_fetch_ever_failed, gw.frames_since_status),
            None => (false, 0),
        };
        {
            let next = frames_since_status.saturating_add(1);
            self.gw().frames_since_status = next;
        }
        if need_immediate || self.gw_ref().map_or(0, |gw| gw.frames_since_status) >= STATUS_INTERVAL_FRAMES {
            self.gw().frames_since_status = 0;
            let (tx, rx) = mpsc::channel();
            let profile_override = Some(self.profile_active.clone());
            let needs_raw_json = self.status_view_mode == StatusViewMode::RawJson;
            std::thread::spawn(move || {
                let result = fetch_gateway_status(profile_override.as_deref(), needs_raw_json);
                let _ = tx.send(result);
            });
            self.gw().status_receiver = Some(rx);
        }
    }

    /// Poll for gateway log fetch result and optionally start a new fetch. Call each frame.
    pub(crate) fn poll_gateway_logs_fetch(&mut self, owned: bool) {
        // Check for in-flight result.
        let logs_rx = self.gw_ref()
            .and_then(|gw| gw.logs_receiver.as_ref())
            .and_then(|rx| rx.try_recv().ok());
        if let Some(result) = logs_rx {
            if let Ok((lines, max_seq)) = result {
                for line in lines {
                    crate::app::state::logs::push_gateway_log_line(line);
                }
                self.gw().logs_cursor = max_seq;
            }
            self.gw().logs_receiver = None;
        }
        // Only fetch logs from external gateways.
        let (responds, logs_rx_is_some) = match self.gw_ref() {
            Some(gw) => (gw.responds, gw.logs_receiver.is_some()),
            None => (false, false),
        };
        if owned || !responds || logs_rx_is_some {
            // When the gateway is owned or not responding, reset the cursor,
            // frame counter, and any in-flight receiver so the next external
            // gateway starts fresh.
            if owned || !responds {
                self.gw().logs_cursor = 0;
                self.gw().frames_since_logs = 0;
                self.gw().logs_receiver = None;
            }
            return;
        }
        {
            let next = self.gw_ref().map_or(0, |gw| gw.frames_since_logs).saturating_add(1);
            self.gw().frames_since_logs = next;
        }
        if self.gw_ref().map_or(0, |gw| gw.frames_since_logs) >= STATUS_INTERVAL_FRAMES {
            self.gw().frames_since_logs = 0;
            let (tx, rx) = mpsc::channel();
            let profile_override = Some(self.profile_active.clone());
            let after_seq = self.gw_ref().map_or(0, |gw| gw.logs_cursor);
            std::thread::spawn(move || {
                let result = fetch_gateway_logs(profile_override.as_deref(), after_seq);
                let _ = tx.send(result);
            });
            self.gw().logs_receiver = Some(rx);
        }
    }

    /// Poll for in-flight `agentDetail` fetch result and optionally start a new fetch.
    pub(crate) fn poll_agent_detail(&mut self) {
        // Check for in-flight result.
        let (detail_rx, in_flight_id) = match self.gw_ref() {
            Some(gw) => {
                let rx = gw.agent_detail_receiver.as_ref()
                    .and_then(|(_, rx)| rx.try_recv().ok());
                let id = gw.agent_detail_receiver.as_ref()
                    .map(|(id, _)| id.clone());
                (rx, id)
            }
            None => (None, None),
        };
        if let Some(result) = detail_rx {
            let gw = self.gw();
            match result {
                Ok(detail) => {
                    gw.agent_detail_cache.insert(detail.agent_id.clone(), detail);
                    gw.agent_detail_fetch_error = None;
                }
                Err(e) => {
                    if let Some(ref in_flight_id) = in_flight_id {
                        if !gw.agent_detail_cache.contains_key(in_flight_id) {
                            gw.agent_detail_fetch_error = Some((in_flight_id.clone(), e));
                        }
                    }
                }
            }
            gw.agent_detail_receiver = None;
        }

        // Don't fetch if gateway isn't running or a fetch is already in flight.
        let (responds, agent_detail_rx_is_some) = match self.gw_ref() {
            Some(gw) => (gw.responds, gw.agent_detail_receiver.is_some()),
            None => (false, false),
        };
        if !responds || agent_detail_rx_is_some {
            return;
        }

        // Determine the selected agent id from the dashboard picker.
        let selected_id = match self.gw_ref() {
            Some(gw) => gw.dashboard_agent_id.clone().or_else(|| {
                gw.status.as_ref().and_then(|gs| gs.orchestrator_id().map(String::from))
            }),
            None => None,
        };

        let Some(ref target_id) = selected_id else {
            return;
        };

        // Only fetch if this agent isn't cached yet.
        let is_cached = self.gw_ref()
            .map_or(false, |gw| gw.agent_detail_cache.contains_key(target_id));
        if is_cached {
            return;
        }

        self.gw().agent_detail_requested_id = Some(target_id.clone());
        let (tx, rx) = mpsc::channel();
        let profile_override = Some(self.profile_active.clone());
        let agent_id = target_id.clone();
        let agent_id_clone = agent_id.clone();
        std::thread::spawn(move || {
            let result = fetch_agent_detail(profile_override.as_deref(), &agent_id_clone);
            let _ = tx.send(result);
        });
        self.gw().agent_detail_receiver = Some((agent_id, rx));
    }

    /// Invalidate the agent detail cache for the active profile.
    pub(crate) fn invalidate_agent_detail_cache(&mut self) {
        let gw = self.gw();
        gw.agent_detail_cache.clear();
        gw.agent_detail_fetch_error = None;
        gw.agent_detail_receiver = None;
        gw.agent_detail_requested_id = None;
    }
}

/// Parse the `providers` block from gateway status into per-provider info.
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

                        let mut agent_rt = AgentSkillsRuntime::default();
                        agent_rt.enabled_skills = entry
                            .get("enabledSkills")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .filter(|s| !s.is_empty())
                                    .collect()
                            })
                            .unwrap_or_default();
                        agent_rt.context_mode = entry
                            .get("contextMode")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        details.agent_skills.insert(id.clone(), agent_rt);

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

        // Wait for connect response
        loop {
            let msg = ws
                .next()
                .await
                .ok_or("no connect response")?
                .map_err(|e| e.to_string())?;
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

        // Build agent turn request
        let mut agent_params = serde_json::json!({
            "message": message,
        });
        if let Some(sid) = &session_id {
            agent_params["sessionId"] = serde_json::Value::String(sid.clone());
        }
        if let Some(p) = provider {
            agent_params["provider"] = serde_json::Value::String(p);
        }
        if let Some(m) = model {
            agent_params["model"] = serde_json::Value::String(m);
        }
        if let Some(oid) = orchestrator_id {
            agent_params["orchestratorId"] = serde_json::Value::String(oid);
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

        // Read agent response
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
                        .unwrap_or("agent turn failed");
                    return Err(err.to_string());
                }
                let payload = res.get("payload").ok_or("missing payload")?;
                let reply = payload
                    .get("reply")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let session_id = payload
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let tool_calls: Vec<serde_json::Value> = payload
                    .get("toolCalls")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let tool_results: Vec<String> = payload
                    .get("toolResults")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let loop_limit_reached = payload
                    .get("loopLimitReached")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let pending_tool_calls: Vec<serde_json::Value> = payload
                    .get("pendingToolCalls")
                    .and_then(|v| v.as_array())
                    .cloned()
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

/// Send a `stop` request to the gateway for the given session.
pub(crate) fn send_stop(
    profile_override: Option<&str>,
    session_id: &str,
) -> Result<bool, String> {
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

        // Wait for connect response
        loop {
            let msg = ws
                .next()
                .await
                .ok_or("no connect response")?
                .map_err(|e| e.to_string())?;
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
                let ok = res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                return Ok(ok);
            }
        }
        Err("no stop response".to_string())
    })
}

/// Fetch the session list from the gateway via `sessions.list` WS method.
pub(crate) fn fetch_sessions_list(
    profile_override: Option<&str>,
    orchestrator_id: Option<&str>,
) -> Result<Vec<super::super::SessionSummary>, String> {
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

        // Wait for connect response.
        loop {
            let msg = ws
                .next()
                .await
                .ok_or("no connect response")?
                .map_err(|e| e.to_string())?;
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

        // Build sessions.list request.
        let mut list_params = serde_json::json!({});
        if let Some(oid) = orchestrator_id {
            list_params["orchestratorId"] = serde_json::Value::String(oid.to_string());
        }

        let list_req = serde_json::json!({
            "type": "req",
            "id": "2",
            "method": "sessions.list",
            "params": list_params
        });
        ws.send(Message::Text(list_req.to_string().into()))
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
                        .unwrap_or("sessions.list failed");
                    return Err(err.to_string());
                }
                let payload = res.get("payload").ok_or("missing payload")?;
                let sessions_arr = payload
                    .get("sessions")
                    .and_then(|v| v.as_array())
                    .ok_or("missing sessions array")?;
                let mut summaries = Vec::new();
                for entry in sessions_arr {
                    let id = entry
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let created_at = entry
                        .get("createdAt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let updated_at = entry
                        .get("updatedAt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let message_count = entry
                        .get("messageCount")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                    let channel_binding = entry.get("channelBinding").and_then(|cb| {
                        let channel_id = cb.get("channelId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let conversation_id = cb.get("conversationId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if channel_id.is_empty() && conversation_id.is_empty() {
                            None
                        } else {
                            Some(super::super::ChannelBinding { channel_id, conversation_id })
                        }
                    });
                    summaries.push(super::super::SessionSummary {
                        id,
                        created_at,
                        updated_at,
                        message_count,
                        channel_binding,
                    });
                }
                return Ok(summaries);
            }
        }
        Err("no sessions.list response".to_string())
    })
}

/// Fetch the session history from the gateway via `sessions.history` WS method.
pub(crate) fn fetch_sessions_history(
    profile_override: Option<&str>,
    session_id: &str,
) -> Result<super::super::SessionHistory, String> {
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

        // Wait for connect response.
        loop {
            let msg = ws
                .next()
                .await
                .ok_or("no connect response")?
                .map_err(|e| e.to_string())?;
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

        let hist_req = serde_json::json!({
            "type": "req",
            "id": "2",
            "method": "sessions.history",
            "params": { "sessionId": session_id }
        });
        ws.send(Message::Text(hist_req.to_string().into()))
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
                        .unwrap_or("sessions.history failed");
                    return Err(err.to_string());
                }
                let payload = res.get("payload").ok_or("missing payload")?;
                let id = payload
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .unwrap_or(session_id)
                    .to_string();
                let created_at = payload
                    .get("createdAt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let updated_at = payload
                    .get("updatedAt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let messages_arr = payload
                    .get("messages")
                    .and_then(|v| v.as_array())
                    .ok_or("missing messages array")?;
                let mut messages = Vec::new();
                for entry in messages_arr {
                    let role = entry
                        .get("role")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let content = entry
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tool_calls = entry
                        .get("toolCalls")
                        .and_then(|v| v.as_array())
                        .cloned();
                    let tool_results = entry
                        .get("toolResults")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                        .filter(|v: &Vec<String>| !v.is_empty());
                    messages.push(super::super::ChatMessage {
                        role,
                        content,
                        tool_calls,
                        tool_results,
                        delegation_event: None,
                        tool_name: None,
                        tool_args: None,
                        tool_result: None,
                        tool_index: None,
                        source: None,
                        pending_tool_calls: None,
                    });
                }
                return Ok(super::super::SessionHistory {
                    id,
                    messages,
                    created_at,
                    updated_at,
                });
            }
        }
        Err("no sessions.history response".to_string())
    })
}

/// Delete a session via `sessions.delete` WS method.
pub(crate) fn fetch_sessions_delete(
    profile_override: Option<&str>,
    session_id: &str,
) -> Result<bool, String> {
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

        // Wait for connect response.
        loop {
            let msg = ws
                .next()
                .await
                .ok_or("no connect response")?
                .map_err(|e| e.to_string())?;
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

        let del_req = serde_json::json!({
            "type": "req",
            "id": "2",
            "method": "sessions.delete",
            "params": { "sessionId": session_id }
        });
        ws.send(Message::Text(del_req.to_string().into()))
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
                let ok = res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                return Ok(ok);
            }
        }
        Err("no sessions.delete response".to_string())
    })
}

/// Delete all sessions via `sessions.delete_all` WS method.
pub(crate) fn fetch_sessions_delete_all(
    profile_override: Option<&str>,
    orchestrator_id: Option<&str>,
) -> Result<usize, String> {
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

        // Wait for connect response.
        loop {
            let msg = ws
                .next()
                .await
                .ok_or("no connect response")?
                .map_err(|e| e.to_string())?;
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

        let mut params = serde_json::json!({});
        if let Some(oid) = orchestrator_id {
            params["orchestratorId"] = serde_json::Value::String(oid.to_string());
        }
        let del_req = serde_json::json!({
            "type": "req",
            "id": "2",
            "method": "sessions.delete_all",
            "params": params
        });
        ws.send(Message::Text(del_req.to_string().into()))
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
                let ok = res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if !ok {
                    let err = res
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("sessions.delete_all failed");
                    return Err(err.to_string());
                }
                let count = res
                    .get("payload")
                    .and_then(|p| p.get("count"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                return Ok(count);
            }
        }
        Err("no sessions.delete_all response".to_string())
    })
}
