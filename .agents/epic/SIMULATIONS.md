---
status: draft
---

# Epic: Simulation and Integration Harness

**Summary** — Capture the direction explored alongside messaging work: **`crates/spike`** today is a **lean integration probe** crate (live Matrix / signal-cli HTTP checks), **not** a simulation engine. This epic defines a possible **future** layer for **repeatable scenarios**, **fixtures**, **gateway-in-process or replay** testing, and **optional CI**—without conflating that with the current spike binaries.

**Status** — **Draft.** No implementation commitment; relationship to **`chai-spike`** is scoped below.

## Problem Statement

The existing **`crates/spike`** crate serves as a **wire / ops validation** tool: `matrix-probe` and `signal-probe` talk to real services **outside** the gateway. It is good for manual smoke tests and docs when external APIs drift, but it only **pokes** live endpoints. There is no layer today for **controlled scenarios**, **assertions**, **determinism or record/replay**, or **in-process / fixture-driven** runs. This gap means multi-turn transcripts, channel-specific envelopes, and regression against known inputs cannot be exercised repeatably—either in CI or locally without live services. The **`.testing/`** playbooks capture what "good looks like" as human-run checklists, but there is no mechanism to automate those sequences.

## Goal

- **Repeatable runs** — Scripts or Rust drivers that exercise **`InboundMessage` → `process_inbound_message`** (or WebSocket **`agent`**) with **known inputs** and **expected outcomes** (text, session binding, **`/new`**).
- **Fixtures** — Serialized Telegram updates, Matrix sync chunks, Signal JSON-RPC notifications, for **offline** regression without live services.
- **Optional live mode** — Reuse **`chai-spike`**-style probes as **adapters** for "smoke against staging homeserver / signal-cli" in CI when secrets exist.
- **Observability** — Timings, transcript logs, optional soak (many turns) **without** claiming to be a full load-testing product.

## Scope

### In Scope

- Repeatable scenario runs against `InboundMessage → process_inbound_message` and WebSocket `agent`
- Fixture format for Telegram updates, Matrix sync chunks, and Signal JSON-RPC notifications
- In-process gateway or direct `process_inbound_message` calls from tests behind a feature flag or separate binary
- CI policy distinguishing fixture-only (always run) from live (optional, secrets) jobs
- Encoding `.testing/` playbook sequences as candidate first scenarios

### Out of Scope

- Replacing unit tests in **`crates/lib`** for pure logic.
- **Production monitoring** or APM (different problem).
- **Guaranteed** Matrix/Signal E2EE simulation without real crypto stacks (if E2EE is simulated, it is explicitly scoped).

## Design

### What We Explored

| Topic | Conclusion |
|-------|------------|
| **`chai-spike` role** | Durable **wire / ops validation**: `matrix-probe`, `signal-probe` talk to real services **outside** the gateway. Good for manual smoke tests and docs when external APIs drift. |
| **vs "simulations"** | **Simulations** imply **controlled scenarios**, **assertions**, **determinism or record/replay**, and often **in-process** or **fixture-driven** runs. The spike crate does **not** do that today—it only **pokes** live endpoints. |
| **Evolution** | A **larger harness** could live in a **sibling crate** (e.g. `crates/sim` or `crates/integration-harness`) that depends on **`lib`**, while **`chai-spike`** stays **small** and **probe-only** to preserve minimal dependencies and clear purpose. |

### Relationship to `crates/spike`

| Component | Role |
|-----------|------|
| **`chai-spike` (current)** | Keep as **integration probes**; document in **`crates/spike/README.md`**; optional extension with new probes for future channels. |
| **Future harness (optional)** | New crate or module: **scenarios**, **assertions**, **fixtures**; may **import** types from **`lib`**; may **invoke** probes or **embed** canned JSON. |

**Principle:** Do not grow **`chai-spike`** into a full simulation framework without a **split**—avoids blurring "minimal deps / smoke" with "heavy test orchestration."

### Relationship to Model Testing (`.testing` Playbooks)

Model-comparison procedures live under **[`.testing/`](../../.testing/)**. They are **numbered markdown playbooks** (e.g. `01-local-ollama-llama.md`, `08-third-party-nim-qwen.md`). Together they define what a **simulation harness would want**: a **fixed message sequence**, **skill context modes**, **multiple runs per configuration**, and an **expected-behavior table** (tool use vs chat-only) so different **models** and **providers** can be compared.

| Aspect | `.testing` playbooks (today) | Simulations epic (future) |
|--------|------------------------------|---------------------------|
| **Purpose** | Human-run **regression** and comparison across **LLM backends** | Automated **repeatable runs**, optional **assertions**, transcripts, timing |
| **Entry** | WebSocket **`agent`** (or desktop) with a **live gateway** | Same **`agent`** path—or **stubbed provider** for channel-only tests |
| **Overlap** | High: both need **scenario = ordered user messages + config** | The harness could **encode** the shared sequence from **[`.testing/README.md`](../../.testing/README.md)** and loop over **`defaultProvider` / `defaultModel`** (or overrides) |
| **Difference** | Does **not** mandate channels; often **Telegram** mentioned as one way to send messages | Originally motivated by **channel fixtures**; model testing is an equally valid **use of the same machinery** |

**View:** Ongoing **model testing** is a **strong fit** for the simulations epic **once** the harness can drive **`agent`** turns with **deterministic config** and capture **tool calls + reply text**. The **`.testing`** playbooks stay the **source of truth for expectations** ("what good looks like"); the epic covers **how** those scenarios get run **repeatedly** (manual checklist → scripted or CI). The **inventory** phase should explicitly include those playbooks as **candidate first scenarios**.

## Requirements

- [ ] Inventory of what is already testable (`lib` tests, `gateway_health` integration test) and gaps (channels, multi-turn transcripts)
- [ ] `.testing/` playbooks enumerated as candidate first scenarios
- [ ] Minimal fixture format agreed upon for `InboundMessage` and channel-specific envelopes (JSON or Rust builders)
- [ ] In-process harness MVP: direct `process_inbound_message` calls or gateway-in-process behind a feature flag or separate binary
- [ ] CI policy documented: fixture-only jobs vs live jobs (secrets-gated)
- [ ] `crates/spike` documented in `crates/spike/README.md` as probe-only with clear separation from simulation harness

## Phases

1. **Inventory** — List what is already testable (`lib` tests, `gateway_health` integration test) and gaps (channels, multi-turn transcripts). Include the **numbered playbooks** in **[`.testing/`](../../.testing/)** (e.g. `01-local-ollama-llama.md`, …) as **scenario candidates** (same sequences, optional automation).
2. **Fixture format** — Agree on minimal JSON (or Rust builders) for **`InboundMessage`** and channel-specific envelopes.
3. **Harness MVP** — In-process gateway or direct **`process_inbound_message`** calls from tests behind a feature flag or separate binary.
4. **CI policy** — Which jobs are **fixture-only** (always run) vs **live** (optional, secrets).

## Open Questions

- Whether simulations should **spin up** Ollama/mock LLM or **stub** the provider layer for channel-only tests.
- How much **desktop** or **CLI** should be included in "integration" vs gateway-only.

## Related Epics and Docs

- [MSG_CHANNELS.md](MSG_CHANNELS.md) — Channels product work; spike probes originated here.
- **`crates/spike/README.md`** — Current probe binaries; how similutions are different.
- [.testing/README.md](../../.testing/README.md) — Numbered model-comparison playbooks by category, provider, and family.
