//! Integration test: start the gateway on a free port, GET /, assert health JSON.
//! Uses a temp HOME with `chai init` layout.

use lib::gateway;
use lib::init;
use std::time::Duration;

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free port");
    listener.local_addr().expect("local_addr").port()
}

#[tokio::test]
async fn gateway_health_http_responds_with_running() {
    let port = free_port();
    let home = std::env::temp_dir().join(format!("chai-gateway-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home).expect("create temp home");

    let old_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", &home);

    let run_body = async {
        init::init_chai_home()?;
        let (mut config, paths) = lib::config::load_config(None)?;
        config.gateway.port = port;
        config.gateway.bind = "127.0.0.1".to_string();
        let gateway_handle = tokio::spawn(async move {
            let _ = gateway::run_gateway(config, paths).await;
        });

        let url = format!("http://127.0.0.1:{}/", port);
        let client = reqwest::Client::new();
        let mut last_err = None;
        for _ in 0..100 {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let json: serde_json::Value = resp.json().await.expect("parse JSON");
                    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("running"));
                    assert_eq!(json.get("protocol").and_then(|v| v.as_u64()), Some(1));
                    assert_eq!(json.get("port").and_then(|v| v.as_u64()), Some(port as u64));
                    gateway_handle.abort();
                    return Ok::<(), anyhow::Error>(());
                }
                Ok(_) => {}
                Err(e) => last_err = Some(e),
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        gateway_handle.abort();
        anyhow::bail!(
            "GET {} did not return 200 with health JSON within 5s; last error: {:?}",
            url,
            last_err
        );
    };

    let result = run_body.await;

    match old_home {
        Some(ref p) => std::env::set_var("HOME", p),
        None => std::env::remove_var("HOME"),
    }

    result.expect("gateway health");
}
