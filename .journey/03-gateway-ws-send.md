# Journey: Gateway WebSocket — send (channel delivery)

**Goal:** Call the `send` method to deliver a message to a channel. With the current setup no channel is registered at startup, so this journey verifies the API and error behavior.

## Prerequisites

- Gateway running (see [01-gateway-cli-health-and-ws.md](01-gateway-cli-health-and-ws.md)).
- WebSocket client connected and already sent `connect` (see step 1 of [02-gateway-ws-agent.md](02-gateway-ws-agent.md)).

## Steps

1. **Connect**
   - Connect to `ws://127.0.0.1:15151/ws` and send:  
     `{"type":"req","id":"1","method":"connect","params":{}}`
   - **Expect:** `hello-ok`.

2. **Send to a non-existent channel**
   - Send:  
     `{"type":"req","id":"2","method":"send","params":{"channelId":"telegram","conversationId":"12345","message":"Hello"}}`
   - **Expect:** `"ok":false`, error message like `"channel not found"` (no channel is registered by default).

3. **Success case (when a channel is registered)**
   - When you have a channel connector that registers with the gateway (e.g. Telegram stub registered under `"telegram"`), the same request with a valid `channelId` should return `"ok":true` and payload `{"sent":true}`. The channel’s `send_message(conversationId, message)` is called.  
   - For now, treat “channel not found” as the expected outcome of this journey.

## Notes

- Params: `channelId` (string), `conversationId` (string, e.g. Telegram chat_id), `message` (string).
- The Telegram channel is currently a stub; even when registered, it does not call the real Telegram API.
