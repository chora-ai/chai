//! Shared gateway WebSocket client: connect, authenticate, send a request, read a response.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

/// An authenticated gateway WebSocket connection ready to send method requests.
pub(crate) struct GatewayConn {
    ws: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    rx: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    next_id: u64,
}

impl GatewayConn {
    /// Connect to the gateway WebSocket, complete the auth handshake, and return
    /// a ready-to-use connection.
    pub(crate) async fn connect(profile: Option<&str>) -> Result<Self> {
        let (config, paths) = lib::config::load_config(profile)?;
        let bind = config.gateway.bind.trim();
        let port = config.gateway.port;
        let token = lib::config::resolve_gateway_token(&config);
        let ws_url = format!("ws://{}:{}/ws", bind, port);

        let (ws, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .context("failed to connect to gateway")?;
        let (mut sink, mut rx) = ws.split();

        // Read challenge frame.
        let first = rx
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("no first frame from gateway"))?
            .map_err(|e| anyhow::anyhow!("ws read error: {}", e))?;
        let Message::Text(challenge_text) = first else {
            anyhow::bail!("expected text challenge frame");
        };
        let challenge: serde_json::Value = serde_json::from_str(&challenge_text)
            .context("failed to parse challenge frame")?;
        let nonce = challenge
            .get("payload")
            .and_then(|p| p.get("nonce").and_then(|n| n.as_str()))
            .ok_or_else(|| anyhow::anyhow!("expected connect.challenge event with nonce"))?
            .to_string();

        let device_token_path = paths.device_token_path();
        let device_json_path = paths.device_json();

        let connect_params =
            if let Some(device_token) = lib::device::load_device_token_from(&device_token_path) {
                serde_json::json!({ "auth": { "deviceToken": device_token } })
            } else {
                let identity = lib::device::DeviceIdentity::load(device_json_path.as_path())
                    .or_else(|| {
                        let id = lib::device::DeviceIdentity::generate().ok()?;
                        let _ = id.save(&device_json_path);
                        Some(id)
                    })
                    .ok_or_else(|| anyhow::anyhow!("failed to load or create device identity"))?;
                let signed_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let token_str = token.as_deref().unwrap_or("");
                let scopes: Vec<String> = vec!["operator.read".into(), "operator.write".into()];
                let payload_str = lib::device::build_connect_payload(
                    &identity.device_id,
                    "chai-cli",
                    "operator",
                    "operator",
                    &scopes,
                    signed_at,
                    token_str,
                    &nonce,
                );
                let signature = identity.sign(&payload_str)?;
                let mut params = serde_json::json!({
                    "client": { "id": "chai-cli", "mode": "operator" },
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
        sink.send(Message::Text(connect_req.to_string().into()))
            .await
            .context("failed to send connect request")?;

        // Wait for connect response.
        while let Some(msg) = rx.next().await {
            let msg = msg.context("ws read error")?;
            let Message::Text(text) = msg else {
                continue;
            };
            let res: serde_json::Value =
                serde_json::from_str(&text).context("failed to parse connect response")?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") {
                continue;
            }
            if res.get("id").and_then(|v| v.as_str()) == Some("1") {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("connect failed");
                    anyhow::bail!("gateway auth failed: {}", err);
                }
                // Save device token if issued.
                if let Some(auth) = res.get("payload").and_then(|p| p.get("auth")) {
                    if let Some(dt) = auth.get("deviceToken").and_then(|v| v.as_str()) {
                        let _ = lib::device::save_device_token_to(&device_token_path, dt);
                    }
                }
                break;
            }
        }

        Ok(Self {
            ws: sink,
            rx,
            next_id: 2, // 1 was used for connect
        })
    }

    /// Send a gateway method request and wait for the matching response.
    ///
    /// Returns the response payload on success, or an error if the method
    /// returned `ok: false` or the connection was lost.
    pub(crate) async fn call(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let id = self.next_id.to_string();
        self.next_id += 1;

        let req = serde_json::json!({
            "type": "req",
            "id": id,
            "method": method,
            "params": params,
        });
        self.ws
            .send(Message::Text(req.to_string().into()))
            .await
            .with_context(|| format!("failed to send {} request", method))?;

        while let Some(msg) = self.rx.next().await {
            let msg = msg.context("ws read error")?;
            let Message::Text(text) = msg else {
                continue;
            };
            let res: serde_json::Value =
                serde_json::from_str(&text).context("failed to parse response")?;
            if res.get("type").and_then(|v| v.as_str()) != Some("res") {
                continue;
            }
            if res.get("id").and_then(|v| v.as_str()) == Some(&id) {
                if !res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let err = res
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("method failed");
                    anyhow::bail!("{}: {}", method, err);
                }
                return Ok(res
                    .get("payload")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null));
            }
        }

        anyhow::bail!("connection closed before {} response", method);
    }
}
