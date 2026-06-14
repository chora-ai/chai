use anyhow::Result;

pub(crate) async fn run_gateway(profile: Option<&str>, port: Option<u16>) -> Result<()> {
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
