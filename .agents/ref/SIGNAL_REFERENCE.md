---
status: current
---

# Signal Reference

Reference for the **Signal** channel in Chai. Gateway behavior is specified in [CHANNELS.md](../spec/CHANNELS.md). [TELEGRAM_REFERENCE.md](TELEGRAM_REFERENCE.md) shows the Telegram mapping for comparison. Signal is positioned as a **privacy-preserving** option ([Signal](https://signal.org/)); the service remains **centralized** (Signal-operated servers), which is a different axis from federation.

**Distribution and license (project policy)** — Chai **does not** ship signal-cli; operators **BYO** an install and run the **HTTP daemon** (see [SIGNAL_CLI_INTEGRATION.md](../adr/SIGNAL_CLI_INTEGRATION.md)). Repository **LICENSE** files are unchanged for this integration.

**Status** — **Implemented** in **`crates/lib/src/channels/signal.rs`**: **`GET /api/v1/events`** (SSE) for JSON-RPC **`receive`** notifications with **`dataMessage.message`**; **`POST /api/v1/rpc`** with **`send`** (`recipient` +E.164 or **`groupId`**). **User journey:** [`.journey/09-channel-signal.md`](../../.journey/09-channel-signal.md).

## Purpose and How to Use

- **Purpose:** Record how Signal could connect to the gateway without assuming a final design; highlight **signal-cli** and operational requirements.
- **How to use:** Read [CHANNELS.md](../spec/CHANNELS.md) first, then this file and [TELEGRAM_REFERENCE.md](TELEGRAM_REFERENCE.md) for a working reference implementation.

## Ecosystem Facts

- **Official clients** — Mobile and desktop apps; no official “bot API” comparable to Telegram’s Bot API for the same automation model.
- **Third-party tooling** — [signal-cli](https://github.com/AsamK/signal-cli) provides an unofficial CLI with **JSON-RPC** and **D-Bus** interfaces, intended for server-style notification use cases. It depends on a **Java runtime** (see upstream docs for current JRE requirements) and **libsignal-client** native libraries; releases must stay current because Signal’s servers and clients evolve frequently.

## Likely Integration Approaches

| Approach | Pros | Cons |
|----------|------|------|
| **signal-cli JSON-RPC daemon** | Documented machine interface; matches “gateway + sidecar” deployment | Separate process to manage; JVM/native deps; GPL-3.0 license (see below) |
| **signal-cli subprocess** | Simple for one-off sends | Poor fit for sustained receive loops vs daemon |
| **D-Bus** | Fine on some Linux desktops | Awkward for headless server portability |

A Chai integration would likely **spawn or connect to** signal-cli, map incoming messages to **`InboundMessage`** (`channel_id: "signal"`, **`conversation_id`** = Signal’s peer/room identifier in string form), and call **`send_message`** for replies—mirroring Telegram semantics.

## Chai Implementation Prerequisites

Everything in [CHANNELS.md](../spec/CHANNELS.md) applies. Signal-specific preparation:

| Topic | Requirement |
|-------|-------------|
| **`channel_id`** | Use a fixed id such as **`"signal"`** for **`ChannelHandle::id()`** and every **`InboundMessage`**; must match **`ChannelRegistry`** registration. |
| **`conversation_id`** | Must be a **stable string** that signal-cli uses to send a reply to the same 1:1 or group context (exact format depends on signal-cli JSON-RPC payloads—spike must pin this down). |
| **Text-only MVP** | Like Telegram today, only populate **`InboundMessage`** when you have a **plain text** body; drop or log other content types until **`InboundMessage`** is extended. |
| **Inbound connector** | Either push to **`GatewayState::inbound_tx`** from a task (analogous to long-poll) or **add an HTTP route** if you use a bridge that POSTs to the gateway (same pattern as **`POST /telegram/webhook`**). |
| **Long-running work** | If a receive loop runs in-process, store its **`JoinHandle`** in **`channel_tasks`**; **`stop()`** should unblock that loop. |
| **Shutdown** | **`stop()`** plus await joined tasks; if a sidecar process is used, define whether the gateway spawns/kills it or expects an external supervisor. |
| **Config** | Extend **`ChannelsConfig`** in **`crates/lib/src/config.rs`**; add startup block in **`run_gateway`** next to the Telegram block. |
| **Gateway route** | Add **`Router`** routes only if Signal ingress is HTTP; otherwise no new route (same as Telegram long-poll). |

**WebSocket `send` / `agent` echo** — Once a session is bound to **`("signal", conversation_id)`**, existing gateway code delivers assistant text to **`ChannelRegistry`**; no change to **`process_inbound_message`** is required if **`InboundMessage`** and **`ChannelHandle`** are correct.

## Registration and Operations (Upstream)

- Account identity is typically a **phone number** in international format; registration uses SMS or voice verification (see signal-cli README for landline and captcha flows).
- **Storage:** Credentials live under **`$XDG_DATA_HOME/signal-cli/data/`** or **`~/.local/share/signal-cli/data/`** (per upstream docs).
- **Receiving:** Signal’s protocol expects **regular receipt** of messages (daemon or periodic `receive`); encryption and group state depend on it.

These constraints affect **deployment**, not just code: the gateway host must safely store keys and keep signal-cli **updated**.

## License and Distribution

- signal-cli is **GPL-3.0**. This project **does not** distribute signal-cli; use **BYO** (see [SIGNAL_CLI_INTEGRATION.md](../adr/SIGNAL_CLI_INTEGRATION.md)). Chai talks to a **user-run** daemon over HTTP only.

## Config Shape (Proposed, Not Final)

Illustrative only until the epic is implemented:

- **`channels.signal`** — paths or connection settings for the signal-cli daemon; account reference; optional flags for receive mode.
- **Environment variables** — TBD (e.g. data directory override if supported by sidecar).

Exact field names should follow the same **`camelCase`** style as `channels.telegram` in **`config.json`**.

## Alignment with Telegram (Conceptual)

| Telegram concept | Signal analogue (planned) |
|------------------|---------------------------|
| Bot token | signal-cli account / key material on disk |
| `getUpdates` / webhook | JSON-RPC or receive loop feeding **`InboundMessage`** |
| `sendMessage` | signal-cli send command or RPC equivalent |
| `conversation_id` | Stable string id for peer or group from signal-cli payloads |

## Risks and Open Questions

- **ToS and automation** — Operators must comply with Signal’s terms and acceptable use; this doc does not interpret policy.
- **Captcha / registration friction** — May block unattended setups.
- **Rate limits and blocking** — Centralized service may throttle or block misconfigured automations.

## Spike (`crates/spike`)

The **`signal-probe`** binary checks a running signal-cli **HTTP** daemon (`GET /api/v1/check`, **`POST /api/v1/rpc`** with **`listGroups`**, sample **`GET /api/v1/events`**). Run **`cargo run -p chai-spike --bin signal-probe`** after starting **`signal-cli daemon --http …`** (see **`crates/spike/README.md`**).

Upstream **JSON-RPC** documents **`receive`** notifications with **`params.envelope`** (1:1 **`dataMessage`**, groups, etc.) and **`send`** with **`recipient`** (E.164) or **`groupId`** (base64). Those fields are the practical basis for **`conversation_id`** and **`text`** in **`InboundMessage`**; exact mapping should be decided when implementing **`ChannelHandle::send_message`** (mirror **`send`** params).

## References

- Signal — https://signal.org/
- signal-cli — https://github.com/AsamK/signal-cli
- JSON-RPC service (wiki) — https://github.com/AsamK/signal-cli/wiki/JSON-RPC-service
