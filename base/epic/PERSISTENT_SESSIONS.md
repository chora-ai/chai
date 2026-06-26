---
status: complete
---

# Epic: Persistent Sessions

**Summary** — Persist chat sessions to disk so they survive gateway restarts and desktop app restarts, and expose session management (listing, loading, deleting) through the gateway protocol, desktop UI, and CLI. Phases 1–4 are complete; sessions survive gateway restarts, four protocol methods enable session management, the desktop sidebar loads sessions on connect with full management, and CLI subcommands provide offline session management. Hardening remains.

**Status** — **Phase 4 complete.** Sessions are persisted to disk, restored on gateway restart, manageable via four WebSocket protocol methods, fully integrated into the desktop UI, and manageable from the CLI via `chai sessions list`, `chai sessions delete <ID>`, and `chai sessions clear`. Hardening (Phase 5) remains.

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

- **`SessionStore`** (`crates/lib/src/session.rs`) — ~~In-memory `HashMap<SessionId, Session>` with no disk I/O.~~ **Phase 1:** Now supports optional `data_dir: PathBuf` via `with_data_dir()`. When set, sessions are persisted to disk as JSON files with write-through on every mutation and lazy loading on `get()`. `Session` now derives `Serialize, Deserialize`. New `scan()` method returns `SessionSummary` metadata without loading full history. **Phase 2:** New `remove_all()` method clears all sessions from memory and disk, deletes all `sess-*.json` files, clears the disk index, and returns the count of removed sessions. **Phase 3 (bug fix):** `get_or_create` now includes a `load_from_disk` fallback when the in-memory index doesn't contain the ID — the index can become stale when `try_write()` fails due to lock contention (e.g. during `create()`, `remove()`, or `remove_all()`). Without this fallback, `get_or_create` would skip the filesystem and create a new empty session, silently overwriting any existing session file on disk. The stale-index entry is also patched when found, consistent with how `get()` already works.
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

- **`session_messages: BTreeMap<String, Vec<ChatMessage>>`** — Per-session transcript storage. **Phase 3:** Now populated from `sessions.history` on session switch for persisted sessions. Empty assistant messages from the history conversion are skipped to match the live event stream behavior (prevents extra spacing between tool calls and messages).
- **`session_order: Vec<String>`** — MRU-ordered session IDs for the sidebar. **Phase 3:** Now populated from `sessions.list` on gateway connect.
- **`session_summaries: HashMap<String, SessionSummary>`** — Per-session summary metadata (id, timestamps, message count, channel binding). **Phase 3:** Replaces the former `session_meta` HashMap. Populated from `sessions.list` on gateway connect and updated via session events.
- **`SessionSummary`** (desktop types) — **Phase 3:** Desktop-side struct with `id`, `created_at`, `updated_at`, `message_count`, and `channel_binding: Option<ChannelBinding>`. The `channel_meta()` helper returns a display string for the sidebar label — channel-bound sessions show `(channel_id)` only (e.g. `(telegram)`), dropping the `conversation_id` which is an internal identifier.
- **Session sidebar** (`crates/desktop/src/app/ui/sessions.rs`) — **Phase 3:** Lists sessions with timestamp labels (e.g. "Jun 10, 12:34"), short session IDs below, channel binding tags, per-session "×" delete buttons (right-aligned via RTL layout section so labels cannot push them off screen), and a "Clear all sessions" button with stacked confirmation dialog. "New session" button is always visible. All sessions from `session_summaries` are shown (not filtered by `session_messages`).
- **"New session" button** — **Phase 3:** Always visible, regardless of whether a session is active. Clicking it calls `start_new_session()`.
- **Session event handling** — **Phase 3:** `session.deleted` and `sessions.cleared` broadcast events are processed: `session.deleted` removes the session from `session_messages`, `session_order`, and `session_summaries` (switching to "New session" mode if it was the selected session); `sessions.cleared` clears all local session state and switches to "New session" mode.
- **Channel-bound session read-only guard** — **Phase 3:** Clicking a channel-bound session in the sidebar sets `selected_session_id` (for viewing) but not `chat_session_id` (for sending). The `can_send_base` guard checks `chat_session_id.is_some()`, correctly disabling the chat input for channel-bound sessions. This prevents the desktop from sending a message that would cause the gateway's `get_or_create` to create a new empty session overwriting the channel session's history on disk.
- **History loading** — **Phase 3:** When switching to a persisted session not in `session_messages`, a `sessions.history` RPC is triggered. The chat area shows "Loading session history…" while the fetch is in flight. The history conversion emits assistant progress text before tool call entries (matching the live event stream order where `session.assistant_progress` arrives before `session.tool_call` events).
- **Gateway stop** — Clears `chat_session_id` and `chat_messages` but **preserves** `session_messages`, `session_order`, and `session_summaries` in memory. Resets `sessions_list_fetched` so the list is re-fetched on reconnect.

### CLI

- **`chai chat --session <ID>`** — Can resume an existing session by id. **Phase 1:** Now works across gateway restarts since the session is persisted to disk and lazy-loaded by `get_or_create`.
- **`chai sessions list`** — **Phase 4:** New CLI subcommand that reads sessions directly from disk via `SessionStore::scan()`, printing session id, timestamps, message count, and channel binding (if any). Sorted by most recently updated. No gateway connection required. Supports `--profile` to inspect a specific profile's sessions.
- **`chai sessions delete <ID>`** — **Phase 4:** New CLI subcommand that removes a session from disk via `SessionStore::remove()` and its binding via `SessionBindingStore::remove_binding()`. Prints confirmation. No gateway connection required.
- **`chai sessions clear`** — **Phase 4:** New CLI subcommand that removes all sessions and bindings from disk via `SessionStore::remove_all()` and `SessionBindingStore::remove_all()`. Prints count of deleted sessions. No gateway connection required.
- **`GatewayConn`** (`crates/cli/src/gateway_conn.rs`) — **Phase 4:** Extracted the gateway WebSocket connect + auth handshake from `chat.rs` into a reusable struct. `GatewayConn::connect(profile)` establishes an authenticated connection. `GatewayConn::call(method, params)` sends a method request and waits for the matching response. Still used by `chat.rs`, which was refactored to use `GatewayConn` instead of hand-rolling the WebSocket handshake (~100 lines of boilerplate reduced to ~10).
- **Design decision: direct disk access vs. gateway protocol** — **Phase 4:** The CLI session commands operate directly on the session store on disk rather than connecting to the gateway via WebSocket. This makes `--profile` genuinely useful (which profile's sessions to inspect) and removes the gateway dependency entirely — especially convenient for cleanup or inspection when the gateway is stopped. The gateway protocol methods remain for the desktop client.

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

#### Session Sidebar

The session sidebar (`crates/desktop/src/app/ui/sessions.rs`) was enhanced in Phase 3:

1. **Load session list from gateway** — On gateway connect, `sessions.list` is called and `session_order` and `session_summaries` are populated from the response. This replaces the previous behavior of only showing sessions observed via WebSocket events.
2. **Session labels** — `created_at` timestamp is displayed as the primary label (e.g. "Jun 10, 12:34") with the short session ID below in dimmer text. Channel binding info is shown as a small tag — channel-bound sessions display `(channel_id)` only (e.g. `(telegram)`), dropping the `conversation_id` which is an internal identifier not useful in the sidebar. The timestamp-only approach was chosen over first-user-message preview because timestamps are available from `sessions.list` metadata alone and require no additional data loading.
3. **Always-visible "New session" button** — The user can explicitly start a new session at any time, even when a session is active. Clicking it sets `selected_session_id = None` and `chat_session_id = None`, so the next message creates a fresh session.
4. **Delete button** — Per-session "×" button right-aligned via a `ui.with_layout(egui::Layout::right_to_left(...))` section (same pattern as `header.rs`). The RTL section reserves space from the right edge first, so the label is constrained to the remaining width and cannot push the button off screen. Calls `sessions.delete` on the gateway.
5. **"Clear all" button** — At the bottom of the sidebar, a "Clear all sessions" button with a stacked confirmation dialog (warning label vertically above the buttons, fitting the narrow 220px panel). Calls `sessions.delete_all`.

#### Session Loading on Switch

When the user clicks a session in the sidebar that is not in the local `session_messages` map (e.g. a session from a previous gateway run), the desktop:

1. Sets `loading_session_id` and triggers a `sessions.history` RPC.
2. Shows "Loading session history…" in the chat area while the fetch is in flight.
3. Converts the returned `SessionMessage` array to desktop `ChatMessage` objects — empty assistant messages are skipped (matching the live event stream behavior where `session.message` events with empty content are dropped), and assistant progress text is emitted before tool call entries (matching the live event stream order where `session.assistant_progress` arrives before `session.tool_call` events).
4. Populates `session_messages[session_id]` with the converted messages.

This is a lazy-load pattern: sessions are listed with metadata only, and full history is loaded on demand when the user selects a session.

#### Channel-Bound Session Read-Only Guard

Clicking a channel-bound session in the sidebar sets `selected_session_id` (for viewing) but not `chat_session_id` (for sending). The `can_send_base` guard checks `chat_session_id.is_some()`, correctly disabling the chat input for channel-bound sessions. This prevents the desktop from sending a message that would cause the gateway's `get_or_create` to create a new empty session, overwriting the channel session's history on disk.

#### Session Event Processing

The `poll_session_events` handler now processes `session.deleted` and `sessions.cleared` broadcast events in addition to the existing real-time message events. `session.deleted` removes the session from `session_messages`, `session_order`, and `session_summaries`, switching to "New session" mode if it was the selected session. `sessions.cleared` clears all local session state and switches to "New session" mode. The RPC result handlers for delete/clear do not duplicate the event handler's work — the broadcast event is the authoritative cleanup path.

### CLI Session Commands

**Phase 4** added `chai sessions list`, `chai sessions delete <ID>`, and `chai sessions clear` CLI subcommands that operate directly on the session store on disk — no gateway connection required.

**Design decision: direct disk access vs. gateway protocol.** The initial implementation connected to the gateway via WebSocket and called the `sessions.list`, `sessions.delete`, and `sessions.delete_all` protocol methods. This was the simplest path since the methods already existed for the desktop client. However, it had two significant problems:

1. **`--profile` flag was misleading** — The flag resolved the gateway's bind address and port from the config, but the CLI could only connect to a *running* gateway. Since only one gateway runs at a time (on a specific profile), `--profile` had to match the running gateway or you'd just get a connection error. The help text literally said "must match the running gateway's profile" — admitting the flag had no practical flexibility.

2. **Required a running gateway unnecessarily** — Sessions are write-through persisted to disk as JSON files (`sess-<uuid>.json`) under `<profile_dir>/agents/<orchestrator_id>/sessions/`, with bindings in `bindings.json`. The data is always on disk and up-to-date. Requiring the gateway to be running just to list or delete sessions is an artificial constraint — especially inconvenient for cleanup or inspection when the gateway is stopped.

The revised implementation opens the `SessionStore` and `SessionBindingStore` directly, using the same `sessions_dir()` resolution that the gateway uses. This makes `--profile` genuinely useful (which profile's sessions to inspect, same semantics as `chai gateway --profile`) and removes the gateway dependency entirely. The gateway protocol methods remain for the desktop client.

**Implementation details:**

- A `sessions` subcommand module was added under `crates/cli` alongside the existing `chat` command, with three clap subcommands (`List`, `Delete`, `Clear`), each carrying an optional `--profile` flag.
- `lib::config::load_config(profile)` resolves the profile, then `lib::config::sessions_dir()` locates the session data directory — same resolution path as the gateway.
- `lib::session::SessionStore::with_data_dir()` and `lib::routing::SessionBindingStore::with_data_dir()` open the stores directly.
- The gateway WebSocket connect + auth handshake was extracted from `chat.rs` into a reusable `GatewayConn` struct (`crates/cli/src/gateway_conn.rs`). `chat.rs` was refactored to use `GatewayConn`, reducing ~100 lines of boilerplate to ~10. The three gateway protocol methods remain for the desktop client — this is CLI-only.

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
- [x] **Desktop session list** — On gateway connect, the desktop loads the session list from `sessions.list` and populates the sidebar.
- [x] **Desktop session labels** — Session sidebar entries display timestamps and/or first-message previews instead of raw UUIDs.
- [x] **Desktop "New session" button** — Always accessible, allowing the user to start a new session at any time.
- [x] **Desktop session loading** — Clicking a session in the sidebar loads its full history via `sessions.history` if not already in memory.
- [x] **Desktop session deletion** — Per-session delete action in the sidebar, calling `sessions.delete`.
- [x] **Desktop "Clear all" action** — A "Clear all sessions" button with confirmation, calling `sessions.delete_all`.
- [x] **CLI `chai sessions list`** — List sessions for the active profile, directly from disk. No gateway required.
- [x] **CLI `chai sessions delete <ID>`** — Delete a session by id, directly from disk. No gateway required.
- [x] **CLI `chai sessions clear`** — Delete all sessions, directly from disk. No gateway required.

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
| 3. Desktop session management | Load session list on connect (sidebar is empty after restart), session history rendering (message history not displayed for persisted sessions), session labels, "New session" button, session loading on switch, delete and clear actions | Complete |
| 4. CLI session management | `chai sessions list`, `chai sessions delete`, `chai sessions clear` subcommands, `GatewayConn` refactor | Complete |
| 5. Hardening | Atomic writes verification, startup performance with many sessions, error recovery for corrupt files, integration tests | Not started |

## Open Questions

- **Session file compaction** — Over time, session files may grow large. Should there be a mechanism to compact or truncate old messages in the on-disk file (separate from the in-memory session history model context limit)?

### Resolved Open Questions

- **Session title generation** — **Resolved (Phase 3):** Deferred to a future phase. Using `createdAt` timestamps as labels for now — timestamps are available from `sessions.list` metadata alone and require no additional data loading or server changes. Auto-generated titles remain a future consideration.
- **Per-agent sessions for workers** — **Resolved (Phase 3):** Deferred. Only the orchestrator's sessions are shown in the desktop. The directory structure already supports per-agent sessions if needed later.

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
