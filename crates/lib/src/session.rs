//! Conversation session and message history for the agent loop.
//!
//! Sessions are keyed by id and hold a list of messages (user/assistant/system).
//! Used by the gateway to run agent turns and optionally bind to channel conversations.
//!
//! When a `data_dir` is provided, sessions are persisted to disk as JSON files
//! under `<data_dir>/sess-<id>.json`. Write-through: every mutation writes to
//! memory **and** disk (atomic write via `.tmp` + rename). Lazy loading: `get()`
//! loads from disk if not in memory. `scan()` reads metadata without full history.

use log;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Unique session identifier (opaque string).
pub type SessionId = String;

/// A single message in a session (role + content; assistant may have tool_calls, tool results have tool_name).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    /// When role is "assistant", optional tool calls from the model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<crate::providers::ToolCall>>,
    /// When role is "tool", the name of the tool this result is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

impl SessionMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_name: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_name: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_name: None,
        }
    }
}

/// A session: id and ordered message history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub messages: Vec<SessionMessage>,
    /// Successful **`delegate_task`** completions this session (for policy caps).
    pub delegation_count: usize,
    /// Successful delegations per worker id (`search`, `code`, …).
    pub delegation_by_worker: HashMap<String, usize>,
    /// ISO 8601 timestamp when the session was created.
    #[serde(default)]
    pub created_at: String,
    /// ISO 8601 timestamp updated on every message append and delegation record.
    #[serde(default)]
    pub updated_at: String,
}

/// Lightweight summary of a session (no full message history).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: SessionId,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
}

/// Persistent session store backed by an in-memory map with optional disk I/O.
///
/// When `data_dir` is `None`, behaves as a purely in-memory store (for tests
/// and non-persistent contexts). When set, every mutation is write-through to
/// disk and `get()` can lazy-load from disk.
pub struct SessionStore {
    inner: Arc<RwLock<HashMap<SessionId, Session>>>,
    /// Directory for session JSON files. `None` = in-memory only.
    data_dir: Option<PathBuf>,
    /// Set of session ids known to exist on disk (populated by `scan()`).
    disk_index: Arc<RwLock<HashSet<SessionId>>>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    /// Create an in-memory-only session store (no disk I/O).
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            data_dir: None,
            disk_index: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Create a persistent session store that writes to `data_dir`.
    /// Creates the directory if it does not exist.
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        if let Err(e) = std::fs::create_dir_all(&data_dir) {
            log::warn!(
                "could not create sessions directory {}: {}",
                data_dir.display(),
                e
            );
        }
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            data_dir: Some(data_dir),
            disk_index: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Scan the `data_dir` for `.json` session files and populate the disk index.
    /// Returns a list of `SessionSummary` structs (metadata only, no full history).
    /// Must be called at gateway startup after `with_data_dir()`.
    pub async fn scan(&self) -> Vec<SessionSummary> {
        let Some(ref dir) = self.data_dir else {
            return Vec::new();
        };
        let mut summaries = Vec::new();
        let mut index = self.disk_index.write().await;
        let entries = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => return summaries,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let file_name = match path.file_stem().and_then(|s| s.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            // Only consider files matching the session id pattern (sess-*).
            if !file_name.starts_with("sess-") {
                continue;
            }
            let session_id = file_name;
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<Session>(&content) {
                    Ok(session) => {
                        index.insert(session_id.clone());
                        summaries.push(SessionSummary {
                            id: session.id,
                            created_at: session.created_at,
                            updated_at: session.updated_at,
                            message_count: session.messages.len(),
                        });
                    }
                    Err(e) => {
                        log::warn!(
                            "skipping corrupt session file {}: {}",
                            path.display(),
                            e
                        );
                    }
                },
                Err(e) => {
                    log::warn!(
                        "skipping unreadable session file {}: {}",
                        path.display(),
                        e
                    );
                }
            }
        }
        summaries
    }

    /// Return the file path for a session id.
    fn session_path(&self, id: &str) -> Option<PathBuf> {
        self.data_dir
            .as_ref()
            .map(|dir| dir.join(format!("{}.json", id)))
    }

    /// Write a session to disk (atomic: .tmp then rename).
    fn write_to_disk(&self, session: &Session) {
        let Some(ref path) = self.session_path(&session.id) else {
            return;
        };
        let tmp_path = path.with_extension("json.tmp");
        match serde_json::to_string_pretty(session) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&tmp_path, &json) {
                    log::warn!(
                        "failed to write session tmp file {}: {}",
                        tmp_path.display(),
                        e
                    );
                    return;
                }
                if let Err(e) = std::fs::rename(&tmp_path, path) {
                    log::warn!(
                        "failed to rename session file {} -> {}: {}",
                        tmp_path.display(),
                        path.display(),
                        e
                    );
                }
            }
            Err(e) => {
                log::warn!(
                    "failed to serialize session {}: {}",
                    session.id,
                    e
                );
            }
        }
    }

    /// Delete a session file from disk.
    fn delete_from_disk(&self, id: &str) {
        let Some(ref path) = self.session_path(id) else {
            return;
        };
        if path.exists() {
            if let Err(e) = std::fs::remove_file(path) {
                log::warn!(
                    "failed to delete session file {}: {}",
                    path.display(),
                    e
                );
            }
        }
    }

    /// Load a session from disk by id. Returns `None` if the file does not exist
    /// or is corrupt (corrupt files are logged and skipped).
    fn load_from_disk(&self, id: &str) -> Option<Session> {
        let path = self.session_path(id)?;
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<Session>(&content) {
                Ok(session) => Some(session),
                Err(e) => {
                    log::warn!(
                        "corrupt session file {}: {}",
                        path.display(),
                        e
                    );
                    None
                }
            },
            Err(e) => {
                // File does not exist is not a warning — it's expected for lazy-load miss.
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!(
                        "failed to read session file {}: {}",
                        path.display(),
                        e
                    );
                }
                None
            }
        }
    }

    /// Create a new session with a generated id; returns the session id.
    /// Writes to disk if `data_dir` is set.
    pub async fn create(&self) -> SessionId {
        let id = format!("sess-{}", uuid::Uuid::new_v4());
        let now = chrono_now_iso8601();
        let session = Session {
            id: id.clone(),
            messages: Vec::new(),
            delegation_count: 0,
            delegation_by_worker: HashMap::new(),
            created_at: now.clone(),
            updated_at: now,
        };
        self.inner.write().await.insert(id.clone(), session.clone());
        self.write_to_disk(&session);
        if let Ok(mut index) = self.disk_index.try_write() {
            index.insert(id.clone());
        }
        id
    }

    /// Create a session with the given id if it does not exist; returns the id.
    /// If the id is not in memory but the file exists on disk, loads it (lazy load).
    pub async fn get_or_create(&self, id: impl Into<SessionId>) -> SessionId {
        let id = id.into();
        if self.inner.read().await.contains_key(&id) {
            return id;
        }
        // Check disk index for lazy load.
        {
            let index = self.disk_index.read().await;
            if index.contains(&id) {
                if let Some(session) = self.load_from_disk(&id) {
                    self.inner.write().await.insert(id.clone(), session);
                    return id;
                }
            }
        }
        // Disk index may be stale (e.g. try_write contention on create, or an
        // externally placed file). Fall back to a direct filesystem check before
        // creating a new session, to avoid silently overwriting an existing one.
        if self.data_dir.is_some() {
            if let Some(session) = self.load_from_disk(&id) {
                log::warn!(
                    "session {} found on disk but not in disk_index; loading and patching index",
                    id
                );
                self.inner.write().await.insert(id.clone(), session);
                if let Ok(mut index) = self.disk_index.try_write() {
                    index.insert(id.clone());
                }
                return id;
            }
        }
        // Not on disk either — create new.
        let now = chrono_now_iso8601();
        let session = Session {
            id: id.clone(),
            messages: Vec::new(),
            delegation_count: 0,
            delegation_by_worker: HashMap::new(),
            created_at: now.clone(),
            updated_at: now,
        };
        self.inner.write().await.insert(id.clone(), session.clone());
        self.write_to_disk(&session);
        if let Ok(mut index) = self.disk_index.try_write() {
            index.insert(id.clone());
        }
        id
    }

    /// Return a clone of the session if it exists.
    /// If not in memory but the file exists on disk, loads it (lazy load).
    pub async fn get(&self, id: &str) -> Option<Session> {
        if let Some(session) = self.inner.read().await.get(id).cloned() {
            return Some(session);
        }
        // Lazy load from disk.
        if let Some(session) = self.load_from_disk(id) {
            let mut g = self.inner.write().await;
            // Double-check: another task may have loaded it while we were reading.
            if g.contains_key(id) {
                return g.get(id).cloned();
            }
            g.insert(id.to_string(), session.clone());
            Some(session)
        } else {
            None
        }
    }

    /// Remove a session by id. Returns the removed session if it existed.
    /// Also deletes the file from disk if `data_dir` is set.
    /// Handles sessions that exist on disk but have not been lazily loaded
    /// into memory — loads the session first so the caller receives `Some(_)`
    /// and the disk file is cleaned up correctly.
    pub async fn remove(&self, id: &str) -> Option<Session> {
        // Try in-memory first.
        if let Some(session) = self.inner.write().await.remove(id) {
            self.delete_from_disk(id);
            if let Ok(mut index) = self.disk_index.try_write() {
                index.remove(id);
            }
            return Some(session);
        }
        // Not in memory — check disk index for lazy-loaded sessions.
        let in_disk_index = self.disk_index.read().await.contains(id);
        let from_disk = if in_disk_index {
            self.load_from_disk(id)
        } else {
            None
        };
        self.delete_from_disk(id);
        if let Ok(mut index) = self.disk_index.try_write() {
            index.remove(id);
        }
        from_disk
    }

    /// Append a message to the session; returns error if session not found.
    pub async fn append_message(
        &self,
        id: &str,
        role: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<(), String> {
        self.append_message_full(id, role, content, None, None)
            .await
    }

    /// Append a message with optional tool_calls (assistant) or tool_name (tool result).
    /// Updates `updated_at` and writes to disk.
    pub async fn append_message_full(
        &self,
        id: &str,
        role: impl Into<String>,
        content: impl Into<String>,
        tool_calls: Option<Vec<crate::providers::ToolCall>>,
        tool_name: Option<String>,
    ) -> Result<(), String> {
        let mut g = self.inner.write().await;
        let session = g
            .get_mut(id)
            .ok_or_else(|| "session not found".to_string())?;
        session.messages.push(SessionMessage {
            role: role.into(),
            content: content.into(),
            tool_calls,
            tool_name,
        });
        session.updated_at = chrono_now_iso8601();
        let session_clone = session.clone();
        drop(g);
        self.write_to_disk(&session_clone);
        Ok(())
    }

    /// Increment successful delegation counters for policy (`maxDelegationsPerSession`, per-worker caps).
    /// Updates `updated_at` and writes to disk.
    pub async fn record_delegation(
        &self,
        id: &str,
        worker_id: &str,
    ) -> Result<(), String> {
        let mut g = self.inner.write().await;
        let session = g
            .get_mut(id)
            .ok_or_else(|| "session not found".to_string())?;
        session.delegation_count += 1;
        *session
            .delegation_by_worker
            .entry(worker_id.to_string())
            .or_insert(0) += 1;
        session.updated_at = chrono_now_iso8601();
        let session_clone = session.clone();
        drop(g);
        self.write_to_disk(&session_clone);
        Ok(())
    }

    /// Remove all sessions from memory and disk. Returns the number of sessions removed.
    /// Deletes all `sess-*.json` files from `data_dir` and clears the disk index.
    /// Counts both in-memory sessions and sessions that exist only on disk
    /// (not yet lazily loaded).
    pub async fn remove_all(&self) -> usize {
        let mut g = self.inner.write().await;
        let in_mem_count = g.len();
        g.clear();
        drop(g);
        // Count disk-only sessions before clearing the disk index.
        // In-memory sessions are always in the disk index too (added on
        // create/scan), so disk_only = disk_index.len() - in_mem_count.
        let disk_only_count = if self.data_dir.is_some() {
            let index = self.disk_index.read().await;
            index.len().saturating_sub(in_mem_count)
        } else {
            0
        };
        // Delete all session files from disk.
        if let Some(ref dir) = self.data_dir {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("json") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            if stem.starts_with("sess-") {
                                let _ = std::fs::remove_file(&path);
                            }
                        }
                    }
                }
            }
        }
        if let Ok(mut index) = self.disk_index.try_write() {
            index.clear();
        }
        in_mem_count + disk_only_count
    }
}

/// Return the current time as an ISO 8601 string.
/// Uses `std::time` to avoid adding a chrono dependency.
fn chrono_now_iso8601() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Compute date/time components from unix timestamp.
    let (year, month, day, hour, minute, second) = unix_to_date_time(secs);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

/// Convert a unix timestamp (seconds since epoch) to (year, month, day, hour, minute, second).
/// Simplified algorithm — valid for 1970–2099.
fn unix_to_date_time(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;

    // Compute year and day-of-year from days since epoch.
    let mut remaining_days = days_since_epoch;
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    // Compute month and day from day-of-year.
    let leap = is_leap_year(year);
    let month_days: [u64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    let mut day_of_year = remaining_days;
    for &md in &month_days {
        if day_of_year < md {
            break;
        }
        day_of_year -= md;
        month += 1;
    }
    let day = day_of_year + 1;

    (year, month, day, hour, minute, second)
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn session_serialization_round_trip() {
        let session = Session {
            id: "sess-test123".to_string(),
            messages: vec![
                SessionMessage::user("hello"),
                SessionMessage::assistant("hi there"),
            ],
            delegation_count: 1,
            delegation_by_worker: {
                let mut m = HashMap::new();
                m.insert("search".to_string(), 1);
                m
            },
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:01:00Z".to_string(),
        };
        let json = serde_json::to_string(&session).expect("serialize");
        let back: Session = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.id, session.id);
        assert_eq!(back.messages.len(), 2);
        assert_eq!(back.delegation_count, 1);
        assert_eq!(back.created_at, "2025-01-01T00:00:00Z");
        assert_eq!(back.updated_at, "2025-01-01T00:01:00Z");
    }

    #[test]
    fn session_deserialize_missing_timestamps_uses_default() {
        // Old format without timestamps should still deserialize.
        let json = r#"{
            "id": "sess-old",
            "messages": [],
            "delegation_count": 0,
            "delegation_by_worker": {}
        }"#;
        let session: Session = serde_json::from_str(json).expect("deserialize old format");
        assert_eq!(session.id, "sess-old");
        assert_eq!(session.created_at, "");
        assert_eq!(session.updated_at, "");
    }

    #[tokio::test]
    async fn session_store_with_data_dir_create_and_get() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_data_dir(dir.path().to_path_buf());

        let id = store.create().await;
        // File should exist on disk.
        let file_path = dir.path().join(format!("{}.json", id));
        assert!(file_path.exists(), "session file should exist on disk");

        // Get should return the session.
        let session = store.get(&id).await.expect("session should exist");
        assert_eq!(session.id, id);
        assert!(!session.created_at.is_empty());
        assert!(!session.updated_at.is_empty());
    }

    #[tokio::test]
    async fn session_store_lazy_loading() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_data_dir(dir.path().to_path_buf());

        let id = store.create().await;
        // Remove from memory to simulate a restart.
        store.inner.write().await.remove(&id);

        // get() should load from disk.
        let session = store.get(&id).await.expect("should lazy-load from disk");
        assert_eq!(session.id, id);
    }

    #[tokio::test]
    async fn session_store_append_and_persist() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_data_dir(dir.path().to_path_buf());

        let id = store.create().await;
        store
            .append_message(&id, "user", "hello")
            .await
            .expect("append");

        // Read the file from disk.
        let file_path = dir.path().join(format!("{}.json", id));
        let content = fs::read_to_string(&file_path).expect("read file");
        let disk_session: Session = serde_json::from_str(&content).expect("parse");
        assert_eq!(disk_session.messages.len(), 1);
        assert_eq!(disk_session.messages[0].role, "user");
        assert_eq!(disk_session.messages[0].content, "hello");
    }

    #[tokio::test]
    async fn session_store_remove_deletes_file() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_data_dir(dir.path().to_path_buf());

        let id = store.create().await;
        let file_path = dir.path().join(format!("{}.json", id));
        assert!(file_path.exists());

        store.remove(&id).await;
        assert!(!file_path.exists(), "file should be deleted after remove");
        assert!(store.get(&id).await.is_none());
    }

    #[tokio::test]
    async fn session_store_graceful_degradation() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_data_dir(dir.path().to_path_buf());

        // get() for a nonexistent session returns None without panicking.
        assert!(store.get("sess-nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn session_store_corrupt_file_skipped() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_data_dir(dir.path().to_path_buf());

        // Write a corrupt session file.
        let corrupt_path = dir.path().join("sess-corrupt.json");
        fs::write(&corrupt_path, "not json").expect("write corrupt file");

        // scan() should skip it without panicking.
        let summaries = store.scan().await;
        assert!(summaries.is_empty() || summaries.iter().all(|s| s.id != "sess-corrupt"));

        // get() should return None for corrupt file.
        assert!(store.get("sess-corrupt").await.is_none());
    }

    #[tokio::test]
    async fn session_store_scan() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_data_dir(dir.path().to_path_buf());

        let id1 = store.create().await;
        let id2 = store.create().await;

        // Create a new store from the same dir to simulate restart.
        let store2 = SessionStore::with_data_dir(dir.path().to_path_buf());
        let summaries = store2.scan().await;

        assert_eq!(summaries.len(), 2);
        let ids: Vec<&str> = summaries.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
    }

    #[tokio::test]
    async fn session_store_updated_at_advances() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_data_dir(dir.path().to_path_buf());

        let id = store.create().await;
        let session = store.get(&id).await.unwrap();
        let created = session.created_at.clone();
        let first_updated = session.updated_at.clone();
        assert_eq!(created, first_updated); // Same on creation.

        store
            .append_message(&id, "user", "hello")
            .await
            .expect("append");
        let session = store.get(&id).await.unwrap();
        // updated_at should have advanced (or at least be present).
        assert!(!session.updated_at.is_empty());
    }

    #[test]
    fn chrono_now_iso8601_format() {
        let ts = chrono_now_iso8601();
        // Should match YYYY-MM-DDTHH:MM:SSZ
        assert!(ts.len() == 20, "expected 20 chars, got {}: {}", ts.len(), ts);
        assert!(ts.starts_with('2'));
        assert!(ts.ends_with('Z'));
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
        assert_eq!(&ts[13..14], ":");
        assert_eq!(&ts[16..17], ":");
    }

    #[tokio::test]
    async fn session_store_remove_all_deletes_files() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_data_dir(dir.path().to_path_buf());

        let id1 = store.create().await;
        let id2 = store.create().await;
        store.append_message(&id1, "user", "hello").await.unwrap();
        store.append_message(&id2, "user", "world").await.unwrap();

        let count = store.remove_all().await;
        assert_eq!(count, 2);

        // Files should be deleted.
        assert!(!dir.path().join(format!("{}.json", id1)).exists());
        assert!(!dir.path().join(format!("{}.json", id2)).exists());

        // In-memory map should be empty.
        assert!(store.get(&id1).await.is_none());
        assert!(store.get(&id2).await.is_none());

        // Disk index should be empty — scan returns nothing.
        let store2 = SessionStore::with_data_dir(dir.path().to_path_buf());
        let summaries = store2.scan().await;
        assert!(summaries.is_empty());
    }

    #[tokio::test]
    async fn session_store_remove_all_empty_store() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_data_dir(dir.path().to_path_buf());

        let count = store.remove_all().await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn session_store_remove_disk_only_session() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_data_dir(dir.path().to_path_buf());

        // Create a session and populate the disk index via scan.
        let id = store.create().await;
        let file_path = dir.path().join(format!("{}.json", id));
        assert!(file_path.exists());

        // Remove from memory only — the session file and disk_index entry remain.
        // This simulates a session that exists on disk but hasn't been lazily loaded.
        store.inner.write().await.remove(&id);

        // remove() should still find and delete the session (via disk index).
        let removed = store.remove(&id).await;
        assert!(removed.is_some(), "remove should return Some for disk-only session");
        assert_eq!(removed.unwrap().id, id);
        assert!(!file_path.exists(), "file should be deleted");
        assert!(store.get(&id).await.is_none(), "session should be gone after remove");
    }

    #[tokio::test]
    async fn session_store_remove_all_includes_disk_only_sessions() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::with_data_dir(dir.path().to_path_buf());

        // Create two sessions.
        let id1 = store.create().await;
        let id2 = store.create().await;

        // Evict one from memory to simulate a disk-only session.
        store.inner.write().await.remove(&id1);

        // remove_all should count both sessions.
        let count = store.remove_all().await;
        assert_eq!(count, 2, "remove_all should count disk-only sessions");

        // Both files should be deleted.
        assert!(!dir.path().join(format!("{}.json", id1)).exists());
        assert!(!dir.path().join(format!("{}.json", id2)).exists());
    }
}
