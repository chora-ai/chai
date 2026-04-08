# Journey: Desktop — start/stop gateway and detection

**Goal:** Use the desktop app to start the gateway, see “Gateway: running”, and stop it. Also verify that an externally started gateway is detected.

## Prerequisites

- Built project: `cargo build`
- Desktop runnable: `cargo run -p desktop` or installed `chai-desktop`
- Config (optional): **`~/.chai/profiles/<active>/config.json`** for the profile pointed at by **`~/.chai/active`** (default **assistant** after **`chai init`**). Set **`gateway.port`** there if you want a non-default port (**15151**). If the file is missing, defaults are used.

## Steps

1. **Launch desktop**
   - Run: `cargo run -p desktop` (or `chai-desktop`).
   - **Expect:** Window opens; gateway status shows “Gateway: stopped” (or “running” if something is already listening on the configured port).

2. **Start gateway from desktop**
   - Click “Start gateway”.
   - **Expect:** Status changes to “Gateway: running” within a few seconds (probe ~1 Hz). No red error message.

3. **Verify gateway is actually running**
   - In a terminal: `curl http://127.0.0.1:15151/` (or your config port).
   - **Expect:** JSON with `"status":"running"`.

4. **Stop gateway from desktop**
   - Click “Stop gateway”.
   - **Expect:** Status shows “Gateway: stopped”. `curl` to the same port should fail (connection refused).

5. **External gateway detection (optional)**
   - With desktop open and gateway stopped, start the gateway from the CLI in another terminal: `chai gateway` (or `cargo run -p cli -- gateway`).
   - **Expect:** Desktop updates to “Gateway: running” (it probes bind:port, so it doesn’t matter who started it).
   - “Stop gateway” in the desktop will **not** appear or will not stop this process (desktop only stops the process it started). Stop the gateway via Ctrl+C in the CLI terminal.

## Notes

- Detection is by TCP connect to the configured bind and port (~1 s interval, 800 ms timeout).
- If config load fails or spawn fails, an error is shown in the UI (e.g. red text).
