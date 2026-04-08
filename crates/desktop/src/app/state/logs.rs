use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use env_logger::Logger;

/// Maximum number of log lines held in memory for the Logs screen.
const LOG_BUFFER_MAX_LINES: usize = 2000;

/// Ring buffer of log lines for the Logs screen. Written by DesktopLogger and gateway stderr reader.
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

/// Filter for desktop log lines: same rules as **`RUST_LOG`** / env_logger, default **`info`**.
static ENV_FILTER_LOGGER: OnceLock<Logger> = OnceLock::new();

/// When **`ENV_FILTER_LOGGER`** is not set yet, match **`env_logger`** default (**info** and above).
fn fallback_enabled(metadata: &log::Metadata) -> bool {
    metadata.level() <= log::LevelFilter::Info
}

/// Logger that appends to LOG_LINES for display in the Logs screen.
struct DesktopLogger;

impl log::Log for DesktopLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        ENV_FILTER_LOGGER
            .get()
            .map(|l| l.enabled(metadata))
            .unwrap_or_else(|| fallback_enabled(metadata))
    }

    fn log(&self, record: &log::Record) {
        let allow = match ENV_FILTER_LOGGER.get() {
            Some(filter) => filter.matches(record),
            None => fallback_enabled(record.metadata()),
        };
        if !allow {
            return;
        }
        let line = format!("{} [{}] {}", chrono_lite(), record.level(), record.args());
        push_log_line(line);
    }

    fn flush(&self) {}
}

static LOGGER: DesktopLogger = DesktopLogger;

/// Initialize global logging for the desktop app.
pub fn init_logging() {
    let _ = LOG_LINES.get_or_init(|| Mutex::new(VecDeque::new()));
    let inner = Logger::from_env(env_logger::Env::default().default_filter_or("info"));
    let max_level = inner.filter();
    let _ = ENV_FILTER_LOGGER.set(inner);
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(max_level);
    log::info!("desktop started");
}

/// Format current time as HH:MM:SS.mmm (UTC, seconds within the current day).
fn chrono_lite() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = t.as_secs();
    let millis = t.subsec_millis();
    let secs_since_midnight_utc = secs % 86400;
    let h = secs_since_midnight_utc / 3600;
    let m = (secs_since_midnight_utc % 3600) / 60;
    let s = secs_since_midnight_utc % 60;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, millis)
}
