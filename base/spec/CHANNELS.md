---
status: stable
---

# Messaging Channels

This document describes how **messaging channels** connect to the Chai gateway: **`InboundMessage`** ingestion, **`ChannelHandle`** registration, **`SessionBindingStore`** routing, WebSocket **`send`** / **`agent`** delivery, and shutdown. It is an internal spec aligned with the current implementation in **`crates/lib`** (`channels/`, `routing.rs`, `gateway/server.rs`, `gateway/protocol.rs`, `config.rs`).

For channel-specific reference documentation, see [TELEGRAM.md](../ref/TELEGRAM.md), [MATRIX.md](../ref/MATRIX.md), and [SIGNAL.md](../ref/SIGNAL.md). For adapter package design decisions, see [MATRIX_ADAPTER.md](../adr/MATRIX_ADAPTER.md) and [SIGNAL_ADAPTER.md](../adr/SIGNAL_ADAPTER.md). For experimental **Matrix** / **Signal** wire probes (not the gateway), see **`crates/spike/`** and **`crates/spike/README.md`**.

## Feature Gates

Optional channel integrations are isolated in adapter packages and gated behind Cargo features:

| Channel | Adapter Package | Feature | Default | Status |
|---------|----------------|---------|---------|--------|
| **Telegram** | — (in **`lib`**) | — | Always on | Supported |
| **Matrix** | **`crates/adapters/matrix`** (`matrix-channel`) | **`matrix`** | Off | Experimental (opt-in) |
| **Signal** | **`crates/adapters/signal`** (`signal-channel`) | **`signal`** | Off | Experimental (opt-in) |

When a feature is off, **`lib`** compiles a stub module so the gateway builds without that channel's types. Operators enable opt-in channels at install time (e.g. **`cargo install --path crates/cli --features matrix`**).

## Core Types

### `InboundMessage`

Defined in **`crates/lib/src/channels/inbound.rs`**:

| Field | Role |
|-------|------|
| **`channel_id`** | Stable string identifier for the integration. Must match the key used in **`ChannelRegistry`** and **`ChannelHandle::id()`** (e.g. `"telegram"`). |
| **`conversation_id`** | Opaque string that identifies one chat/thread on that channel. Must be whatever **`send_message`** needs to deliver an outbound text message to the same place (e.g. Telegram chat id as decimal string). |
| **`text`** | User message body. Only plain text is modeled today. |

There is no attachment, reply-to, or user-id field on **`InboundMessage`**; anything beyond text requires extending the type or a parallel path.

### `ChannelHandle`

Defined in **`crates/lib/src/channels/registry.rs`** (`async_trait`):

| Method | Contract |
|--------|----------|
| **`id()`** | Returns **`channel_id`** for this connector. Inbound messages for this channel must use the same string. |
| **`stop()`** | Called on shutdown for every registered channel; should end long-poll loops or disconnect. Safe to call idempotently. |
| **`send_message(conversation_id, text)`** | Delivers **`text`** to **`conversation_id`**. Used for normal replies, **`/new`** confirmation, WebSocket **`send`**, and agent turns that echo to a bound channel. Errors are logged or returned to the WebSocket client; there is no automatic retry. |
| **`status_detail()`** | Returns a JSON object (no secrets) merged into **`status.channels.<id>`** for operators and desktop; default empty object. See [GATEWAY_STATUS.md](GATEWAY_STATUS.md). |

**`ChannelRegistry::channel_status_details`** collects **`status_detail().await`** for each registered id.

### Session binding

**`SessionBindingStore`** (**`crates/lib/src/routing.rs`**) maps **`(channel_id, conversation_id)` ↔ `session_id`**. Inbound processing uses this so each conversation on a channel gets its own session history.

## Inbound Path

1. **Queue** — **`GatewayState::inbound_tx`** is an **`mpsc::Sender<InboundMessage>`** with buffer **64** (see **`run_gateway`** in **`crates/lib/src/gateway/server.rs`**). If the queue is full, **`send().await`** blocks; if the receiver is gone, sends fail (the Telegram webhook handler returns **503**).

2. **Processor** — A single spawned task drains the queue and, for each message, runs **`process_inbound_message`** (same file). Processing is **sequential** (one inbound at a time globally across all channels).

3. **`process_inbound_message`** (text channels):
   - Trims inbound text. If it equals **`/new`** (case-insensitive), creates a new session, rebinds **`(channel_id, conversation_id)`**, removes the old session store entry, sends a fixed confirmation string via **`send_message`**, and returns.
   - Otherwise: resolve or create **`session_id`**, **`bindings.bind`**, append the user message to **`SessionStore`**, **`broadcast_session_message`** (WebSocket **`session.message`** with **`channelId`** / **`conversationId`**), run **`agent::run_turn_dyn`** with orchestrator tools, then if the turn produced non-empty assistant **`content`**, broadcast again and **`send_message`** with that reply.
   - On agent error, sends a fallback error string via **`send_message`** if the channel handle exists.
   - **`channel_reply_text`** trims assistant content; **empty content means no outbound message** (e.g. tool-only turns with no assistant text).

4. **Registry lookup** — Replies and **`/new`** confirmations use **`state.channel_registry.get(&msg.channel_id)`**. If registration is missing, outbound sends are skipped (a warning is logged for the normal reply path).

## Outbound Path (Non-Inbound)

- **WebSocket `send`** — Method **`send`**, params **`channelId`**, **`conversationId`**, **`message`** (**`SendParams`** in **`crates/lib/src/gateway/protocol.rs`**). Looks up **`ChannelRegistry`** by **`channel_id`** and calls **`send_message`**. Used for explicit delivery (e.g. desktop or scripts), not for the main agent loop.

- **WebSocket `agent`** — After a successful turn, if the session has a **channel binding** and the reply has non-empty text, the gateway also calls **`send_message`** to that channel. Sessions created only over WebSocket may have **no** binding until an inbound channel message ties them (typical desktop flow).

## Registration at Startup

- **`ChannelRegistry::register(id, handle)`** stores **`Arc<dyn ChannelHandle>`**. Registering the same id **replaces** the handle and calls **`stop()`** on the previous one.

- **Telegram** — If the token is present: in webhook mode the handle is registered without a long-poll task; in long-poll mode **`start_inbound`** spawns a **`JoinHandle`** pushed to **`channel_tasks`** so shutdown can **`await`** it.

- **Matrix** — If credentials are present and the **`matrix`** feature is enabled: **`connect_matrix_client`** builds a **`MatrixChannel`**, registers it, and starts the sync loop as a **`JoinHandle`** in **`channel_tasks`**.

- **Signal** — If **`httpBase`** is present and the **`signal`** feature is enabled: **`SignalChannel`** is registered and the SSE events loop starts as a **`JoinHandle`** in **`channel_tasks`**.

- **New channels** — Must register under a **unique** **`channel_id`** before any inbound message is processed. Any long-running task should be tracked in **`channel_tasks`** the same way.

## HTTP Routes

- **Telegram** — **`POST /telegram/webhook`** with an optional secret header (**`X-Telegram-Bot-Api-Secret-Token`**).
- **Matrix** — When the Matrix client is connected, **`GET` / `POST`** routes under **`/matrix/verification/*`** support **E2EE interactive verification** (SAS) without relying on Element; see [MATRIX.md](../ref/MATRIX.md).

New channels that use HTTP push need new routes on the **`Router`** in **`run_gateway`** and **`State<GatewayState>`** to access **`inbound_tx`**.

## Shutdown

**`shutdown_signal`** (**`server.rs`**):

1. Broadcast shutdown JSON to WebSockets.
2. For every id in **`channel_registry`**, **`handle.stop()`**.
3. **Telegram-specific:** if webhook mode was used, **`delete_webhook`** on the **`TelegramChannel`** instance kept for shutdown.
4. **`await`** each **`JoinHandle`** in **`channel_tasks`**.

New channels with extra cleanup (logout, disconnect Matrix client, stop sidecar) should follow the same pattern: prefer **`stop()`** plus awaited tasks; add dedicated shutdown parameters only when necessary (as with Telegram's webhook delete).

## Configuration

**`ChannelsConfig`** in **`crates/lib/src/config.rs`** includes **`telegram`**, **`matrix`**, and **`signal`**. Each channel has its own struct, **`serde(rename_all = "camelCase")`** fields, env resolution helpers where applicable, and JSON / desktop config UI updates as needed.

## Desktop and Events

**`session.message`** events include optional **`channelId`** and **`conversationId`** when the gateway knows them. The desktop stores these per session for display and for follow-up **`send`** calls. New channels should use the same event shape when originating from inbound processing.

## Checklist for a New Channel

| Requirement | Detail |
|-------------|--------|
| **`channel_id`** | Single stable id string; consistent across **`InboundMessage`**, registry, and config docs. |
| **`conversation_id`** | Round-trips through **`send_message`**; stable for the lifetime of a "chat" on that network. |
| **Text-only MVP** | Match current **`InboundMessage`** unless you extend the struct. |
| **Register + optional tasks** | **`ChannelRegistry::register`**; push **`JoinHandle`**s to **`channel_tasks`** for work that must complete on shutdown. |
| **`stop()`** | Unblock long-poll, cancel sync, or signal subprocess shutdown. |
| **Gateway wiring** | Startup registration in **`run_gateway`**; **`Router`** routes for HTTP ingress; extend **`shutdown_signal`** if channel-specific teardown is required beyond **`stop()`** + task await. |
| **Config** | Extend **`ChannelsConfig`** and resolution; document env vars. |
| **Feature gate** | Optional channels should live in **`crates/adapters/<name>`** behind a Cargo feature on **`lib`**; include a stub when the feature is off. |

## Key Files

| File | Responsibility |
|------|----------------|
| **`crates/lib/src/channels/inbound.rs`** | **`InboundMessage`**. |
| **`crates/adapters/matrix`** (package **`matrix-channel`**) | **`MatrixInner`**, **`connect_with_params`**, **`RawInbound`** ([matrix-sdk](https://github.com/matrix-org/matrix-rust-sdk): SQLite + E2EE, **`/sync`**, **`m.room.message`** send). |
| **`crates/lib/src/channels/matrix.rs`** | **`MatrixChannel`** newtype + **`ChannelHandle`**, bridges **`RawInbound`** → **`InboundMessage`** (when **`lib`** **`matrix`** feature is on). |
| **`crates/adapters/signal`** (package **`signal-channel`**) | **`SignalInner`**, SSE events loop, JSON-RPC **`send`** (when **`lib`** **`signal`** feature is on; currently in **`crates/lib/src/channels/signal.rs`**). |
| **`crates/lib/src/channels/signal.rs`** | **`SignalChannel`**, **`resolve_signal_daemon_config`** (thin wrapper; to migrate to adapter package). |
| **`crates/lib/src/channels/registry.rs`** | **`ChannelHandle`**, **`ChannelRegistry`**. |
| **`crates/lib/src/routing.rs`** | **`SessionBindingStore`**. |
| **`crates/lib/src/gateway/server.rs`** | **`process_inbound_message`**, queue, registration, shutdown, webhook handlers. |
| **`crates/lib/src/gateway/protocol.rs`** | **`SendParams`** (`channelId`, `conversationId`, `message`). |
| **`crates/lib/src/config.rs`** | **`ChannelsConfig`**, per-channel config structs. |
