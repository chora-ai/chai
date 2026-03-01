//! Gateway WebSocket protocol types (connect, health, etc.).

use serde::{Deserialize, Serialize};

/// Wire request: `{ "type": "req", "id", "method", "params" }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsRequest {
    #[serde(rename = "type")]
    pub typ: String,
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Wire response: `{ "type": "res", "id", "ok", "payload" or "error" }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsResponse {
    #[serde(rename = "type")]
    pub typ: String,
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Client connect params (subset needed for handshake).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectParams {
    pub min_protocol: Option<u32>,
    pub max_protocol: Option<u32>,
    #[serde(default)]
    pub client: ConnectClient,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub auth: ConnectAuth,
    /// Optional device identity for pairing: id, publicKey, signature, signedAt, nonce.
    #[serde(default)]
    pub device: Option<ConnectDevice>,
}

/// Device identity sent with connect when using device signing (pairing).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectDevice {
    pub id: String,
    pub public_key: String,  // wire: publicKey, base64-encoded Ed25519 public key
    pub signature: String,  // base64-encoded Ed25519 signature of the canonical payload
    pub signed_at: u64,     // wire: signedAt, Unix ms
    pub nonce: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectClient {
    pub id: Option<String>,
    pub version: Option<String>,
    pub platform: Option<String>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectAuth {
    pub token: Option<String>,
    /// When set, connect is authenticated by device token (pairing); token and device signing are optional.
    pub device_token: Option<String>,
}

/// Server hello-ok payload after successful connect.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloOk {
    #[serde(rename = "type")]
    pub typ: String,
    pub protocol: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<HelloPolicy>,
    /// Set when the connection is authenticated by device (pairing) or a new device token was issued.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<HelloAuth>,
}

/// Auth info returned in hello-ok (device token and scopes).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloAuth {
    pub device_token: String,
    pub role: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloPolicy {
    pub tick_interval_ms: Option<u64>,
}

/// Parsed connect payload for handler use.
#[derive(Debug, Clone)]
pub struct ConnectPayload {
    pub params: ConnectParams,
    pub request_id: String,
}

/// Params for WS method "send": deliver message to a channel conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendParams {
    pub channel_id: String,
    pub conversation_id: String,
    pub message: String,
}

/// Params for WS method "agent": run one turn (optional session, message, optional backend and model override).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentParams {
    #[serde(default)]
    pub session_id: Option<String>,
    pub message: String,
    /// Override backend for this turn: "ollama" or "lmstudio". When set, the model is resolved within this backend.
    #[serde(default)]
    pub backend: Option<String>,
    /// Override model for this turn. When backend is also set, must be a model id for that backend.
    #[serde(default)]
    pub model: Option<String>,
}

impl WsResponse {
    pub fn ok(id: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            typ: "res".to_string(),
            id: id.into(),
            ok: true,
            payload: Some(payload),
            error: None,
        }
    }

    pub fn err(id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            typ: "res".to_string(),
            id: id.into(),
            ok: false,
            payload: None,
            error: Some(error.into()),
        }
    }
}
