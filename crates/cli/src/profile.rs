use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum ProfileCmd {
    /// List profile names under ~/.chai/profiles
    List,
    /// Show persistent profile (~/.chai/active) and effective profile if CHAI_PROFILE differs
    Current,
    /// Set ~/.chai/active to profiles/<name> (gateway must not be running for that profile)
    Switch {
        /// Profile name
        name: String,
    },
}

pub(crate) fn run_profile(cmd: ProfileCmd) -> Result<()> {
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
                if env_trim != persistent {
                    println!("{} (CHAI_PROFILE overrides active: {})", env_trim, persistent);
                } else {
                    println!("{}", persistent);
                }
            } else {
                println!("{}", persistent);
            }
        }
        ProfileCmd::Switch { name } => {
            let profile_name = name.trim();
            if lib::profile::gateway_is_running(&chai_home, profile_name) {
                anyhow::bail!("gateway is running for profile {:?}; stop it before switching", profile_name);
            }
            lib::profile::switch_active_profile(&chai_home, profile_name)?;
            println!("active profile is now {}", profile_name);
        }
    }
    Ok(())
}
