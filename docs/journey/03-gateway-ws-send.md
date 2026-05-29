# Journey: Gateway WebSocket — send (channel delivery)

**Goal:** Call the WebSocket `send` method to deliver a message to a channel. This journey verifies the API and error behavior.

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
   - When a channel is registered (e.g. Telegram per [05-channel-telegram.md](05-channel-telegram.md)), the same request with that `channelId` returns `"ok":true` and payload `{"sent":true}`; the gateway calls the channel’s `send_message(conversationId, message)`.  
   - For this journey, treat “channel not found” as the expected outcome if you have not set up a channel.

## Notes

- This journey exercises the WebSocket `send` API. Params: `channelId` (string), `conversationId` (string, e.g. chat id), `message` (string).
- For end-to-end Telegram (receive message, agent reply, deliver to chat), see [05-channel-telegram.md](05-channel-telegram.md).
