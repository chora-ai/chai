## Connections

### WebSocket

Clients connect at `ws://<bind>:<port>/ws` (from **`gateway.bind`** and **`gateway.port`**), call **`connect`**, then **`agent`** (run a model turn), **`send`** (deliver text on a channel), **`status`** (runtime snapshot), or **`health`** (lightweight probe). Used by the desktop application and for scripting.

When **`gateway.bind`** is not loopback, use **`gateway.auth`** with **`mode`** **`token`** and a secret (or **`CHAI_GATEWAY_TOKEN`**).

### Telegram

**Long-poll** — The gateway calls Telegram’s **`getUpdates`**; good for local use. Set **`channels.telegram.botToken`** (or **`TELEGRAM_BOT_TOKEN`**).

**Webhook** — Telegram POSTs updates to your URL; better for a public gateway. Set **`channels.telegram.webhookUrl`** and optionally **`channels.telegram.webhookSecret`** (or **`TELEGRAM_WEBHOOK_SECRET`**).

### Signal

The gateway connects to a **BYO** signal-cli **`daemon --http`** instance: **`GET /api/v1/events`** (SSE) for inbound messages and **`POST /api/v1/rpc`** with method **`send`** for replies. Install and run signal-cli yourself (see upstream docs); start the daemon before the gateway, e.g. **`signal-cli -a +1234567890 daemon --http 127.0.0.1:7583`**, then set **`channels.signal.httpBase`** or **`SIGNAL_CLI_HTTP`**. Policy: **`base/adr/SIGNAL_CLI_INTEGRATION.md`**. **`/new`** in a 1:1 or group context starts a fresh session for that **`conversation_id`**, same as other channels.

### Matrix

The gateway uses **[matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk)** with a **SQLite** store fixed at **`<active-profile>/matrix`** (matrix-sdk state and E2EE keys). It syncs with the **Client-Server API**, decrypts **encrypted** rooms when the account has keys, and sends replies with **`m.room.message`** (**plain text**; encrypted in **encrypted** rooms). Configure **`channels.matrix`** (see **Configuration → Channels**) or the **`MATRIX_*`** environment variables. The bot user must already be a member of rooms you expect to use; invite the bot from Element (or another client) first. **`/new`** in a room starts a fresh session for that room, same as Telegram.
