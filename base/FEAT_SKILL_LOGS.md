# FEAT: Logs Skill

Expose chai process logs as a chai skill so the agent can read its own diagnostic output during a session.

## Problem

The diagnostic logging in `agent.rs` (`log::info!` with usage/tokens/finish_reason/tool_calls per agent loop iteration) produces valuable data, but the agent has no way to access it. The agent can't check whether:

- `finish_reason` was `"stop"` or `"length"` for a given response
- The provider returned fewer `completion_tokens` than expected
- The `prompt_tokens` count was high (suggesting context pressure)

This means the diagnostic logging is only useful to a human reading server process output after the fact — the agent can't use it to self-diagnose or adjust its behavior in real time.

## Proposed Skill: `logs`

### Tools

| Tool | Description |
|------|-------------|
| `logs_recent` | Return the most recent N lines of chai process log output. |
| `logs_search` | Search log output for a pattern (e.g., `"finish_reason"`, `"truncated"`, `"assistant_progress"`). |

### Parameters

**`logs_recent`:**
- `lines` (optional): Number of recent lines to return (default: 50, max: 200)
- `level` (optional): Filter by log level (`info`, `warn`, `error`, `debug`)

**`logs_search`:**
- `pattern`: Pattern to search for (regex or substring)
- `lines` (optional): Context lines around each match (default: 2)

### Design Considerations

1. **Log source**: The chai process writes logs via `log::info!`/`log::warn!`/`log::error!` macros. The skill needs access to a log buffer or log file. Options:
   - **In-memory ring buffer**: A bounded circular buffer in the lib crate that captures recent log entries. The skill reads from this buffer. No filesystem dependency.
   - **Log file**: If chai writes to a log file, the skill can tail/search it. Simpler but requires file path configuration.
   - **Channel-based**: Agent loop emits diagnostic events to a channel; a log skill reads from it.

   The in-memory ring buffer is likely the cleanest approach — it's self-contained, has bounded memory usage, and doesn't require filesystem access.

2. **Context budget**: Log output can be large. The skill should truncate aggressively and prioritize structured data (finish_reason, usage, tool_call counts) over free-form log messages.

3. **Privacy**: Log output may contain parts of the conversation (e.g., message content in debug logs). The skill should not expose full message content — only metadata like token counts and finish reasons.

4. **Timing**: The agent should be able to check logs after a suspected issue to see what happened in the provider response for that specific loop iteration.

## Background

The `maxToolLoopIterations` configuration (default 100) addresses the primary phantom-edit mechanism by giving the agent loop enough headroom for typical workflows. However, diagnostic logging remains valuable for observing provider behavior — the agent can see token usage, `finish_reason` values, and iteration counts to understand how the model is using its iteration budget. This skill makes that data accessible to the agent during sessions.

## Priority

Medium-high. This skill completes the feedback loop started by the diagnostic logging, but it's less urgent than the cargo skill because the agent can already detect issues by verifying writes (it just can't diagnose *why* they happened).
