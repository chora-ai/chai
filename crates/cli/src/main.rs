use clap::{Parser, Subcommand};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

#[derive(Parser)]
#[command(name = "chai")]
#[command(about = "Chai CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show version
    Version,

    /// Create ~/.chai with profiles (assistant, developer), active symlink, and shared skills
    Init,

    /// Run the gateway (HTTP + WebSocket control plane). Uses CHAI_PROFILE or ~/.chai/active unless --profile is set.
    Gateway {
        /// Profile name (overrides CHAI_PROFILE and ~/.chai/active for this process)
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,

        /// WebSocket and HTTP port (default from config or 15151)
        #[arg(long, short)]
        port: Option<u16>,
    },

    /// Chat with the default agent via the gateway (interactive)
    Chat {
        /// Profile name for config resolution (must match the running gateway's profile)
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,

        /// Optional existing session id to continue.
        #[arg(long, value_name = "ID")]
        session: Option<String>,
    },

    /// List profiles, switch the active symlink, or show current profile
    Profile {
        #[command(subcommand)]
        sub: ProfileCmd,
    },
}

#[derive(Subcommand)]
enum ProfileCmd {
    /// List profile names under ~/.chai/profiles
    List,
    /// Show persistent profile (~/.chai/active) and effective profile if CHAI_PROFILE differs
    Current,
    /// Set ~/.chai/active to profiles/<name> (gateway must not be running)
    Switch {
        /// Profile name
        name: String,
    },
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Version) => {
            println!("chai {}", env!("CARGO_PKG_VERSION"));
        }
        Some(Commands::Init) => {
            if let Err(e) = run_init() {
                log::error!("init failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Gateway { profile, port }) => {
            if let Err(e) = run_gateway(profile.as_deref(), port).await {
                log::error!("gateway failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Chat { profile, session }) => {
            if let Err(e) = run_chat(profile.as_deref(), session).await {
                log::error!("chat failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Profile { sub }) => {
            if let Err(e) = run_profile(sub) {
                log::error!("profile: {}", e);
                std::process::exit(1);
            }
        }
        None => {
            println!("Run with --help for usage");
        }
    }
}

fn run_init() -> anyhow::Result<()> {
    let chai_home = lib::init::init_chai_home()?;
    println!("initialized ~/.chai at {}", chai_home.display());
    Ok(())
}

fn run_profile(cmd: ProfileCmd) -> anyhow::Result<()> {
    let chai_home = lib::profile::chai_home()?;
    match cmd {
        ProfileCmd::List => {
            let names = lib::profile::list_profile_names(&chai_home)?;
            for n in names {
                println!("{}", n);
            }
        }
        ProfileCmd::Current => {
            let persistent = lib::profile::read_persistent_profile_name(&chai_home)?;
            if let Ok(env_name) = std::env::var("CHAI_PROFILE") {
                let env_trim = env_name.trim();
                if !env_trim.is_empty() && env_trim != persistent {
                    println!("persistent: {}", persistent);
                    println!("effective: {} (CHAI_PROFILE)", env_trim);
                } else {
                    println!("{}", persistent);
                }
            } else {
                println!("{}", persistent);
            }
        }
        ProfileCmd::Switch { name } => {
            if lib::profile::gateway_is_running(&chai_home) {
                anyhow::bail!("gateway is running; stop it before switching profile");
            }
            lib::profile::switch_active_profile(&chai_home, name.trim())?;
            println!("active profile is now {}", name.trim());
        }
    }
    Ok(())
}

async fn run_gateway(profile: Option<&str>, port: Option<u16>) -> anyhow::Result<()> {
    let (mut config, paths) = lib::config::load_config(profile)?;
    if let Some(p) = port {
        config.gateway.port = p;
    }
    log::info!(
        "starting gateway profile={} on {}:{}",
        paths.profile_name,
        config.gateway.bind,
        config.gateway.port
    );
    lib::gateway::run_gateway(config, paths).await
}

#[derive(Debug)]
struct AgentReply {
    session_id: String,
    reply: String,
}

async fn run_chat(profile: Option<&str>, session: Option<String>) -> anyhow::Result<()> {
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

        match agent_turn_via_gateway(profile, current_session.clone(), input.to_string()).await {
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

    let connect_params = if let Some(device_token) =
        lib::device::load_device_token_from(&device_token_path)
    {
        serde_json::json!({ "auth": { "deviceToken": device_token } })
    } else {
        let identity =
            lib::device::DeviceIdentity::load(device_json_path.as_path()).or_else(|| {
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
    ws.send(Message::Text(connect_req.to_string()))
        .await
        .map_err(|e| e.to_string())?;

    while let Some(msg) = ws.next().await {
        let msg = msg.map_err(|e| e.to_string())?;
        let Message::Text(text) = msg else { continue };
        let res: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| e.to_string())?;
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
    ws.send(Message::Text(agent_req.to_string()))
        .await
        .map_err(|e| e.to_string())?;

    while let Some(msg) = ws.next().await {
        let msg = msg.map_err(|e| e.to_string())?;
        let Message::Text(text) = msg else { continue };
        let res: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| e.to_string())?;
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
