//! Gateway log buffer and logging initialization.
//!
//! Provides a global ring buffer that captures formatted log lines produced by
//! the gateway process. The `logs` WebSocket method reads from this buffer so
//! that connected clients (e.g. the desktop app) can display gateway logs
//! alongside their own.
//!
//! The CLI gateway command calls [`init_gateway_logging`] instead of
//! `env_logger::init()` so that both stderr output and the in-memory buffer
//! receive every log record.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::Mutex;

/// Maximum number of log lines held in memory for the `logs` WebSocket method.
const LOG_BUFFER_MAX_LINES: usize = 2000;

/// A log entry in the ring buffer: the formatted line and its monotonically
/// increasing sequence number.
#[derive(Clone)]
struct LogEntry {
    seq: u64,
    line: String,
}

/// Global ring buffer state.
struct LogBuffer {
    entries: VecDeque<LogEntry>,
    next_seq: u64,
}

impl LogBuffer {
    fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            next_seq: 1,
        }
    }

    fn push(&mut self, line: String) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.entries.push_back(LogEntry { seq, line });
        while self.entries.len() > LOG_BUFFER_MAX_LINES {
            self.entries.pop_front();
        }
    }

    /// Return entries with sequence numbers greater than `after_seq`, plus the
    /// current maximum sequence number (0 if the buffer is empty).
    fn after(&self, after_seq: u64) -> (Vec<&LogEntry>, u64) {
        let max_seq = self.entries.back().map(|e| e.seq).unwrap_or(0);
        let entries: Vec<&LogEntry> = self
            .entries
            .iter()
            .filter(|e| e.seq > after_seq)
            .collect();
        (entries, max_seq)
    }
}

static LOG_BUFFER: std::sync::OnceLock<Mutex<LogBuffer>> = std::sync::OnceLock::new();

fn log_buffer() -> &'static Mutex<LogBuffer> {
    LOG_BUFFER.get_or_init(|| Mutex::new(LogBuffer::new()))
}

/// Push a formatted log line to the ring buffer.
fn push_log_line(line: String) {
    if let Ok(mut buf) = log_buffer().lock() {
        buf.push(line);
    }
}

/// Return log lines with sequence numbers greater than `after_seq`.
///
/// Returns `(lines, max_seq)` where `max_seq` is the highest sequence number
/// in the buffer (0 if empty). Callers can pass `max_seq` as `after_seq` on
/// the next call to only get new lines.
pub fn log_lines_after(after_seq: u64) -> (Vec<String>, u64) {
    let buf = log_buffer().lock().unwrap_or_else(|e| e.into_inner());
    let (entries, max_seq) = buf.after(after_seq);
    let lines: Vec<String> = entries.iter().map(|e| e.line.clone()).collect();
    (lines, max_seq)
}

/// Initialize global logging for the gateway process.
///
/// Sets up `env_logger` with a custom format that:
/// 1. Writes formatted, colorized output to stderr (consistent with the desktop app)
/// 2. Pushes plain-text copies of each line to an in-memory ring buffer
///    accessible via [`log_lines_after`]
///
/// The default filter is `lib=info,cli=info` so that only chai-related log lines appear
/// at info level. Noisy dependency logs (e.g. zbus D-Bus dispatch) are suppressed unless
/// the user explicitly enables them via `RUST_LOG`. Note that `RUST_LOG` overrides the
/// default filter entirely — a bare level like `RUST_LOG=debug` enables all crates at that
/// level. Use target-scoped directives (e.g. `RUST_LOG=lib=debug,cli=debug`) to get
/// chai-only debug output.
///
/// # Panics
///
/// Panics if a logger has already been set (same as `env_logger::init()`).
pub fn init_gateway_logging() {
    let _ = LOG_BUFFER.get_or_init(|| Mutex::new(LogBuffer::new()));
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("lib=info,cli=info"))
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
}
