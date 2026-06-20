# Connections

Channels connect the gateway to messaging platforms. All channels deliver messages to the same agent, so you can start a conversation on one channel and continue on another.

## WebSocket

Clients connect at `ws://<bind>:<port>/ws` (from `gateway.bind` and `gateway.port`), call `connect`, then `agent` (run a model turn), `send` (deliver text on a channel), `stop` (pause the current agent turn), `agentDetail` (heavy per-agent data on demand), `status` (runtime snapshot), or `health` (lightweight probe). Used by the desktop application and for scripting.

When `gateway.bind` is not loopback, use `gateway.auth` with `mode` `token` and a secret (or `CHAI_GATEWAY_TOKEN`). See [Configuration](03-configuration.md#securing-the-gateway) for the security setup.

**Device pairing and token fallback:** the desktop pairs with the gateway using a device identity and signature, then receives a `device_token` for subsequent connections. If a paired `device_token` becomes stale (e.g., the profile's `paired.json` was deleted), the gateway rejects the connect with `"invalid device token"`. The desktop deletes the stale token and falls back to the device identity + signature flow to re-pair automatically.

**Try it:** [Gateway (CLI) — health and WebSocket connect](../journey/01-gateway-cli-health-and-ws.md) · [Gateway WebSocket — agent & send](../journey/02-gateway-ws-agent.md)

## Telegram

**Long-poll** — The gateway calls Telegram's `getUpdates`; good for local use. Set `channels.telegram.botToken` (or `TELEGRAM_BOT_TOKEN`).

**Webhook** — Telegram POSTs updates to your URL; better for a public gateway. Set `channels.telegram.webhookUrl` and optionally `channels.telegram.webhookSecret` (or `TELEGRAM_WEBHOOK_SECRET`).

**Try it:** [Telegram — receive message and reply](../journey/04-channel-telegram.md)

## Signal (Experimental)

Signal is an **experimental** channel. It requires the `signal` Cargo feature at build time (`--features signal`) and a running signal-cli HTTP daemon. Basic text messaging and attachment metadata work; reconnect tuning uses exponential backoff.

The gateway connects to a bring-your-own signal-cli `daemon --http` instance: `GET /api/v1/events` (SSE) for inbound messages and `POST /api/v1/rpc` with method `send` for replies. Install and run signal-cli yourself (see upstream docs); start the daemon before the gateway, for example:

```bash
signal-cli -a +1234567890 daemon --http 127.0.0.1:7583
```

Then set `channels.signal.httpBase` or `SIGNAL_CLI_HTTP`.

`/new` in a 1:1 or group context starts a fresh session for that `conversation_id`, same as other channels.

**Try it:** [Signal — receive message and reply](../journey/09-channel-signal.md)

## Matrix (Experimental)

Matrix is an **experimental** channel. It requires the `matrix` Cargo feature at build time (`--features matrix`). Basic messaging and E2EE work; sync retries use exponential backoff with rate-limit awareness.

The gateway uses [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk) with a SQLite store fixed at `<active-profile>/matrix` (matrix-sdk state and E2EE keys). It syncs with the Client-Server API, decrypts encrypted rooms when the account has keys, and sends replies with `m.room.message` (plain text; encrypted in encrypted rooms).

Configure `channels.matrix` (see [Configuration](03-configuration.md#configuring-channels)) or the `MATRIX_*` environment variables. The bot user must already be a member of rooms you expect to use; invite the bot from Element (or another client) first. `/new` in a room starts a fresh session for that room, same as in other channels.

**Try it:** [Matrix — receive message and reply](../journey/08-channel-matrix.md)

## Secrets and Rotation

Chai channels use secrets (bot tokens, access tokens, passwords) that should be rotated periodically. All secrets can be provided via environment variables, which is the recommended approach for production deployments:

| Secret | Environment Variable | Notes |
|--------|---------------------|-------|
| Gateway auth token | `CHAI_GATEWAY_TOKEN` | Rotate by changing the env var and restarting the gateway |
| Telegram bot token | `TELEGRAM_BOT_TOKEN` | Revoke and reissue via BotFather |
| Telegram webhook secret | `TELEGRAM_WEBHOOK_SECRET` | Rotate by changing the env var; gateway calls `setWebhook` on next startup |
| Signal daemon URL | `SIGNAL_CLI_HTTP` | Not a secret per se, but changing it requires restarting signal-cli |
| Signal account | `SIGNAL_CLI_ACCOUNT` | Phone number for multi-account daemon mode |
| Matrix access token | `MATRIX_ACCESS_TOKEN` | Revoke and reissue via your homeserver admin panel; delete `<profile>/matrix` store if device id changes |
| Matrix password | `MATRIX_PASSWORD` | Change via your homeserver; no gateway restart needed beyond the normal reconnect |
| Matrix device id | `MATRIX_DEVICE_ID` | Required for token restore when `/account/whoami` omits it |

To rotate a secret:

1. Update the environment variable (or `config.json` field).
2. Restart the gateway: `chai gateway` (or restart the desktop app).
3. For Matrix access token changes, also delete the `<profile>/matrix` directory if the new token corresponds to a different device.
4. For Telegram webhook secrets, the gateway re-registers the webhook on startup, so no manual step is needed.

**Prefer environment variables over `config.json` for secrets.** This keeps credentials out of the profile directory and makes rotation a single env-change + restart.
