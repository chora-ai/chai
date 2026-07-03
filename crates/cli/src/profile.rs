use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum ProfileCmd {
    /// List profile names under ~/.chai/profiles
    List,
    /// Show persistent profile (~/.chai/active)
    Current,
    /// Set ~/.chai/active to profiles/<name>
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
            println!("{}", persistent);
        }
        ProfileCmd::Switch { name } => {
            let profile_name = name.trim();
            lib::profile::switch_active_profile(&chai_home, profile_name)?;
            println!("active profile is now {}", profile_name);
        }
    }
    Ok(())
}
