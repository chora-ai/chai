---
status: stable
---

# Logging

This document specifies how chai processes produce, capture, and expose diagnostic log output. It covers the gateway log buffer, the `logs` WebSocket method, and how the desktop app merges gateway logs into its Logs screen.

## Purpose

Chai uses the `log` crate for diagnostic output throughout the gateway and agent loop. This spec describes how those log records are captured into an in-memory ring buffer in the gateway process, how connected clients fetch them via WebSocket, and how the desktop app presents a unified view of gateway and desktop logs.

## Log Record Format

Both the desktop and the gateway format log records the same way:

```
[<timestamp> <LEVEL> <target>] <message>
```

| Component | Format | Example |
|-----------|--------|---------|
| `timestamp` | ISO 8601 UTC from `env_logger` | `2024-01-15T10:30:45.123Z` |
| `LEVEL` | Left-aligned 5-char log level | `INFO `, `WARN `, `ERROR`, `DEBUG`, `TRACE` |
| `target` | `record.target()` — the module path of the log source | `lib::gateway::server`, `desktop::app::state::chat`, `zbus::connection` |
| `message` | `record.args()` | `gateway listening on 127.0.0.1:15151` |

The `target` field uses `record.target()` from the `log` crate, which is the module path where the log macro was invoked. This is the same string that `RUST_LOG` filter directives match against (e.g. `RUST_LOG=lib::gateway=debug`). For chai's own code, the target will start with `lib::`, `cli::`, or `desktop::`. For dependencies, it will be their crate name (e.g. `zbus::connection`).

On stderr, both processes add ANSI color codes (dimmed brackets, colored level). The ring buffer stores plain-text lines only.

## Gateway Log Buffer

### Initialization

The gateway process calls `lib::logging::init_gateway_logging()` instead of `env_logger::init()`. This sets up `env_logger` with a custom `format` closure that:

1. Builds a plain-text line in the format above with `target` set to `record.target()` (the actual module path)
2. Pushes the plain-text line to the global ring buffer
3. Writes the colorized line to stderr

The default log filter is `lib=info,cli=info`, which restricts info-level output to chai crates. Dependency logs (e.g. zbus D-Bus dispatch) are suppressed unless the user explicitly enables them via `RUST_LOG` (e.g. `RUST_LOG=zbus=debug`). The `RUST_LOG` environment variable overrides the default filter entirely — a bare level like `RUST_LOG=debug` enables debug output from **all** crates, including dependencies. See [Log Filtering](#log-filtering) for details on how to set `RUST_LOG` to get chai-only debug output.

The CLI gateway command selects this logger based on the subcommand. Other CLI subcommands (init, chat, profile, skill, file) use plain `env_logger` with the same default filter (`lib=info,cli=info`).

### Ring Buffer

The gateway log buffer is a global `Mutex<LogBuffer>` initialized via `OnceLock`. It holds up to 1000 entries. Each entry has a monotonically increasing sequence number (`seq`, starting at 1) and the formatted line string.

When the buffer is full, the oldest entry is evicted (FIFO). Sequence numbers continue incrementing regardless of eviction — they are never reused.

### Public API

| Function | Returns | Description |
|----------|---------|-------------|
| `log_lines_after(after_seq)` | `(Vec<String>, u64)` | Returns all lines with `seq > after_seq`, plus the current `max_seq` (0 if empty) |
| `init_gateway_logging()` | — | Initializes `env_logger` with ring buffer capture. Panics if called twice. |

Clients pass the returned `max_seq` as `after_seq` on the next call to receive only new lines, even across buffer wraps.

## `logs` WebSocket Method

The gateway exposes a `logs` method on the same WebSocket connection used for `connect`, `status`, `agent`, and `stop`.

### Request

```json
{
  "type": "req",
  "id": "<string>",
  "method": "logs",
  "params": {
    "afterSeq": 0,
    "lines": 200
  }
}
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `afterSeq` | `u64` | `0` | Return lines with sequence numbers greater than this value. Pass `0` to get all lines in the buffer. |
| `lines` | `u64` | `200` | Maximum number of lines to return. |

### Response

```json
{
  "type": "res",
  "id": "<string>",
  "ok": true,
  "payload": {
    "lines": ["[2024-01-15T10:30:45.123Z INFO  lib::gateway::server] gateway listening on 127.0.0.1:15151", "..."],
    "maxSeq": 42
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `lines` | `string[]` | Formatted log lines with `seq > afterSeq`, up to `lines` count |
| `maxSeq` | `u64` | Highest sequence number in the buffer (0 if empty). Use as `afterSeq` on the next request to get only new lines. |

### Cursor-Based Deduplication

Clients should track `maxSeq` from each response and pass it as `afterSeq` on the next request. This ensures:

- No duplicate lines across requests
- Correct behavior when the buffer wraps (evicted lines are skipped by sequence number, not buffer position)
- Minimal data transfer (only new lines are returned)

When a client first connects to a gateway, it should pass `afterSeq: 0` to get all buffered lines, then use the returned `maxSeq` for subsequent requests.

### Authentication

The `logs` method requires a successful `connect` handshake, same as all other WebSocket methods. Unauthenticated connections receive an error for `logs` requests.

### Error Response

```json
{
  "type": "res",
  "id": "<string>",
  "ok": false,
  "error": "<message>"
}
```

## Desktop Logs Screen

The desktop app's Logs screen displays a unified view of desktop and gateway log output in a single ring buffer.

### Log Sources

| Source | How logs are captured | When active |
|--------|----------------------|-------------|
| Desktop process | `env_logger` format closure pushes to the desktop ring buffer (`state::logs::push_log_line`) | Always |
| Owned gateway | stderr/stdout reader threads push raw lines to the desktop ring buffer | When the desktop spawned the gateway subprocess |
| External gateway | Periodic `logs` WS method fetch pushes returned lines to the desktop ring buffer | When an external gateway is detected and the desktop didn't spawn it |

### Owned Gateway Capture

When the desktop starts a gateway via `start_gateway()`, it spawns stderr and stdout reader threads that push each line into the desktop's ring buffer. The gateway's `env_logger` already formats lines with the `[timestamp LEVEL target]` prefix, so lines are pushed as-is — no additional wrapper is needed.

### External Gateway Fetch

When the desktop detects an external gateway (one it did not spawn), it polls the `logs` WS method on the same cadence as status fetches (~0.5 Hz). The fetch uses cursor-based deduplication:

1. Start with `gateway_logs_cursor = 0`
2. Fetch `logs { afterSeq: cursor }`
3. Push returned lines into the desktop ring buffer
4. Update `gateway_logs_cursor = maxSeq` from the response
5. On the next poll, use the updated cursor

When the gateway stops responding or the desktop spawns its own gateway, the cursor resets to 0 so the next external gateway starts fresh.

### Buffer Size

The desktop ring buffer holds up to 1000 lines. When full, the oldest line is evicted. This is the same capacity as the gateway's ring buffer.

## CLI Logging

The `chai` CLI uses plain `env_logger` for all subcommands except `gateway`. The gateway subcommand uses `lib::logging::init_gateway_logging()` to capture log records into the ring buffer for the `logs` WS method. All CLI subcommands use the default filter `lib=info,cli=info`, overridable via `RUST_LOG`.

No CLI subcommand other than `gateway` initializes the ring buffer or exposes the `logs` method.

## Log Filtering

All chai processes use `env_logger` with crate-scoped default filters. When `RUST_LOG` is not set, the defaults suppress dependency noise:

| Process | Default Filter |
|---------|---------------|
| Gateway (CLI) | `lib=info,cli=info` |
| CLI (non-gateway) | `lib=info,cli=info` |
| Desktop | `desktop=info,lib=info` |
| Desktop-spawned gateway | `lib=info,cli=info` |

When `RUST_LOG` is set, it overrides the default filter entirely. A bare level like `RUST_LOG=debug` enables that level for **all** crates, including dependencies (zbus, tungstenite, etc.). To get debug-level output from only chai crates, use target-scoped directives:

```
RUST_LOG=lib=debug,cli=debug,desktop=debug
```

This sets debug level for chai crates only; all other crates remain at their default (off). To add a specific dependency without enabling all of them, append a directive:

```
RUST_LOG=lib=debug,cli=debug,desktop=debug,zbus=info
```

### Desktop-Spawned Gateway

When the desktop app starts a gateway subprocess, it sets `RUST_LOG=lib=info,cli=info` on the child process if `RUST_LOG` is not already set in the environment. This ensures the spawned gateway uses the same chai-only default as a standalone `chai gateway` invocation. If `RUST_LOG` is already set (e.g. in the profile `.env`), it is passed through unchanged.

## Related Documents

| Document | Purpose |
|----------|---------|
| [GATEWAY_STATUS.md](GATEWAY_STATUS.md) | Gateway WebSocket methods and status payload |
| [DESKTOP.md](DESKTOP.md) | Desktop app architecture and screens |
