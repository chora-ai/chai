//! Signal channel is disabled for this build (`lib` built without the `signal` feature).

use crate::config::Config;

/// Always returns `None` — Signal channel is not built.
pub fn resolve_signal_daemon_config(_config: &Config) -> Option<()> {
    None
}
