use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

/// Maximum number of log lines held in memory per buffer for the Logging screen.
const LOG_BUFFER_MAX_LINES: usize = 2000;

/// Ring buffer of desktop log lines for the Logging screen. Written by the
/// env_logger format closure.
static DESKTOP_LOG_LINES: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

/// Ring buffer of gateway log lines for the Logging screen. Written by the
/// gateway stderr/stdout reader (owned gateway) and the `logs` WS method fetch
/// (external gateway).
static GATEWAY_LOG_LINES: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

/// Get the global desktop log buffer.
pub fn desktop_log_buffer() -> &'static Mutex<VecDeque<String>> {
    DESKTOP_LOG_LINES.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Get the global gateway log buffer.
pub fn gateway_log_buffer() -> &'static Mutex<VecDeque<String>> {
    GATEWAY_LOG_LINES.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Push a line into the desktop log buffer.
pub(crate) fn push_desktop_log_line(line: String) {
    if let Ok(mut buf) = desktop_log_buffer().lock() {
        buf.push_back(line);
        while buf.len() > LOG_BUFFER_MAX_LINES {
            buf.pop_front();
        }
    }
}

/// Push a line into the gateway log buffer.
pub(crate) fn push_gateway_log_line(line: String) {
    if let Ok(mut buf) = gateway_log_buffer().lock() {
        buf.push_back(line);
        while buf.len() > LOG_BUFFER_MAX_LINES {
            buf.pop_front();
        }
    }
}

/// Initialize global logging for the desktop app.
///
/// Loads the profile `.env` (so `RUST_LOG` takes effect) before building the logger.
/// Uses the tracked `.env` loader (`state::env::load_profile_env_tracked`) so that
/// variables can be properly cleaned up when the user switches profiles later.
///
/// Uses `env_logger` for formatting and stderr output (consistent with the `chai` CLI),
/// and pushes plain-text copies of each line to the in-memory desktop ring buffer for
/// the Logging screen.
///
/// The default filter is `desktop=info,lib=info` so that only chai-related log lines appear
/// at info level. Noisy dependency logs (e.g. zbus D-Bus dispatch) are suppressed unless
/// the user explicitly enables them via `RUST_LOG`. Note that `RUST_LOG` overrides the
/// default filter entirely — a bare level like `RUST_LOG=debug` enables all crates at that
/// level. Use target-scoped directives (e.g. `RUST_LOG=desktop=debug,lib=debug`) to get
/// chai-only debug output.
pub fn init_logging() {
    let _ = DESKTOP_LOG_LINES.get_or_init(|| Mutex::new(VecDeque::new()));
    let _ = GATEWAY_LOG_LINES.get_or_init(|| Mutex::new(VecDeque::new()));

    // Load .env from the resolved profile directory using the tracked loader.
    // This records which variables were set so they can be removed on profile switch.
    // Note: errors are written to stderr because the logger is not yet initialized.
    let profile_dir = lib::profile::resolve_profile_dir(None)
        .map(|p| p.profile_dir)
        .ok();
    if let Some(ref dir) = profile_dir {
        if let Err(e) = super::env::load_profile_env_tracked(dir) {
            eprintln!("failed to load .env at {}: {}", dir.display(), e);
        }
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("desktop=info,lib=info"))
        .format(|buf, record| {
            let target = record.target();

            // Build the plain-text line for the desktop ring buffer first (no ANSI codes).
            let plain = format!(
                "[{} {:<5} {}] {}",
                buf.timestamp(),
                record.level(),
                target,
                record.args()
            );
            push_desktop_log_line(plain);

            let dimmed = anstyle::RgbColor(150, 150, 150).on_default();
            let color = buf.default_level_style(record.level());

            writeln!(
                buf,
                "{dimmed}[{dimmed:#}{} {color}{:<5}{color:#} {target}{dimmed}]{dimmed:#} {}",
                buf.timestamp(),
                record.level(),
                record.args()
            )
        })
        .init();
    log::info!("desktop started");
}
