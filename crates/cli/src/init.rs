use anyhow::Result;

pub(crate) fn run_init() -> Result<()> {
    let chai_home = lib::init::init_chai_home()?;
    log::info!("initialized ~/.chai at {}", chai_home.display());
    Ok(())
}
