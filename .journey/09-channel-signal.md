# Journey: Signal — receive message and reply

**Goal:** Confirm a **user-run** signal-cli **HTTP daemon** is reachable, the gateway consumes **`receive`** events from **`GET /api/v1/events`**, and agent replies are sent via **`POST /api/v1/rpc`** (**`send`**).

Chai **does not** install signal-cli. Install it from [upstream](https://github.com/AsamK/signal-cli) and register your number separately. Policy: **`.agents/adr/SIGNAL_CLI_INTEGRATION.md`**.

## Prerequisites

- **`chai init`** has been run.
- **signal-cli** installed; your Signal account registered with signal-cli (`register` / `verify` per upstream).
- **Daemon** — In a separate terminal, start the HTTP daemon (example):

  ```bash
  signal-cli -a +1234567890 daemon --http 127.0.0.1:7583
  ```

  Use your real **`+E.164`** account. Leave this running while testing.

- **Config** — Set **`channels.signal.httpBase`** to match the daemon (e.g. **`http://127.0.0.1:7583`**) or export **`SIGNAL_CLI_HTTP`**. For multi-account daemon mode, set **`channels.signal.account`** or **`SIGNAL_CLI_ACCOUNT`**.
- **Ollama** (or your default provider) running with the configured model.

## Steps

1. **Start signal-cli** (daemon command above) and wait until it is listening.

2. **Start the gateway**
   - `chai gateway` or `cargo run -p cli -- gateway`
   - Optional: `RUST_LOG=info`
   - **Expect:** `signal: daemon check ok at http://.../api/v1/check` and `signal channel registered and SSE events loop started`.

3. **Send a message** to the Signal number used by signal-cli from **another** phone or Signal client (1:1 chat), or send in a **group** the account has joined.

4. **Verify**
   - **In Signal:** The Chai-linked account should reply with the model’s text.
   - **Logs:** `inbound` / agent errors if something fails.

5. **Stop the gateway** with Ctrl+C. Stop the signal-cli daemon when finished.

## `/new` and session

Send **`/new`** (case-insensitive) to start a **new session** for that conversation (1:1 or group context). The gateway sends a short confirmation via **`send`**.

## If something fails

- **`signal: daemon check failed`** — signal-cli is not running or **`httpBase`** / **`SIGNAL_CLI_HTTP`** does not match the daemon’s **`--http`** host and port.
- **No reply** — Provider/model down (`inbound: agent turn failed`); or inbound event not parsed (e.g. non-text message type). Only plain **`dataMessage.message`** text is handled today.
- **Multi-account** — If the daemon serves multiple accounts without **`-a`**, set **`account`** / **`SIGNAL_CLI_ACCOUNT`** so JSON-RPC `params` include the right **`+E.164`**.

## Wire-only check (no gateway)

`cargo run -p chai-spike --bin signal-probe` — see **`crates/spike/README.md`**.

## Summary

| Step | Action |
|------|--------|
| Daemon | `signal-cli -a +… daemon --http HOST:PORT` |
| Config | `channels.signal.httpBase` or `SIGNAL_CLI_HTTP` |
| Gateway | `chai gateway` |
| Message | Send a text message to that Signal account |
| Verify | Reply in Signal; optional logs |
