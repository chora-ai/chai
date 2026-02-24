# Journey: Gateway (CLI) — health and WebSocket connect

**Goal:** Run the gateway from the CLI and verify HTTP health and WebSocket connect/handshake.

## Prerequisites

- Built project: `cargo build`
- CLI available: `cargo run -p cli --` or installed `chai`

## Steps

1. **Start the gateway**
   - From repo root: `cargo run -p cli -- gateway`
   - Or: `chai gateway`
   - Optional: `RUST_LOG=info` for logs. Optional: `--port 18790` to override port (default 15151).

2. **HTTP health**
   - In another terminal: `curl http://127.0.0.1:15151/`
   - **Expect:** JSON with `"runtime": "running"`, `"protocol": 1`, `"port": 15151` (or your port).

3. **WebSocket connect**
   - Use a WebSocket client (e.g. `websocat`, browser console, or script) to connect to `ws://127.0.0.1:15151/ws`.
   - Send first frame (JSON):  
     `{"type":"req","id":"1","method":"connect","params":{}}`
   - **Expect:** Response with `"type":"res"`, `"ok":true`, payload containing `"type":"hello-ok"` and `"protocol":1`.

4. **Health over WebSocket**
   - Send: `{"type":"req","id":"2","method":"health","params":{}}`
   - **Expect:** `"ok":true`, payload with `"runtime":"running"`, `"protocol":1`.

5. **Status over WebSocket**
   - Send: `{"type":"req","id":"3","method":"status","params":{}}`
   - **Expect:** `"ok":true`, payload with `"runtime":"running"`, `"port"`, `"bind"`, `"auth":"none"` (or `"token"` if configured).

6. **Stop the gateway**
   - In the gateway terminal: Ctrl+C.
   - **Expect:** Process exits; no timeout.

## Notes

- If you use token auth (set `gateway.auth.mode` to `"token"` and set the token), include in connect: `"params":{"auth":{"token":"YOUR_TOKEN"}}`.
- Config: `~/.chai/config.json` or `CHAI_CONFIG_PATH`; port in config is used unless overridden by `--port`.
