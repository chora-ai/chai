---
status: in-progress
---

# Epic: Persistent Sessions

**Summary** — Persist chat sessions to disk so they survive gateway restarts and desktop app restarts, and expose session management (listing, loading, deleting) through the gateway protocol and desktop UI. Phases 1–2 are complete; sessions and bindings survive gateway restarts, and four protocol methods enable session discovery, inspection, and deletion. Desktop integration and CLI commands remain.

**Status** — **Phase 2 complete.** Sessions are persisted to disk, restored on gateway restart, and manageable via four WebSocket protocol methods (`sessions.list`, `sessions.history`, `sessions.delete`, `sessions.delete_all`). Desktop integration (Phase 3) and CLI commands (Phase 4) remain.

## Problem Statement

Today, **`SessionStore`** (`crates/lib/src/session.rs`) holds all sessions in an **`Arc<RwLock<HashMap<SessionId, Session>>>`** — a purely in-memory data structure. When the gateway restarts, every session is lost. When the desktop application is closed and reopened, it clears its local `session_messages` and cannot recover previous conversations. The gateway does not expose a method to list sessions, load a previous session by id, or delete sessions. The desktop sidebar only shows sessions that were created during the current app lifetime, using raw UUIDs as labels.

This creates several user-facing problems:

1. **Lost context on restart** — A gateway restart (config change, update, crash) destroys all conversation history. Users cannot pick up where they left off.
2. **No session continuity** — Switching between sessions requires that both sessions were created within the same gateway lifetime. A session from a previous gateway run is irrecoverable.
3. **No session management** — Users cannot delete stale sessions, clear history, or organize sessions. Sessions accumulate in memory until the process exits.
4. **No session metadata** — Sessions carry no title, creation timestamp, or last-active timestamp, making it impossible to display a useful session list even if persistence existed.

## Goal

- **Durability** — Conversation history survives gateway restarts and desktop application restarts. A user who restarts the gateway can continue a previous session.
- **Session continuity** — A user can send a message in session A, switch to session B, send a message there, then switch back to session A and continue. This works both within a single gateway run and across restarts.
- **Discoverability** — Users can list previous sessions and select one to resume, from both the CLI and the desktop application.
- **Management** — Users can delete individual sessions, clear all sessions, or clear the sessions directory on disk to reset history.
- **Per-agent isolation** — Sessions are stored per agent under the agent's context directory (`<profile>/agents/<agentId>/sessions/`), keeping orchestrator and worker sessions separate.
- **Minimal disruption** — The default behavior (new session per conversation) remains the same. Persistence is transparent: sessions are saved automatically and loaded on demand.

## Current State

### Server-Side Session Storage

- **`SessionStore`** (`crates/lib/src/session.rs`) — ~~In-memory `HashMap<SessionId, Session>` with no disk I/O.~~ **Phase 1:** Now supports optional `data_dir: PathBuf` via `with_data_dir()`. When set, sessions are persisted to disk as JSON files with write-through on every mutation and lazy loading on `get()`. `Session` now derives `Serialize, Deserialize`. New `scan()` method returns `SessionSummary` metadata without loading full history. **Phase 2:** New `remove_all()` method clears all sessions from memory and disk, deletes all `sess-*.json` files, clears the disk index, and returns the count of removed sessions.
- **`Session`** struct — Fields: `id: SessionId`, `messages: Vec<SessionMessage>`, `delegation_count: usize`, `delegation_by_worker: HashMap<String, usize>`. **Phase 1:** Added `created_at: String` and `updated_at: String` (ISO 8601 timestamps with `#[serde(default)]`).
- **`SessionMessage`** struct — Fields: `role: String`, `content: String`, `tool_calls: Option<Vec<ToolCall>>`, `tool_name: Option<String>`. Derives `Serialize, Deserialize`.
- **`SessionBindingStore`** (`crates/lib/src/routing.rs`) — ~~Bidirectional in-memory mapping between `(channel_id, conversation_id)` and `session_id`. Also lost on restart.~~ **Phase 1:** Now supports optional `data_dir: PathBuf` via `with_data_dir()`. When set, bindings are persisted to `bindings.json` on every mutation. New `remove_binding()` and `load_from_disk()` methods. `ChannelConvKey` now derives `Serialize, Deserialize`. **Phase 2:** New `remove_all()` method clears both in-memory maps and rewrites `bindings.json` as an empty array.

### Agent Context Directories

- **`agent_context_dir(profile_dir, agent_id)`** (`crates/lib/src/config.rs`) — Returns `<profile_dir>/agents/<agent_id>/`.
- **`orchestrator_context_dir()`** — Resolves to `<profile>/agents/<orchestratorId>/` (defaults to `"orchestrator"`).
- **`worker_context_dir()`** — Resolves to `<profile>/agents/<workerId>/` (returns `None` if worker id is empty).
- **`sessions_dir(profile_dir, agent_id)`** (`crates/lib/src/config.rs`) — **Phase 1:** New helper returning `<profile_dir>/agents/<agent_id>/sessions/`.

### Gateway Protocol

- **`agent`** method — Accepts optional `sessionId`. If absent, creates a new session. If present, resumes via `get_or_create`. **Phase 1:** `get_or_create` now lazy-loads persisted sessions from disk, so `sessionId` can reference a session from a previous gateway run.
- **`send`** method — Delivers message to a channel conversation; creates/reuses session via `SessionBindingStore`. **Phase 1:** `process_inbound_message` now calls `session_store.get()` to ensure the session is loaded before appending, and creates a new session if the bound session no longer exists on disk.
- **`sessions.list`** method — **Phase 2:** Returns summary metadata for all sessions (id, timestamps, message count, optional channel binding), sorted by `updatedAt` descending. Rescans the sessions directory on each call so the disk is the source of truth.
- **`sessions.history`** method — **Phase 2:** Returns full message history for a given session id, with optional `limit` and `offset` pagination. Messages are serialized with camelCase keys (`toolCalls`, `toolName`) for the wire protocol. Returns an error for nonexistent sessions.
- **`sessions.delete`** method — **Phase 2:** Deletes a session from memory and disk, removes the associated binding, and broadcasts a `session.deleted` event. Returns an error for nonexistent sessions.
- **`sessions.delete_all`** method — **Phase 2:** Deletes all sessions from memory and disk, clears all bindings, and broadcasts a `sessions.cleared` event. Returns the count of deleted sessions.
- **Session events** — **Phase 2:** `session.deleted` (payload: `{ "sessionId": "..." }`) broadcast after `sessions.delete`; `sessions.cleared` (empty payload) broadcast after `sessions.delete_all`.

### Desktop Application

- **`session_messages: BTreeMap<String, Vec<ChatMessage>>`** — Per-session transcript storage, entirely in-memory. Cleared implicitly when the app closes. **Phase 2:** The `sessions.history` protocol method now exists for loading history from the gateway; desktop integration (calling it on session switch) is Phase 3.
- **`session_order: Vec<String>`** — MRU-ordered session IDs for the sidebar. Only populated from events observed during the current app session. **Phase 2:** The `sessions.list` protocol method now exists for discovering sessions on gateway connect; desktop integration (calling it on connect) is Phase 3.
- **`session_meta: HashMap<String, (Option<String>, Option<String>)>`** — Per-session channel metadata. Not persisted.
- **Session sidebar** (`crates/desktop/src/app/ui/sessions.rs`) — Lists sessions in MRU order. Labels are raw session IDs (e.g. `sess-a1b2c3...`) optionally with channel metadata. No delete, rename, or clear buttons.
- **"New session" button** — Only visible when `chat_session_id` is `None` (before the first message or after a gateway restart clears it). No way to explicitly start a new session once one exists.
- **Gateway stop** — Clears `chat_session_id` and `chat_messages` but **preserves** `session_messages`, `session_order`, and `session_meta` in memory.

### CLI

- **`chai chat --session <ID>`** — Can resume an existing session by id. **Phase 1:** Now works across gateway restarts since the session is persisted to disk and lazy-loaded by `get_or_create`.

## Scope

### In Scope

- **Disk persistence for sessions** — Save session messages and metadata to per-agent `sessions/` directories under the profile. Load on demand when a session is resumed. Write-through or periodic flush on every message append.
- **Session metadata** — Add `created_at` and `updated_at` timestamps to `Session`. Derive `Serialize, Deserialize` on `Session` for JSON serialization.
- **Gateway protocol methods** — Add `sessions.list`, `sessions.history`, and `sessions.delete` WebSocket methods so clients can discover, inspect, and manage sessions.
- **Session restoration on gateway start** — Load persisted sessions into `SessionStore` on gateway startup (or lazy-load on demand) so existing `agent` calls with `sessionId` resume seamlessly.
- **Desktop session sidebar enhancements** — Load session list from gateway on connect. Display session titles or first-message previews and timestamps. Add "New session" button (always accessible). Add delete (individual) and "Clear all" actions.
- **Session binding restoration** — Persist `SessionBindingStore` mappings so that channel-conversation → session routing survives restarts.
- **Manual cleanup** — Users can delete the `sessions/` directory on disk to clear all session history. This is the zero-config escape hatch.

### Out of Scope

- **Session search** — Full-text search across session transcripts (future consideration).
- **Session export/import** — Exporting sessions to standalone files or importing from other tools.
- **Cross-profile session sharing** — Sessions are profile-local and per-agent; no sharing between profiles.
- **Session summarization** — Auto-summarizing old sessions or truncating history (the existing session history config already handles context window limits).
- **Channel-specific session UX** — Delete/manage sessions from Telegram, Matrix, or Signal (desktop and CLI only for now).
- **Encryption at rest** — Session files are stored as plain JSON on disk. Encryption is a future hardening step.

## Design

### Storage Layout

Sessions are stored per agent under the agent's context directory:

```
~/.chai/profiles/<profile>/
├── agents/
│   ├── orchestrator/
│   │   ├── AGENT.md                         # existing
│   │   └── sessions/                        # new
│   │       ├── sess-a1b2c3d4.json           # one file per session
│   │       ├── sess-e5f6g7h8.json
│   │       └── bindings.json                # session binding store
│   └── <worker-id>/
│       ├── AGENT.md
│       └── sessions/
│           └── ...
```

**Design decisions:**

- **One file per session** — A single JSON file per session keeps reads and writes proportional to the session being accessed, not the total number of sessions. A monolithic store would require reading/writing the entire history on any change.
- **Named by session id** — The filename is `{session_id}.json`, making it trivial to locate a session by id. The `sess-` prefix in the id ensures filenames are human-recognizable.
- **`bindings.json` alongside sessions** — The `SessionBindingStore` mappings are persisted per agent. On gateway start, bindings are loaded alongside sessions so that inbound channel messages route to the correct session.
- **Directory-level deletion** — Deleting the `sessions/` directory (or individual `.json` files within it) is a valid cleanup mechanism. The gateway should handle missing files gracefully (treat as "session not found").

### Session File Format

Each session file contains the serialized `Session` struct:

```json
{
  "id": "sess-a1b2c3d4-...",
  "messages": [
    { "role": "user", "content": "Hello" },
    { "role": "assistant", "content": "Hi! How can I help?" }
  ],
  "delegation_count": 0,
  "delegation_by_worker": {},
  "created_at": "2025-06-10T12:34:56Z",
  "updated_at": "2025-06-10T12:35:01Z"
}
```

**`created_at`** and **`updated_at`** are new fields added to `Session`. They are ISO 8601 timestamps set when the session is created and updated on every message append, respectively. Adding these requires deriving `Serialize, Deserialize` on `Session` (which currently only derives `Debug, Clone`).

### Session Store Refactor

The current `SessionStore` is an in-memory `HashMap` with no disk awareness. The refactored store adds a persistence layer:

1. **`SessionStore` gains a `data_dir: PathBuf`** — The `sessions/` directory for the agent it belongs to. Set at construction time via `SessionStore::with_data_dir(data_dir)`. The existing `SessionStore::new()` (no data_dir, no disk I/O) is unchanged for tests and non-persistent contexts.
2. **On `create()`** — Create the session in memory **and** write the initial JSON file to disk. Create the `sessions/` directory if it does not exist.
3. **On `get_or_create()`** — If creating a new session, write to disk. If resuming an existing in-memory session, no disk write. If the id is not in memory but the file exists on disk, load it (lazy load).
4. **On `append_message_full()` / `record_delegation()`** — Update the in-memory session **and** write the updated session file to disk. Use atomic writes (write to `.tmp`, then rename) to avoid corruption on crash. The `updated_at` timestamp is advanced on every write.
5. **On `get()`** — Return from memory if present. If not in memory but the file exists on disk, load it, insert into the HashMap, update `updated_at`, and return. This enables lazy loading.
6. **On `remove()`** — Remove from memory **and** delete the file from disk.
7. **On `remove_all()`** — **Phase 2:** Clear all sessions from the in-memory map, delete all `sess-*.json` files from `data_dir`, clear the disk index, and return the count of removed sessions.
8. **On gateway start** — Call `scan()` to scan the `sessions/` directory for `.json` files and read metadata only (id, timestamps, message count) without loading full message history. This populates a metadata index that enables lazy loading: `get()` can check the index to see if a session exists on disk before attempting to load it.
9. **New `SessionSummary` struct** — Lightweight summary returned by `scan()`: `id`, `created_at`, `updated_at`, `message_count`.
10. **New `config::sessions_dir()` helper** — `sessions_dir(profile_dir, agent_id) -> PathBuf` returns `<profile_dir>/agents/<agent_id>/sessions/`. Kept alongside `orchestrator_context_dir()` and `worker_context_dir()`.

**Why write-through instead of periodic flush?** Every message append is a state change that the user expects to be durable. A periodic flush risks losing the last few messages on a crash. The per-file granularity means writes are small (one session's data), so the I/O cost is acceptable.

### Session Binding Persistence

**`SessionBindingStore`** currently holds `(channel_id, conversation_id) ↔ session_id` mappings in memory. To restore routing after a restart:
- **`SessionBindingStore::with_data_dir(data_dir)`** — Constructor that sets the data directory and loads `bindings.json` from disk if it exists. The existing `SessionBindingStore::new()` (no data_dir, no disk I/O) is unchanged for tests and non-persistent contexts.
- **`bindings.json`** is stored in the agent's `sessions/` directory alongside session files. The format is a JSON array of `{ "channel_id", "conversation_id", "session_id" }` objects — a `Vec` rather than a `HashMap`, since `ChannelConvKey` is a composite key and serializing it as a HashMap key would require string interpolation or custom serde logic.
- **`ChannelConvKey`** derives `Serialize, Deserialize` to enable JSON serialization.
- **On `bind()`** — Update in-memory map **and** write `bindings.json` to disk (atomic write: `.tmp` then rename).
- **On `remove_binding()`** — Removes a binding by session_id from both in-memory maps and rewrites `bindings.json` to disk. Used by the `/new` session trigger cleanup and by `sessions.delete` (Phase 2).
- **On `remove_all()`** — **Phase 2:** New method that clears both in-memory maps and rewrites `bindings.json` as an empty array. Used by `sessions.delete_all`.
- **On gateway start** — `load_from_disk()` is called at construction time by `with_data_dir()`, populating the in-memory maps from `bindings.json`.
- **Stale binding handling** — If `bindings.json` references a session whose file was deleted from disk, `process_inbound_message` detects this (session not found via `session_store.get()`), creates a new session, and rebinds the channel conversation. The old binding is overwritten.
- **Graceful degradation** — If `bindings.json` is missing, the store starts with empty bindings. If it's corrupt, log a warning and start empty. Channel conversations will create new sessions on their next inbound message, which is the current behavior anyway.

### Gateway Integration

The gateway wires the persistent stores into the runtime:

- **`GatewayState` construction** — `SessionStore` and `SessionBindingStore` are constructed with `with_data_dir()`, passing the orchestrator's `sessions/` directory path (`orchestrator_context_dir.join("sessions")`). The sessions directory path is computed once during `run_gateway()` and passed to the store constructors; `GatewayState` does not store a `profile_dir` field.
- **Startup scan** — After constructing `GatewayState`, call `session_store.scan()` to populate a metadata index. This enables lazy loading: `get()` can check the index to see if a session exists on disk before attempting to load it.
- **Inbound message session resolution** — When `process_inbound_message` resolves a binding to a session ID, it calls `session_store.get()` to ensure the session is loaded in memory (lazy-load from disk). If the session no longer exists on disk (deleted or corrupt), a new session is created and the binding is updated. This fix was required because the previous code passed the session ID directly to `append_message`, which failed after a restart since the in-memory store was empty while the binding still referenced the old session.

### Gateway Protocol Additions

Four new WebSocket methods:

#### `sessions.list`

List all persisted sessions for the active profile's agents.

```json
{
  "type": "req",
  "id": "1",
  "method": "sessions.list",
  "params": {}
}
```

Response:

```json
{
  "type": "res",
  "id": "1",
  "ok": true,
  "payload": {
    "sessions": [
      {
        "id": "sess-a1b2c3d4",
        "createdAt": "2025-06-10T12:34:56Z",
        "updatedAt": "2025-06-10T12:35:01Z",
        "messageCount": 5,
        "channelBinding": { "channelId": "telegram", "conversationId": "123" }
      }
    ]
  }
}
```

Returns summary metadata (no full message history) for each session, sorted by `updatedAt` descending (most recent first). Includes channel binding info if present (field omitted when no binding exists).

#### `sessions.history`

Retrieve the full message history for a specific session.

```json
{
  "type": "req",
  "id": "2",
  "method": "sessions.history",
  "params": { "sessionId": "sess-a1b2c3d4" }
}
```

Response:

```json
{
  "type": "res",
  "id": "2",
  "ok": true,
  "payload": {
    "id": "sess-a1b2c3d4",
    "messages": [ ... ],
    "createdAt": "2025-06-10T12:34:56Z",
    "updatedAt": "2025-06-10T12:35:01Z"
  }
}
```

Supports optional `limit` and `offset` params for pagination (useful for very long sessions).

#### `sessions.delete`

Delete a session by id.

```json
{
  "type": "req",
  "id": "3",
  "method": "sessions.delete",
  "params": { "sessionId": "sess-a1b2c3d4" }
}
```

Response:

```json
{
  "type": "res",
  "id": "3",
  "ok": true,
  "payload": { "deleted": true }
}
```

Removes the session from memory, deletes the file from disk, removes any associated binding entry, and broadcasts a `session.deleted` event.

#### `sessions.delete_all`

Delete all sessions for the active profile.

```json
{
  "type": "req",
  "id": "4",
  "method": "sessions.delete_all",
  "params": {}
}
```

Response:

```json
{
  "type": "res",
  "id": "4",
  "ok": true,
  "payload": { "deletedCount": 12 }
}
```

Clears all sessions from memory, deletes all session files and `bindings.json` from disk, and broadcasts a `sessions.cleared` event.

#### Session Events

Two server-sent events notify clients of session deletion in real time, so desktop clients can update their local state without polling:

| Event | Payload | When |
|-------|---------|------|
| `session.deleted` | `{ "sessionId": "..." }` | After `sessions.delete` succeeds |
| `sessions.cleared` | `{}` | After `sessions.delete_all` succeeds |

### Desktop Application Changes

#### Session Sidebar Enhancements

The session sidebar (`crates/desktop/src/app/ui/sessions.rs`) needs several improvements:

1. **Load session list from gateway** — On gateway connect, call `sessions.list` and populate `session_order` and `session_meta` from the response. This replaces the current behavior of only showing sessions observed via WebSocket events.
2. **Session labels** — Display `created_at` timestamp (e.g. "Jun 10, 12:34") alongside the session ID. Optionally show the first user message as a preview line.
3. **Always-visible "New session" button** — Allow the user to explicitly start a new session at any time, even when a session is active. Clicking it sets `selected_session_id = None` and `chat_session_id = None`, so the next message creates a fresh session.
4. **Delete button** — Per-session delete affordance (e.g. a small "×" or right-click context menu). Calls `sessions.delete` on the gateway and removes the session from local state.
5. **"Clear all" button** — At the bottom of the sidebar, a "Clear all sessions" button that calls `sessions.delete_all`. Requires confirmation.

#### Session Loading on Switch

When the user clicks a session in the sidebar that is not in the local `session_messages` map (e.g. a session from a previous gateway run), the desktop should:

1. Call `sessions.history` with the session id.
2. Convert the returned `SessionMessage` array to desktop `ChatMessage` objects.
3. Populate `session_messages[session_id]` with the converted messages.
4. Set `selected_session_id` to the session id.

This is a lazy-load pattern: sessions are listed with metadata only, and full history is loaded on demand when the user selects a session.

#### Session Event Processing

The existing `poll_session_events` logic continues to work for real-time updates. The key change is that session events can now arrive for sessions that were restored from disk. The deduplication logic already handles this (checks for existing messages with the same role+content), so no changes should be needed there.

#### Phase 1 Testing Findings

Manual testing of Phase 1 identified two desktop issues that need to be addressed in Phase 3:

1. **Session sidebar is empty after gateway restart** — After restarting the gateway, the desktop sidebar shows no sessions, even though persisted sessions exist on disk. The desktop only populates `session_order` from events observed during the current app session; it does not call any gateway method to discover existing sessions. Fix: on gateway connect, call `sessions.list` and populate the sidebar from the response.

2. **Message history not displayed for persisted sessions** — When selecting a session in the desktop that was persisted from a previous gateway run, the chat area does not show the message history, even though the session JSON file on disk contains the full conversation. The desktop's `session_messages` map is empty for sessions not observed in the current app session, and there is no mechanism to load history from the gateway. Fix: call `sessions.history` when the user selects a session whose messages are not in the local `session_messages` map.

**Phase 2 resolved:** The `sessions.list` and `sessions.history` protocol methods now exist (Phase 2). These issues are now desktop-side integration tasks for Phase 3.

### CLI Changes

- **`chai chat`** — On startup, optionally list recent sessions (from `sessions.list`) and allow the user to select one to resume.
- **`chai chat --session <ID>`** — Works as today, but the session id can now refer to a persisted session from a previous run.
- **`chai sessions list`** — New subcommand to list sessions for the active profile.
- **`chai sessions delete <ID>`** — New subcommand to delete a session.
- **`chai sessions clear`** — New subcommand to delete all sessions.

### Multi-Agent Session Ownership

Each agent (orchestrator, workers) has its own `sessions/` directory. When the gateway processes an `agent` request:

1. The orchestrator's session is stored in `<profile>/agents/<orchestratorId>/sessions/`.
2. Worker delegation runs through the orchestrator's session — the `delegate_task` result is appended to the orchestrator's session transcript. Workers do not maintain independent sessions in the current architecture (worker turns build `[system?, user(instruction)]` only and results are merged by the orchestrator). Therefore, **only the orchestrator's `sessions/` directory is populated** in the initial implementation.
3. If a future design gives workers their own persistent sessions, the per-agent directory structure already supports it.

### Migration and Backward Compatibility

Chai has not reached v0.1.0, so backward compatibility is not a concern per project conventions. There is no existing on-disk session format to migrate from. If a `sessions/` directory does not exist, the gateway creates it on first session creation — zero migration path.

### Concurrency and I/O Considerations

- **Atomic writes** — Session files are written to a `.tmp` file and renamed to the final path. This prevents corrupt files if the gateway crashes mid-write.
- **File-level locking is not needed** — The `SessionStore` already serializes access via `RwLock`. All disk writes happen inside the same write guard that updates the in-memory HashMap, so there is no risk of concurrent writes to the same file.
- **Lazy loading** — On gateway start, only session metadata (id, timestamps, message count) is loaded. Full message history is loaded on first `get()`. This keeps startup fast. Lazy loading is chosen over eager loading for scalability; this can be revisited in Phase 5 (Hardening) if performance data shows eager loading would be better.
- **Large sessions** — Sessions with many messages produce larger JSON files. The existing session history config limits what is sent to the model but does not limit what is stored. No truncation of on-disk history is proposed in this epic.
- **Concurrent gateway instances** — The existing `gateway.lock` (`acquire_gateway_lock` in `profile.rs`) uses an advisory exclusive flock to prevent two gateway processes from running against the same profile. File-level concurrency for session files is therefore not a concern — the lock guarantees single-gateway access. If the lock is removed or weakened in the future, session file writes would need a concurrency strategy.

## Requirements

### Functional

- [x] **Session persistence** — Every session created by the gateway is written to disk as a JSON file under `<profile>/agents/<agentId>/sessions/`. Messages, delegation counters, and timestamps are persisted.
- [x] **Session restoration** — On gateway start, persisted sessions are discoverable and loadable. The `agent` method with a `sessionId` that refers to a persisted session resumes that session's history.
- [x] **Session metadata** — `Session` includes `created_at` and `updated_at` timestamps. `Session` derives `Serialize, Deserialize`.
- [x] **`sessions.list` protocol method** — Returns summary metadata for all sessions (id, timestamps, message count, channel binding), sorted by most recently updated.
- [x] **`sessions.history` protocol method** — Returns full message history for a given session id, with optional pagination.
- [x] **`sessions.delete` protocol method** — Deletes a session from memory and disk, removes associated bindings.
- [x] **`sessions.delete_all` protocol method** — Deletes all sessions for the active profile from memory and disk.
- [x] **Binding persistence** — `SessionBindingStore` mappings are persisted to `bindings.json` and restored on gateway start.
- [x] **Directory auto-creation** — The `sessions/` directory is created automatically on gateway start (empty if no persisted sessions exist). Session files are created within it on first session creation.
- [x] **Manual cleanup** — Deleting the `sessions/` directory on disk is a valid way to clear all session history. The gateway handles missing files gracefully.
- [ ] **Desktop session list** — On gateway connect, the desktop loads the session list from `sessions.list` and populates the sidebar.
- [ ] **Desktop session labels** — Session sidebar entries display timestamps and/or first-message previews instead of raw UUIDs.
- [ ] **Desktop "New session" button** — Always accessible, allowing the user to start a new session at any time.
- [ ] **Desktop session loading** — Clicking a session in the sidebar loads its full history via `sessions.history` if not already in memory.
- [ ] **Desktop session deletion** — Per-session delete action in the sidebar, calling `sessions.delete`.
- [ ] **Desktop "Clear all" action** — A "Clear all sessions" button with confirmation, calling `sessions.delete_all`.
- [ ] **CLI `chai sessions list`** — List sessions for the active profile.
- [ ] **CLI `chai sessions delete <ID>`** — Delete a session by id.
- [ ] **CLI `chai sessions clear`** — Delete all sessions.

### Non-functional

- [x] **Startup performance** — Gateway startup with many persisted sessions should not degrade significantly. Lazy-load message history; only scan metadata on start.
- [x] **Write safety** — Session files use atomic writes (write to temp file, rename) to prevent corruption on crash.
- [x] **Graceful degradation** — Missing or corrupt session files are logged and skipped, not fatal. Missing `bindings.json` is treated as empty bindings.
- [x] **No cross-profile leakage** — Sessions are stored under the profile directory and never accessed by another profile.

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1. Core persistence | Add `Serialize`/`Deserialize` on `Session`, timestamps, `sessions/` directory layout, write-through `SessionStore`, binding persistence, gateway start restoration | Complete |
| 2. Protocol methods | `sessions.list`, `sessions.history`, `sessions.delete`, `sessions.delete_all` WebSocket methods | Complete |
| 3. Desktop session management | Load session list on connect (sidebar is empty after restart), session history rendering (message history not displayed for persisted sessions), session labels, "New session" button, session loading on switch, delete and clear actions | Not started |
| 4. CLI session management | `chai sessions list`, `chai sessions delete`, `chai sessions clear` subcommands | Not started |
| 5. Hardening | Atomic writes verification, startup performance with many sessions, error recovery for corrupt files, integration tests | Not started |

## Open Questions

- **Session title generation** — Should the gateway auto-generate a session title (e.g. from the first user message or via an LLM call) and store it as metadata? This would improve the sidebar display but adds complexity. A simpler first step is to use the timestamp and first-user-message preview.
- **Per-agent sessions for workers** — Workers currently do not maintain independent sessions. If a future design adds worker sessions, the directory structure supports it. Should this be explicitly designed for now, or deferred?
- **Session file compaction** — Over time, session files may grow large. Should there be a mechanism to compact or truncate old messages in the on-disk file (separate from the in-memory session history model context limit)?

## Follow-ups

### Session Search

Full-text search across session transcripts. Would require an index or scan-based approach. Not in scope for this epic but the per-file storage layout makes `grep`-based search trivial for CLI users.

### Session Export/Import

Export sessions to standalone formats (Markdown, JSON) for sharing or backup. Import from external tools.

### Encryption at Rest

Encrypt session files on disk so that conversation history is not readable without the gateway's credentials. Relevant for multi-user or shared-host deployments.

### Session Summarization

Auto-summarize old sessions to reduce context window pressure when resuming very old conversations. The existing session history truncation handles the model context window, but the user-visible history could benefit from summarization.

## Related Epics and Docs

- [PROFILES.md](../spec/PROFILES.md) — Profile directory layout, `ChaiPaths`, and the `agents/` directory structure where sessions will be stored.
- [CHANNELS.md](../spec/CHANNELS.md) — Session binding and inbound processing; `SessionBindingStore` routing that needs persistence.
- [CONTEXT.md](../spec/CONTEXT.md) — Session vs turn semantics, session history, how session history is loaded per turn.
- [ORCHESTRATION.md](../spec/ORCHESTRATION.md) — Delegation and session-scoped policy caps (`maxDelegationsPerSession`).
- [DESKTOP.md](../spec/DESKTOP.md) — Desktop session event processing and chat timeline rendering.
- [TOOL_APPROVAL.md](TOOL_APPROVAL.md) — Tool call approval epic (draft); split-turn persistence would benefit from the session persistence layer this epic introduces.
- [RUNTIME_PROFILES.md](../adr/RUNTIME_PROFILES.md) — ADR on profile switching; notes that session state is torn down on restart (this epic addresses that gap).
