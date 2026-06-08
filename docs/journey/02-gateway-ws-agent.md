# Journey: Gateway WebSocket — agent and send

**Goal:** Exercise the two main WebSocket methods: `agent` (run a model turn) and `send` (deliver a message to a channel). Run an agent turn and observe session continuity, then test the send method's error behavior when no channel is registered.

**Background:** [Connections](../guides/04-connections.md) · [Agents](../guides/05-agents.md)

## Prerequisites

- **Setup complete** — You have installed chai, run `chai init`, and verified Ollama is available (see [00-setup-init.md](00-setup-init.md)).
- **Gateway running** — Start the gateway and verify health (see [01-gateway-cli-health-and-ws.md](01-gateway-cli-health-and-ws.md)).
- **Ollama** running locally with at least one model (e.g. `ollama run llama3.2:3b` or have `llama3.2:3b` pulled).
- **Default model:** config `agents.defaultModel` (e.g. `llama3.2:3b`) or the provider's fallback (`llama3.2:3b` for Ollama).

## Agent Method

1. **Connect to the gateway**
   - Connect to `ws://127.0.0.1:15151/ws` (or your gateway port).
   - Send: `{"type":"req","id":"1","method":"connect","params":{}}`
   - **Expect:** `"ok":true` with `hello-ok` payload.

2. **Send agent request (new session)**
   - Send:  
     `{"type":"req","id":"2","method":"agent","params":{"message":"Say hello in one short sentence."}}`
   - **Expect:** `"ok":true`, payload with `"reply"` (short greeting from the model) and `"sessionId"` (e.g. `sess-<uuid>`).

3. **Send agent request (same session, follow-up)**
   - Send again with the same `sessionId`:  
     `{"type":"req","id":"3","method":"agent","params":{"sessionId":"<paste sessionId from step 2>","message":"What did you just say?"}}`
   - **Expect:** `"ok":true`, payload with `"reply"` that refers to the previous answer (session history is used).

4. **Optional: new session**
   - Send: `{"type":"req","id":"4","method":"agent","params":{"message":"What is 2+2?"}}`  
     (omit `sessionId` to create a new session.)
   - **Expect:** `"ok":true`, new `sessionId`, reply e.g. "4".

## Send Method

5. **Send to a non-existent channel**
   - Send:  
     `{"type":"req","id":"5","method":"send","params":{"channelId":"telegram","conversationId":"12345","message":"Hello"}}`
   - **Expect:** `"ok":false`, error message like `"channel not found"` (no channel is registered by default — this is the expected outcome).

6. **Success case (after setting up a channel)**
   - When a channel is registered (e.g. Telegram per [04-channel-telegram.md](04-channel-telegram.md)), the same request with that `channelId` returns `"ok":true` and payload `{"sent":true}`; the gateway calls the channel's `send_message(conversationId, message)`.
   - This journey does not set up a channel. Treat "channel not found" as the expected result. Return to this step after completing a channel journey.

7. **Disconnect**
   - Close the WebSocket connection.

## If Something Fails

- **`connect` returns `"ok":false`** — If token auth is enabled, include the token in `params.auth.token`. See [01 — Gateway health & WebSocket](01-gateway-cli-health-and-ws.md) for token auth details.
- **`agent` returns `"ok":false` with a model/provider error** — Ensure Ollama is running and the default model exists (`ollama list`). Check config `agents.defaultModel` or use a model you have (e.g. `llama3.2:3b`). The reply may include `"error"` with details.
- **`agent` hangs (no response)** — The model may be loading into memory for the first time (can take 30–60 s). If it hangs longer than that, check gateway logs for errors. With `RUST_LOG=info chai gateway`, look for `agent turn failed` or provider errors.
- **`send` returns `"ok":false` with `"channel not found"`** — This is the expected outcome when no channel is configured. To test the success case, complete one of the channel journeys ([04 — Telegram](04-channel-telegram.md), [08 — Matrix](08-channel-matrix.md), [09 — Signal](09-channel-signal.md)) first.
- **`send` response is unexpected** — Verify the params: `channelId` (string), `conversationId` (string, e.g. chat id), `message` (string). All three are required.
- **Connection refused** — Gateway not running or wrong port; confirm with `curl http://127.0.0.1:15151/`.
- **Reply is empty or generic** — The model may not have followed the instruction. Try a more specific message. Local models can vary in response quality; if the model truly produces no output, check logs for `agent turn failed`.

## Summary

| Step | Action | Expected Outcome |
|------|--------|-------------------|
| 1 | WebSocket `connect` | `"ok":true`, `hello-ok` payload |
| 2 | `agent` with message (no session) | `"ok":true`, `reply` + `sessionId` |
| 3 | `agent` with same `sessionId` | `"ok":true`, reply references previous answer |
| 4 | `agent` without `sessionId` (optional) | `"ok":true`, new `sessionId` |
| 5 | `send` with `channelId: "telegram"` | `"ok":false`, `"channel not found"` |
| 6 | `send` after channel setup | `"ok":true`, `{"sent":true}` |
| 7 | Disconnect | Connection closed |

**Next:** [03 — Desktop: start/stop gateway](03-desktop-start-stop-gateway.md) · [04 — Channel: Telegram](04-channel-telegram.md)
