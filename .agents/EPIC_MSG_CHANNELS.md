# Epic: Messaging Channels (Privacy and Federation)

**Summary** — Expand Chai’s messaging integrations so users can interact with the gateway through multiple chat surfaces, prioritizing **privacy-preserving** options and, where possible, **decentralized or user-controlled** infrastructure. **Matrix**, **Telegram**, and **Signal** (BYO signal-cli) are implemented in the gateway. **Integration probes** live in **`crates/spike`**; a **broader simulation / harness** story is tracked separately in [EPIC_SIMULATIONS.md](EPIC_SIMULATIONS.md).

**Status** — **In progress.** Matrix (including **E2EE** via matrix-sdk), Telegram, and Signal are shipped; further hardening and follow-ups remain.

## Goal

- **User choice** — Let operators pick chat networks that match their threat model and hosting constraints, not only Telegram.
- **Privacy** — Prefer channels where end-to-end encryption, minimal metadata exposure, or open review are strong defaults or well understood. **Signal** is widely cited for strong E2EE ([Signal](https://signal.org/)). **Matrix** offers decentralized hosting and E2EE with operator-chosen homeservers ([Matrix](https://matrix.org/)).
- **Decentralization** — **Matrix** stands out for federation and self-hosting. **Signal** and **Telegram** (typical bot flows) rely on **centralized** services operated by their respective organizations.

## Current State

- **Implemented:** **Telegram** — Bot API via **`getUpdates`** (long-poll) or **`setWebhook`** + **`POST /telegram/webhook`**; outbound **`sendMessage`**. **Matrix** — **[matrix-sdk](https://github.com/matrix-org/matrix-rust-sdk)** in **`crates/adapters/matrix`** (SQLite + E2EE); **`lib`** wraps it behind the **`matrix`** Cargo feature (opt-in on **`cli`** / **`desktop`**: **`--features matrix`**). Omit Matrix by building without that feature (e.g. **`cargo install --path crates/cli`**). **`sync_once`** loop; **`m.room.message`** / **`m.text`** inbound; outbound **`room.send`** (Megolm when the room is encrypted). Optional **room allowlist** (**`channels.matrix.roomIds`**, **`MATRIX_ROOM_ALLOWLIST`**). **E2EE verification (SAS)** without Element: gateway HTTP under **`/matrix/verification/*`** (see [MATRIX_REFERENCE.md](ref/MATRIX_REFERENCE.md)). **Signal** — User-run signal-cli **HTTP** daemon: **`GET /api/v1/events`** (SSE) + JSON-RPC **`send`**. All map to **`InboundMessage`** (`channel_id`, `conversation_id`, `text`) and the same session/agent path.
- **Configuration:** **`channels.telegram`**; **`channels.matrix`**; **`channels.signal`** (`httpBase`, optional **`account`**); env **`TELEGRAM_BOT_TOKEN`**, **`MATRIX_*`** (including **`MATRIX_ROOM_ALLOWLIST`**), **`SIGNAL_CLI_HTTP`**, **`SIGNAL_CLI_ACCOUNT`** (see [README.md](../../README.md)).
- **References:** [TELEGRAM_REFERENCE.md](ref/TELEGRAM_REFERENCE.md), [MATRIX_REFERENCE.md](ref/MATRIX_REFERENCE.md), [SIGNAL_REFERENCE.md](ref/SIGNAL_REFERENCE.md), [CHANNELS.md](spec/CHANNELS.md), [SIGNAL_CLI_INTEGRATION.md](adr/SIGNAL_CLI_INTEGRATION.md). User journeys: [`.journey/05-channel-telegram.md`](../.journey/05-channel-telegram.md), [`.journey/08-channel-matrix.md`](../.journey/08-channel-matrix.md), [`.journey/09-channel-signal.md`](../.journey/09-channel-signal.md).
- **Probes (non-gateway):** **`crates/spike`** — **`matrix-probe`**, **`signal-probe`**; see **`crates/spike/README.md`**.

## Planned First-Wave Additions

| Channel | Role in epic | Privacy notes | Decentralization notes |
|---------|----------------|---------------|-------------------------|
| **Signal** | First-wave (**shipped** in `crates/lib`) | Strong E2EE on Signal clients; Chai uses **signal-cli** (BYO) over HTTP | [signal-cli](https://github.com/AsamK/signal-cli); [SIGNAL_REFERENCE.md](ref/SIGNAL_REFERENCE.md), [SIGNAL_CLI_INTEGRATION.md](adr/SIGNAL_CLI_INTEGRATION.md) |
| **Matrix / Element** | First-wave (**shipped** in `crates/lib`) | **E2EE** via matrix-sdk for encrypted rooms | **Federation** and **self-hosted homeservers**; [Element](https://matrix.org/ecosystem/clients/element/) |

## Additional Options Under Consideration (Not Committed)

Before the epic is finalized, the following may be evaluated for a later wave or rejected explicitly:

- **WhatsApp / Meta stacks** — Huge reach; different trust and API constraints; often poor fit for privacy-first positioning.
- **Discord / Slack** — Common for teams; typically centralized; weaker privacy story for sensitive workflows.
- **XMPP** — Mature federation; smaller ecosystem than Matrix in many communities; could appeal to minimal or legacy deployments.
- **IRC / Mattermost / Zulip / others** — Niche or self-hosted team chat; useful for specific operators.
- **Session / SimpleX / other “metadata-light” messengers** — May align with privacy goals; API and automation maturity varies.

Criteria for inclusion in a future wave should be documented here when the epic leaves draft: **API or bridge stability**, **license compatibility**, **operational burden** (daemon, keys, registration), and **overlap** with Matrix vs Signal.

## Non-Goals (Draft)

- **Parity with every feature** of each messenger (voice, video, stickers, spaces) in v1 — text-first agent replies are the likely MVP for new channels.
- **Replacing** the official Signal or Matrix clients for human-to-human chat — the gateway is an **automation surface**, not a full client.
- **Bundling** a full Matrix SDK in **`lib`** without a separate decision — current Matrix path is **`crates/adapters/matrix`** (matrix-sdk, SQLite store, E2EE), optional via **`lib`**’s **`matrix`** feature. **Signal:** BYO signal-cli only — [SIGNAL_CLI_INTEGRATION.md](adr/SIGNAL_CLI_INTEGRATION.md).

## Technical Direction (High Level)

- **Shared contract** — [CHANNELS.md](spec/CHANNELS.md): **`InboundMessage`** → **`process_inbound_message`** → **`ChannelHandle::send_message`**. Telegram: [TELEGRAM_REFERENCE.md](ref/TELEGRAM_REFERENCE.md).
- **Signal** — No official bot API comparable to Telegram’s. Practical approaches include **signal-cli** (JSON-RPC HTTP daemon, etc.); see [SIGNAL_REFERENCE.md](ref/SIGNAL_REFERENCE.md).
- **Matrix** — **Shipped:** **`MatrixChannel`** (`lib`) over **`matrix_channel::MatrixInner`** with SQLite + **E2EE**; **room allowlist**; **gateway HTTP verification** (SAS) under **`/matrix/verification/*`**. Reference: [MATRIX_REFERENCE.md](ref/MATRIX_REFERENCE.md); journey: [`.journey/08-channel-matrix.md`](../.journey/08-channel-matrix.md).
  - **Follow-ups (optional):** client backoff / respect for homeserver rate limits; **sync** reconnect tuning when sync errors; richer **timeline** handling (non-text, edits, reactions) beyond plain **`m.text`**.
  - **Modularity:** **matrix-sdk** is isolated in **`crates/adapters/matrix`**; **`lib`** feature **`matrix`** (opt-in via **`cli`** / **`desktop`** **`--features matrix`**) controls whether Matrix code is compiled. **Future:** optional **runtime** loading of Matrix (e.g. only connect when the gateway starts with Matrix enabled in config) to avoid linking Matrix in processes that never use it — see **Next Steps** below.

## Phases

| Phase | Status | Notes |
|-------|--------|--------|
| 1. Spikes (wire validation) | **Done** | **`crates/spike`**: **`matrix-probe`**, **`signal-probe`**. |
| 2. Matrix gateway integration | **Done** | **`MatrixChannel`** in **`crates/lib`** + **`crates/adapters/matrix`** (matrix-sdk); README + desktop config surfacing. |
| 3. Signal gateway integration | **Done** | **`SignalChannel`**: SSE + JSON-RPC **`send`** — [SIGNAL_REFERENCE.md](ref/SIGNAL_REFERENCE.md). |
| 4. Matrix hardening | **Partial / ongoing** | Allowlist + verification HTTP: **done**. **Remaining:** rate limits / backoff, reconnect tuning, richer timeline events (optional) — see **Technical Direction** above. |
| 5. Config and docs polish | **Ongoing** | **`channels.signal`** documented; Matrix **ref + journey** updated; channel-agnostic quickstart. |
| 6. Operational hardening | **Not started** | Gateway **`status`** channel list; structured logging; secrets rotation notes. |

## Next Steps (Messaging Epic)

1. **Signal hardening** — Richer **`receive`** payloads (attachments, edits), reconnect tuning, optional **`account`** UX in desktop.
2. **Matrix hardening** — Rate limits / backoff, sync reconnect tuning, optional richer timeline events (aligns with phase 4).
3. **Epic hygiene** — Move from **draft** to **tracked** when scope is decided; keep [EPIC_SIMULATIONS.md](EPIC_SIMULATIONS.md) for **harness / fixtures** (orthogonal).

## Future Considerations (Matrix Modularity)

- **Runtime vs compile-time Matrix** — **`matrix`** feature + **`matrix-channel`** crate give **optional** Matrix at **compile** time (default **off** for **`cargo install --path crates/cli`** unless **`--features matrix`**). A follow-up is **optional Matrix at runtime**: e.g. **only** initialize **`connect_matrix_client`** when the operator enables Matrix in config at gateway start. A further step (optional **sidecar** process) is orthogonal to this epic; see prior discussion on **modular** Matrix.
- **Documentation** — README now notes **`--features matrix`** for **`cargo install`** / **`cargo run`**; keep in sync if defaults change again.

## Open Questions

- **Signal:** Registration (dedicated number vs landline); captcha and ToS. **Distribution:** BYO signal-cli only — see [SIGNAL_CLI_INTEGRATION.md](adr/SIGNAL_CLI_INTEGRATION.md).
- **Matrix:** **Resolved** — **room allowlist** (**`channels.matrix.roomIds`**, **`MATRIX_ROOM_ALLOWLIST`**) and **SAS verification** over gateway HTTP (**`/matrix/verification/*`**) so operators are not required to use Element for device verification. Element remains usable for manual verification if preferred.
- **Telegram:** Whether docs stay Telegram-first for quickstarts or **channel-agnostic**.

## References

- Signal — https://signal.org/
- signal-cli — https://github.com/AsamK/signal-cli
- Matrix — https://matrix.org/
- Element (client) — https://matrix.org/ecosystem/clients/element/
- matrix-commander — https://matrix.org/ecosystem/clients/matrix-commander/

## Related Epics and Docs

- [SIGNAL_CLI_INTEGRATION.md](adr/SIGNAL_CLI_INTEGRATION.md) — BYO signal-cli; HTTP daemon; no bundling.
- [EPIC_SIMULATIONS.md](EPIC_SIMULATIONS.md) — Simulation / harness (draft); relationship to **`crates/spike`**.
- [CHANNELS.md](spec/CHANNELS.md) — Internal spec: gateway channel types, binding, **`process_inbound_message`**, shutdown.
- [EPIC_API_ALIGNMENT.md](EPIC_API_ALIGNMENT.md) — LLM backends (orthogonal to messaging surfaces).
- [EPIC_ORCHESTRATION.md](EPIC_ORCHESTRATION.md) — Agent orchestration (orthogonal to which channel delivers messages).
