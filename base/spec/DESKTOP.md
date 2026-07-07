---
status: stable
---

# Desktop Application

This spec describes the current behavior of **`crates/desktop`** (`chai-desktop`), the egui/eframe local control UI for the gateway. It documents what the desktop app does today — screens, data sources, interactions, and known limits.

## Purpose

The desktop app is a **local operator console** for Chai. It does **not** embed the gateway as a library; it may **spawn** the `chai gateway` subprocess, **attach** to an already listening gateway on the configured bind/port, or **connect** to a remote gateway over the network. This spec captures the implemented behavior so agents working on the desktop package can understand the current state without reading the source code.

## Application Model

### Gateway Lifecycle

| Mode | Behavior |
|------|----------|
| **Spawn** | App starts `chai gateway` as a subprocess. Header shows **Start/Stop** controls. |
| **Attach** | Another process owns the port. Header shows disabled "Gateway running". |
| **Remote** | Desktop connects to a remote gateway via WebSocket. Header shows **Connect/Disconnect** controls. No local gateway is spawned. |

A periodic **TCP probe** (~1 Hz) detects liveness. For local profiles, the probe targets `gateway.bind`:`gateway.port` and cross-checks against `running_profiles` (based on per-profile gateway lock files): if the active profile is not in `running_profiles`, the probe returns `responds = false` even if the port is open — preventing the desktop from connecting to another profile's gateway. For remote profiles, the probe targets the host:port extracted from the remote entry's `url` field and skips lock file cross-checking. The probe is suppressed when the user explicitly disconnects from a remote profile (via the `remote_disconnected` flag in `GatewayState`). When the probe confirms the gateway is responding, the app opens a WebSocket connection.

### Runtime Profiles

The header shows the **active** profile (from `~/.chai/active`). A **ComboBox** rewrites the symlink to switch the active profile. Profile switching is always allowed regardless of whether a gateway is running.

When switching profiles, the desktop resets the new profile's `GatewayState` (status, active orchestrator ID, dashboard agent ID, session data, chat messages, etc.) to prevent stale data from a previous visit leaking across profile switches. Only the `process` field (the owned gateway subprocess) is preserved — it is real infrastructure state, not derived from the gateway connection.

When the gateway is running on a different profile than the active one (detected by scanning per-profile lock files for a held lock):

- **Profile mismatch hint**: when a gateway is running on a different profile than the active one, an amber label indicates which profile the gateway is using.
- **Spawn propagation**: when the desktop starts the gateway, the active profile is passed via `--profile` so the subprocess uses the same profile.
- All `load_config` calls use the active profile (from `~/.chai/active`) to load the correct per-profile configuration (port, token, skills, etc.).

### Remote Profiles

When the selected profile is a remote entry (its `id` matches an entry in the `remote` array of `desktop.json`), the desktop operates in **Connect/Disconnect** mode instead of Start/Stop:

- The header shows **Connect/Disconnect** controls. Clicking Connect opens a WebSocket connection to the remote gateway's `url`. Clicking Disconnect closes it.
- The desktop does not spawn a local gateway process.
- The WebSocket URL and auth token come from the remote entry (not from `config.json`).
- Device identity (`device.json`, `device_token`) is stored under `~/.chai/profiles/<remote-id>/`.
- Switching away from a connected remote profile auto-disconnects first.
- After explicit disconnect, the Connect button remains enabled (the `remote_disconnected` flag prevents the TCP probe from re-detecting the remote gateway).

Remote entries also appear in the Settings dashboard under a "Remote Profiles" section listing each entry's id and URL.

### WebSocket Protocol

When the gateway is responding: `connect` (device identity or token + device pairing) then `status`; caches `GatewayStatusDetails`.

## Screens

The sidebar organizes screens into groups:

| Group | Screens |
|-------|---------|
| **Chat** | Chat (ungrouped) |
| **Skills** | Skills (ungrouped) |
| **Agents** | Agent, Tools |
| **System** | Config, Gateway, Logging |
| **Desktop** | Settings |

### Chat

- **`agent`** RPC over WebSocket with provider/model overrides.
- Session list with `session.message` / orchestration events for timelines.
- **Session sidebar** loads persisted sessions on gateway connect via `sessions.list`, populating `session_order` and `session_summaries` from the response. The fetch is gated on `active_orchestrator_id.is_some()` so that `sessions.list` does not fire before the orchestrator ID is resolved from gateway status (preventing a race condition that would return sessions for the wrong orchestrator). Session entries display `created_at` timestamps (e.g. "Jun 10, 12:34") as the primary label, with short session IDs below in dimmer text. Channel-bound sessions show a channel tag (e.g. `(telegram)`). The "New session" button is always visible regardless of whether a session is active.
- **Session history on switch**: when the user clicks a persisted session not in the local `session_messages` map, a `sessions.history` RPC is triggered. The chat area shows "Loading session history…" while the fetch is in flight. The history conversion decomposes assistant messages with `toolCalls`: text content is emitted as one `ChatMessage`, and each tool call is emitted as a separate `tool_call` role entry with `tool_name`, `tool_args`, and `tool_index`. Tool result messages (role `"tool"`) are emitted as `tool_result` entries. A `merge_tool_results_into_calls()` pass then matches each `tool_result` to the next unmatched `tool_call` by tool name and merges the result content into the call entry; merged `tool_result` entries are removed from the message list. This produces the same granular `ChatMessage` format used during live sessions (🔧 icons, tool names, collapsible arguments and results). The conversion emits assistant progress text before tool call entries (matching live event stream order) and skips empty assistant messages (matching live event stream behavior).
- **Channel-bound session read-only guard**: clicking a channel-bound session sets `selected_session_id` (for viewing) but not `chat_session_id` (for sending). The `can_send_base` guard checks `chat_session_id.is_some()`, disabling the chat input for channel-bound sessions. This prevents the desktop from sending a message that would cause the gateway's `get_or_create` to create a new empty session, overwriting the channel session's history on disk.
- **Session deletion**: per-session "×" delete buttons in the sidebar (right-aligned via RTL layout so labels cannot push them off screen), calling `sessions.delete`. "Clear all sessions" button at the bottom with a stacked confirmation dialog, calling `sessions.delete_all` with `orchestratorId` to scope deletion to the active orchestrator. RPC result handlers perform immediate local cleanup on success so the sidebar updates without delay. If the gateway returns a "session not found" error (e.g. the session was on disk but not in memory), the desktop also cleans up local state — the session is already gone server-side. Broadcast events (`session.deleted`, `sessions.cleared`) serve as a redundant fallback — if the broadcast arrives after the RPC handler has already cleaned up, the removal is a no-op (idempotent).
- **Session event processing**: `session.deleted` removes the session from `session_messages`, `session_order`, and `session_summaries` (switching to "New session" mode if it was the selected session) when `orchestratorId` matches the active orchestrator or is absent. `sessions.cleared` clears all local session state and switches to "New session" mode when `orchestratorId` matches the active orchestrator or is absent. Events from other orchestrators are ignored. These handlers are idempotent — they tolerate being called after the RPC handler has already performed the same cleanup.
- **Orchestrator selector**: An "Agent" section heading with a ComboBox above "Sessions" in the right sidebar. Populated from `status.agents` filtered to orchestrator role. Switching updates the session list (re-fetches for the new orchestrator) and resets the chat area to "New Session" state. Disabled when only one orchestrator is configured or when an agent turn is in progress. During the loading state (gateway running but status not yet received), the agent ComboBox falls back to config-based orchestrator IDs via `effective_orchestrator_ids()` and `effective_active_orchestrator_id()`, and is disabled. Provider/model ComboBoxes update to reflect the selected orchestrator's defaults. During loading, the provider ComboBox shows the config-based default provider and is disabled; the model ComboBox shows the config-based default model (resolved via `resolve_effective_provider_and_model` for the first orchestrator) and is disabled. All three comboboxes become enabled once gateway status is received.
- Hint for `/help` and Ctrl/Cmd+Enter when gateway is running.
- **First-turn session binding**: streamed tool calls and results appear in real time on the first turn of a new chat session. When the first WebSocket session event arrives while `chat_session_id` is `None` and `pending_user_message` is `Some`, both IDs are immediately bound.
- **Tool loop limit banner**: when `maxToolLoopsPerTurn` is reached, a `session.tool_loop_limit` WebSocket event (and/or the `agent` RPC response with `loopLimitReached: true`) produces a banner in the chat timeline. The banner explains the turn was interrupted, lists the pending tool call names, and notes that `maxToolLoopsPerTurn` is configurable. The user must send another message to continue. Dedup guards prevent duplicate `assistant` messages when both the WebSocket event and RPC response arrive for the same limit hit.
- **Stop button**: next to the send button in the chat input area. Enabled when an agent turn is in progress (when `chat_turn_receiver` is `Some`). Clicking it sends a `stop` WebSocket method to the gateway, which sets the stop flag for the active session. The agent finishes the current tool call or model request, then pauses before the next iteration. The stop request is idempotent — stopping an idle session is a no-op. The send button is disabled while an agent turn is in progress; both the send and stop buttons transition once the turn completes or is stopped.
- **Turn stopped banner**: when the agent turn is stopped (either via the stop button or the `session.turn_stopped` WebSocket event), an amber-bordered info banner appears in the chat timeline. The banner explains that the agent turn was stopped and the user can send a new message to continue. The `agent` RPC response includes a `stopped: true` field; the desktop adds the banner on receipt if not already present from the WebSocket event. Dedup guards prevent duplicate banners when both the WebSocket event and RPC response arrive for the same stop.
- **Tool event deduplication**: when a `session.tool_call` event arrives, the desktop checks for an existing entry in the current turn with the same **`tool_index`**, **`tool_name`**, and **`source`**. Matching events are treated as duplicates and silently dropped. This dedup prevents replay artifacts on WebSocket reconnect, but relies on the gateway producing non-overlapping indices across successive delegations within the same turn (see [ORCHESTRATION.md](ORCHESTRATION.md) — Tool Event Index Semantics).
- **Worker reply rendering**: when `orchestration.delegate.complete` arrives with a `reply` field, the desktop emits a separate chat message with role `"worker"` and source `"worker"`, rendered with a blue border and the worker id as a label. This shows the worker's actual text response as a first-class chat line, not only inside the collapsed `delegate_task` tool result JSON. When the worker was stopped mid-loop, `delegate.complete` omits `reply` (the content was already shown via `session.assistant_progress`) and the desktop does not render a separate worker reply line.

### Skills

When the selected profile is a remote entry (its `id` matches an entry in the `remote` array of `desktop.json`), the Skills screen displays a message instead of loading local skill data:

> This profile connects to a remote gateway.
> Use the Gateway screen to view the gateway's loaded skill packages.

The message is shown via an `is_remote_profile()` early return at the top of `ui_skills_screen`. No local skills or config data are loaded. The Gateway screen is the source of truth for skill packages on a remote gateway.

For local profiles, all available skills are listed in alphabetical order with no agent selector or enabled/disabled section headings. When the gateway is running, `enabledSkills` from `status.agents[]` determines which skills are enabled for each agent (falls back to config when the gateway is down). Within each skill card, green text indicates the skill is enabled for an orchestrator ("Enabled for {orchestratorId}"), and blue text indicates the skill is enabled for a worker ("Enabled for {workerId}"). All orchestrator agents (not just the default) are correctly identified — non-default orchestrators are shown in green, not blue. A skill not enabled for any agent shows no indicator. Detail pane for SKILL.md and `tools.json` — **read-only**.

### Agent

`agentDetail` (on-demand WebSocket method) supplies per-agent heavy data (`systemContext`, `tools`, `skillsContext`). The Agent combo (orchestrator vs each worker) is populated from `status.agents` (lightweight fields: `id`, `role`, `enabledSkills`, `enabledWorkers`, `contextMode`). The gateway is the sole authoritative source — no on-disk fallbacks.

| Agent | Layout |
|-------|--------|
| Orchestrator | Two columns: system text + skill bodies from `agentDetail` (`skillsContext`). When skills context is empty, shows "No skills context for this agent." |
| Workers | Single scroll: full text from `agentDetail`. |

The screen follows a clear data flow: gateway not running → "Start the gateway to load agent context." subtitle; gateway running but status not loaded → "Loading from gateway status..." placeholder; gateway running but `agentDetail` not loaded → "Loading agent detail..." placeholder (or red error text on fetch failure); data available → render from gateway data.

### Tools

Merged Tools JSON from `status`.

### Config

When the selected profile is a remote entry (its `id` matches an entry in the `remote` array of `desktop.json`), the Config screen displays a message instead of attempting to load a local `config.json`:

> This profile connects to a remote gateway.
> Use the Gateway screen to view the gateway's effective configuration.

The message is shown via an `is_remote_profile()` early return at the top of `ui_config_screen`. No config file is loaded. The Gateway screen is the source of truth for the remote gateway's effective configuration.

For local profiles, the Config screen provides a read-only summary of `config.json` (loaded via `lib::config::load_config`, same as CLI). No JSON editor.

| Field | Shown |
|-------|------|
| Workers with `effective_worker_defaults` | ✓ |
| `maxToolLoopsPerTurn` | ✓ |
| Delegation caps (per turn, per session, per worker) | ✓ |
| Worker provider/model defaults | ✓ |
| Instruction routes | ✓ |
| All orchestrators (not just the default) | ✓ |
| Per-orchestrator `enabledWorkers` | ✓ |
| Full providers block enumeration | ✓ (all provider entries with endpoint type, resolved base URL, API key status, default model, model discovery, static models, and auto load) |

### Gateway

Gateway `status` only — no `config.json` fallback when the gateway is down or status is pending.

| Section | Content |
|---------|---------|
| **Agents** | Orchestrator entries (id, default provider/model, enabled workers) and workers (id, effective provider/model) from `status.agents`. When multiple orchestrators are configured, each is displayed separately. `enabledWorkers` display: absent/`null` → "(none)", empty array → "(all)", non-empty → comma-separated worker ids. |
| **Models** | Discovery lists for all backends from `status.providers`. Orchestration catalog shows all rows. |

### Logging

In-memory buffer (1000 lines, monospace display) fed by gateway stderr/stdout when started from desktop, or by the `logs` WebSocket method when connected to an external gateway. No clear button.

See [LOGGING.md](LOGGING.md) for the full logging specification.

## Shared UI Helpers

The desktop uses shared UI modules for consistency:

| Module | Purpose |
|--------|---------|
| `app/ui/spacing` | Named spacing constants |
| `dashboard` | Two-column layout, section groups, key/value rows |
| `readonly_code` | Read-only code display |
| `view_toggle` | Toggle between view modes |
| `layout::central_padded` | Central panel with padding |

Dashboard `kv` uses a fixed-width key column (`KV_LABEL_COLUMN_WIDTH`) for alignment. Keys and values use default body text. Grid column headers use **strong**. Secondary hints use **weak** only where appropriate.

## Desktop Configuration

The desktop app reads `~/.chai/desktop.json` at startup for client-side settings that are machine-local and user-specific, not tied to any profile. This file is separate from per-profile `config.json`: `config.json` is a server-side operator document, while `desktop.json` holds desktop application preferences.

When `desktop.json` is absent, all values use their defaults — no change from current behavior. Invalid values (bad theme, non-positive fontSize/bufferSize, invalid remote entries) are rejected at load time and the desktop falls back to defaults.

### File Location

`~/.chai/desktop.json` — the chai home root, not per-profile. Desktop settings are machine-local: a user who switches profiles does not change their desktop preferences.

### Schema

```json
{
  "appearance": {
    "theme": "dark",
    "fontSize": 14
  },
  "logs": {
    "bufferSize": 1000
  },
  "remote": [
    {
      "id": "assistant-remote",
      "url": "wss://gateway.example.com/ws",
      "token": "<gateway-token>"
    }
  ]
}
```

All blocks and fields are optional.

#### `appearance` Block

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `theme` | `string` | `"dark"` | Color theme: `"dark"` or `"light"`. |
| `fontSize` | `number` | `14` | Base font size in points. Applied as a scale factor relative to the default size. |

#### `logs` Block

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `bufferSize` | `number` | `1000` | Maximum number of log lines retained in memory per buffer (desktop and gateway). |

#### `remote` Block

The `remote` array defines remote gateway connections. Each entry represents a remote gateway the desktop can connect to instead of spawning a local gateway. Remote entries appear as profiles in the ComboBox alongside local profiles.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | `string` | Yes | Profile name and ComboBox label. Also determines the profile directory under `~/.chai/profiles/` where device identity is stored. Must be non-empty. Must not collide with existing local profile directories (enforced at load time; disk wins). |
| `url` | `string` | Yes | WebSocket connection URL. Must start with `ws://` or `wss://`. Supports full paths for reverse proxy configurations (e.g., `wss://example.com/chai/ws`). The desktop passes the full URL to the WebSocket client library. |
| `token` | `string` | Yes | Gateway auth token for device pairing. Must be non-empty. Used in the `auth.token` field of the connection payload instead of `config.json` `gateway.auth.token`. |

Invalid entries (empty `id`, non-`ws://`/`wss://` `url`, empty `token`) are rejected at load time with a logged warning. Entries whose `id` collides with an existing local profile directory (one containing `config.json` or `gateway.lock`) are also rejected — disk wins over configuration.

When `desktop.json` is loaded at startup, the desktop creates `~/.chai/profiles/<id>/` for each valid remote entry that does not already exist. This ensures remote entries appear in the ComboBox before the user has ever connected.

### Loading

The desktop loads `desktop.json` once at startup. Settings are not hot-reloaded. When the file is absent, all values use their built-in defaults. When the file fails to parse or validate, the desktop logs a warning and falls back to defaults.

### Relationship to `config.json`

| File | Owner | Who reads it | Contains |
|------|-------|-------------|----------|
| `config.json` (per-profile) | Operator / developer | `chai gateway` | Bind, port, auth, channels, providers, agents, skills |
| `desktop.json` (home root) | Client / end user | `chai-desktop` | Appearance, logs, remote profiles |

Nothing moves out of `config.json`. The desktop still reads `config.json` for display purposes (providers, agents, channels) and for the `gateway.bind:port` fallback. `desktop.json` is purely additive.

### `CHAI_HOME` Environment Variable

The `CHAI_HOME` environment variable overrides the default `~/.chai` home directory. When set, `profile::chai_home()` returns its value instead of `dirs::home_dir()/.chai`. This enables isolated testing of split deployment on a single machine: the server and client can each point at different `CHAI_HOME` directories without interfering with the user's real `~/.chai`.

| `CHAI_HOME` value | Behavior |
|-------------------|----------|
| Set to an existing absolute path | Canonicalized path is returned |
| Set to a nonexistent absolute path | Value returned as-is (supports `chai init` creating the directory) |
| Set to a relative path | Resolved against the current working directory |
| Set to an empty string | Treated as unset; falls back to default `~/.chai` |
| Not set | Default `~/.chai` behavior |

The `resolve.rs` `sandbox_raw()` function (CLI) also respects `CHAI_HOME` — previously it read `$HOME` directly. Seven bundled skill shell scripts were updated from `$HOME/.chai` to `${CHAI_HOME:-$HOME/.chai}`.

## Error Handling

The desktop app surfaces errors from multiple sources. Errors are always displayed as red text on the relevant screen, and critical gateway errors are also shown in the header (visible from any screen). Subtitles are never hidden when an error is present — every screen shows a subtitle regardless of error state, consistent with the Gateway screen pattern.

### Gateway Errors

Gateway errors are the highest-priority error surface because they prevent the gateway from functioning. The `gateway_error` field holds an error string set when the gateway fails to start or exits unexpectedly.

| Condition | Error Message | Where Displayed |
|-----------|---------------|-----------------|
| Config load fails | `"failed to load config: {detail}"` | Gateway screen (red text above ScrollArea) + header |
| `CHAI_BIN` in `.env` points to non-existent path | `"CHAI_BIN={path} does not exist (set in .env)"` | Gateway screen + header |
| `chai` binary not found | `"could not find chai binary (expected next to desktop binary or on PATH)"` | Gateway screen + header |
| `cmd.spawn()` fails | `"failed to start gateway: {detail}"` | Gateway screen + header |
| Gateway exits unexpectedly (crash) | Extracted from log buffer (last ERROR-level message, or WARN fallback) | Gateway screen + header |
| Gateway exits with no log output | `"gateway exited unexpectedly (no log output captured)"` | Gateway screen + header |

**Crash vs. user-initiated stop:** A `gateway_was_stopped_by_user` flag distinguishes intentional stops from unexpected exits. Only unexpected exits surface an error. When the user clicks "Stop gateway", no error is shown.

**Log buffer extraction:** When the gateway crashes, `extract_gateway_error_message(n)` searches the most recent `n` gateway log lines for the last ERROR-level entry, strips the `[timestamp LEVEL target]` prefix, and returns just the message. Falls back to WARN-level if no ERROR is found. If no error-level line is found at all, the generic "gateway exited unexpectedly (no log output captured)" message is used. Raw log lines are never shown as error messages — formatted log output belongs on the Logging screen.

**Error lifecycle:** `gateway_error` is cleared when `start_gateway()` runs (at the top of the function, before validation), so clicking "Start gateway" always clears the previous error. It is also cleared when the gateway transitions from not-running to running (e.g. an external gateway comes online).

**Header truncation:** Gateway errors shown in the header are truncated to 80 characters with an ellipsis. Hovering over a truncated label shows the full message in a tooltip. The full (non-truncated) error is always shown on the Gateway screen itself, wrapping as needed.

### Profile Switch Errors

Profile switch errors are shown in the header as right-aligned red text, matching the position and size of amber profile-mismatch warnings.

| Condition | Error Message |
|-----------|---------------|
| `switch_active_profile()` fails | Propagated error string |

These are also truncated in the header with a hover tooltip (same 80-character limit as gateway errors).

### Config Load Errors

When `load_config_cached()` fails on the Config or Skills screens, the actual error message is shown as red error text with a `"failed to load config: "` prefix. The error includes the file path and the specific parse/read error (e.g. `"failed to load config: parsing config from /path/to/config.json: expected ',' or '}' at line 12 column 34"`). The subtitle is always shown alongside the error, matching the pattern of other screens.

### Skills Fetch Errors

When `fetch_skills()` fails and there is no cached data, the error is shown on the Skills screen as red text instead of "Loading skills...". The error message is user-facing (e.g. `"failed to load skills from ~/.chai/skills: ..."`) rather than raw log output. The error is cleared on the next successful fetch, on cache invalidation, and when the gateway stops. Error messages on the Skills screen are not truncated.

### Agent Detail Fetch Errors

When `fetch_agent_detail()` fails for a selected agent and the agent is not in the cache, the error is shown on the Agent and Tools screens as red text instead of "Loading agent detail...". The error is only displayed when it matches the currently selected agent. The error is cleared on the next successful fetch, on cache invalidation, and when the gateway stops. Error messages on these screens are not truncated.

### Desktop Config Load Failure

When `load_desktop_config()` fails (e.g. corrupt `desktop.json`), the Settings screen shows an amber notice above the dashboard: `"Using default settings (failed to load desktop.json: {error})"`. Missing `desktop.json` is a valid state (not an error) and shows no notice.

### Chat Turn Errors

Chat turn errors are rendered inline in the chat message stream as `ChatMessage::error()` with red border and text styling. This is the established mechanism; there is no separate error field for chat errors.

### Profile Mismatch Warnings

Profile mismatch warnings (from gateway lock profile ≠ active symlink) are shown as right-aligned amber text in the header, matching the position and size of error labels but in amber color. These are truncated in the header with a hover tooltip (same 80-character limit).

## Known Gaps

These gaps describe what the system exposes but the desktop does not yet surface:

| Gap | Source | Notes |
|-----|--------|-------|
| HTTP health endpoint | Gateway `GET /` | Desktop uses TCP probe only |
| Clear buffer button | Logs screen | No in-memory log clear button |
| Gateway status fetch failure | `fetch_gateway_status()` WebSocket | Silent; user sees stale status or "Loading from gateway status..." placeholder |
| Session events listener disconnection | WebSocket reconnect loop | Silent with exponential backoff retry; session deletion state is reconciled immediately by the RPC result handler, so missed `session.deleted` broadcasts no longer leave ghost sessions in the sidebar |

## Related Documents

| Document | Purpose |
|----------|---------|
| [epic `DESKTOP_FILES`](../epic/DESKTOP_FILES.md) | File explorer and file writing work |
| [adr/DESKTOP_FRAMEWORK.md](../adr/DESKTOP_FRAMEWORK.md) | Why egui/eframe |
| [spec/CONTEXT.md](CONTEXT.md) | Context on every turn: system message, session history, tool schemas |
| [spec/TOOLS_SCHEMA.md](TOOLS_SCHEMA.md) | `tools.json` validation reference |
| [spec/GATEWAY_STATUS.md](GATEWAY_STATUS.md) | WebSocket `status` payload |
| [spec/LOGGING.md](LOGGING.md) | Log buffer, `logs` WS method, and desktop log merging |
| [SESSIONS.md](SESSIONS.md) | Session persistence, storage layout, gateway protocol methods, and CLI session commands |
| [spec/CONFIGURATION.md](CONFIGURATION.md) | On-disk `config.json` blocks |
| [adr/SPLIT_DEPLOYMENT.md](../adr/SPLIT_DEPLOYMENT.md) | Architectural decisions for remote gateway support |
