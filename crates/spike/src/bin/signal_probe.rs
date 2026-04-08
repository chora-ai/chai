//! Spike: talk to a running signal-cli HTTP daemon (JSON-RPC over HTTP + SSE).
//!
//! Start the daemon separately, for example:
//!   signal-cli -a +1234567890 daemon --http 127.0.0.1:7583
//!
//! Then:
//!   SIGNAL_CLI_HTTP=http://127.0.0.1:7583 cargo run -p chai-spike --bin signal-probe
//!
//! This binary only checks `/api/v1/check` and optionally reads a few SSE lines from
//! `/api/v1/events` so you can see live JSON. Full Chai integration would map those payloads
//! to `InboundMessage` + `send_message` via JSON-RPC `send` methods (see upstream docs).

use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<()> {
    let base =
        std::env::var("SIGNAL_CLI_HTTP").unwrap_or_else(|_| "http://127.0.0.1:7583".to_string());
    let base = base.trim_end_matches('/').to_string();

    let client = reqwest::Client::new();

    let check_url = format!("{}/api/v1/check", base);
    let r = client.get(&check_url).send().await.context("GET check")?;
    eprintln!("GET {} -> {}", check_url, r.status());
    if !r.status().is_success() {
        anyhow::bail!(
            "signal-cli HTTP daemon not reachable. Start it, e.g.: signal-cli -a ACCOUNT daemon --http 127.0.0.1:7583"
        );
    }

    let rpc_url = format!("{}/api/v1/rpc", base);
    // Same method names as CLI; see signal-cli-jsonrpc(5) and upstream wiki.
    let list_groups = json!({
        "jsonrpc": "2.0",
        "method": "listGroups",
        "id": 1
    });
    let r = client
        .post(&rpc_url)
        .json(&list_groups)
        .send()
        .await
        .context("POST rpc")?;
    let status = r.status();
    let text = r.text().await.unwrap_or_default();
    eprintln!(
        "POST {} (listGroups) -> status {} body (truncated): {}...",
        rpc_url,
        status,
        text.chars().take(400).collect::<String>()
    );

    let events_url = format!("{}/api/v1/events", base);
    eprintln!(
        "opening SSE {} (one line sample, 5s timeout)...",
        events_url
    );
    let res = client.get(&events_url).send().await.context("GET events")?;
    if !res.status().is_success() {
        let text = res.text().await.unwrap_or_default();
        anyhow::bail!("events failed: {}", text);
    }
    let mut stream = res.bytes_stream();
    let mut buf = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        tokio::select! {
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(b)) => {
                        buf.extend_from_slice(&b);
                        let s = String::from_utf8_lossy(&buf);
                        if s.contains('\n') {
                            for line in s.lines().take(3) {
                                if line.starts_with("data:") {
                                    println!("{}", line);
                                }
                            }
                            break;
                        }
                    }
                    Some(Err(e)) => eprintln!("sse read error: {}", e),
                    None => break,
                }
            }
        }
    }

    eprintln!("spike: map JSON-RPC + SSE payloads to `conversation_id` + text (see SIGNAL.md)");
    Ok(())
}
