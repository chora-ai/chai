//! Communication channels (e.g. Telegram).
//!
//! Channel trait and registry so the gateway can start/stop channel connectors
//! and route messages. Inbound messages are sent to the gateway for session/agent handling.

mod inbound;
mod registry;
mod telegram;

pub use inbound::InboundMessage;
pub use registry::{ChannelHandle, ChannelRegistry};
pub use telegram::{TelegramChannel, TelegramUpdate};