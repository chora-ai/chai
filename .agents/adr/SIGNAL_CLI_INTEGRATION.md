# Signal (`signal-cli`) Integration

This document records how Chai will integrate with **[signal-cli](https://github.com/AsamK/signal-cli)** for a future **Signal** messaging channel: **bring your own (BYO)** binary, **no distribution** of signal-cli by this project, and **no change** to Chai’s **LICENSE** file for this integration.

## Context

- **signal-cli** is **GPL-3.0** and implements access to the Signal service. Chai remains **LGPL-3.0** (see the repository **LICENSE**).
- Integrating via **separate process** and **IPC** (HTTP JSON-RPC and SSE, or stdin/stdout JSON-RPC) keeps Chai’s code **independent** of signal-cli **source**—Chai is a client of a **user-run** program, not a linked derivative of signal-cli.

## Decision

| Topic | Choice |
|-------|--------|
| **Distribution** | **Do not** ship, bundle, or redistribute signal-cli or a build of it as part of Chai releases, installers, or container images **maintained here**. Operators **install and run** signal-cli themselves (**BYO**). |
| **Repository licenses** | **No** updates to **LICENSE** or crate licenses solely for this integration. Compliance for **signal-cli** itself is between the **user** and **upstream** when they install it. |
| **Wire protocol** | Prefer the **HTTP daemon** (`signal-cli … daemon --http HOST:PORT`): Chai uses **`reqwest`** (or equivalent) to **`POST /api/v1/rpc`** and **`GET /api/v1/events`** (see upstream docs). Matches [signal-probe](../../crates/spike/src/bin/signal_probe.rs) and [SIGNAL_REFERENCE.md](../ref/SIGNAL_REFERENCE.md). |
| **Configuration** | Document **base URL** (e.g. `http://127.0.0.1:7583`) and expectations: daemon running before or alongside the gateway. |

## Rationale

- **BYO** avoids **GPL redistribution** obligations **for this repository** when shipping Chai (no bundled GPL artifact to offer source for).
- **HTTP sidecar** gives a **stable** receive path (SSE) and **send** path (JSON-RPC) without embedding Java or linking GPL code into Rust binaries.
- **License file unchanged** — Chai’s license terms stay clear; signal-cli remains a **third-party** dependency in the **operational** sense only.

## Alternatives Considered

| Alternative | Why not (for now) |
|-------------|-------------------|
| **Bundle signal-cli** in installers | Would require **GPL-3.0** compliance for that component (source offer, notices) in **our** distribution story; explicitly out of scope. |
| **Subprocess-only** (`signal-cli` per send) | Possible later; heavier and worse for continuous **receive**; HTTP daemon is the default integration shape. |

## Summary

Chai may implement a **Signal channel** that talks to a **locally installed, user-operated** signal-cli **HTTP daemon** only. **No** signal-cli binaries are distributed **by this project**; **no** **LICENSE** updates are required **for BYO**. Users install signal-cli from upstream or their OS vendor and run it under **their** responsibilities.

## Related Documents

- [SIGNAL_REFERENCE.md](../ref/SIGNAL_REFERENCE.md) — Integration notes and upstream links.
- [EPIC_MSG_CHANNELS.md](../EPIC_MSG_CHANNELS.md) — Roadmap for the Signal channel.
