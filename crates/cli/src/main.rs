mod chat;
mod file;
mod gateway;
mod gateway_conn;
mod git;
mod init;
mod logs;
mod profile;
mod resolve;
mod sessions;
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

        /// Orchestrator agent id (defaults to the first orchestrator)
        #[arg(long, value_name = "ID")]
        agent: Option<String>,
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

    /// Git operations for skill tool backends
    Git {
        #[command(subcommand)]
        sub: git::GitCmd,
    },

    /// Read and search the gateway's in-memory log buffer
    Logs {
        #[command(subcommand)]
        sub: logs::LogsCmd,
    },

    /// Manage sessions (list, delete, clear)
    Sessions {
        #[command(subcommand)]
        sub: sessions::SessionsCmd,
    },

    /// Sandbox-aware path resolution for tool resolve commands
    Resolve {
        #[command(subcommand)]
        sub: resolve::ResolveCmd,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Load .env from the profile directory before initializing the logger so that
    // environment-driven configuration like RUST_LOG takes effect.
    let cli_profile = match &cli.command {
        Some(Commands::Gateway { profile, .. }) => profile.as_deref(),
        Some(Commands::Chat { profile, .. }) => profile.as_deref(),
        Some(Commands::Sessions { sub }) => match sub {
            sessions::SessionsCmd::List { profile, .. } => profile.as_deref(),
            sessions::SessionsCmd::Delete { profile, .. } => profile.as_deref(),
            sessions::SessionsCmd::Clear { profile, .. } => profile.as_deref(),
        },
        _ => None,
    };
    lib::config::load_profile_env(cli_profile);

    // Use the gateway logger for the gateway command (captures to ring buffer
    // for the `logs` WebSocket method), plain env_logger for everything else.
    if matches!(&cli.command, Some(Commands::Gateway { .. })) {
        lib::logging::init_gateway_logging();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("lib=info,cli=info"))
            .init();
    }

    match cli.command {
        Some(Commands::Version) => {
            println!("chai {}", env!("CARGO_PKG_VERSION"));
        }
        Some(Commands::Init) => {
            if let Err(e) = init::run_init() {
                eprintln!("init failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Gateway { profile, port }) => {
            if let Err(e) = gateway::run_gateway(profile.as_deref(), port).await {
                log::error!("gateway failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Chat { profile, session, agent }) => {
            if let Err(e) = chat::run_chat(profile, session, agent).await {
                eprintln!("chat error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Profile { sub }) => {
            if let Err(e) = profile::run_profile(sub) {
                eprintln!("profile: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Skill { sub }) => {
            if let Err(e) = skill::run_skill(sub) {
                eprintln!("skill: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::File { sub }) => {
            if let Err(e) = file::run_file(sub) {
                eprintln!("file: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Git { sub }) => {
            if let Err(e) = git::run_git(sub) {
                eprintln!("git: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Logs { sub }) => {
            if let Err(e) = logs::run_logs(sub) {
                eprintln!("logs: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Sessions { sub }) => {
            if let Err(e) = sessions::run_sessions(sub).await {
                eprintln!("sessions: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Resolve { sub }) => {
            if let Err(e) = resolve::run_resolve(sub) {
                eprintln!("resolve: {}", e);
                std::process::exit(1);
            }
        }
        None => {
            println!("Run with --help for usage");
        }
    }
}