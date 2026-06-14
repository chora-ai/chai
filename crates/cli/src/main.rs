mod chat;
mod file;
mod gateway;
mod init;
mod profile;
mod skill;

use clap::{Parser, Subcommand};

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

    /// Create ~/.chai with default profiles, active symlink, bundled skills, and skills.lock for new profiles
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
        sub: profile::ProfileCmd,
    },

    /// Manage skill packages (discover CLIs, generate, validate, inspect)
    Skill {
        #[command(subcommand)]
        sub: skill::SkillCmd,
    },

    /// File operations for skill tool backends
    File {
        #[command(subcommand)]
        sub: file::FileCmd,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let default_log_filter = match &cli.command {
        Some(Commands::Gateway { .. }) => "info",
        _ => "warn",
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_log_filter))
        .init();

    match cli.command {
        Some(Commands::Version) => {
            println!("chai {}", env!("CARGO_PKG_VERSION"));
        }
        Some(Commands::Init) => {
            if let Err(e) = init::run_init() {
                log::error!("init failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Gateway { profile, port }) => {
            if let Err(e) = gateway::run_gateway(profile.as_deref(), port).await {
                log::error!("gateway failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Chat { profile, session }) => {
            if let Err(e) = chat::run_chat(profile, session).await {
                log::error!("chat error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Profile { sub }) => {
            if let Err(e) = profile::run_profile(sub) {
                log::error!("profile: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Skill { sub }) => {
            if let Err(e) = skill::run_skill(sub) {
                log::error!("skill: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::File { sub }) => {
            if let Err(e) = file::run_file(sub) {
                log::error!("file: {}", e);
                std::process::exit(1);
            }
        }
        None => {
            println!("Run with --help for usage");
        }
    }
}
