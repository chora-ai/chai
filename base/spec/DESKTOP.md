---
status: stable
---

# Desktop Application

This spec describes the current behavior of **`crates/desktop`** (`chai-desktop`), the egui/eframe local control UI for the gateway. It documents what the desktop app does today — screens, data sources, interactions, and known limits.

## Purpose

The desktop app is a **local operator console** for Chai. It does **not** embed the gateway as a library; it may **spawn** the `chai gateway` subprocess or attach to an **already listening** gateway on the configured bind/port. This spec captures the implemented behavior so agents working on the desktop package can understand the current state without reading the epic or source code.

## Application Model

### Gateway Lifecycle

| Mode | Behavior |
|------|----------|
| **Spawn** | App starts `chai gateway` as a subprocess. Header shows **Start/Stop** controls. |
| **Attach** | Another process owns the port. Header shows disabled "Gateway running". |

A periodic **TCP probe** (~1 Hz) to `gateway.bind`:`gateway.port` detects liveness. When the gateway responds, the app opens a WebSocket connection.

### Runtime Profiles

The header shows the **persistent** active profile (from `~/.chai/active`). A **ComboBox** rewrites the symlink when the gateway is **not** running. Profile switching is disabled while the gateway is up (same rule as `chai profile switch`).

When the gateway is running, the desktop resolves an **effective profile** that may differ from the persistent symlink:

- **Resolution order**: `CHAI_PROFILE` environment variable → `gateway.lock` profile → `~/.chai/active` symlink.
- **Profile mismatch hint**: when the effective profile differs from the persistent one (because `CHAI_PROFILE` is set or the gateway was started with a different profile), the ComboBox is disabled and an amber label below the header indicates which profile the gateway is using.
- **Spawn propagation**: when the desktop starts the gateway, the effective profile is passed via `--profile` so the subprocess uses the same profile.
- All `load_config` calls use `effective_profile_override()` to load the correct per-profile configuration (port, token, skills, etc.).

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
- Hint for `/help` and Ctrl/Cmd+Enter when gateway is running.
- **First-turn session binding**: streamed tool calls and results appear in real time on the first turn of a new chat session. When the first WebSocket session event arrives while `chat_session_id` is `None` and `pending_user_message` is `Some`, both IDs are immediately bound.
- **Tool loop limit banner**: when `maxToolLoopsPerTurn` is reached, a `session.tool_loop_limit` WebSocket event (and/or the `agent` RPC response with `loopLimitReached: true`) produces a banner in the chat timeline. The banner explains the turn was interrupted, lists the pending tool call names, and notes that `maxToolLoopsPerTurn` is configurable. The user must send another message to continue. Dedup guards prevent duplicate `assistant` messages when both the WebSocket event and RPC response arrive for the same limit hit.
- **Stop button**: next to the send button in the chat input area. Enabled when an agent turn is in progress (when `chat_turn_receiver` is `Some`). Clicking it sends a `stop` WebSocket method to the gateway, which sets the stop flag for the active session. The agent finishes the current tool call or model request, then pauses before the next iteration. The stop request is idempotent — stopping an idle session is a no-op. The send button is disabled while an agent turn is in progress; both the send and stop buttons transition once the turn completes or is stopped.
- **Turn stopped banner**: when the agent turn is stopped (either via the stop button or the `session.turn_stopped` WebSocket event), an amber-bordered info banner appears in the chat timeline. The banner explains that the agent turn was stopped and the user can send a new message to continue. The `agent` RPC response includes a `stopped: true` field; the desktop adds the banner on receipt if not already present from the WebSocket event. Dedup guards prevent duplicate banners when both the WebSocket event and RPC response arrive for the same stop.
- **Tool event deduplication**: when a `session.tool_call` event arrives, the desktop checks for an existing entry in the current turn with the same **`tool_index`**, **`tool_name`**, and **`source`**. Matching events are treated as duplicates and silently dropped. This dedup prevents replay artifacts on WebSocket reconnect, but relies on the gateway producing non-overlapping indices across successive delegations within the same turn (see [ORCHESTRATION.md](ORCHESTRATION.md) — Tool Event Index Semantics).
- **Worker reply rendering**: when `orchestration.delegate.complete` arrives with a `reply` field, the desktop emits a separate chat message with role `"worker"` and source `"worker"`, rendered with a blue border and the worker id as a label. This shows the worker's actual text response as a first-class chat line, not only inside the collapsed `delegate_task` tool result JSON. When the worker was stopped mid-loop, `delegate.complete` omits `reply` (the content was already shown via `session.assistant_progress`) and the desktop does not render a separate worker reply line.

### Skills

All available skills are listed in alphabetical order with no agent selector or enabled/disabled section headings. When the gateway is running, `enabledSkills` from `status.agents[]` determines which skills are enabled for each agent (falls back to config when the gateway is down). Within each skill card, green text indicates the skill is enabled for the orchestrator ("Enabled for {orchestratorId}"), and blue text indicates the skill is enabled for a worker ("Enabled for {workerId}"). A skill not enabled for any agent shows no indicator. Detail pane for SKILL.md and `tools.json` — **read-only**.

### Agent


`agentDetail` (on-demand WebSocket method) supplies per-agent heavy data (`systemContext`, `tools`, `skillsContext`). The Agent combo (orchestrator vs each worker) is populated from `status.agents` (lightweight fields: `id`, `role`, `enabledSkills`, `contextMode`). The gateway is the sole authoritative source — no on-disk fallbacks.

| Agent | Layout |
|-------|--------|
| Orchestrator | Two columns: system text + skill bodies from `agentDetail` (`skillsContext`). When skills context is empty, shows "No skills context for this agent." |
| Workers | Single scroll: full text from `agentDetail`. |

The screen follows a clear data flow: gateway not running → "Start the gateway to load agent context." subtitle; gateway running but status not loaded → "Loading from gateway status..." placeholder; gateway running but `agentDetail` not loaded → "Loading agent detail..." placeholder (or red error text on fetch failure); data available → render from gateway data.

### Tools

Merged Tools JSON from `status`.

### Config

Read-only summary of `config.json` (loaded via `lib::config::load_config`, same as CLI). No JSON editor.

| Field | Shown |
|-------|-------|
| Workers with `effective_worker_defaults` | ✓ |
| `maxToolLoopsPerTurn` | ✓ |
| Delegation caps (per turn, per session, per worker) | ✓ |
| Worker provider/model defaults | ✓ |
| Instruction routes | ✓ |
| Full providers block enumeration | ✓ (all provider entries with endpoint type, resolved base URL, API key status, default model, model discovery, static models, and auto load) |

### Gateway

Gateway `status` only — no `config.json` fallback when the gateway is down or status is pending.

| Section | Content |
|---------|---------|
| **Agents** | Orchestrator (id, date, default provider/model) and workers (id, effective provider/model) from `status.agents`. |
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

When `desktop.json` is absent, all values use their defaults — no change from current behavior. Invalid values (bad theme, non-positive fontSize/bufferSize) are rejected at load time and the desktop falls back to defaults.

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
  }
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

### Loading

The desktop loads `desktop.json` once at startup. Settings are not hot-reloaded. When the file is absent, all values use their built-in defaults. When the file fails to parse or validate, the desktop logs a warning and falls back to defaults.

### Relationship to `config.json`

| File | Owner | Who reads it | Contains |
|------|-------|-------------|----------|
| `config.json` (per-profile) | Operator / developer | `chai gateway` | Bind, port, auth, channels, providers, agents, skills |
| `desktop.json` (home root) | Client / end user | `chai-desktop` | Appearance, logs |

Nothing moves out of `config.json`. The desktop still reads `config.json` for display purposes (providers, agents, channels) and for the `gateway.bind:port` fallback. `desktop.json` is purely additive.

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
| Gateway is running | `"gateway is running; stop it before switching profile"` |
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

Profile mismatch warnings (from `CHAI_PROFILE` or gateway lock profile ≠ active symlink) are shown as right-aligned amber text in the header, matching the position and size of error labels but in amber color. These are truncated in the header with a hover tooltip (same 80-character limit).

## Known Gaps

These gaps describe what the system exposes but the desktop does not yet surface:

| Gap | Source | Notes |
|-----|--------|-------|
| HTTP health endpoint | Gateway `GET /` | Desktop uses TCP probe only |
| Clear buffer button | Logs screen | No in-memory log clear button |
| Gateway status fetch failure | `fetch_gateway_status()` WebSocket | Silent; user sees stale status or "Loading from gateway status..." placeholder |
| Session events listener disconnection | WebSocket reconnect loop | Silent with exponential backoff retry; events may be missed during reconnection gap |

## Related Documents

| Document | Purpose |
|----------|---------|
| [epic `DESKTOP_FILES`](../epic/DESKTOP_FILES.md) | File explorer and file writing work |
| [adr/DESKTOP_FRAMEWORK.md](../adr/DESKTOP_FRAMEWORK.md) | Why egui/eframe |
| [spec/CONTEXT.md](CONTEXT.md) | Context on every turn: system message, session history, tool schemas |
| [spec/TOOLS_SCHEMA.md](TOOLS_SCHEMA.md) | `tools.json` validation reference |
| [spec/GATEWAY_STATUS.md](GATEWAY_STATUS.md) | WebSocket `status` payload |
| [spec/LOGGING.md](LOGGING.md) | Log buffer, `logs` WS method, and desktop log merging |
| [spec/CONFIGURATION.md](CONFIGURATION.md) | On-disk `config.json` blocks |
