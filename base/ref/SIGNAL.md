---
status: current
---

# Signal Reference

Reference for the **Signal** channel in Chai. Gateway behavior is specified in [CHANNELS.md](../spec/CHANNELS.md). [TELEGRAM.md](TELEGRAM.md) shows the Telegram mapping for comparison. Signal is positioned as a **privacy-preserving** option ([Signal](https://signal.org/)); the service remains **centralized** (Signal-operated servers), which is a different axis from federation.

**Package and feature gate** — Signal lives in **`crates/adapters/signal`** (Cargo package **`signal-channel`**), behind **`lib`**'s optional **`signal`** Cargo feature. This mirrors the Matrix adapter pattern (see [MATRIX_ADAPTER.md](../adr/MATRIX_ADAPTER.md) and [SIGNAL_ADAPTER.md](../adr/SIGNAL_ADAPTER.md)). The thin wrapper in **`crates/lib/src/channels/signal.rs`** implements `ChannelHandle` via a newtype pattern; all signal-cli HTTP/SSE logic resides in the adapter package.

**Experimental status** — Signal is an **experimental** channel for v0.1.0. Basic text messaging works (SSE inbound, JSON-RPC send). Hardening items (reconnect tuning, richer receive payloads for attachments and edits) remain as follow-ups. Operators opt in via **`--features signal`**; the default build does not include Signal.

**Status** — **Implemented** in **`crates/adapters/signal/src/lib.rs`** (`SignalInner`) with a thin wrapper in **`crates/lib/src/channels/signal.rs`** (`SignalChannel` newtype): **`GET /api/v1/events`** (SSE) for JSON-RPC **`receive`** notifications with **`dataMessage.message`**; **`POST /api/v1/rpc`** with **`send`** (`recipient` +E.164 or **`groupId`**).

## Purpose and How to Use

- **Purpose:** Document how Signal connects to the gateway via BYO signal-cli; record config, wire protocol, and current capabilities and limitations.
- **How to use:** Read [CHANNELS.md](../spec/CHANNELS.md) first, then this file and [TELEGRAM.md](TELEGRAM.md) for a working reference implementation.

## Ecosystem Facts

- **Official clients** — Mobile and desktop apps; no official "bot API" comparable to Telegram's Bot API for the same automation model.
- **Third-party tooling** — [signal-cli](https://github.com/AsamK/signal-cli) provides an unofficial CLI with **JSON-RPC** and **D-Bus** interfaces, intended for server-style notification use cases. It depends on a **Java runtime** (see upstream docs for current JRE requirements) and **libsignal-client** native libraries; releases must stay current because Signal's servers and clients evolve frequently.

## Integration Approaches

| Approach | Pros | Cons |
|----------|------|------|
| **signal-cli JSON-RPC daemon** | Documented machine interface; matches "gateway + sidecar" deployment | Separate process to manage; JVM/native deps; GPL-3.0 license (see below) |
| **signal-cli subprocess** | Simple for one-off sends | Poor fit for sustained receive loops vs daemon |
| **D-Bus** | Fine on some Linux desktops | Awkward for headless server portability |

Chai uses the **signal-cli JSON-RPC HTTP daemon** approach.

## Chai Implementation Prerequisites

Everything in [CHANNELS.md](../spec/CHANNELS.md) applies. Signal-specific preparation:

| Topic | Requirement |
|-------|-------------|
| **`channel_id`** | Fixed id **`"signal"`** for **`ChannelHandle::id()`** and every **`InboundMessage`**; must match **`ChannelRegistry`** registration. |
| **`conversation_id`** | **E.164 phone number** (e.g. `+1234567890`) for 1:1 chats, or **base64 group id** from **`dataMessage.groupId`** for groups. Round-trips through **`send_message`** via **`recipient`** or **`groupId`** in the JSON-RPC `send` params. |
| **Text-only MVP** | Only **`dataMessage.message`** plain text is mapped to **`InboundMessage`**. Reactions, stickers, images, and other non-text types are ignored. Attachments and edits are not yet supported. |
| **Inbound connector** | SSE loop (**`GET /api/v1/events`**) runs as a **`JoinHandle`** in **`channel_tasks`**; **`stop()`** sets a flag that breaks the loop. |
| **Long-running work** | The SSE receive loop is tracked in **`channel_tasks`**; **`stop()`** unblocks it. |
| **Shutdown** | **`stop()`** plus await joined tasks; the gateway does not spawn or kill signal-cli (operators manage the daemon externally). |
| **Config** | **`ChannelsConfig.signal`** in **`crates/lib/src/config.rs`**; startup block in **`run_gateway`** (behind `#[cfg(feature = "signal")]`) next to the Telegram block. |
| **Gateway route** | No HTTP route needed — SSE is outbound from the gateway to signal-cli, not inbound push. |

**WebSocket `send` / `agent` echo** — Once a session is bound to **`("signal", conversation_id)`**, existing gateway code delivers assistant text to **`ChannelRegistry`**; no change to **`process_inbound_message`** is required if **`InboundMessage`** and **`ChannelHandle`** are correct.

## Registration and Operations (Upstream)

- Account identity is typically a **phone number** in international format; registration uses SMS or voice verification (see signal-cli README for landline and captcha flows).
- **Storage:** Credentials live under **`$XDG_DATA_HOME/signal-cli/data/`** or **`~/.local/share/signal-cli/data/`** (per upstream docs).
- **Receiving:** Signal's protocol expects **regular receipt** of messages (daemon or periodic `receive`); encryption and group state depend on it.

These constraints affect **deployment**, not just code: the gateway host must safely store keys and keep signal-cli **updated**.

## License and Distribution

- signal-cli is **GPL-3.0**. This project **does not** distribute signal-cli. Chai talks to a **user-run** daemon over HTTP only.

## Config Shape (Current)

Fields live on **`SignalChannelConfig`** (**`channels.signal`**, **`camelCase`** in JSON). Env vars override where noted.

| Field | Role |
|-------|------|
| **`httpBase`** | Base URL of the signal-cli HTTP daemon (e.g. `http://127.0.0.1:7583`). Env: **`SIGNAL_CLI_HTTP`**. Required for Signal. |
| **`account`** | Optional **`+E.164`** phone number for multi-account daemon mode. Included in JSON-RPC `params` when set. Env: **`SIGNAL_CLI_ACCOUNT`**. |

## Alignment with Telegram (Conceptual)

| Telegram concept | Signal analogue |
|------------------|-----------------|
| Bot token | signal-cli account / key material on disk |
| `getUpdates` / webhook | SSE **`GET /api/v1/events`** (JSON-RPC `receive` notifications) |
| `sendMessage` | JSON-RPC **`send`** via **`POST /api/v1/rpc`** |
| `conversation_id` | E.164 number or base64 group id from signal-cli payloads |

## Risks and Open Questions

- **ToS and automation** — Operators must comply with Signal's terms and acceptable use; this doc does not interpret policy.
- **Captcha / registration friction** — May block unattended setups.
- **Rate limits and blocking** — Centralized service may throttle or block misconfigured automations.
- **Hardening** — Reconnect tuning and richer receive payloads (attachments, edits) are follow-up items, not blockers for the experimental feature.

## Spike (`crates/spike`)

The **`signal-probe`** binary checks a running signal-cli **HTTP** daemon (`GET /api/v1/check`, **`POST /api/v1/rpc`** with **`listGroups`**, sample **`GET /api/v1/events`**). Run **`cargo run -p chai-spike --bin signal-probe`** after starting **`signal-cli daemon --http …`** (see **`crates/spike/README.md`**).

Upstream **JSON-RPC** documents **`receive`** notifications with **`params.envelope`** (1:1 **`dataMessage`**, groups, etc.) and **`send`** with **`recipient`** (E.164) or **`groupId`** (base64). Those fields are the practical basis for **`conversation_id`** and **`text`** in **`InboundMessage`**.

## References

- Signal — https://signal.org/
- signal-cli — https://github.com/AsamK/signal-cli
- JSON-RPC service (wiki) — https://github.com/AsamK/signal-cli/wiki/JSON-RPC-service
