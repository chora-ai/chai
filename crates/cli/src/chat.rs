use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug)]
struct AgentReply {
    session_id: String,
    reply: String,
}

/// Same copy as desktop chat **`/help`** (see `crates/desktop/src/app.rs`).
const CHAT_HELP_TEXT: &str = "available commands:\n\n/new - start a new session (clear conversation history)\n/help - show this help message";

/// Same acknowledgment as desktop **`/new`** (see `crates/desktop/src/app.rs`).
const CHAT_NEW_SESSION_ACK: &str =
    "New session. Next message will start with a clean history.";

pub(crate) async fn run_chat(profile: Option<String>, session: Option<String>) -> Result<()> {
    use std::io::{self, Write};

    let mut current_session = session;
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        write!(stdout, "> ")?;
        stdout.flush()?;
        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        if input.eq_ignore_ascii_case("/exit") || input.eq_ignore_ascii_case("/quit") {
            break;
        }
        if input.eq_ignore_ascii_case("/new") {
            current_session = None;
            println!("< {}", CHAT_NEW_SESSION_ACK);
            continue;
        }
        if input.eq_ignore_ascii_case("/help") {
            println!("{}", CHAT_HELP_TEXT);
            continue;
        }

        match agent_turn_via_gateway(profile.as_deref(), current_session.clone(), input.to_string()).await {
            Ok(reply) => {
                current_session = Some(reply.session_id);
                println!("< {}", reply.reply.trim());
            }
            Err(e) => {
                eprintln!("chat error: {}", e);
            }
        }
    }

    Ok(())
}

async fn agent_turn_via_gateway(
    profile: Option<&str>,
    session_id: Option<String>,
    message: String,
) -> Result<AgentReply, String> {
    let (config, paths) = lib::config::load_config(profile).map_err(|e| e.to_string())?;
    let bind = config.gateway.bind.trim();
    let port = config.gateway.port;
    let token = lib::config::resolve_gateway_token(&config);
    let ws_url = format!("ws://{}:{}/ws", bind, port);

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
                .ok_or("failed to load or create device identity")?;
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
            let signature = identity.sign(&payload_str).map_err(|e| e.to_string())?;
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
                return Err(err.to_string());
            }
            if let Some(auth) = res.get("payload").and_then(|p| p.get("auth")) {
                if let Some(dt) = auth.get("deviceToken").and_then(|v| v.as_str()) {
                    let _ = lib::device::save_device_token_to(&device_token_path, dt);
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
            return Ok(AgentReply { session_id, reply });
        }
    }

    Err("no agent response".to_string())
}
