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
| **Runtime** | Status, Context, Tools |
| **Source** | Config, Skills |
| **Diagnostics** | Logs |

### Chat

- **`agent`** RPC over WebSocket with provider/model overrides.
- Session list with `session.message` / orchestration events for timelines.
- Hint for `/help` and Ctrl/Cmd+Enter when gateway is running.
- **First-turn session binding**: streamed tool calls and results appear in real time on the first turn of a new chat session. When the first WebSocket session event arrives while `chat_session_id` is `None` and `pending_user_message` is `Some`, both IDs are immediately bound.
- **Tool loop limit banner**: when `maxToolLoopsPerTurn` is reached, a `session.tool_loop_limit` WebSocket event (and/or the `agent` RPC response with `loopLimitReached: true`) produces a banner in the chat timeline. The banner explains the turn was interrupted, lists the pending tool call names, and notes that `maxToolLoopsPerTurn` is configurable. The user must send another message to continue. Dedup guards prevent duplicate `assistant` messages when both the WebSocket event and RPC response arrive for the same limit hit.
- **Stop button**: next to the send button in the chat input area. Enabled when an agent turn is in progress (when `chat_turn_receiver` is `Some`). Clicking it sends a `stop` WebSocket method to the gateway, which sets the stop flag for the active session. The agent finishes the current tool call or model request, then pauses before the next iteration. The stop request is idempotent — stopping an idle session is a no-op. The send button is disabled while an agent turn is in progress; both the send and stop buttons transition once the turn completes or is stopped.
- **Turn stopped banner**: when the agent turn is stopped (either via the stop button or the `session.turn_stopped` WebSocket event), an amber-bordered info banner appears in the chat timeline. The banner explains that the agent turn was stopped and the user can send a new message to continue. The `agent` RPC response includes a `stopped: true` field; the desktop adds the banner on receipt if not already present from the WebSocket event. Dedup guards prevent duplicate banners when both the WebSocket event and RPC response arrive for the same stop.
- **Worker reply rendering**: when `orchestration.delegate.complete` arrives with a `reply` field, the desktop emits a separate chat message with role `"worker"` and source `"worker"`, rendered with a blue border and the worker id as a label. This shows the worker's actual text response as a first-class chat line, not only inside the collapsed `delegate_task` tool result JSON.

### Status

Gateway `status` only — no `config.json` fallback when the gateway is down or status is pending.

| Section | Content |
|---------|---------|
| **Agents** | Orchestrator (id, date, default provider/model) and workers (id, effective provider/model) from `status.agents`. |
| **Models** | Discovery lists for all backends from `status.providers`. Orchestration catalog shows all rows. |

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

### Context

`status.agents` supplies the data. Agent combo (orchestrator vs each worker) selects from each row's `systemContext`.

| Agent | Layout |
|-------|--------|
| Orchestrator | Two columns: system text + skill bodies from gateway status (`skillsContext`); falls back to disk when gateway is down. |
| Workers | Single scroll: full text from gateway. |

Falls back to a single orchestrator string when the `agents` array is absent.

### Skills

All available skills are listed in alphabetical order with no agent selector or enabled/disabled section headings. When the gateway is running, `enabledSkills` from `status.agents[]` determines which skills are enabled for each agent (falls back to config when the gateway is down). Within each skill card, green text indicates the skill is enabled for the orchestrator ("Enabled for {orchestratorId}"), and blue text indicates the skill is enabled for a worker ("Enabled for {workerId}"). A skill not enabled for any agent shows no indicator. Detail pane for SKILL.md and `tools.json` — **read-only**.

### Tools

Merged Tools JSON from `status`.

### Logs

In-memory buffer (2000 lines, monospace display) fed by gateway stderr/stdout when started from desktop, or by the `logs` WebSocket method when connected to an external gateway. No clear button.

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
