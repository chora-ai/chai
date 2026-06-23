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

`agentDetail` (on-demand WebSocket method) supplies per-agent heavy data (`systemContext`, `tools`, `skillsContext`). The Agent combo (orchestrator vs each worker) is populated from `status.agents` (lightweight fields: `id`, `role`, `enabledSkills`, `contextMode`).

| Agent | Layout |
|-------|--------|
| Orchestrator | Two columns: system text + skill bodies from `agentDetail` (`skillsContext`); falls back to disk when gateway is down. |
| Workers | Single scroll: full text from `agentDetail`. |

Falls back to disk reads when the `agents` array is absent or `agentDetail` is not yet loaded.

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

## Known Gaps

These gaps describe what the system exposes but the desktop does not yet surface:

| Gap | Source | Notes |
|-----|--------|-------|
| HTTP health endpoint | Gateway `GET /` | Desktop uses TCP probe only |
| Clear buffer button | Logs screen | No in-memory log clear button |

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
