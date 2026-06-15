use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

/// Maximum number of log lines held in memory for the Logging screen.
const LOG_BUFFER_MAX_LINES: usize = 2000;

/// Ring buffer of log lines for the Logging screen. Written by the env_logger format closure,
/// the gateway stderr/stdout reader (owned gateway), and the `logs` WS method fetch
/// (external gateway).
static LOG_LINES: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

/// Get the global log buffer.
pub fn log_buffer() -> &'static Mutex<VecDeque<String>> {
    LOG_LINES.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub(crate) fn push_log_line(line: String) {
    if let Ok(mut buf) = log_buffer().lock() {
        buf.push_back(line);
        while buf.len() > LOG_BUFFER_MAX_LINES {
            buf.pop_front();
        }
    }
}

/// Initialize global logging for the desktop app.
///
/// Loads the profile `.env` (so `RUST_LOG` takes effect) before building the logger.
/// Uses `env_logger` for formatting and stderr output (consistent with the `chai` CLI),
/// and pushes plain-text copies of each line to the in-memory ring buffer for the Logging screen.
///
/// The default filter is `desktop=info,lib=info` so that only chai-related log lines appear
/// at info level. Noisy dependency logs (e.g. zbus D-Bus dispatch) are suppressed unless
/// the user explicitly enables them via `RUST_LOG`. Note that `RUST_LOG` overrides the
/// default filter entirely — a bare level like `RUST_LOG=debug` enables all crates at that
/// level. Use target-scoped directives (e.g. `RUST_LOG=desktop=debug,lib=debug`) to get
/// chai-only debug output.
pub fn init_logging() {
    let _ = LOG_LINES.get_or_init(|| Mutex::new(VecDeque::new()));
    lib::config::load_profile_env(None);
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("desktop=info,lib=info"))
        .format(|buf, record| {
            let target = record.target();

            // Build the plain-text line for the ring buffer first (no ANSI codes).
            let plain = format!(
                "[{} {:<5} {}] {}",
                buf.timestamp(),
                record.level(),
                target,
                record.args()
            );
            push_log_line(plain);

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
