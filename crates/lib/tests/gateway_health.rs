//! Integration test: start the gateway on a free port, GET /, assert health JSON.
//! Does not require Ollama or Telegram. The server task is left running when the test ends.

use lib::config::Config;
use lib::gateway;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free port");
    listener.local_addr().expect("local_addr").port()
}

fn temp_config_dir() -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("chai-gateway-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(dir.join("skills")).expect("create skills dir");
    std::fs::create_dir_all(dir.join("workspace")).expect("create workspace dir");
    let config_path = dir.join("config.json");
    std::fs::File::create(&config_path)
        .and_then(|mut f| f.write_all(b"{}"))
        .expect("write config.json");
    (dir, config_path)
}

#[tokio::test]
async fn gateway_health_http_responds_with_running() {
    let port = free_port();
    let (_temp_dir, config_path) = temp_config_dir();

    let mut config = Config::default();
    config.gateway.port = port;
    config.gateway.bind = "127.0.0.1".to_string();
    config.agents.workspace = Some(_temp_dir.join("workspace"));

    let config_path = config_path;
    let gateway_handle = tokio::spawn(async move {
        let _ = gateway::run_gateway(config, config_path).await;
    });

    let url = format!("http://127.0.0.1:{}/", port);
    let client = reqwest::Client::new();
    let mut last_err = None;
    for _ in 0..100 {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let json: serde_json::Value = resp.json().await.expect("parse JSON");
                assert_eq!(json.get("runtime").and_then(|v| v.as_str()), Some("running"));
                assert_eq!(json.get("protocol").and_then(|v| v.as_u64()), Some(1));
                assert_eq!(json.get("port").and_then(|v| v.as_u64()), Some(port as u64));
                return;
            }
            Ok(_) => {}
            Err(e) => last_err = Some(e),
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let _ = gateway_handle.abort();
    panic!(
        "GET {} did not return 200 with health JSON within 5s; last error: {:?}",
        url, last_err
    );
}
