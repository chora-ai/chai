---
status: accepted
---

# Matrix Adapter Package Design

This document records why Matrix lives in a separate adapter crate with an optional Cargo feature, rather than being compiled into the main library unconditionally.

## Context

Matrix integration depends on **matrix-sdk** and its transitive dependencies (SQLite store, Olm/Megolm crypto, etc.), which add significant compile time and binary size. Not all operators use Matrix — many run Telegram-only or Signal-only setups. Matrix is also **experimental** for v0.1.0: the adapter is functional (E2EE, room allowlist, SAS verification), but hardening items (reconnect tuning, richer error handling) remain as follow-ups. Including Matrix unconditionally would force every build to carry these dependencies and would imply production readiness that isn't there yet.

## Decision

| Topic | Choice |
|-------|--------|
| **Crate** | Matrix code lives in **`crates/adapters/matrix`** (Cargo package **`matrix-channel`**), not in **`crates/lib`**. |
| **Feature gate** | **`lib`** exposes a **`matrix`** Cargo feature (opt-in). **`cli`** and **`desktop`** pass it through via **`--features matrix`**. |
| **Default** | Matrix is **off** by default. **`cargo install --path crates/cli`** builds without Matrix. Operators who want Matrix add **`--features matrix`**. |
| **Stub** | When the feature is off, **`lib`** compiles a stub module (**`matrix_stub.rs`**) that provides no-op types so the rest of the gateway compiles without Matrix symbols. |
| **Thin wrapper** | **`crates/lib/src/channels/matrix.rs`** contains only a thin **`MatrixChannel`** newtype (to implement **`ChannelHandle`** from **`lib`**'s trait) and the **`connect_matrix_client`** wiring. All matrix-sdk logic stays in the adapter crate. |

## Rationale

- **Smaller default binary** — Operators who only use Telegram or Signal don't pay the matrix-sdk cost.
- **Clean separation** — matrix-sdk and its crypto dependencies are isolated in a dedicated crate; **`lib`** only sees a small surface area (**`MatrixInner`**, **`connect_with_params`**, **`RawInbound`**).
- **Consistent pattern** — This adapter-crate + feature-gate pattern is the same one used for Signal (see [SIGNAL_ADAPTER.md](SIGNAL_ADAPTER.md)). Future optional integrations can follow the same approach.
- **Clear expectations** — An opt-in feature flag communicates "this works but is not yet first-class" more honestly than shipping it unconditionally.

## Alternatives Considered

| Alternative | Why not |
|-------------|---------|
| **Always compile Matrix into `lib`** | Forces all builds to depend on matrix-sdk; increases compile time and binary size for operators who don't use Matrix. |
| **Runtime-only gating (no compile-time feature)** | The code and dependencies are still compiled and linked even if never used; negates the size and compile-time benefits. |
| **Sidecar process** | Orthogonal concern — a sidecar could communicate with the gateway over HTTP, but the in-process adapter remains the simpler default for a single-gateway deployment. |

## Related Documents

- [SIGNAL_ADAPTER.md](SIGNAL_ADAPTER.md) — The same adapter-crate pattern for Signal (also experimental).
- [CHANNELS.md](../spec/CHANNELS.md) — Internal spec for gateway channel behavior.
