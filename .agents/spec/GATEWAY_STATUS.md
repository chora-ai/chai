# Gateway Status

**Status:** Draft — the **preferred** wire shape is **nested** under `gateway`, `channels`, `providers`, `agents`, and `skills` (see [Target Shape](#target-shape-preferred)) so status aligns with [CONFIGURATION.md](CONFIGURATION.md) for cross-checking. The gateway **today** still emits a largely **flat** payload (see [Current Wire Behavior](#current-wire-behavior)); implementation should migrate toward nesting. Update this document when the payload or client parsing changes.

## Purpose

This document is the **specification for the gateway `status` WebSocket response payload**: what it represents, how it may be grouped for clarity and cross-checking with configuration, what belongs in each block, and what must **not** appear (secrets, full config echo).

It also defines how **configuration** (on-disk `config.json`) relates to **status** (runtime truth): same mental model, different responsibilities.

### Relationship To Configuration

| Artifact | Role | Authority |
|----------|------|-----------|
| **`config.json`** (see [CONFIGURATION.md](CONFIGURATION.md), `lib::config`, user `README`) | What the operator **asked** for: bind, channels, provider URLs/keys, agents, skills, etc. | **File** — edit to change behavior after reload/restart as applicable. |
| **`status` payload** | What the running gateway **is** using: listeners, discovery results, merged catalogs, resolved context summaries, tool list, etc. | **Runtime** — read-only snapshot for operators and clients. |

**Paired specs:** **[CONFIGURATION.md](CONFIGURATION.md)** defines the **configuration file** blocks and intent; **this document** defines **status** and what to put in each aligned block. Update **both** drafts together until the nested status shape and dashboard cross-checks are implemented. Authoritative field lists remain in **`crates/lib/src/config.rs`** and **`README.md`** — avoid duplicating serde in prose.

### Design Principles

1. **Status is not a copy of config** — It may echo *derived* or *sanitized* facts so operators can verify load behavior; it must not dump secrets or unnecessary raw file content.
2. **Top-level nesting mirrors config** — Prefer a payload whose top-level keys match **`config.json`**: `gateway`, `channels`, `providers`, `agents`, `skills`. Field names inside each block need not match the file; values are runtime/sanitized. This makes **cross-checking** in UIs (e.g. desktop Config vs Status) straightforward.
3. **Prefer summaries and health** where full payloads are large (skills context, tools JSON) — heavy blobs can remain in dedicated fields or other views (Context, Tools).
4. **Version or document breaking changes** to the wire shape when clients (CLI, desktop) depend on specific keys.

---

## Status Blocks And Config Alignment

The table below is the **canonical guide** for what each **nested block** should contain in the **preferred** status shape. Update rows as the implementation evolves.

| Block | In status, prefer | Avoid |
|-------|-------------------|--------|
| **`gateway`** | Effective bind, port, auth mode, protocol; optional `runtime` / uptime-style hints | Secrets; values that contradict actual listen socket without explanation |
| **`channels`** | **Registration / mode / health** per channel (e.g. Telegram polling vs webhook, Matrix client state summary) — enough to see “configured vs active” | Bot tokens, passwords, Matrix access tokens, webhook secrets |
| **`providers`** | Whether each backend is **configured** and **reachable**; discovery outcome (e.g. model count, last error code/message); not full URL strings if they embed credentials | API keys; raw provider config copied from file |
| **`agents`** | Orchestrator id, effective defaults, `enabledProviders` (discovery scope), **workers** with effective provider/model, **orchestration catalog** (merged discovery + allowlist), optional resolved **workspace** path | Full duplication of every delegation policy field if redundant with catalog + separate policy doc |
| **`skills`** | `contextMode`; optional resolved roots, skill counts; pointers to large text in other fields | Full skill bodies in status summary (use `skillsContext*` / Context screen as today) |

**Cross-checking goal:** Operators should answer “is what I configured actually what the gateway loaded?” **without** comparing secret material. Status should make gaps obvious (channel not running, provider unreachable, zero models, catalog empty).

---

## Wire Protocol (Today)

- **Transport:** WebSocket `req` / `res` envelope (`type`, `id`, `ok`, `payload` or `error`) as elsewhere in the gateway protocol.
- **Method:** `"status"` with params `{}`.
- **Payload:** JSON object; keys are **camelCase** on the wire.

Desktop and other clients parse a subset into `GatewayStatusDetails` and may show the full `res` in a raw JSON view.

---

## Current Wire Behavior

Implementation reference: `crates/lib/src/gateway/server.rs` (`"status"` handler).

The live payload is **mostly flat** at the top level, for example:

- `runtime`, `protocol`, `port`, `bind`, `auth`
- `orchestratorId`, `defaultProvider`, `defaultModel`, `enabledProviders`, `workers`
- Per-provider model arrays (`ollamaModels`, …) when discovery is enabled for that provider
- `orchestrationCatalog`, `agentContext`, `systemContext`, `date`, `skillsContext`, `skillsContextFull`, `skillsContextBodies`, `contextMode`, `tools`

Some keys are **omitted** when empty or irrelevant (e.g. a provider’s model array may be absent if discovery is off). Clients should treat **missing** arrays like **empty** where appropriate.

---

## Target Shape (Preferred)

**Nested** top-level objects under the same names as `config.json`:

```json
{
  "gateway": { },
  "channels": { },
  "providers": { },
  "agents": { },
  "skills": { }
}
```

Cross-cutting or global fields (if any) should live **under the block they belong to** when possible; only use the payload root for keys that truly span blocks, and document them here.

**Migration from today’s flat payload:** Preserve **backward compatibility** for existing clients where needed (deprecation period, dual keys during transition, or protocol version bump) unless all consumers are updated together. New clients should assume the **nested** shape once migration is complete.

---

## Open Questions

- Exact **channel** status schema (per integration) and error surfaces.
- Whether **resolved workspace** appears under `agents` or the payload root.
- **Protocol version** field in `status` vs relying on connect `hello` only.
- **Migration timeline** for removing flat keys (after desktop/CLI parse nested payloads).

---

## Related Documents

- **[CONFIGURATION.md](CONFIGURATION.md)** — On-disk `config.json` blocks and relationship to status.
- **[CHANNELS.md](CHANNELS.md)** — Channel behavior inside the gateway.
- **[ORCHESTRATION.md](ORCHESTRATION.md)** — Delegation and catalog semantics.
- **[PROVIDERS.md](PROVIDERS.md)** — Provider ids and discovery.
- **[CONTEXT.md](CONTEXT.md)** — How context strings relate to skills mode.
- **`crates/lib/src/gateway/protocol.rs`** — Protocol notes for `status` and `orchestrationCatalog`.
