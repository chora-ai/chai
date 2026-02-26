# Journey: Telegram — receive message and reply

**Goal:** Confirm the Telegram bot is connected to the gateway, messages from Telegram are received, and the agent’s reply is sent back to the chat.

The implementation supports two modes: **long-poll** (gateway pulls updates from Telegram) and **webhook** (Telegram POSTs updates to the gateway). Use long-poll for local testing; use webhook when the gateway is reachable from the internet.

## Prerequisites

- **`chai init`** has been run.
- **Config** has the Telegram bot token: in `~/.chai/config.json` set `channels.telegram.botToken` to your bot token, or set the `TELEGRAM_BOT_TOKEN` environment variable.
- **Ollama** is running locally with the default model (e.g. `llama3.2:latest`). The gateway runs one agent turn per inbound message and sends the model’s reply back to Telegram; if Ollama is unavailable or the model is missing, the message is received but no reply is sent (see verification below).
- **Long-poll only:** No webhook is currently set for your bot. If a webhook is set, Telegram sends updates only to that URL and getUpdates returns nothing. Remove the webhook first (e.g. via Bot API `deleteWebhook`, or by pointing the webhook at this gateway in webhook mode).

## Option A: Long-poll (local, no webhook)

Best when the gateway runs on your machine and is not exposed to the internet.

1. **Do not set** `channels.telegram.webhookUrl` in config (omit it or leave it null).

2. **Start the gateway**
   - From repo root: `cargo run -p cli -- gateway`
   - Or: `chai gateway`
   - Optional: `RUST_LOG=info` to see channel and agent logs.
   - **Expect:** Log line like `telegram channel registered and getUpdates loop started`.

3. **Send a message to your bot** in the Telegram app (any chat with the bot).

4. **Verify receipt and reply**
   - **In Telegram:** The bot should reply with the model’s response (e.g. a short greeting or answer). If the bot does not reply, see “If something fails” below.
   - **In logs (optional):** With `RUST_LOG=info` or `RUST_LOG=debug` you can confirm the channel is running and, if something fails, see lines such as `inbound: agent turn failed` or `inbound: send_message failed`.

5. **Stop the gateway** with Ctrl+C in the gateway terminal.

## Option B: Webhook (gateway reachable from internet)

Use when the gateway is exposed via a public URL (e.g. reverse proxy or tunnel like ngrok). Telegram will POST updates to your URL.

1. **Choose a public URL** for the webhook, e.g. `https://your-domain.com/telegram/webhook` or an ngrok URL like `https://abc123.ngrok.io/telegram/webhook`. The path must end with `/telegram/webhook` (the gateway serves `POST /telegram/webhook`).

2. **Config**
   - Set `channels.telegram.webhookUrl` to that full URL.
   - Optionally set `channels.telegram.webhookSecret` and configure the same secret in your reverse proxy or Telegram (BotFather / setWebhook `secret_token`) so the gateway can verify requests.

3. **Start the gateway**
   - Ensure the gateway is reachable at the webhook URL (e.g. start ngrok or your proxy).
   - Run: `chai gateway` (or `cargo run -p cli -- gateway`).
   - **Expect:** Log line like `telegram channel registered (webhook mode): <your url>`.

4. **Send a message to your bot** in the Telegram app.

5. **Verify**
   - **In Telegram:** The bot should reply with the model’s response.
   - **In logs:** With `RUST_LOG=info` you can confirm webhook registration; with `RUST_LOG=debug` you can see more detail if something fails.

6. **Stop the gateway** with Ctrl+C. The gateway will call `deleteWebhook` on shutdown so the bot can use getUpdates again if you switch back to long-poll.

## How to verify the message was received by the gateway

- **Primary:** The bot replies in Telegram. That implies the gateway received the update (via getUpdates or webhook), ran the agent turn, and called `send_message` successfully.
- **Logs:** With `RUST_LOG=info`, you should see the channel registered at startup. If a message is received but the agent fails (e.g. Ollama not running), you will see `inbound: agent turn failed` and no reply. If the agent succeeds but sending back fails, you will see `inbound: send_message failed`.
- **No programmatic “message received” API:** The current implementation does not expose a separate API or log line that says “message received” before the agent runs; receipt is evidenced by the agent running and/or the reply being sent.

## If something fails

- **Bot never replies (long-poll)**  
  - Ensure no webhook is set for the bot (getUpdates is ignored while a webhook is set). Call the Bot API `getWebhookInfo`; if it returns a URL, delete it with `deleteWebhook` or switch to webhook mode and point it at this gateway.  
  - Ensure the token in config (or `TELEGRAM_BOT_TOKEN`) is correct.  
  - Check logs for `telegram channel registered and getUpdates loop started` and for `telegram getUpdates error` (e.g. network or token issues).

- **Bot never replies (webhook)**  
  - Ensure the webhook URL is reachable from the internet and the path is exactly `/telegram/webhook`.  
  - If you use a secret, ensure it matches (header `X-Telegram-Bot-Api-Secret-Token`).  
  - Check logs for `telegram channel registered (webhook mode): ...` and for HTTP errors if Telegram cannot reach the URL.

- **“inbound: agent turn failed”**  
  - Ollama is not running, or the default model (e.g. `llama3.2:latest`) is not available. Start Ollama and pull the model, or set `agents.defaultModel` in config to a model you have.

- **“inbound: send_message failed”**  
  - The gateway received the message and ran the agent but could not send the reply (e.g. token invalid, network error, or Telegram API error). Check token and network.

## Restarting the session

The bot keeps conversation history per chat. To start with a clean history (new session), send **`/new`** in the chat. The bot will reply with a short confirmation and the next message you send will use a fresh session with no prior messages.

## Seeing if the model is thinking or if there was an error

The bot does **not** send a “typing” or “thinking” indicator to Telegram. You only see either the final reply or nothing.

To tell what is happening:

1. **Run the gateway with logging** so you can see activity and errors in the terminal:
   - `RUST_LOG=info cargo run -p cli -- gateway` or `RUST_LOG=info chai gateway`
   - For more detail (e.g. tool loops, model steps): `RUST_LOG=debug chai gateway`

2. **In the logs:**
   - **No new log line after you send a message** — The message may not have reached the gateway yet (webhook/network), or the agent turn is still running (Ollama thinking or a tool, e.g. Obsidian, still running). With `RUST_LOG=debug` you can see when the agent starts and when tools run.
   - **`inbound: agent turn failed: …`** — Something went wrong (Ollama down, model missing, session error, etc.). The gateway will send a short error message back to the chat when possible; the full error is in the log.
   - **`agent: tool … failed: …`** — A tool (e.g. Obsidian) failed. The model is given the error and may reply; if the turn later fails or the model returns nothing, you get no reply unless the gateway sends the generic error.

3. **After adding skills or tools (e.g. Obsidian):** If the model often uses tools, turns can take longer. If a tool hangs (e.g. Obsidian CLI not responding), the gateway will appear to “think” until it times out or errors. Use `RUST_LOG=debug` to see which tool ran and whether it failed.

## Summary

| Step              | Long-poll                          | Webhook                                      |
|-------------------|------------------------------------|----------------------------------------------|
| Config            | `botToken` set; no `webhookUrl`    | `botToken` and `webhookUrl` (and optional `webhookSecret`) |
| Start gateway     | `chai gateway`                     | Gateway reachable at webhook URL; then `chai gateway` |
| Send message      | In Telegram to your bot            | Same                                         |
| Verify            | Bot replies in Telegram; logs      | Same                                         |

The implementation is complete for this flow: messages are received (getUpdates or webhook), processed by `process_inbound_message` (session, agent turn, reply), and replies are sent with `send_message`. Verification is by observing the bot’s reply in Telegram and, if needed, logs.
