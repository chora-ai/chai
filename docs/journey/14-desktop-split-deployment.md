# Journey: Desktop — Split Deployment (Remote Gateway)

**Goal:** Connect the desktop app to a remote gateway, send a chat message, and verify the Config and Skills screens show a remote-gateway message instead of local data.

**Background:** [Desktop App](../guides/09-desktop.md)

## Prerequisites

- **Setup complete** — You have installed chai, run `chai init`, and have a working `config.json` with at least one provider (see [00-setup-init.md](00-setup-init.md)).
- **A running remote gateway** — `chai gateway` is running on a server with token auth (`gateway.auth.mode: "token"`) on a non-loopback binding. You know the gateway URL and auth token. See [12-gateway-auth.md](12-gateway-auth.md) for token auth setup.
- **Desktop runnable:** `cargo run -p desktop` (or an installed `chai-desktop` binary) on the client machine.
- **TLS (if connecting over the network)** — A reverse proxy (Caddy, nginx, or Traefik) configured in front of the gateway with TLS termination so the desktop can connect via `wss://`. See the [Reverse Proxy Setup for TLS](../guides/09-desktop.md#reverse-proxy-setup-for-tls) section in the Desktop App guide. For local testing on a single machine, `ws://` with `CHAI_HOME` isolation is sufficient.

## Steps

1. **Configure the client's `desktop.json`**
   - Edit `~/.chai/desktop.json` (create it if it does not exist) and add a `remote` entry:
     ```json
     {
       "remote": [
         {
           "id": "my-remote",
           "url": "wss://gateway.example.com/ws",
           "token": "your-gateway-token"
         }
       ]
     }
     ```
   - For local single-machine testing, use `ws://` with a different port: `"url": "ws://localhost:15151/ws"`.
   - **Expect:** The file is saved with valid JSON.

2. **Launch the desktop app**
   - Run: `cargo run -p desktop` (or `chai-desktop`).
   - **Expect:** Window opens. The profile ComboBox includes "my-remote" alongside any local profiles.

3. **Select the remote profile**
   - Click the profile ComboBox in the header and select "my-remote".
   - **Expect:** The header shows **Connect** and **Disconnect** buttons instead of Start/Stop.

4. **Connect to the remote gateway**
   - Click **Connect**.
   - **Expect:** Status changes to "Gateway: running" within a few seconds. No red error message.

5. **Send a chat message**
   - Switch to the **Chat** screen in the sidebar. Type a message and press Enter.
   - **Expect:** The agent replies (confirming the remote gateway is processing requests).

6. **Check the Gateway screen**
   - Navigate to the **Gateway** screen in the sidebar.
   - **Expect:** The screen shows the remote gateway's status — agents, providers, discovered models, and skill packages loaded from the remote server.

7. **Check the Config screen**
   - Navigate to the **Config** screen in the sidebar.
   - **Expect:** Instead of a local config summary or a config load error, the screen displays: "This profile connects to a remote gateway. Use the Gateway screen to view the gateway's effective configuration."

8. **Check the Skills screen**
   - Navigate to the **Skills** screen in the sidebar.
   - **Expect:** Instead of local skills or a config load error, the screen displays: "This profile connects to a remote gateway. Use the Gateway screen to view the gateway's loaded skill packages."

9. **Disconnect from the remote gateway**
   - Click **Disconnect** in the header.
   - **Expect:** Status shows "Gateway: stopped". The **Connect** button is enabled again (the desktop does not automatically reconnect after explicit disconnect).

## If Something Fails

- **"my-remote" does not appear in the ComboBox** — The remote entry may have been rejected at load time. Check for a logged warning about an invalid entry (empty `id`, non-`ws://`/`wss://` `url`, or empty `token`) or a collision with an existing local profile directory. Fix the entry in `desktop.json` and restart the desktop.
- **"Connect" shows an error or status stays "stopped"** — The remote gateway may not be running, or the URL/port may be wrong. Verify the gateway is running on the server (`curl http://127.0.0.1:15151/`). If using `wss://`, verify the reverse proxy is correctly forwarding WebSocket upgrades. Check that the `token` in `desktop.json` matches the server's `gateway.auth.token`.
- **Chat message gets no reply** — The gateway may not have a provider configured, or the provider may not be running. Check the Gateway screen for provider and model status. Verify the server's `config.json` has a working provider (e.g., Ollama running on the server).
- **Config or Skills screen shows a load error instead of the remote-gateway message** — This should not happen. If the screen shows "failed to load config: ...", verify you have selected the remote profile (not a local one).

## Summary

| Step | Action | Expected Outcome |
|------|--------|-------------------|
| 1 | Add `remote` entry to `desktop.json` | File saved with valid JSON |
| 2 | Launch desktop | "my-remote" appears in profile ComboBox |
| 3 | Select "my-remote" profile | Header shows Connect/Disconnect buttons |
| 4 | Click Connect | Status → "Gateway: running" |
| 5 | Send a chat message | Agent replies |
| 6 | Check Gateway screen | Shows remote gateway status (agents, providers, models, skills) |
| 7 | Check Config screen | Shows remote-gateway message directing to Gateway screen |
| 8 | Check Skills screen | Shows remote-gateway message directing to Gateway screen |
| 9 | Click Disconnect | Status → "Gateway: stopped"; Connect button enabled |

**Next:** [13 — Profile: manage and switch](13-profile-manage.md)
