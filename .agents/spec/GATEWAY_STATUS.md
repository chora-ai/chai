---
status: stable
---

# Gateway Status

This document specifies the gateway **`status`** WebSocket response **payload**: grouped runtime snapshot for cross-checking with [CONFIGURATION.md](CONFIGURATION.md). The shape matches the implementation in **`crates/lib/src/gateway/server.rs`**. Authoritative **`config.json`** field lists remain in **`crates/lib/src/config.rs`** and **`README.md`**.

## Purpose

**`status`** is **not** a copy of config: it omits secrets and includes derived summaries (context strings, tool JSON, discovery lists).

### Relationship To Configuration

| Artifact | Role | Authority |
|----------|------|-----------|
| **`config.json`** | What the operator configured. | File |
| **`status` payload** | What the running gateway is using (listeners, channels, provider discovery, agents, shared skill store). | Runtime |

**Protocol:** WebSocket `req` / `res` envelope; method **`"status"`**, params **`{}`**. Payload keys are **camelCase**. **`gateway.protocol`** in the **`status`** payload and in connect **`hello-ok`** is **`1`**.

---

## Payload Shape

Top-level key order matches **`config.json`** blocks for cross-check, with **`clock`** first and **`skillPackages`** last: **`clock`**, **`gateway`**, **`channels`**, **`providers`**, **`agents`**, **`skillPackages`**. The gateway emits keys in this order (see **`serde_json`** **`preserve_order`** in the gateway build).

```json
{
  "clock": { },
  "gateway": { },
  "channels": { },
  "providers": { },
  "agents": { },
  "skillPackages": { }
}
```

There is **no** top-level **`skills`** object. Skill **packages** on disk are under **`skillPackages`**; per-agent skill **runtime** is in each **`agents.entries[]`** object under **`skills`**.

### `gateway`

| Field | Meaning |
|-------|---------|
| **`runtime`** | **`"running"`** while the gateway serves **`status`**. |
| **`protocol`** | Wire protocol version (**`1`**). |
| **`port`**, **`bind`** | Effective listen address. |
| **`auth`** | **`"none"`** or **`"token"`**. |

### `channels`

Per integration: **`telegram`**, **`matrix`**, **`signal`**. Each value includes **`active`** (registered with the gateway) and **`configured`** (non-secret prerequisites present in config/env). Additional keys are merged from the channel implementation (no secrets):

| Channel | Fields |
|---------|--------|
| **`telegram`** | **`transport`**: **`longPoll`** \| **`webhook`**; **`lastError`**: last inbound/poll error (truncated) or null. |
| **`matrix`** | **`sessionActive`**, **`syncRunning`**, **`lastSyncError`**, **`pendingVerificationCount`**, **`pendingVerifications`**, **`roomAllowlistActive`**. |
| **`signal`** | **`transport`**: **`sse`**; **`daemonCheckOk`**: startup **`GET …/api/v1/check`** succeeded; **`lastError`**: SSE/connect issues. |

### `providers`

Keys: **`ollama`**, **`lms`**, **`vllm`**, **`nim`**, **`openai`**, **`hf`**. Each value:

| Field | Meaning |
|-------|---------|
| **`discovery`** | Whether model discovery ran for this id (per orchestrator **`enabledProviders`** in config; see [PROVIDERS.md](PROVIDERS.md)). |
| **`models`** | Array of model objects (includes **`name`** where applicable); empty when discovery is off or the backend is unreachable. |

### `clock`

| Field | Meaning |
|-------|---------|
| **`date`** | Calendar date (**`YYYY-MM-DD`**) used in system context (**`TODAYS_DATE=`**); not a skill field. |

### `skillPackages`

Shared skill store on disk (not per-agent):

| Field | Meaning |
|-------|---------|
| **`discoveryRoot`** | Directory the gateway scans for packages (resolved at startup; default layout under **`~/.chai`** — see **`README.md`**). |
| **`packagesDiscovered`** | Package count on disk before per-agent **`skillsEnabled`** filtering. |

### `agents`

| Field | Meaning |
|-------|---------|
| **`orchestrationCatalog`** | Merged allowlist + discovery rows (**`{ provider, model, discovered, local?, toolCapable? }`**). |
| **`entries`** | Per-agent runtime rows (below). Orchestrator first (**`role`**: **`orchestrator`**), then workers sorted by **`id`**. |

#### `agents.entries[]`

Each object corresponds to one **`config.json`** agent row (orchestrator or worker):

| Field | Meaning |
|-------|---------|
| **`id`**, **`role`** | Agent id; **`orchestrator`** or **`worker`**. |
| **`contextDirectory`** | Absolute path to **`AGENTS.md`** home (**`<profile>/agents/<id>/`**). Workers use **`""`** when not resolved. |
| **`defaultProvider`**, **`defaultModel`** | Effective routing defaults for that row. |
| **`enabledProviders`** | Orchestrator: provider ids for discovery scope (same semantics as config). Workers: **`null`**. |
| **`systemContext`** | Full static system string for that role for the current date (**`TODAYS_DATE=`**, **`SKILLS_ENABLED=`**; orchestrator adds **`WORKERS_ENABLED=`** when workers exist). |
| **`tools`** | Pretty-printed JSON array of that agent’s tool definitions, or **`null`**. |
| **`skills`** | Per-agent skill runtime (below). |

#### `agents.entries[].skills`

| Field | Meaning |
|-------|---------|
| **`enabledSkills`** | Skill package names loaded for that agent (subset under **`skillPackages.discoveryRoot`**). |
| **`contextMode`** | **`"full"`** or **`"readOnDemand"`**. |
| **`skillsContext`**, **`skillsContextFull`**, **`skillsContextBodies`** | Skill text slices (see [CONTEXT.md](CONTEXT.md)). |

---

## Status Blocks And Redaction

| Block | Include | Omit |
|-------|---------|------|
| **`clock`** | Date for operator verification | Secrets |
| **`gateway`** | Bind, port, auth mode, protocol | Secrets |
| **`channels`** | Active vs configured, transport hints | Tokens, passwords, Matrix access tokens |
| **`providers`** | Discovery flag, model lists | API keys, URLs that embed credentials |
| **`agents`** | Effective defaults, catalog, **`entries`** | Full raw **`config.json`** |
| **`skillPackages`** | Store root path, disk package count | Full directory trees |

---

## Related Documents

- **[CONFIGURATION.md](CONFIGURATION.md)** — On-disk blocks vs **`status`** blocks.
- **[CHANNELS.md](CHANNELS.md)** — Channel runtime behavior.
- **[ORCHESTRATION.md](ORCHESTRATION.md)** — Catalog and delegation.
- **[PROVIDERS.md](PROVIDERS.md)** — Provider ids and discovery rules.
- **[CONTEXT.md](CONTEXT.md)** — Context strings and skills mode.
- **`crates/lib/src/gateway/protocol.rs`** — WebSocket protocol notes.

---

## Open Questions

- **Wire versioning:** When **`gateway.protocol`** should increment, how breaking changes are announced, and how clients (desktop, CLI, scripts) should behave on mismatch — policy TBD outside this payload spec.
