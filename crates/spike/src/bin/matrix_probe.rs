//! Spike: password login + one sync against a Matrix homeserver (raw Client-Server API).
//!
//! Usage:
//!   MATRIX_HOMESERVER=https://matrix.example.org \
//!   MATRIX_USER=localpart_or_full_mxid \
//!   MATRIX_PASSWORD=secret \
//!   cargo run -p chai-spike --bin matrix-probe
//!
//! If `MATRIX_USER` has no `@`, it is combined with the server name derived from the homeserver URL.
//! Prints timeline `m.room.message` events (room id, body) for mapping to Chai `conversation_id`.

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

fn server_name_from_homeserver(hs: &str) -> Result<String> {
    let s = hs.trim();
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .ok_or_else(|| anyhow!("MATRIX_HOMESERVER must start with https:// or http://"))?;
    let host = rest.split('/').next().unwrap_or(rest);
    if host.is_empty() {
        anyhow::bail!("homeserver URL has no host");
    }
    Ok(host.to_string())
}

fn normalize_mxid(user: &str, server: &str) -> String {
    let u = user.trim();
    if u.starts_with('@') {
        u.to_string()
    } else {
        format!("@{}:{}", u.trim_start_matches('@'), server)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let homeserver =
        std::env::var("MATRIX_HOMESERVER").context("set MATRIX_HOMESERVER (e.g. https://matrix.org)")?;
    let user = std::env::var("MATRIX_USER").context("set MATRIX_USER (localpart or @user:server)")?;
    let password =
        std::env::var("MATRIX_PASSWORD").context("set MATRIX_PASSWORD")?;

    let server_name = server_name_from_homeserver(&homeserver)?;
    let mxid = normalize_mxid(&user, &server_name);

    let client = reqwest::Client::new();
    let login_url = format!(
        "{}/_matrix/client/v3/login",
        homeserver.trim_end_matches('/')
    );

    let body = json!({
        "type": "m.login.password",
        "identifier": { "type": "m.id.user", "user": mxid },
        "password": password,
    });

    let res = client
        .post(&login_url)
        .json(&body)
        .send()
        .await
        .context("login request")?;
    if !res.status().is_success() {
        let text = res.text().await.unwrap_or_default();
        anyhow::bail!("login failed: {}", text);
    }
    let login: Value = res.json().await.context("login json")?;
    let access_token = login
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("no access_token in login response"))?;
    let user_id = login
        .get("user_id")
        .and_then(|v| v.as_str())
        .unwrap_or("?");

    eprintln!("logged in as {}", user_id);

    let sync_url = format!(
        "{}/_matrix/client/v3/sync?timeout=3000",
        homeserver.trim_end_matches('/')
    );
    let res = client
        .get(&sync_url)
        .bearer_auth(access_token)
        .send()
        .await
        .context("sync request")?;
    if !res.status().is_success() {
        let text = res.text().await.unwrap_or_default();
        anyhow::bail!("sync failed: {}", text);
    }
    let sync: Value = res.json().await.context("sync json")?;

    // Print room -> message mapping (what we would map to InboundMessage.conversation_id + text).
    if let Some(rooms) = sync.get("rooms").and_then(|r| r.get("join")) {
        if let Some(obj) = rooms.as_object() {
            for (room_id, room) in obj {
                if let Some(timeline) = room.get("timeline").and_then(|t| t.get("events")) {
                    if let Some(events) = timeline.as_array() {
                        for ev in events {
                            if ev.get("type").and_then(|t| t.as_str()) != Some("m.room.message") {
                                continue;
                            }
                            let body = ev
                                .pointer("/content/body")
                                .and_then(|b| b.as_str())
                                .unwrap_or("");
                            let msgtype = ev
                                .pointer("/content/msgtype")
                                .and_then(|m| m.as_str())
                                .unwrap_or("?");
                            if msgtype == "m.text" && !body.is_empty() {
                                println!("room_id={}\t{}", room_id, body);
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "spike: use room_id as Chai conversation_id for Matrix (see CHANNELS.md / MATRIX.md)"
    );
    Ok(())
}
