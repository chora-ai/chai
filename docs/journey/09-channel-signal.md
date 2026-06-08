# Journey: Signal — receive message and reply

**Goal:** Confirm a **user-run** signal-cli **HTTP daemon** is reachable, the gateway consumes **`receive`** events from **`GET /api/v1/events`**, and agent replies are sent via **`POST /api/v1/rpc`** (**`send`**).

**Background:** [Connections → Signal](../guides/04-connections.md#signal) · [Configuration → Channels](../guides/03-configuration.md#configuring-channels)

Chai **does not** install signal-cli. Install it from [upstream](https://github.com/AsamK/signal-cli) and register your number separately. Policy: **`base/adr/SIGNAL_CLI_INTEGRATION.md`**.

## Prerequisites

- **Setup complete** — You have installed chai, run `chai init`, and verified Ollama is available (see [00-setup-init.md](00-setup-init.md)).
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

4. **Verify receipt and reply**
   - **In Signal:** The Chai-linked account should reply with the model's text.
   - **In logs (optional):** With `RUST_LOG=info` or `RUST_LOG=debug`, you can confirm the channel is running and, if something fails, see lines such as `inbound: agent turn failed` or `inbound: send_message failed`.

5. **Stop the gateway** with Ctrl+C. Stop the signal-cli daemon when finished.

## How to Verify the Message Was Received by the Gateway

- **Primary:** The Chai-linked account replies in Signal. That implies the gateway received the SSE event, ran the agent turn, and called `send_message` successfully.
- **Logs:** With `RUST_LOG=info`, you should see the channel registered at startup. If a message is received but the agent fails (e.g. Ollama not running), you will see `inbound: agent turn failed` and no reply. If the agent succeeds but sending back fails, you will see `inbound: send_message failed`.
- **No programmatic "message received" API:** The current implementation does not expose a separate API or log line that says "message received" before the agent runs; receipt is evidenced by the agent running and/or the reply being sent.

## `/new` and Session

Send **`/new`** (case-insensitive) to start a **new session** for that conversation (1:1 or group context). The gateway sends a short confirmation via **`send`**.

## If Something Fails

- **`signal: daemon check failed`** — signal-cli is not running or **`httpBase`** / **`SIGNAL_CLI_HTTP`** does not match the daemon's **`--http`** host and port. Verify the daemon URL with `curl http://127.0.0.1:7583/api/v1/check` (adjust host/port to match your daemon).
- **`signal channel registered` but no reply** — The provider or model may be down. Check gateway logs for `inbound: agent turn failed`. Ensure Ollama is running and the default model is available (`ollama list`).
- **`inbound: send_message failed`** — The gateway received the message and ran the agent but could not send the reply. Check signal-cli logs for send errors (e.g. unregistered number, network issue, or rate limiting).
- **Inbound event received but no agent turn** — Only plain **`dataMessage.message`** text is handled today. If the inbound message is a reaction, sticker, image, or other non-text type, the gateway ignores it. Send a plain text message.
- **Multi-account daemon** — If the daemon serves multiple accounts without **`-a`**, set **`account`** / **`SIGNAL_CLI_ACCOUNT`** so JSON-RPC `params` include the right **`+E.164`**. Without this, signal-cli may route to the wrong account or reject the request.
- **Message received but reply goes to wrong conversation** — The gateway uses the same `conversationId` from the inbound event for the reply. If signal-cli is misconfigured (wrong account or number mapping), the reply may not reach you. Verify the account number matches.
- **SSE connection drops** — The gateway will attempt to reconnect. If signal-cli was restarted, the SSE stream may need a moment to re-establish. Check gateway logs for reconnect messages.

## Wire-only Check (no Gateway)

`cargo run -p chai-spike --bin signal-probe` — see **`crates/spike/README.md`**.

## Summary

| Step | Action | Expected Outcome |
|------|--------|-------------------|
| 1 | Start signal-cli daemon | Listening on configured host:port |
| 2 | `chai gateway` | `signal: daemon check ok` + `channel registered` |
| 3 | Send text message to Signal number | Gateway receives SSE event |
| 4 | Verify reply in Signal logs | Agent reply sent to conversation |
| 5 | Ctrl+C gateway + signal-cli | Processes exit |

**See also:** [04 — Channel: Telegram](04-channel-telegram.md) · [08 — Channel: Matrix](08-channel-matrix.md)
