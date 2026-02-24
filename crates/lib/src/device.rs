//! Device identity for gateway pairing: keypair load/generate, canonical payload, and signing.
//!
//! Payload format must match the gateway's `device_signature_payload` (deviceId, client id/mode,
//! role, scopes, signedAt, token, nonce, newline-separated).

use anyhow::Result;
use base64::Engine;
use ed25519_dalek::Signer;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Persisted device identity (deviceId, public key, private key). Stored at e.g. ~/.chai/device.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceIdentity {
    pub device_id: String,
    pub public_key: String,
    pub private_key: String,
}

/// Build the canonical payload string that the gateway expects for signature verification.
/// Order: deviceId, client_id, client_mode, role, scopes (comma-joined), signed_at, token, nonce.
pub fn build_connect_payload(
    device_id: &str,
    client_id: &str,
    client_mode: &str,
    role: &str,
    scopes: &[String],
    signed_at: u64,
    token: &str,
    nonce: &str,
) -> String {
    let scopes_str = scopes.join(",");
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        device_id, client_id, client_mode, role, scopes_str, signed_at, token, nonce
    )
}

impl DeviceIdentity {
    /// Sign the payload string and return the signature as base64. The payload must already include signed_at (same format as gateway expects).
    pub fn sign(&self, payload: &str) -> Result<String> {
        let key_bytes = base64::engine::general_purpose::STANDARD
            .decode(self.private_key.as_bytes())
            .map_err(|e| anyhow::anyhow!("decode private key: {}", e))?;
        let key_arr: [u8; 32] = key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid private key length"))?;
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&key_arr);
        let sig = signing_key.sign(payload.as_bytes());
        Ok(base64::engine::general_purpose::STANDARD.encode(sig.to_bytes()))
    }

    /// Load from JSON file. Returns None if file missing or invalid.
    pub fn load(path: &Path) -> Option<Self> {
        let s = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&s).ok()
    }

    /// Save to JSON file. Creates parent dirs if needed.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let s = serde_json::to_string_pretty(self).map_err(|e| anyhow::anyhow!("{}", e))?;
        std::fs::write(path, s)?;
        Ok(())
    }

    /// Generate a new keypair. device_id is the first 16 chars of base64(public_key).
    pub fn generate() -> Result<Self> {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).map_err(|e| anyhow::anyhow!("getrandom: {}", e))?;
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&bytes);
        let verifying_key = signing_key.verifying_key();
        let public_key = base64::engine::general_purpose::STANDARD.encode(verifying_key.as_bytes());
        let private_key = base64::engine::general_purpose::STANDARD.encode(signing_key.as_bytes());
        let device_id = public_key.chars().take(16).collect::<String>();
        Ok(Self {
            device_id,
            public_key,
            private_key,
        })
    }
}

/// Default path for device identity file.
pub fn default_device_path() -> std::path::PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".chai").join("device.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("device.json"))
}

/// Default path for stored device token (after pairing).
pub fn default_device_token_path() -> std::path::PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".chai").join("device_token"))
        .unwrap_or_else(|| std::path::PathBuf::from("device_token"))
}

/// Load stored device token if present.
pub fn load_device_token() -> Option<String> {
    let path = default_device_token_path();
    let s = std::fs::read_to_string(&path).ok()?;
    let t = s.trim().to_string();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

/// Persist device token (e.g. after hello-ok.auth.deviceToken).
pub fn save_device_token(token: &str) -> Result<()> {
    let path = default_device_token_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, token)?;
    Ok(())
}
