# Telegram Reference

Reference for how the Telegram Bot API is used in this codebase and how this channel maps into the gateway. **Shared behavior for all channels** (inbound queue, **`process_inbound_message`**, bindings, shutdown) is specified in [CHANNELS.md](../spec/CHANNELS.md); this document focuses on Telegram-specific wiring.

## Purpose and How to Use

- **Purpose:** Document the current Telegram integration: config, long-poll vs webhook, HTTP route, and what Telegram does *not* send through **`InboundMessage`** today.
- **How to use:** Read [CHANNELS.md](../spec/CHANNELS.md) first; use this file for Bot API details and file-level pointers.

## Official Telegram Bot API

- **Overview:** https://core.telegram.org/bots/api  
- **Base URL:** `https://api.telegram.org/bot<token>/`  
- **Long-poll:** `getUpdates` with optional `timeout`  
- **Webhook:** `setWebhook`, `deleteWebhook`; updates POSTed to the gateway  

## Gateway Mapping (Telegram-Specific)

| Chai concept | Telegram source |
|--------------|-----------------|
| **`channel_id`** | Literal **`"telegram"`** from **`TelegramChannel::id()`** and inbound construction. |
| **`conversation_id`** | **`message.chat.id`** as decimal string (same value **`sendMessage`** expects as **`chat_id`**). |
| **`text`** | **`message.text`** only. Updates **without** a text body (photos, stickers, edited messages, etc.) are ignored: webhook returns **200** with no inbound; long-poll skips non-text messages. |

**`/new`** — Handled entirely in **`process_inbound_message`** in **`server.rs`** (same as every channel): trimmed text compared to **`/new`**; confirmation sent via **`send_message`**.

## Current Usage in the Codebase

### Modules

- **`crates/lib/src/channels/telegram.rs`** — **`TelegramChannel`**, **`TelegramUpdate`**, **`TelegramMessage`**, **`TelegramChat`**; long-poll loop **`run_get_updates_loop`**; **`get_updates`**, **`set_webhook`**, **`delete_webhook`**, **`send_message`**.
- **`crates/lib/src/channels/mod.rs`** — Re-exports **`TelegramChannel`**, **`TelegramUpdate`** for the gateway webhook handler.
- **`crates/lib/src/gateway/server.rs`** — Token resolution, **`register`** webhook vs long-poll, **`POST /telegram/webhook`**, **`shutdown_signal`** Telegram branch (**`delete_webhook`**).

### Configuration

| Source | Field / variable |
|--------|------------------|
| Config | **`channels.telegram.botToken`**, **`webhookUrl`**, **`webhookSecret`** (JSON **`camelCase`**) |
| Environment | **`TELEGRAM_BOT_TOKEN`** overrides config token when set |

Resolution: **`config::resolve_telegram_token()`** in **`crates/lib/src/config.rs`**.

### Startup (`run_gateway`)

When **`resolve_telegram_token`** returns **`Some`**:

1. **`TelegramChannel::new(Some(token))`** — **`Arc`** wrapped.
2. **Webhook** — If **`channels.telegram.webhookUrl`** is set: **`set_webhook(url, secret)`**; **`channel_registry.register("telegram", …)`**; **`telegram_webhook_for_shutdown`** kept for **`delete_webhook`**. No **`start_inbound`** task.
3. **Long-poll** — Else: **`start_inbound(inbound_tx)`** returns a **`JoinHandle`** stored in **`channel_tasks`**; **`channel_registry.register`**.

If the token is missing, no Telegram handle is registered and **`POST /telegram/webhook`** is still routed but inbound delivery depends on registry + handler behavior (updates would not be processed meaningfully without a registered send path for replies—token is required for **`send_message`** anyway).

### Inbound

| Mode | Mechanism |
|------|-----------|
| **Long-poll** | **`LONG_POLL_TIMEOUT`** = 30 s; on error, sleep 2 s and retry. **`stop()`** clears **`running`** flag; loop exits. |
| **Webhook** | **`telegram_webhook`**: optional **`X-Telegram-Bot-Api-Secret-Token`** vs **`webhookSecret`** (**403** if mismatch); JSON **`TelegramUpdate`**; **`503`** if **`inbound_tx`** closed. |

### Outbound

- **`ChannelHandle::send_message`** → **`TelegramChannel::send_message`** → **`sendMessage`** JSON body **`chat_id`**, **`text`**.
- **`reqwest::Client`** shared on **`TelegramChannel`**.

### Shutdown

See [CHANNELS.md](../spec/CHANNELS.md#shutdown). Telegram-specific: if **`telegram_webhook_for_shutdown`** is **`Some`**, **`delete_webhook`** runs after **`stop()`** on all channels; long-poll **`JoinHandle`** is awaited via **`channel_tasks`**.

### Environment (Optional)

- **`TELEGRAM_API_BASE`** — **`telegram_api_base()`** in **`telegram.rs`** (default **`https://api.telegram.org`**); not wired into **`get_updates`** / **`send_message`** in the main path (constant **`TELEGRAM_API_BASE`** used).

## Capabilities We Do Not Use Yet

- Edited messages, channels, inline queries, callback buttons, file uploads, typing indicators, parse modes (Markdown/HTML), or streaming partial replies to the chat.

## Related: Desktop

**`crates/desktop/src/app/screens/config.rs`** — Shows whether Telegram is configured (token or webhook URL). New channels will need parallel UI if the desktop should display them.

## User-Facing Verification

See [`.journey/05-channel-telegram.md`](../../.journey/05-channel-telegram.md) for end-to-step confirmation.

## See Also

- [CHANNELS.md](../spec/CHANNELS.md) — Internal spec for gateway channel behavior; read before implementing another channel.
