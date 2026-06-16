---
status: accepted
---

# Signal Adapter Package Design

This document records why Signal lives in a separate adapter package with an optional Cargo feature, rather than being compiled into the main library unconditionally.

## Context

Signal integration depends on a user-run **signal-cli** HTTP daemon. Not all operators use Signal — many run Telegram-only or Matrix-only setups. Signal is also **experimental** for v0.1.0: basic text messaging works, but hardening (reconnect tuning, richer receive payloads) is not yet complete. Including Signal unconditionally would force every build to carry code that most operators don't use and would imply production readiness that isn't there yet.

## Decision

| Topic | Choice |
|-------|--------|
| **Package** | Signal code lives in **`crates/adapters/signal`** (Cargo package **`signal-channel`**), not in **`crates/lib`**. |
| **Feature gate** | **`lib`** exposes a **`signal`** Cargo feature (opt-in). **`cli`** and **`desktop`** pass it through via **`--features signal`**. |
| **Default** | Signal is **off** by default. **`cargo install --path crates/cli`** builds without Signal. Operators who want Signal add **`--features signal`**. |
| **Stub** | When the feature is off, **`lib`** compiles a stub module (**`signal_stub.rs`**) that provides no-op types so the rest of the gateway compiles without Signal symbols. |
| **Thin wrapper** | **`crates/lib/src/channels/signal.rs`** contains only a thin **`SignalChannel`** newtype (to implement **`ChannelHandle`** from **`lib`**'s trait) and the **`resolve_signal_daemon_config`** wiring. All signal-cli HTTP/SSE logic stays in the adapter package. |

## Rationale

- **Smaller default binary** — Operators who only use Telegram or Matrix don't carry the Signal adapter code.
- **Clean separation** — Signal's HTTP/SSE logic and its `reqwest` + `futures-util` dependencies are isolated in a dedicated package; **`lib`** only sees a small surface area (**`SignalInner`**, **`SignalDaemonConfig`**, **`RawInbound`**).
- **Consistent pattern** — This adapter-package + feature-gate pattern is the same one used for Matrix (see [MATRIX_ADAPTER.md](MATRIX_ADAPTER.md)). Future optional integrations (Discord, Slack, etc.) can follow the same approach.
- **Clear expectations** — An opt-in feature flag communicates "this works but is not yet first-class" more honestly than shipping it unconditionally.

## Alternatives Considered

| Alternative | Why not |
|-------------|---------|
| **Always compile Signal into `lib`** | Forces all builds to include Signal code; implies production readiness that isn't there yet. |
| **Ship Signal as a default-on feature** | Premature — hardening items remain; BYO signal-cli setup is non-trivial; licensing/ToS considerations are different from Telegram. |
| **Runtime-only gating (no compile-time feature)** | The code is still compiled and linked even if never used; negates the size benefits and still implies first-class support. |
| **Sidecar process** | Orthogonal concern — a sidecar could wrap signal-cli and talk to the gateway over HTTP, but the in-process adapter remains the simpler default for a single-gateway deployment. |

## Related Documents

- [MATRIX_ADAPTER.md](MATRIX_ADAPTER.md) — The same adapter-package pattern for Matrix.
- [CHANNELS.md](../spec/CHANNELS.md) — Internal spec for gateway channel behavior.
