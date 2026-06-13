# Journey: Setup — init, configure, and verify

**Goal:** Go from a clean slate to a running gateway that responds to a chat message. This is the on-ramp: all other journeys assume you have completed these steps (or have equivalent setup).

**Background:** [Getting Started](../guides/02-getting-started.md) · [Configuration](../guides/03-configuration.md)

## Prerequisites

- **Rust toolchain** — Install via [rustup](https://rustup.rs/) if you don't have it. Verify: `rustc --version`.
- **Ollama** — The default provider. Install from [ollama.com](https://ollama.com) and pull a model:
  ```bash
  ollama pull llama3.2:3b
  ```
  Ollama must be running before you start the gateway (`ollama serve` or the system tray app).

## Steps

1. **Install the CLI**

   - From the repo root:
     ```bash
     cargo install --path crates/cli
     ```
     Add `--features matrix` if you plan to use the Matrix channel adapter.
   - Verify:
     ```bash
     chai version
     ```
   - **Expect:** A version string (e.g. `chai 0.1.0`).

2. **Initialize the configuration**

   - Run:
     ```bash
     chai init
     ```
   - **Expect:** Output confirming the configuration directory was created.
   - Verify the directory:
     ```bash
     ls ~/.chai/
     ```
   - **Expect:** `active` (symlink), `profiles/`, `skills/`, and `gateway.lock` (may be absent until the gateway runs).

3. **Review the default profile**

   - Check the active profile:
     ```bash
     chai profile current
     ```
   - **Expect:** `assistant` (the default).
   - Inspect the config:
     ```bash
     cat ~/.chai/profiles/assistant/config.json
     ```
   - **Expect:** A JSON file — possibly just `{}` (empty is valid; built-in defaults supply Ollama + `llama3.2:3b`).

4. **Confirm Ollama is running and the model is available**

   - Check:
     ```bash
     ollama list
     ```
   - **Expect:** A table listing at least one model (e.g. `llama3.2:3b`). If the list is empty or the model is missing, pull it: `ollama pull llama3.2:3b`.

5. **Start the gateway**

   - Run:
     ```bash
     chai gateway
     ```
   - **Expect:** Log output confirming startup, provider discovery, and skill loading. Look for lines like:
     - `provider ollama discovered N model(s)`
     - `loaded 0 skill(s) for agent context` (no skills enabled by default — this is fine)
     - Gateway listening on `127.0.0.1:15151`

6. **Verify the gateway is healthy**

   - In another terminal:
     ```bash
     curl http://127.0.0.1:15151/
     ```
   - **Expect:** JSON with `"status": "running"`, `"protocol": 1`, `"port": 15151`.

7. **Chat via the CLI**

   - In another terminal:
     ```bash
     chai chat
     ```
   - Type a message: `Say hello in one short sentence.` Press Enter.
   - **Expect:** A reply from the model (e.g. a short greeting).
   - Type `/exit` to leave the chat.

8. **Stop the gateway**

   - In the gateway terminal: Ctrl+C.
   - **Expect:** Process exits; no timeout.

## If Something Fails

- **`chai: command not found`** — The CLI is not installed or not on your PATH. Re-run `cargo install --path crates/cli` and check your Cargo bin directory is on PATH.
- **`chai init` errors** — Ensure `~/.chai/` is writable and not locked by another process. If the directory exists and is in a bad state, you can remove it and re-run `chai init` (this destroys all profiles and skills).
- **`ollama list` is empty or model missing** — Pull the model: `ollama pull llama3.2:3b`. Ensure Ollama is running: `ollama serve` (or the system tray app).
- **Gateway exits immediately** — Check the error output. Common causes: Ollama not running, another gateway already running (the advisory lock at `~/.chai/gateway.lock` prevents duplicates), or a config error.
- **`curl` returns "connection refused"** — The gateway is not running or is on a different port. Check the gateway terminal for the actual port, or set `gateway.port` in config (see [Configuration](../guides/03-configuration.md)).
- **`chai chat` returns no reply or errors** — Ensure Ollama is running and the model is available. Check the gateway terminal for error lines like `agent turn failed`.
- **`curl` returns `"status": "running"` but `chai chat` hangs** — The gateway is up but the model may be slow to respond (first inference can take time while the model loads into memory). Wait 30–60 s; if still no reply, check gateway logs for errors.

## Summary

| Step | Action | Expected Outcome |
|------|--------|-------------------|
| 1 | Install CLI (`cargo install`) | `chai version` prints a version |
| 2 | `chai init` | `~/.chai/` created with profiles, skills, active symlink, and skills.lock |
| 3 | `chai profile current` | `assistant` |
| 4 | `ollama list` | At least one model listed |
| 5 | `chai gateway` | Startup logs; listening on `127.0.0.1:15151` |
| 6 | `curl http://127.0.0.1:15151/` | `"status": "running"` |
| 7 | `chai chat` → send message | Model replies |
| 8 | Ctrl+C gateway | Process exits |

**Next:** Continue to [01 — Gateway (CLI): health and WebSocket connect](01-gateway-cli-health-and-ws.md) to test the WebSocket protocol, or pick a channel journey ([04 — Telegram](04-channel-telegram.md), [08 — Matrix](08-channel-matrix.md), [09 — Signal](09-channel-signal.md)) or a skill journey ([05 — Files](05-skill-files.md), [06 — Knowledge Base](06-skill-kb.md), [07 — Skills](07-skill-skills.md)) to explore further.
