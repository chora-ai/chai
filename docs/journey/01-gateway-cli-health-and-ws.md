# Journey: Gateway (CLI) — health and WebSocket connect

**Goal:** Run the gateway from the CLI and verify HTTP health and WebSocket connect/handshake.

**Background:** [Configuration](../guides/03-configuration.md)

## Prerequisites

- **Setup complete** — You have installed chai, run `chai init`, and verified Ollama is available (see [00-setup-init.md](00-setup-init.md)).
- **Gateway not already running** — Only one gateway can run at a time (advisory lock at `~/.chai/gateway.lock`).

## Steps

1. **Start the gateway**
   - From repo root: `cargo run -p cli -- gateway`
   - Or: `chai gateway`
   - Optional: `RUST_LOG=info` for logs. Optional: `--port 12345` to override port (default 15151).
   - **Expect:** Log output confirming startup, provider discovery, and gateway listening. Look for `gateway listening on 127.0.0.1:15151`.

2. **HTTP health**
   - In another terminal: `curl http://127.0.0.1:15151/`
   - **Expect:** JSON with `"status": "running"`, `"protocol": 1`, `"port": 15151` (or your port).

3. **WebSocket connect**
   - Use a WebSocket client (e.g. `websocat`, browser console, or script) to connect to `ws://127.0.0.1:15151/ws`.
   - Send first frame (JSON):  
     `{"type":"req","id":"1","method":"connect","params":{}}`
   - **Expect:** Response with `"type":"res"`, `"ok":true`, payload containing `"type":"hello-ok"` and `"protocol":1`.

4. **Health over WebSocket**
   - Send: `{"type":"req","id":"2","method":"health","params":{}}`
   - **Expect:** `"ok":true`, payload with `"status":"running"`, `"protocol":1`.

5. **Status over WebSocket**
   - Send: `{"type":"req","id":"3","method":"status","params":{}}`
   - **Expect:** `"ok":true`, payload with top-level keys in order **`gateway`**, **`channels`**, **`providers`**, **`agents`**, **`skills`**. **`gateway`** includes **`status`**, **`protocol`**, **`port`**, **`bind`**, **`auth`** (**`none`** or **`token`**).

6. **Stop the gateway**
   - In the gateway terminal: Ctrl+C.
   - **Expect:** Process exits; no timeout.

## If Something Fails

- **Gateway exits immediately** — Check the error output. Common causes: Ollama not running (start `ollama serve`), another gateway already running (remove `~/.chai/gateway.lock` if stale), or a config error in `config.json`.
- **`curl` returns "connection refused"** — Gateway not running or wrong port. Check the gateway terminal for the actual port, or set `gateway.port` in config (see [Configuration](../guides/03-configuration.md)).
- **WebSocket connect returns `"ok":false`** — If token auth is enabled (`gateway.auth.mode` is `"token"`), include the token in the connect params: `"params":{"auth":{"token":"YOUR_TOKEN"}}`.
- **`status` response missing expected keys** — The protocol or version may differ. Check the gateway version with `chai version` and verify the response includes at least `gateway`, `channels`, `providers`, `agents`, `skills`.
- **Ctrl+C does not stop the gateway** — The process may be stuck. Use `kill` or `killall chai` from another terminal. Remove the stale lock file at `~/.chai/gateway.lock` if needed.

## Token Auth (Optional)

If you set `gateway.auth.mode` to `"token"` and configure a token (in config or via `CHAI_GATEWAY_TOKEN`):

- **WebSocket:** Include in the connect frame: `{"type":"req","id":"1","method":"connect","params":{"auth":{"token":"YOUR_TOKEN"}}}`.
- **HTTP:** The health endpoint does not require auth. Other HTTP routes may require `Authorization: Bearer YOUR_TOKEN`.

## Summary

| Step | Action | Expected Outcome |
|------|--------|-------------------|
| 1 | `chai gateway` | Startup logs; gateway listening on `127.0.0.1:15151` |
| 2 | `curl http://127.0.0.1:15151/` | JSON: `"status": "running"`, `"protocol": 1` |
| 3 | WebSocket `connect` | `"ok":true`, `hello-ok` payload, `"protocol": 1` |
| 4 | WebSocket `health` | `"ok":true`, `"status": "running"` |
| 5 | WebSocket `status` | `"ok":true`, keys: `gateway`, `channels`, `providers`, `agents`, `skills` |
| 6 | Ctrl+C | Process exits |

**Next:** [02 — Gateway WebSocket: agent & send](02-gateway-ws-agent.md)
