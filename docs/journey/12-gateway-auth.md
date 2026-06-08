# Journey: Gateway — auth (token mode)

**Goal:** Enable token authentication on the gateway, verify that connections with the correct token succeed and connections without (or with the wrong) token are rejected, and confirm that protected HTTP routes require the token.

**Background:** [Configuration → Securing the Gateway](../guides/03-configuration.md#securing-the-gateway) · [Connections → WebSocket](../guides/04-connections.md#websocket)

By default, the gateway binds to `127.0.0.1` with no auth — safe for local use. When you expose the gateway to a non-loopback address (so other machines can connect), token auth prevents unauthorized access.

## Prerequisites

- **Setup complete** — You have installed chai, run `chai init`, and verified the gateway works with defaults (see [00-setup-init.md](00-setup-init.md)).
- **Ollama** running with a model.
- **`websocat`** or another WebSocket client for manual testing (optional for HTTP-only testing). Install with `cargo install websocat` or your package manager.

## Steps

1. **Edit config.json — enable token auth**
   - Open `~/.chai/profiles/assistant/config.json` and add:
   ```json
   {
     "gateway": {
       "auth": {
         "mode": "token",
         "token": "test-secret-123"
       }
     }
   }
   ```
   - This tells the gateway to require a matching token on every WebSocket `connect` call.

2. **Start the gateway**
   ```bash
   chai gateway
   ```
   - **Expect:** The gateway starts normally. The log output does not show the token (it is a secret), but auth mode is active.

3. **Verify: HTTP health still works (no auth required)**
   ```bash
   curl http://127.0.0.1:15151/
   ```
   - **Expect:** JSON with `"status": "running"` — the root health endpoint does not require auth.

4. **Verify: WebSocket connect without token is rejected**
   - Connect to `ws://127.0.0.1:15151/ws` and send:
   ```json
   {"type":"req","id":"1","method":"connect","params":{}}
   ```
   - **Expect:** `"ok":false` with an error like `"unauthorized: gateway token missing"`.

5. **Verify: WebSocket connect with wrong token is rejected**
   - Send:
   ```json
   {"type":"req","id":"2","method":"connect","params":{"auth":{"token":"wrong-token"}}}
   ```
   - **Expect:** `"ok":false` with an error like `"unauthorized: gateway token mismatch"`.

6. **Verify: WebSocket connect with correct token succeeds**
   - Send:
   ```json
   {"type":"req","id":"3","method":"connect","params":{"auth":{"token":"test-secret-123"}}}
   ```
   - **Expect:** `"ok":true` with the `hello-ok` payload. The session is now authenticated.

7. **Verify: Matrix verification routes require auth (if Matrix is configured)**
   - If you have Matrix configured (see [08 — Matrix](08-channel-matrix.md)), test:
   ```bash
   curl -s http://127.0.0.1:15151/matrix/verification/pending
   ```
   - **Expect:** A 401 or 403 response (unauthorized). With the token:
   ```bash
   curl -s -H "Authorization: Bearer test-secret-123" http://127.0.0.1:15151/matrix/verification/pending
   ```
   - **Expect:** A 200 response with the pending verification list (may be empty).

8. **Test with env variable instead of config (optional)**
   - Stop the gateway. Remove the `token` field from config (keep `mode: "token"`), or remove the entire `gateway.auth` block.
   - Start the gateway with an environment variable:
   ```bash
   CHAI_GATEWAY_TOKEN=env-secret-456 chai gateway
   ```
   - Connect with `params.auth.token: "env-secret-456"`.
   - **Expect:** `"ok":true`. The env variable takes precedence over config.

9. **Revert config**
   - Remove the `gateway.auth` block from `config.json` (or set `"mode": "none"`).
   - Stop the gateway.

## If Something Fails

- **Gateway refuses to bind to non-loopback without auth** — If `gateway.bind` is set to a non-loopback address (e.g. `"0.0.0.0"`) and auth is `"none"`, the gateway will refuse to start for safety: `"refusing to bind gateway to <bind> without auth"`. Enable token auth first.
- **Connect with correct token still fails** — Check for whitespace or encoding issues. The token is trimmed before comparison. If you set the token via env variable, ensure no trailing newline is included. Compare the exact string.
- **HTTP routes return 401 even without auth configured** — This happens when `gateway.bind` is not loopback and the gateway auto-enforces auth. Ensure `gateway.auth.mode` is `"token"` and the token matches.
- **Desktop app cannot connect** — The desktop app needs to know the gateway token. It reads the configured token from the active profile's config (same `gateway.auth.token`). If using `CHAI_GATEWAY_TOKEN`, the desktop may not pick it up — set the token in config instead.
- **`curl` to health endpoint fails after enabling auth** — The root `/` health endpoint does not require auth. If `curl` fails, the gateway may not be running. Check the gateway terminal for errors.

## Summary

| Step | Action | Expected Outcome |
|------|--------|-------------------|
| 1 | Set `gateway.auth.mode: "token"` + token | Config updated |
| 2 | `chai gateway` | Gateway starts with auth |
| 3 | `curl /` (HTTP health) | `"status": "running"` (no auth) |
| 4 | WS `connect` without token | `"ok":false`, unauthorized |
| 5 | WS `connect` with wrong token | `"ok":false`, unauthorized |
| 6 | WS `connect` with correct token | `"ok":true`, authenticated |
| 7 | Matrix HTTP route without token (optional) | 401/403; with token: 200 |
| 8 | `CHAI_GATEWAY_TOKEN` env var (optional) | Token from env works |
| 9 | Revert config | Auth disabled |

**Next:** [11 — Agent: multi-agent configuration](11-agent-multi.md) · [13 — Profile: manage](13-profile-manage.md)
