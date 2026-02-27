use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "chai")]
#[command(about = "Chai CLI — multi-agent management system", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show version
    Version,

    /// Create the configuration directory and default files (config, workspace, bundled skills). Skills need a tools.json in their directory to expose tools to the agent.
    Init {
        /// Config file path (default: CHAI_CONFIG_PATH or ~/.chai/config.json)
        #[arg(long, short, value_name = "PATH")]
        config: Option<std::path::PathBuf>,
    },

    /// Run the gateway (HTTP + WebSocket control plane). Loads skills from the config skill root (or skills.directory); only skills with a tools.json have callable tools.
    Gateway {
        /// Config file path (default: CHAI_CONFIG_PATH or ~/.chai/config.json)
        #[arg(long, short, value_name = "PATH")]
        config: Option<std::path::PathBuf>,

        /// WebSocket and HTTP port (default from config or 15151)
        #[arg(long, short)]
        port: Option<u16>,
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
        Some(Commands::Init { config }) => {
            if let Err(e) = run_init(config) {
                log::error!("init failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Gateway { config, port }) => {
            if let Err(e) = run_gateway(config, port).await {
                log::error!("gateway failed: {}", e);
                std::process::exit(1);
            }
        }
        None => {
            println!("Run with --help for usage");
        }
    }
}

fn run_init(config_path: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    let path = config_path.unwrap_or_else(lib::config::default_config_path);
    let _dir = lib::init::init_config_dir(&path)?;
    println!("initialized configuration at {}", path.parent().unwrap_or(std::path::Path::new(".")).display());
    Ok(())
}

async fn run_gateway(
    config_path: Option<std::path::PathBuf>,
    port: Option<u16>,
) -> anyhow::Result<()> {
    let (mut config, path) = lib::config::load_config(config_path)?;
    if let Some(p) = port {
        config.gateway.port = p;
    }
    log::info!("starting gateway on {}:{}", config.gateway.bind, config.gateway.port);
    lib::gateway::run_gateway(config, path).await
}
