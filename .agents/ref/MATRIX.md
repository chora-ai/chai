---
status: current
---

# Matrix / Element Reference

Reference for the **Matrix** channel in Chai. Gateway behavior is specified in [CHANNELS.md](../spec/CHANNELS.md). [TELEGRAM.md](TELEGRAM.md) shows the Telegram mapping for comparison. Matrix emphasizes **open federation** and **decentralization**: users and rooms live on **homeservers** that interoperate over the protocol ([Matrix](https://matrix.org/)). **Element** is a common [client](https://matrix.org/ecosystem/clients/element/) for humans; the gateway uses a normal Matrix user account (access token or password login). **Interactive device verification (SAS)** can be completed via **gateway HTTP** so Element is not required for that step.

**Status** — **Implemented** in **`crates/adapters/matrix`** (Cargo package **`matrix-channel`**, matrix-sdk) with a thin **`MatrixChannel`** wrapper in **`crates/lib/src/channels/matrix.rs`** when the **`lib`** **`matrix`** Cargo feature is enabled (opt-in: **`cargo build -p cli --features matrix`**, **`cargo install --path crates/cli --features matrix`**, or **`--features matrix`** on **`desktop`**). SQLite state store is fixed at **`<active-profile>/matrix`** (see **`connect_matrix_client`** in **`crates/lib/src/channels/matrix.rs`**), **Megolm/Olm** for **encrypted** rooms, Client-Server **`sync_once`** loop, **`OriginalSyncRoomMessageEvent`** for **`m.text`**, outbound **`room.send(RoomMessageEventContent::text_plain(…))`** (encrypts when the room is encrypted). Access-token restore needs **`user_id`** and **`device_id`** (from **`GET /account/whoami`**, or **`MATRIX_DEVICE_ID`** / **`channels.matrix.deviceId`**).

**Room allowlist** — Optional: only listed rooms generate inbound agent turns. **`channels.matrix.roomIds`** (JSON array of room ids such as **`!abc:example.org`**) or env **`MATRIX_ROOM_ALLOWLIST`** (comma-separated; overrides config when set and non-empty). Unset / empty means all joined rooms (backward compatible). Implemented via **`resolve_matrix_room_allowlist`** in **`crates/lib/src/config.rs`**.

**E2EE verification (gateway)** — To-device **`m.key.verification.request`** events are tracked; operators drive SAS through **`crates/lib/src/gateway/matrix_routes.rs`** routes on the same port as the WebSocket gateway. See **Gateway HTTP: verification** below.

## Purpose and How to Use

- **Purpose:** Describe how Matrix feeds the same gateway pipeline as Telegram; document config, allowlist, and verification HTTP.
- **How to use:** Read [CHANNELS.md](../spec/CHANNELS.md) first, then this file and [TELEGRAM.md](TELEGRAM.md).

## Ecosystem Facts

- **Protocol** — Matrix is an open standard; **Client-Server API** for sending/receiving events, room membership, sync.
- **Federation** — Homeservers (self-hosted or hosted) join the public federation by default unless configured otherwise.
- **Encryption** — **Matrix E2EE** (Olm/Megolm) is used in Chai via **matrix-sdk** (see **Status** above).

## Integration Approaches

| Approach | Pros | Cons |
|----------|------|------|
| **In-process Matrix client (e.g. Rust SDK)** | Fits Chai’s Rust stack; full control over sync loop | Must handle sync, room routing, E2EE |
| **matrix-commander** ([ecosystem listing](https://matrix.org/ecosystem/clients/matrix-commander/)) | CLI send/receive patterns; good for ops experiments | May not match production needs for a long-running gateway; evaluate license and process model |
| **Application Service (appservice)** | Central integration point for multiple rooms | Heavier to deploy; more Matrix-admin surface area |

For parity with Telegram’s “single gateway process,” a **dedicated Matrix client task** inside the gateway (sync API, map **`m.room.message`** text to **`InboundMessage`**) is what Chai ships.

## Chai Implementation Prerequisites

Everything in [CHANNELS.md](../spec/CHANNELS.md) applies. Matrix-specific preparation:

| Topic | Requirement |
|-------|-------------|
| **`channel_id`** | Fixed id such as **`"matrix"`** on **`ChannelHandle::id()`** and all **`InboundMessage`** instances. |
| **`conversation_id`** | Natural choice is the **room id** (e.g. **`!abcdef:example.org`**) as a string—the same value the client uses to **`send_message`** for **`m.room.message`**. Room aliases (**`#room:server`**) can be resolved once at startup or stored in config; the id must stay stable for **`SessionBindingStore`**. |
| **Text-only MVP** | Map **`m.room.message`** with **`msgtype` `m.text`** (and plain **`body`**). Ignore or skip non-text until **`InboundMessage`** gains media. |
| **Inbound connector** | A **sync loop** (or streaming) is the Matrix analogue of Telegram long-poll: run as a **`JoinHandle`** in **`channel_tasks`**, **`stop()`** aborts sync. |
| **HTTP** | Matrix uses the shared gateway **`Router`**: verification endpoints under **`/matrix/verification/*`** (see below). |
| **Config** | **`ChannelsConfig.matrix`** in **`crates/lib/src/config.rs`**; **`run_gateway`** registers the channel when credentials exist. |
| **Room policy** | Optional **allowlist** — only respond in listed rooms (recommended for public homeservers). |

**Multi-room** — Each room is a distinct **`conversation_id`**; bindings give each room its own session, matching Telegram’s per-chat behavior.

**WebSocket `send` / `agent` echo** — Unchanged on the gateway side if the session is bound to **`("matrix", room_id)`**.

## Gateway HTTP: verification

When Matrix is connected, **`GatewayState.matrix_channel`** is set and these routes are registered (**`crates/lib/src/gateway/server.rs`**). Request and response JSON use **camelCase** for verification fields: **`userId`**, **`flowId`**, and **`fromDevice`** on **`pending`** items (Matrix user id, verification **`transaction_id`**, and sender device id). **`GET /matrix/verification/sas`** uses query **`userId`** and **`flowId`**.

| Method | Path | Role |
|--------|------|------|
| GET | **`/matrix/verification/pending`** | List pending verification requests seen since startup |
| POST | **`/matrix/verification/accept`** | Accept an incoming verification request |
| POST | **`/matrix/verification/start-sas`** | Start SAS after the request is accepted and ready |
| GET | **`/matrix/verification/sas`** | Short auth string (emoji / decimals) and flags |
| POST | **`/matrix/verification/confirm`** | Confirm SAS matches the other device |
| POST | **`/matrix/verification/mismatch`** | SAS did not match |
| POST | **`/matrix/verification/cancel`** | Cancel verification request or SAS flow |

**Auth** — If the gateway is configured with a connect token, use header **`Authorization: Bearer <token>`**. If no token is configured, these routes are only allowed when the gateway binds to a **loopback** address (same policy as other sensitive gateway behavior).

Typical flow: **`pending`** → **`accept`** → **`start-sas`** → **`sas`** (compare with the other client) → **`confirm`** or **`mismatch`** / **`cancel`**.

## Identity and Homeserver

- **Homeserver** — Could be **matrix.org**, a commercial host, or **self-hosted** (Synapse, Dendrite, Conduit, etc.). Decentralization goals often point to **self-hosted** or trusted small providers.
- **Bot user** — A normal Matrix user account the gateway logs in as; **`conversation_id`** is the **room id** (`!xxx:server`) that **`send_message`** uses to route **`m.room.message`** sends.

Exact mapping must be one-to-one with **`ChannelHandle::send_message`**.

## Config Shape (current)

Fields live on **`MatrixChannelConfig`** (**`channels.matrix`**, **`camelCase`** in JSON). Env vars override where noted in **`crates/lib/src/config.rs`** / README.

| Field | Role |
|-------|------|
| **`homeserver`** | HTTPS base URL of the homeserver (e.g. `https://matrix.example.org`). Env: **`MATRIX_HOMESERVER`**. |
| **`accessToken`** / **`user`** / **`password`** | Access token login or password login. Env: **`MATRIX_ACCESS_TOKEN`**, **`MATRIX_USER`**, **`MATRIX_PASSWORD`**, etc. |
| **`userId`** | **`@user:server`** when needed for token restore. Env: **`MATRIX_USER_ID`**. |
| **`deviceId`** | Device id for token restore. Env: **`MATRIX_DEVICE_ID`**. |
| **`roomIds`** | Optional list of room ids; inbound agent turns only from these rooms. Env: **`MATRIX_ROOM_ALLOWLIST`** (comma-separated, overrides when set). |

**Store path** — Not configurable: always **`<profile_dir>/matrix`** (same directory the active profile’s **`config.json`** lives under).

## Alignment with Telegram (Conceptual)

| Telegram concept | Matrix analogue (Chai) |
|------------------|------------------------|
| Bot token | Access token + device (or password login) |
| `getUpdates` / webhook | **`sync_once`** loop |
| `sendMessage` | **`room.send`** **`m.room.message`** |
| `conversation_id` | Room id stable for outbound |

## Privacy and Decentralization Notes

- **Metadata** — Room membership, server participation, and traffic patterns may be visible to participating homeservers; E2EE protects content for supported setups, not all metadata.
- **Compared to Signal** — Matrix offers **federation and self-hosting**; Signal offers a **single well-reviewed** centralized service with strong E2EE defaults. Product messaging should not conflate the two.

## Risks and Open Questions

- **Homeserver limits** — Rate limits and quotas vary; self-hosting shifts control to the operator.
- **Verification UX** — Gateway HTTP covers SAS; QR or room-based verification flows are not exposed here unless extended later.

## Spike (`crates/spike`)

The **`matrix-probe`** binary performs **`m.login.password`** against **`/_matrix/client/v3/login`**, then one **`GET /_matrix/client/v3/sync`**, and prints **`m.room.message`** **`m.text`** events as **`room_id<TAB>body`**. That confirms **room id** as a natural **`conversation_id`** for Chai. Run **`cargo run -p chai-spike --bin matrix-probe`** with **`MATRIX_HOMESERVER`**, **`MATRIX_USER`**, **`MATRIX_PASSWORD`** (see **`crates/spike/README.md`**). A production client would persist **`next_batch`** and use a longer-lived sync loop.

## User journey

End-to-end test with Element or another client, an encrypted or unencrypted room, and the gateway: [`.journey/08-channel-matrix.md`](../../.journey/08-channel-matrix.md). For device verification without Element, use **`/matrix/verification/*`** as above.

## References

- Matrix — https://matrix.org/
- Element — https://matrix.org/ecosystem/clients/element/
- matrix-commander — https://matrix.org/ecosystem/clients/matrix-commander/
