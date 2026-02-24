//! Gateway: HTTP + WebSocket control plane.
//!
//! Single port serves HTTP and WebSocket. Protocol: first frame must be `connect`;
//! then requests (req/res) and events. Minimal implementation for short-term goals.

mod pairing;
mod protocol;
mod server;

pub use protocol::{ConnectParams, ConnectPayload, HelloOk, WsRequest, WsResponse};
pub use server::run_gateway;