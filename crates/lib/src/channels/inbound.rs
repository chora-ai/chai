//! Inbound message from a channel: delivered to the gateway for session/agent handling.

/// A message from a channel to be routed to a session and optionally answered by the agent.
#[derive(Debug, Clone)]
pub struct InboundMessage {
    pub channel_id: String,
    pub conversation_id: String,
    pub text: String,
}
