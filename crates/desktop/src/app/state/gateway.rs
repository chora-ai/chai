use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use super::super::{ChaiApp, PROBE_INTERVAL_FRAMES, STATUS_INTERVAL_FRAMES};
use crate::app::fetch_gateway_status;

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

