use anyhow::Result;

use crate::gateway_conn::GatewayConn;

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

pub(crate) async fn run_chat(profile: Option<String>, session: Option<String>, agent: Option<String>) -> Result<()> {
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

        match agent_turn_via_gateway(profile.as_deref(), current_session.clone(), input.to_string(), agent.as_deref()).await {
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
    orchestrator_id: Option<&str>,
) -> Result<AgentReply> {
    let mut conn = GatewayConn::connect(profile).await?;

    let mut agent_params = serde_json::json!({
        "message": message,
    });
    if let Some(id) = session_id {
        agent_params["sessionId"] = serde_json::Value::String(id);
    }
    if let Some(id) = orchestrator_id {
        agent_params["orchestratorId"] = serde_json::Value::String(id.to_string());
    }

    let payload = conn.call("agent", agent_params).await?;
    let session_id = payload
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing sessionId in agent response"))?
        .to_string();
    let reply = payload
        .get("reply")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(AgentReply { session_id, reply })
}