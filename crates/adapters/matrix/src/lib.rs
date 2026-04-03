//! Matrix client for Chai: matrix-sdk sync, E2EE, and `m.room.message` handling.
//! Depends only on `matrix-sdk` (no `lib` crate) so the workspace can compile without Matrix when `lib` disables the `matrix` feature.

pub use matrix_sdk;

use matrix_sdk::{
    authentication::matrix::MatrixSession,
    config::SyncSettings,
    deserialized_responses::EncryptionInfo,
    ruma::{
        events::{
            room::message::{MessageType, OriginalSyncRoomMessageEvent, RoomMessageEventContent},
            AnyToDeviceEvent,
        },
        OwnedDeviceId, OwnedUserId, RoomId,
    },
    Client, Room, SessionMeta, SessionTokens,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::convert::TryInto;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const CHANNEL_ID: &str = "matrix";

/// Inbound payload for the gateway queue (same shape as `lib::InboundMessage`).
#[derive(Debug, Clone)]
pub struct RawInbound {
    pub channel_id: String,
    pub conversation_id: String,
    pub text: String,
}

/// A pending to-device verification request (for gateway HTTP UX).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingMatrixVerification {
    pub user_id: String,
    pub flow_id: String,
    pub from_device: String,
}

/// Resolved login for [`connect_with_params`].
pub enum MatrixLogin {
    AccessToken {
        token: String,
        user_id: String,
        device_id: String,
    },
    Password {
        mxid: String,
        password: String,
    },
}

/// Parameters to build a Matrix [`Client`] (no `config::Config` — filled by `lib`).
pub struct MatrixConnectParams {
    pub homeserver: String,
    pub store_path: PathBuf,
    pub login: MatrixLogin,
}

fn server_name_from_homeserver(hs: &str) -> Option<String> {
    let s = hs.trim();
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))?;
    let host = rest.split('/').next().unwrap_or(rest);
    if host.is_empty() {
        return None;
    }
    Some(host.to_string())
}

/// Build `@localpart:server` for password login. Appends `server` when the user string is only a
/// localpart (with or without a leading `@`). If a full MXID is already present (`@localpart:domain`
/// with non-empty domain), it is returned with a single leading `@`.
fn normalize_mxid(user: &str, server: &str) -> String {
    let u = user.trim();
    let without_at = u.trim_start_matches('@');
    match without_at.split_once(':') {
        Some((local, domain)) if !local.is_empty() && !domain.is_empty() => {
            format!("@{}:{}", local, domain)
        }
        _ => {
            let local = without_at.trim_end_matches(':').trim();
            format!("@{}:{}", local, server)
        }
    }
}

/// Build a logged-in [`Client`] with SQLite + E2EE, or [`None`] on failure.
pub async fn connect_with_params(params: MatrixConnectParams) -> Option<Client> {
    let base = params.homeserver.trim_end_matches('/').to_string();

    if let Err(e) = std::fs::create_dir_all(&params.store_path) {
        log::warn!(
            "matrix: failed to create store directory {}: {}",
            params.store_path.display(),
            e
        );
        return None;
    }

    let client = match Client::builder()
        .homeserver_url(&base)
        .sqlite_store(&params.store_path, None)
        .build()
        .await
    {
        Ok(c) => c,
        Err(e) => {
            log::warn!("matrix: failed to build client: {}", e);
            return None;
        }
    };

    match params.login {
        MatrixLogin::AccessToken {
            token,
            user_id,
            device_id,
        } => {
            let user_id: OwnedUserId = match user_id.as_str().try_into() {
                Ok(id) => id,
                Err(e) => {
                    log::warn!("matrix: invalid user id: {}", e);
                    return None;
                }
            };
            let device_id: OwnedDeviceId = device_id.as_str().into();

            let session = MatrixSession {
                meta: SessionMeta { user_id, device_id },
                tokens: SessionTokens {
                    access_token: token,
                    refresh_token: None,
                },
            };

            if let Err(e) = client.restore_session(session).await {
                log::warn!("matrix: restore_session failed: {}", e);
                return None;
            }
            log::info!(
                "matrix: session restored (e2ee store: {})",
                params.store_path.display()
            );
            Some(client)
        }
        MatrixLogin::Password { mxid, password } => {
            if let Err(e) = client.matrix_auth().login_username(&mxid, &password).send().await {
                log::warn!("matrix: login failed: {}", e);
                return None;
            }
            log::info!(
                "matrix: logged in with password (e2ee store: {})",
                params.store_path.display()
            );
            Some(client)
        }
    }
}

/// Build [`MatrixConnectParams`] for password login (homeserver URL + user + password + store path).
pub fn password_login_params(
    homeserver: String,
    store_path: PathBuf,
    user: String,
    password: String,
) -> Option<MatrixConnectParams> {
    let base = homeserver.trim_end_matches('/').to_string();
    let server_name = match server_name_from_homeserver(&base) {
        Some(s) => s,
        None => {
            log::warn!("matrix: invalid homeserver URL for password login");
            return None;
        }
    };
    let mxid = normalize_mxid(&user, &server_name);
    Some(MatrixConnectParams {
        homeserver: base,
        store_path,
        login: MatrixLogin::Password { mxid, password },
    })
}

/// Response from `GET /account/whoami` (used when building access-token login).
pub struct Whoami {
    pub user_id: String,
    pub device_id: Option<String>,
}

/// Used by `lib` when resolving access-token login. `base` is homeserver HTTPS base URL.
pub async fn fetch_whoami(base: &str, access_token: &str) -> Option<Whoami> {
    let client = reqwest::Client::new();
    let url = format!("{}/_matrix/client/v3/account/whoami", base.trim_end_matches('/'));
    let res = match client.get(&url).bearer_auth(access_token).send().await {
        Ok(r) => r,
        Err(e) => {
            log::warn!("matrix whoami request failed: {}", e);
            return None;
        }
    };
    if !res.status().is_success() {
        log::warn!("matrix whoami failed: {}", res.status());
        return None;
    }
    let v: Value = match res.json().await {
        Ok(v) => v,
        Err(e) => {
            log::warn!("matrix whoami json: {}", e);
            return None;
        }
    };
    let user_id = match v.get("user_id").and_then(|x| x.as_str()) {
        Some(u) => u.to_string(),
        None => {
            log::warn!("matrix whoami: no user_id in response");
            return None;
        }
    };
    let device_id = v
        .get("device_id")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    Some(Whoami { user_id, device_id })
}

fn truncate_sync_status_msg(s: &str) -> String {
    let t = s.trim();
    const MAX: usize = 512;
    if t.len() > MAX {
        format!("{}…", &t[..MAX])
    } else {
        t.to_string()
    }
}

async fn matrix_sync_loop(client: Client, running: Arc<AtomicBool>, last_error: Arc<Mutex<Option<String>>>) {
    let settings = SyncSettings::new().timeout(Duration::from_secs(30));
    while running.load(Ordering::SeqCst) {
        match client.sync_once(settings.clone()).await {
            Ok(_) => {
                if let Ok(mut g) = last_error.lock() {
                    *g = None;
                }
            }
            Err(e) => {
                log::debug!("matrix sync error: {}", e);
                if let Ok(mut g) = last_error.lock() {
                    *g = Some(truncate_sync_status_msg(&e.to_string()));
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

/// Matrix-sdk sync (decrypts encrypted rooms) and send (encrypts when needed).
#[derive(Clone)]
pub struct MatrixInner {
    id: String,
    running: Arc<AtomicBool>,
    client: Client,
    room_allowlist: Option<Arc<HashSet<String>>>,
    pending_verifications: Arc<Mutex<Vec<PendingMatrixVerification>>>,
    last_sync_error: Arc<Mutex<Option<String>>>,
}

impl MatrixInner {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn new(client: Client, room_allowlist: Option<HashSet<String>>) -> Self {
        Self {
            id: CHANNEL_ID.to_string(),
            running: Arc::new(AtomicBool::new(false)),
            client,
            room_allowlist: room_allowlist.map(Arc::new),
            pending_verifications: Arc::new(Mutex::new(Vec::new())),
            last_sync_error: Arc::new(Mutex::new(None)),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn pending_verifications(&self) -> Arc<Mutex<Vec<PendingMatrixVerification>>> {
        Arc::clone(&self.pending_verifications)
    }

    pub fn remove_pending_verification(&self, user_id: &str, flow_id: &str) {
        let mut g = self.pending_verifications.lock().unwrap();
        g.retain(|p| !(p.user_id == user_id && p.flow_id == flow_id));
    }

    /// Sanitized snapshot for gateway `status.channels.matrix` (no secrets).
    pub fn status_detail(&self) -> Value {
        let session_active = self.client.session_meta().is_some();
        let sync_running = self.running.load(Ordering::SeqCst);
        let last_sync_error = self
            .last_sync_error
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .flatten();
        let pending = self
            .pending_verifications
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let pending_verifications: Vec<Value> = pending
            .iter()
            .map(|p| {
                json!({
                    "userId": p.user_id,
                    "flowId": p.flow_id,
                    "fromDevice": p.from_device,
                })
            })
            .collect();
        json!({
            "sessionActive": session_active,
            "syncRunning": sync_running,
            "lastSyncError": last_sync_error,
            "pendingVerificationCount": pending.len(),
            "pendingVerifications": pending_verifications,
            "roomAllowlistActive": self.room_allowlist.is_some(),
        })
    }

    pub fn start_inbound(self: Arc<Self>, inbound_tx: mpsc::Sender<RawInbound>) -> JoinHandle<()> {
        self.running.store(true, Ordering::SeqCst);
        log::info!("matrix channel: starting sync loop");
        tokio::spawn(async move {
            let Some(meta) = self.client.session_meta() else {
                log::warn!("matrix: no session meta; sync not started");
                return;
            };
            let my_user = meta.user_id.clone();
            let client = self.client.clone();

            let pending = self.pending_verifications.clone();
            client.add_event_handler(
                move |ev: AnyToDeviceEvent, _info: Option<EncryptionInfo>| {
                    let pending = pending.clone();
                    async move {
                        let AnyToDeviceEvent::KeyVerificationRequest(ev) = ev else {
                            return;
                        };
                        let user_id = ev.sender.to_string();
                        let flow_id = ev.content.transaction_id.as_str().to_string();
                        let from_device = ev.content.from_device.as_str().to_string();
                        let rec = PendingMatrixVerification {
                            user_id: user_id.clone(),
                            flow_id: flow_id.clone(),
                            from_device,
                        };
                        let mut g = pending.lock().unwrap();
                        g.retain(|p| !(p.user_id == rec.user_id && p.flow_id == rec.flow_id));
                        g.push(rec);
                        log::info!(
                            "matrix: key verification request from {} flow_id={} (use POST /matrix/verification/accept then /matrix/verification/start-sas)",
                            user_id,
                            flow_id
                        );
                    }
                },
            );

            let allowlist = self.room_allowlist.clone();
            client.add_event_handler(
                move |ev: OriginalSyncRoomMessageEvent, room: Room| {
                    let inbound_tx = inbound_tx.clone();
                    let my_user = my_user.clone();
                    let allowlist = allowlist.clone();
                    async move {
                        if ev.sender == my_user {
                            return;
                        }
                        let body = match &ev.content.msgtype {
                            MessageType::Text(t) => t.body.as_str(),
                            _ => return,
                        };
                        let trimmed = body.trim();
                        if trimmed.is_empty() {
                            return;
                        }
                        let room_id = room.room_id().to_string();
                        if let Some(ref allow) = allowlist {
                            if !allow.contains(&room_id) {
                                log::debug!("matrix: ignoring message from non-allowlisted room {}", room_id);
                                return;
                            }
                        }
                        let inbound = RawInbound {
                            channel_id: CHANNEL_ID.to_string(),
                            conversation_id: room_id,
                            text: trimmed.to_string(),
                        };
                        if inbound_tx.send(inbound).await.is_err() {
                            log::debug!("matrix: inbound channel closed");
                        }
                    }
                },
            );

            let running = Arc::clone(&self.running);
            let last_err = Arc::clone(&self.last_sync_error);
            matrix_sync_loop(client, running, last_err).await;
            log::info!("matrix channel: sync loop stopped");
        })
    }

    pub async fn send_message(&self, room_id: &str, text: &str) -> Result<(), String> {
        let rid = RoomId::parse(room_id).map_err(|e| e.to_string())?;
        let Some(room) = self.client.get_room(&rid) else {
            return Err("matrix: room not loaded yet; wait for sync after join".to_string());
        };
        room
            .send(RoomMessageEventContent::text_plain(text))
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod normalize_mxid_tests {
    use super::normalize_mxid;

    #[test]
    fn local_only_gets_server() {
        assert_eq!(
            normalize_mxid("alice", "matrix.example.org"),
            "@alice:matrix.example.org"
        );
    }

    #[test]
    fn at_local_without_server_gets_suffix() {
        assert_eq!(
            normalize_mxid("@alice", "matrix.example.org"),
            "@alice:matrix.example.org"
        );
    }

    #[test]
    fn full_mxid_unchanged_except_leading_at() {
        assert_eq!(
            normalize_mxid("@alice:matrix.org", "matrix.example.org"),
            "@alice:matrix.org"
        );
        assert_eq!(
            normalize_mxid("alice:matrix.org", "matrix.example.org"),
            "@alice:matrix.org"
        );
    }

    #[test]
    fn trailing_colon_treated_as_incomplete() {
        assert_eq!(
            normalize_mxid("@alice:", "matrix.example.org"),
            "@alice:matrix.example.org"
        );
    }
}
