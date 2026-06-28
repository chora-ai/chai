---
status: stable
---

# Sessions

This document specifies how **chat sessions** are persisted to disk, discovered, loaded, and managed through the gateway protocol, desktop UI, and CLI. It covers the storage layout, session file format, session store behavior, binding persistence, and all session-related operations. For how session history is loaded per turn, see [CONTEXT.md](CONTEXT.md). For session binding and inbound message routing, see [CHANNELS.md](CHANNELS.md). For the profile directory structure where sessions live, see [PROFILES.md](PROFILES.md).

## Storage Layout

Sessions are stored per agent under the agent's context directory:

```text
~/.chai/profiles/<profile>/
├── agents/
│   ├── orchestrator/
│   │   ├── AGENT.md
│   │   └── sessions/
│   │       ├── sess-a1b2c3d4.json
│   │       ├── sess-e5f6g7h8.json
│   │       └── bindings.json
│   └── <worker-id>/
│       ├── AGENT.md
│       └── sessions/
│           └── ...
```

- **One file per session** — Each session is stored as `{session_id}.json`. The `sess-` prefix in session IDs makes filenames human-recognizable.
- **`bindings.json` alongside sessions** — Session binding mappings are persisted in the same directory.
- **Per-orchestrator session stores** — Each orchestrator has its own `sessions/` directory, populated with its own `SessionStore` at `<profile_dir>/agents/<orchestrator_id>/sessions/`. Sessions from one orchestrator are completely separate from another — switching orchestrators switches session stores.

## Session File Format

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

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | Session identifier (e.g. `sess-<uuid>`) |
| `messages` | `array` | Ordered message history (user, assistant with `tool_calls`, tool results with `tool_name`) |
| `delegation_count` | `number` | Total delegation calls in this session |
| `delegation_by_worker` | `object` | Per-worker delegation counts |
| `created_at` | `string` | ISO 8601 timestamp set on creation |
| `updated_at` | `string` | ISO 8601 timestamp advanced on every mutation |

`created_at` and `updated_at` use `#[serde(default)]` for backward compatibility during deserialization.

## Session Store

### Constructors

| Constructor | Behavior |
|-------------|----------|
| `SessionStore::new()` | In-memory only; no disk I/O. Used by tests and non-persistent contexts. |
| `SessionStore::with_data_dir(data_dir)` | Enables persistence to the given `sessions/` directory. Creates the directory if it does not exist. |

### Operations

| Operation | Behavior |
|-----------|----------|
| `create()` | Create the session in memory **and** write the initial JSON file to disk. Creates the `sessions/` directory if it does not exist. |
| `get_or_create()` | If the ID is in memory, return it. If not in memory but the file exists on disk, lazy-load it. If neither, create a new session and write to disk. Includes a `load_from_disk` fallback when the in-memory index is stale due to lock contention. |
| `get()` | Return from memory if present. If not in memory but the file exists on disk, load it, insert into the HashMap, update `updated_at`, and return. Enables lazy loading. |
| `append_message_full()` / `record_delegation()` | Update the in-memory session **and** write the updated session file to disk. `updated_at` is advanced on every write. |
| `remove()` | Remove from memory **and** delete the file from disk. If the session is not in memory but exists in the disk index (lazy-loaded session), loads it from disk first so the caller receives `Some(_)`. Returns `None` only if the session is truly absent from both memory and disk. |
| `remove_all()` | Clear all sessions from the in-memory map, delete all `sess-*.json` files from `data_dir`, clear the disk index, and return the count of removed sessions (including sessions that exist only on disk and haven't been lazily loaded). |
| `scan()` | Scan the `sessions/` directory for `.json` files and read metadata only (id, timestamps, message count) without loading full message history. Populates a metadata index that enables lazy loading. Returns `SessionSummary` structs. |

### Session Summary

`SessionSummary` is a lightweight metadata struct returned by `scan()`:

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | Session identifier |
| `created_at` | `string` | ISO 8601 creation timestamp |
| `updated_at` | `string` | ISO 8601 last-mutation timestamp |
| `message_count` | `number` | Number of messages in the session |

### Write Safety

Session files use **atomic writes** — write to a `.tmp` file, then rename to the final path. This prevents corrupt files if the gateway crashes mid-write.

### Lazy Loading

On gateway start, `session_store.scan()` reads metadata only. Full message history is loaded on the first `get()` call for that session. This keeps startup fast regardless of the number of persisted sessions.

### Graceful Degradation

- Missing or corrupt session files are logged (warn level) and skipped; they are not fatal.
- A missing `data_dir` is treated as empty (no sessions on disk).
- Deleting the `sessions/` directory on disk is a valid way to clear all session history. The gateway handles missing files gracefully.

### Concurrency

- The `SessionStore` serializes access via `RwLock`. All disk writes happen inside the same write guard that updates the in-memory HashMap, so there is no risk of concurrent writes to the same file.
- The gateway lock (`gateway.lock`) uses an advisory exclusive flock to prevent two gateway processes from running against the same profile. File-level concurrency for session files is therefore not a concern.

## Session Binding Persistence

`SessionBindingStore` maps `(channel_id, conversation_id) ↔ session_id` and persists to disk so that channel→session routing survives gateway restarts.

### Constructors

| Constructor | Behavior |
|-------------|----------|
| `SessionBindingStore::new()` | In-memory only; no disk I/O. |
| `SessionBindingStore::with_data_dir(data_dir)` | Sets the data directory and loads `bindings.json` from disk if it exists. |

### File Format

`bindings.json` is a JSON array stored in the agent's `sessions/` directory:

```json
[
  { "channel_id": "telegram", "conversation_id": "123", "session_id": "sess-a1b2c3d4" }
]
```

A `Vec` rather than a `HashMap` is used since `ChannelConvKey` is a composite key. `ChannelConvKey` derives `Serialize, Deserialize`.

### Operations

| Operation | Behavior |
|-----------|----------|
| `bind()` | Update in-memory maps **and** write `bindings.json` to disk (atomic write). |
| `remove_binding(session_id)` | Removes a binding by session_id from both in-memory maps and rewrites `bindings.json` to disk. Used by the `/new` session trigger cleanup and by `sessions.delete`. |
| `remove_all()` | Clears both in-memory maps and rewrites `bindings.json` as an empty array. Used by `sessions.delete_all`. |
| `load_from_disk()` | Called at construction time by `with_data_dir()`, populating the in-memory maps from `bindings.json`. |

### Stale Binding Handling

If `bindings.json` references a session whose file was deleted from disk, `process_inbound_message` detects this (session not found via `session_store.get()`), creates a new session, and rebinds the channel conversation. The old binding is overwritten.

### Graceful Degradation

If `bindings.json` is missing, the store starts with empty bindings. If it is corrupt, a warning is logged and the store starts empty. Channel conversations will create new sessions on their next inbound message.

## Gateway Integration

### Startup

1. For each orchestrator in `config.agents.orchestrators`, a `SessionStore` and `SessionBindingStore` are constructed with `with_data_dir()`, passing that orchestrator's `sessions/` directory path. Store references are held in `GatewayState.session_stores: Arc<HashMap<String, Arc<SessionStore>>>`, keyed by orchestrator ID.
2. Each session store's `scan()` is called to populate a metadata index, enabling lazy loading.

### Inbound Message Session Resolution

When `process_inbound_message` resolves a binding to a session ID, it calls the **default orchestrator's** `session_store.get()` to ensure the session is loaded in memory (lazy-load from disk). If the session no longer exists on disk, a new session is created and the binding is updated. Channel-bound messages always use the default (first) orchestrator — there is no `orchestratorId` parameter in the channel path.

### Agent Method

The `agent` WebSocket method accepts an optional `sessionId` and an optional `orchestratorId`. When `orchestratorId` is omitted, the default (first) orchestrator is used. When provided, the gateway resolves the matching `OrchestratorRuntime` and its `SessionStore`, and rejects unknown orchestrator IDs with an error. If `sessionId` is absent, a new session is created in the selected orchestrator's store. If present, the session is resumed via `get_or_create` — which lazy-loads persisted sessions from disk, so `sessionId` can reference a session from a previous gateway run.

## Gateway Protocol Methods

### `sessions.list`

List all persisted sessions for the active profile's agents.

**Request:**

```json
{
  "type": "req",
  "id": "1",
  "method": "sessions.list",
  "params": { "orchestratorId": "reviewer" }
}
```

- `orchestratorId` is optional. When omitted, the default (first) orchestrator's session store is queried. When provided, the matching orchestrator's session store is queried.

**Response:**

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

- Returns summary metadata (no full message history) for each session.
- Sorted by `updatedAt` descending (most recent first).
- `channelBinding` is omitted when no binding exists for that session.

### `sessions.history`

Retrieve the full message history for a specific session.

**Request:**

```json
{
  "type": "req",
  "id": "2",
  "method": "sessions.history",
  "params": { "sessionId": "sess-a1b2c3d4" }
}
```

**Response:**

```json
{
  "type": "res",
  "id": "2",
  "ok": true,
  "payload": {
    "id": "sess-a1b2c3d4",
    "messages": [ "..." ],
    "createdAt": "2025-06-10T12:34:56Z",
    "updatedAt": "2025-06-10T12:35:01Z"
  }
}
```

- Supports optional `limit` and `offset` params for pagination.
- The gateway searches **across all orchestrator session stores** for the session ID, so a session can be retrieved regardless of which orchestrator created it.
- Returns an error for nonexistent sessions.
- Messages are serialized with camelCase keys (`toolCalls`, `toolName`).

### `sessions.delete`

Delete a session by id.

**Request:**

```json
{
  "type": "req",
  "id": "3",
  "method": "sessions.delete",
  "params": { "sessionId": "sess-a1b2c3d4" }
}
```

**Response:**

```json
{
  "type": "res",
  "id": "3",
  "ok": true,
  "payload": { "deleted": true }
}
```

- The gateway searches **across all orchestrator session stores** for the session ID, so a session can be deleted regardless of which orchestrator created it.
- Removes the session from memory and disk.
- Removes any associated binding entry.
- Broadcasts a `session.deleted` event.

### `sessions.delete_all`

Delete all sessions for the active profile.

**Request:**

```json
{
  "type": "req",
  "id": "4",
  "method": "sessions.delete_all",
  "params": { "orchestratorId": "researcher" }
}
```

- `orchestratorId` is optional. When provided, only clears sessions for that orchestrator's session store and selectively removes its bindings. When omitted, clears all orchestrators' sessions (backward compatible).

**Response:**

```json
{
  "type": "res",
  "id": "4",
  "ok": true,
  "payload": { "deletedCount": 12 }
}
```

- When `orchestratorId` is provided: clears that orchestrator's sessions from memory and disk, selectively removes associated bindings.
- When `orchestratorId` is omitted: clears all sessions from memory and disk, clears all bindings.
- Broadcasts a `sessions.cleared` event with `orchestratorId` in the payload.

### Session Events

| Event | Payload | When |
|-------|---------|------|
| `session.deleted` | `{ "sessionId": "...", "orchestratorId": "..." }` | After `sessions.delete` succeeds |
| `sessions.cleared` | `{ "orchestratorId": "..." }` | After `sessions.delete_all` succeeds |

`orchestratorId` in event payloads enables clients to filter events by active orchestrator. When absent (backward compatibility with older gateway versions), the event applies to all orchestrators.

RPC result handlers perform immediate local cleanup on success so the UI updates without delay. Broadcast events serve as a redundant fallback — if the broadcast arrives after the RPC handler has already cleaned up, the removal is a no-op (idempotent).

## Desktop Session Management

### Session Sidebar

On gateway connect, `sessions.list` is called to populate `session_order` and `session_summaries`.

| Element | Behavior |
|---------|----------|
| Session label | `created_at` timestamp as primary label (e.g. "Jun 10, 12:34"); short session ID below in dimmer text |
| Channel binding tag | Channel-bound sessions show `(channel_id)` only (e.g. `(telegram)`); `conversation_id` is an internal identifier and is not displayed |
| "New session" button | Always visible, regardless of whether a session is active |
| Delete button ("×") | Per-session, right-aligned via RTL layout so labels cannot push the button off screen; calls `sessions.delete` |
| "Clear all" button | At the bottom of the sidebar with a stacked confirmation dialog; calls `sessions.delete_all` |

### Session History on Switch

When the user clicks a persisted session not in the local `session_messages` map:

1. Sets `loading_session_id` and triggers a `sessions.history` RPC.
2. Shows "Loading session history…" in the chat area while the fetch is in flight.
3. Converts the returned `SessionMessage` array to desktop `ChatMessage` objects — empty assistant messages are skipped, and assistant progress text is emitted before tool call entries (matching live event stream order).
4. Populates `session_messages[session_id]` with the converted messages.

This is a lazy-load pattern: sessions are listed with metadata only, and full history is loaded on demand.

### Channel-Bound Session Read-Only Guard

Clicking a channel-bound session sets `selected_session_id` (for viewing) but not `chat_session_id` (for sending). The `can_send_base` guard checks `chat_session_id.is_some()`, disabling the chat input for channel-bound sessions. This prevents the desktop from sending a message that would cause the gateway's `get_or_create` to create a new empty session, overwriting the channel session's history on disk.
### Session Event Processing

| Event | Desktop Behavior |
|-------|-----------------|
| `session.deleted` | When `orchestratorId` matches the active orchestrator (or is absent), removes the session from `session_messages`, `session_order`, and `session_summaries`. Switches to "New session" mode if it was the selected session. When `orchestratorId` doesn't match the active orchestrator, the event is ignored. |
| `sessions.cleared` | When `orchestratorId` matches the active orchestrator (or is absent), clears all local session state and switches to "New session" mode. When `orchestratorId` doesn't match the active orchestrator, the event is ignored. |

RPC result handlers perform immediate local cleanup on success so the UI updates without delay. Broadcast events serve as a redundant fallback — if the broadcast arrives after the RPC handler has already cleaned up, the removal is a no-op (idempotent).

### Gateway Stop

Clears `chat_session_id` and `chat_messages` but **preserves** `session_messages`, `session_order`, and `session_summaries` in memory. Resets `sessions_list_fetched` so the list is re-fetched on reconnect.

## CLI Session Commands

CLI session commands operate **directly on the session store on disk** — no gateway connection required. This makes `--profile` genuinely useful (which profile's sessions to inspect) and removes the gateway dependency.

| Command | Behavior |
|---------|----------|
| `chai sessions list` | Lists sessions for the active profile from disk via `SessionStore::scan()`. Shows session id, timestamps, message count, and channel binding (if any). Sorted by most recently updated. Supports `--profile` and `--agent <id>` (scopes to a specific orchestrator's session store). |
| `chai sessions delete <ID>` | Removes a session from disk via `SessionStore::remove()` and its binding via `SessionBindingStore::remove_binding()`. Prints confirmation. Supports `--profile`. |
| `chai sessions clear` | Removes all sessions and bindings from disk via `SessionStore::remove_all()` and `SessionBindingStore::remove_all()`. Prints count of deleted sessions. Supports `--profile` and `--agent <id>` (scopes to a specific orchestrator's session store; without `--agent`, clears the default orchestrator's sessions). |
### `GatewayConn` Refactor

The gateway WebSocket connect + auth handshake was extracted from `chat.rs` into a reusable `GatewayConn` struct (`crates/cli/src/gatewayconn.rs`). `GatewayConn::connect(profile)` establishes an authenticated connection. `GatewayConn::call(method, params)` sends a method request and waits for the matching response. The `chat` command was refactored to use `GatewayConn`, reducing boilerplate. The gateway protocol session methods remain for the desktop client.

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| One file per session | Reads and writes are proportional to the session being accessed, not the total number. A monolithic store would require reading/writing the entire history on any change. |
| Named by session id | The filename is `{session_id}.json`, making it trivial to locate a session by id. The `sess-` prefix ensures filenames are human-recognizable. |
| Write-through persistence | Every message append is a state change that the user expects to be durable. A periodic flush risks losing the last few messages on a crash. Per-file granularity keeps I/O cost acceptable. |
| Atomic writes (`.tmp` then rename) | Prevents corrupt files if the gateway crashes mid-write. |
| Lazy loading on startup | Keeps startup fast regardless of the number of persisted sessions. Full history is loaded on demand. |
| Direct disk access for CLI | Sessions are always on disk and up-to-date (write-through). Requiring a running gateway just to list or delete sessions is an artificial constraint. `--profile` becomes genuinely useful. |
| Timestamps as sidebar labels | Timestamps are available from `sessions.list` metadata alone and require no additional data loading. Auto-generated titles remain a future consideration. |

## Related Documents

| Document | Purpose |
|----------|---------|
| [CONTEXT.md](CONTEXT.md) | How session history is loaded per turn and composed into the model request |
| [CHANNELS.md](CHANNELS.md) | Session binding and inbound message routing |
| [PROFILES.md](PROFILES.md) | Profile directory structure and the `agents/` directory layout |
| [DESKTOP.md](DESKTOP.md) | Desktop session sidebar, history loading, and event processing |
| [ORCHESTRATION.md](ORCHESTRATION.md) | Delegation and session-scoped policy caps |
| [SECURITY.md](../SECURITY.md) | Security considerations including encryption at rest |
