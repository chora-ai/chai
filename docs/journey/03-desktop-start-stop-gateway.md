# Journey: Desktop — start/stop gateway and detection

**Goal:** Use the desktop app to start the gateway, see "Gateway: running", and stop it. Also verify that an externally started gateway is detected.

**Background:** [Getting Started](../guides/02-getting-started.md) · [Desktop App](../guides/09-desktop.md)

## Prerequisites

- **Setup complete** — You have installed chai, run `chai init`, and verified Ollama is available (see [00-setup-init.md](00-setup-init.md)).
- **Desktop runnable:** `cargo run -p desktop` or installed `chai-desktop`.
- **Gateway not already running** — Only one gateway can run at a time.

## Steps

1. **Launch desktop**
   - Run: `cargo run -p desktop` (or `chai-desktop`).
   - **Expect:** Window opens; gateway status shows "Gateway: stopped" (or "running" if something is already listening on the configured port).

2. **Start gateway from desktop**
   - Click "Start gateway".
   - **Expect:** Status changes to "Gateway: running" within a few seconds (probe ~1 Hz). No red error message.

3. **Verify gateway is actually running**
   - In a terminal: `curl http://127.0.0.1:15151/` (or your config port).
   - **Expect:** JSON with `"status":"running"`.

4. **Stop gateway from desktop**
   - Click "Stop gateway".
   - **Expect:** Status shows "Gateway: stopped". `curl` to the same port should fail (connection refused).

5. **External gateway detection (optional)**
   - With desktop open and gateway stopped, start the gateway from the CLI in another terminal: `chai gateway` (or `cargo run -p cli -- gateway`).
   - **Expect:** Desktop updates to "Gateway: running" (it probes bind:port, so it doesn't matter who started it).
   - "Stop gateway" in the desktop will **not** stop this process (desktop only stops the process it started). Stop the gateway via Ctrl+C in the CLI terminal.

## If Something Fails

- **Desktop window does not open** — Check that the desktop binary is built (`cargo build -p desktop`) and your display is available (Wayland/X11). Run from a terminal to see error output.
- **"Start gateway" shows an error** — The gateway spawn may have failed. Common causes: Ollama not running, config error in `config.json`, or another gateway already running on the same profile (stale `~/.chai/profiles/<name>/gateway.lock`). Check the desktop error text and the config file.
- **Status stays "stopped" after clicking "Start"** — The gateway process may have exited immediately. Run `chai gateway` from a terminal to see the error output directly.
- **"Stop gateway" does not stop it** — The desktop can only stop the gateway process it started. If the gateway was started externally (CLI), use the CLI terminal's Ctrl+C.
- **Desktop does not detect an external gateway** — The detection probes the configured `gateway.bind`:`gateway.port` (default `127.0.0.1:15151`) at ~1 s intervals. If the external gateway uses a different port or bind, the desktop will not detect it.
- **`curl` returns "connection refused" after "Start"** — The gateway may not have finished starting yet. Wait a few seconds and retry. If still refused, check gateway logs for errors.

## Summary

| Step | Action | Expected Outcome |
|------|--------|-------------------|
| 1 | Launch `chai-desktop` | Window opens; "Gateway: stopped" |
| 2 | Click "Start gateway" | Status → "Gateway: running" |
| 3 | `curl http://127.0.0.1:15151/` | JSON: `"status": "running"` |
| 4 | Click "Stop gateway" | Status → "Gateway: stopped"; `curl` refused |
| 5 | Start gateway from CLI (optional) | Desktop detects it → "Gateway: running" |

**Next:** [04 — Channel: Telegram](04-channel-telegram.md) · [05 — Skill: Files](05-skill-files.md)
