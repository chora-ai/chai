# Journey: Gateway WebSocket — agent (one turn)

**Goal:** Run one agent turn over the WebSocket: send a user message and receive an assistant reply from Ollama.

## Prerequisites

- Gateway running (see [01-gateway-cli-health-and-ws.md](01-gateway-cli-health-and-ws.md)).
- **Ollama** running locally with at least one model (e.g. `ollama run llama3.2` or have `llama3.2:latest` pulled).
- Default model: config `agents.defaultModel` (e.g. `llama3.2:latest`) or fallback `llama3.2:latest`.

## Steps

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

## If Something Fails

- **Error about session or model:** Ensure Ollama is running and the default model exists (`ollama list`). Check config `agents.defaultModel` or use a model you have (e.g. `llama3.2:latest`).
- **Connection refused:** Gateway not running or wrong port; confirm with `curl http://127.0.0.1:15151/`.
