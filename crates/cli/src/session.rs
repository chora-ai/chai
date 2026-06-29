//! CLI session management: list, delete, and clear sessions via direct disk access.

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum SessionCmd {
    /// List sessions for the active profile
    List {
        /// Profile name
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,

        /// Orchestrator agent id (defaults to the first orchestrator)
        #[arg(long, value_name = "ID")]
        agent: Option<String>,
    },
    /// Delete a session by id
    Delete {
        /// Session id to delete
        id: String,
        /// Profile name
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,
    },
    /// Delete all sessions
    Clear {
        /// Profile name
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,

        /// Orchestrator agent id (defaults to the first orchestrator)
        #[arg(long, value_name = "ID")]
        agent: Option<String>,
    },
}

pub(crate) async fn run_session(cmd: SessionCmd) -> Result<()> {
    match cmd {
        SessionCmd::List { profile, agent } => list_sessions(profile, agent).await,
        SessionCmd::Delete { id, profile } => delete_session(id, profile).await,
        SessionCmd::Clear { profile, agent } => clear_sessions(profile, agent).await,
    }
}

/// Load the session store and binding store for the given profile and orchestrator.
fn open_stores(profile: Option<&str>, agent: Option<&str>) -> Result<(lib::session::SessionStore, lib::routing::SessionBindingStore)> {
    let (config, paths) = lib::config::load_config(profile)?;
    let orch_id = match agent {
        Some(id) => id,
        None => config.agents.default_orchestrator().id.trim(),
    };
    let orch_id = if orch_id.is_empty() { "orchestrator" } else { orch_id };
    // Validate that orch_id refers to an actual orchestrator.
    config.agents.orchestrator(Some(orch_id)).map_err(|e| anyhow::anyhow!(e))?;
    let sessions_path = lib::config::sessions_dir(&paths.profile_dir, orch_id);
    let session_store = lib::session::SessionStore::with_data_dir(sessions_path.clone());
    let binding_store = lib::routing::SessionBindingStore::with_data_dir(sessions_path);
    Ok((session_store, binding_store))
}

async fn list_sessions(profile: Option<String>, agent: Option<String>) -> Result<()> {
    let (session_store, binding_store) = open_stores(profile.as_deref(), agent.as_deref())?;
    let summaries = session_store.scan().await;

    if summaries.is_empty() {
        println!("no sessions");
        return Ok(());
    }

    // Sort by updated_at descending (most recently updated first).
    let mut summaries = summaries;
    summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    for s in &summaries {
        let short_id = shorten_id(&s.id);
        let binding = binding_store.get_channel_binding(&s.id).await;
        match binding {
            Some((ch, _conv)) => {
                println!("{:<12} {:>3} msg  {}  [{}]", short_id, s.message_count, s.updated_at, ch)
            }
            None => println!("{:<12} {:>3} msg  {}", short_id, s.message_count, s.updated_at),
        }
    }

    Ok(())
}

async fn delete_session(id: String, profile: Option<String>) -> Result<()> {
    let (session_store, binding_store) = open_stores(profile.as_deref(), None)?;
    match session_store.remove(&id).await {
        Some(_) => {
            binding_store.remove_binding(&id).await;
            println!("deleted session {}", shorten_id(&id));
            Ok(())
        }
        None => anyhow::bail!("session not found: {}", id),
    }
}

async fn clear_sessions(profile: Option<String>, agent: Option<String>) -> Result<()> {
    let (session_store, binding_store) = open_stores(profile.as_deref(), agent.as_deref())?;
    let count = session_store.remove_all().await;
    binding_store.remove_all().await;
    println!("deleted {} session(s)", count);
    Ok(())
}

/// Shorten a session id for display: show the first 12 characters (e.g. "sess-a1b2c3d4").
fn shorten_id(id: &str) -> &str {
    if id.len() >= 12 {
        &id[..12]
    } else {
        id
    }
}
