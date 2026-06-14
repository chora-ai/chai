//! Communication channels (Telegram, Matrix, Signal).
//!
//! Channel trait and registry so the gateway can start/stop channel connectors
//! and route messages. Inbound messages are sent to the gateway for session/agent handling.

mod inbound;
#[cfg(feature = "matrix")]
mod matrix;
#[cfg(not(feature = "matrix"))]
mod matrix_stub;
mod registry;
#[cfg(feature = "signal")]
mod signal;
#[cfg(not(feature = "signal"))]
mod signal_stub;
mod telegram;

pub use inbound::InboundMessage;
#[cfg(feature = "matrix")]
pub use matrix::{connect_matrix_client, MatrixChannel, PendingMatrixVerification};
pub use registry::{ChannelHandle, ChannelRegistry};
#[cfg(feature = "signal")]
pub use signal::{resolve_signal_daemon_config, SignalChannel};
#[cfg(not(feature = "signal"))]
pub use signal_stub::resolve_signal_daemon_config;
pub use telegram::{TelegramChannel, TelegramTransport, TelegramUpdate};
