# FEAT: Stop Button for Desktop App

Add a stop button to the desktop app that pauses the current agent turn, allowing the user to interject with a new message. Currently the only way to stop a running agent is to stop the gateway process, which destroys the in-memory session and all conversation context.

## Problem

During an agent turn, the tool loop may execute many iterations (up to `maxToolLoopIterations`). If the agent needs information from the user — for example, a git identity error reveals that the remote URL is wrong, or a tool fails in a way the agent can't self-correct — the user has no way to interrupt and provide guidance. The only escape is to stop the gateway, which:

1. **Destroys the session** — All conversation history is lost (sessions are in-memory only; persistent sessions are not yet implemented per `EPIC_PERSISTENT_SESSIONS.md`).
2. **Wastes context** — The agent's accumulated understanding of the task, files read, and decisions made are gone.
3. **Forces a restart** — The user must start a new session and re-explain the task from scratch.

This is a usability gap that affects every session where the agent encounters an issue it can't resolve autonomously. It is particularly painful during long-running agent workflows (e.g., multi-file refactors, cross-skill audits) where significant context has been built up.

## Proposed Solution

Add a **stop button** next to the **send button** in the desktop chat input area. When clicked during an active agent turn:

1. The agent turn stops after the current tool call or model response completes (no mid-stream cancellation of running processes).
2. The session is preserved — all conversation history remains intact.
3. The user can type a new message and send it to start a new turn, providing the guidance the agent needs.

This is a **pause, not a cancel**. The agent's work so far remains in the session transcript. The next turn continues from where the session left off, with the user's interjection as the latest context.

## Design

### Behavior

| State | Stop button | Send button |
|-------|-------------|-------------|
| Idle (no active turn) | Hidden or disabled | Enabled — sends user message, starts agent turn |
| Agent turn in progress | Visible and enabled | Disabled — can't send while agent is running |
| Agent turn stopped (paused) | Hidden or disabled | Enabled — sends user message, starts new agent turn |

### Stop Semantics

- **Graceful stop** — The stop signal is checked between tool loop iterations, not during a tool execution or model API call. This means the agent finishes whatever tool call or model request is currently in flight, then stops before starting the next iteration.
- **No partial tool results** — If the agent has requested multiple tool calls in a single response, all of them execute (they run as a batch within one iteration). The stop takes effect after the batch completes.
- **Transcript is valid** — The session transcript remains well-formed: `user → assistant → tool → ... → assistant (partial or final)`. The next user message continues the conversation naturally.

### Implementation Approach

The tool loop in `crates/lib/src/agent.rs` runs iterations until the model returns no `tool_calls` or `maxToolLoopIterations` is reached. Adding a stop check:

1. **Add a stop flag to the session or agent state** — An `AtomicBool` or channel-based signal that the desktop can set via the gateway protocol.
2. **Check the flag between iterations** — After each tool round completes and before the next model call, check if the stop flag is set. If so, break out of the loop and return the last assistant message (or a synthetic "turn paused" marker).
3. **Expose a gateway method** — Add a `stop` WebSocket method that sets the stop flag for the specified session. The desktop calls this when the user clicks the stop button.
4. **Reset the flag on next turn** — When a new user message starts the next agent turn, the stop flag is cleared.

### Gateway Protocol Addition

One new WebSocket method:

#### `stop`

Signal the agent to stop the current turn after the current iteration completes.

```json
{
  "type": "req",
  "id": "1",
  "method": "stop",
  "params": { "sessionId": "sess-a1b2c3d4" }
}
```

Response:

```json
{
  "type": "res",
  "id": "1",
  "ok": true,
  "payload": { "stopped": true }
}
```

The method is idempotent — calling stop on an already-idle session is a no-op that returns success.

### Desktop UI

- **Stop button placement** — Next to the send button in the chat input area. Same visual weight as send, but with a stop icon (square or stop-hand).
- **Visibility** — The stop button appears (or becomes enabled) only when an agent turn is active. It is hidden or disabled when the session is idle.
- **Send button state** — When the agent turn is active, the send button is disabled. When the turn stops (either naturally or via stop), the send button re-enables.
- **Input field** — When the agent turn is active, the input field can still accept typing (so the user can compose their interjection while waiting for the turn to stop), but sending is blocked until the turn completes or is stopped.

### Why Not Cancel?

Canceling (aborting a running tool process mid-execution) is significantly more complex:

- It requires process signaling (SIGTERM/SIGKILL) for spawned commands, which may leave the filesystem in an inconsistent state.
- It requires aborting an in-flight HTTP request to the model API, which may not be supported by all providers.
- It may produce partial or corrupt tool results that the model can't reason about.

The graceful stop (finish current iteration, then pause) avoids all of these issues. It's the same semantics as the natural end of a turn — the transcript stays well-formed, no cleanup is needed. The tradeoff is that the user must wait for the current tool call or model request to finish, but this is typically seconds, not minutes.

If faster interruption is needed in the future, it can be added as a separate "force stop" that cancels in-flight operations. That would be a superset of this feature, not a replacement.

## Relationship to Existing Epics

- **`EPIC_PERSISTENT_SESSIONS.md`** — The stop feature is complementary to persistent sessions. Persistent sessions solve the data-loss-on-restart problem; the stop button solves the can't-interrupt-during-turn problem. Together they ensure the user never loses context and can always provide guidance. The stop feature does NOT depend on persistent sessions — it works with the current in-memory session model.
- **`EPIC_TOOL_APPROVAL.md`** — Tool approval (human-in-the-loop before execution) is a separate, more complex feature that gates every tool call. The stop button is a simpler, lighter-weight mechanism: it lets the user pause after the fact rather than approve before the fact. The stop feature does NOT depend on tool approval.

## Priority

High for v0.1.0. This is the most impactful UX improvement for the desktop app because it directly addresses the worst user experience (losing all context to stop a runaway or stuck agent). It is simpler than persistent sessions or tool approval, and it provides immediate value without requiring those epics.

## Requirements

### Functional

- [ ] Stop button in desktop chat input area, visible when agent turn is active
- [ ] Clicking stop pauses the agent turn after the current iteration completes
- [ ] Session transcript is preserved after stop (no data loss)
- [ ] User can send a new message after stop to continue the session
- [ ] Send button is disabled during active turn, enabled when idle or paused
- [ ] Stop is idempotent — stopping an idle session is a no-op
- [ ] Gateway `stop` WebSocket method sets the stop flag for a session

### Non-functional

- [ ] Stop check adds negligible overhead (single atomic read per iteration)
- [ ] Stop response time is bounded by the duration of the current tool call or model request (typically seconds)
- [ ] No risk of corrupt tool results or malformed session transcripts from stop

## Open Questions

- **Stop during streaming** — Should the stop button also interrupt a streaming model response (e.g., stop generating tokens mid-stream)? This would require aborting the SSE connection, which is more complex. The initial implementation could wait for streaming to complete before stopping.
- **Visual feedback** — Should the desktop show a "stopping..." state while waiting for the current iteration to finish? This would improve perceived responsiveness.
- **CLI equivalent** — Should the CLI also support stopping a turn (e.g., Ctrl+C behavior that preserves the session rather than killing the gateway)?
