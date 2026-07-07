---
status: stable
---

# Gateway Status

This document specifies the gateway **`status`** WebSocket response **payload**: grouped runtime snapshot for cross-checking with [CONFIGURATION.md](CONFIGURATION.md). The shape matches the implementation in **`crates/lib/src/gateway/server.rs`**. Authoritative **`config.json`** field lists remain in **`crates/lib/src/config.rs`** and **`README.md`**.

## Purpose

**`status`** is **not** a copy of config: it omits secrets and includes derived summaries (context strings, tool JSON, model lists).

### Relationship To Configuration

| Artifact | Role | Authority |
|----------|------|-----------|
| **`config.json`** | What the operator configured. | File |
| **`status` payload** | What the running gateway is using (listeners, channels, model discovery, agents, shared skill store). | Runtime |

**Protocol:** WebSocket `req` / `res` envelope; method **`"status"`**, params **`{}`**. Payload keys are **camelCase**. **`gateway.protocol`** in the **`status`** payload and in connect **`hello-ok`** is **`1`**.

---

## Payload Shape

Top-level key order matches **`config.json`** blocks for cross-check: **`gateway`**, **`channels`**, **`providers`**, **`sandbox`**, **`agents`**, **`skills`**. The gateway emits keys in this order (see **`serde_json`** **`preserve_order`** in the gateway build).

```json
{
  "gateway": { },
  "channels": { },
  "providers": { },
  "sandbox": { },
  "agents": [ ],
  "skills": { }
}
```

The top-level **`skills`** block holds shared skill store metadata (not per-agent). Per-agent skill fields (**`enabledSkills`**, **`enabledWorkers`**, **`contextMode`**) are flat fields on each **`agents[]`** object. Heavy per-agent data (**`systemContext`**, **`tools`**, **`skillsContext`**) is not included in the polling `status` response; it is available on-demand via the **`agentDetail`** WebSocket method (see below).

### `gateway`

| Field | Meaning |
|-------|---------|
| **`status`** | **`"running"`** while the gateway serves **`status`**. |
| **`protocol`** | Wire protocol version (**`1`**). |
| **`port`**, **`bind`** | Effective listen address. |
| **`auth`** | **`"none"`** or **`"token"`**. |
| **`maxConnections`** | Effective max authenticated WebSocket connections. **`null`** = unlimited; integer = cap. Derived from **`gateway.maxConnections`** config and bind address (loopback defaults to unlimited; non-loopback defaults to 1). |

### `sandbox`

| Field | Meaning |
|-------|---------|
| **`mode`** | Effective sandbox mode from **`config.json`** **`sandbox.mode`**: **`"strict"`**, **`"current"`**, or **`"unsafe"`**. |
| **`roots`** | Number of writable roots in the sandbox (0 when sandbox is missing and mode is `"unsafe"`). |

### `channels`

Per integration: **`telegram`** (always on), **`matrix`** (requires `matrix` feature), **`signal`** (requires `signal` feature). A channel that is not compiled in does not appear in the status payload. Each value includes **`active`** (registered with the gateway) and **`configured`** (non-secret prerequisites present in config/env). Additional keys are merged from the channel implementation (no secrets):

| Channel | Fields |
|---------|--------|
| **`telegram`** | **`transport`**: **`longPoll`** \| **`webhook`**; **`lastError`**: last inbound/poll error (truncated) or null. |
| **`matrix`** | **`sessionActive`**, **`syncRunning`**, **`lastSyncError`**, **`pendingVerificationCount`**, **`pendingVerifications`**, **`roomAllowlistActive`**. |
| **`signal`** | **`transport`**: **`sse`**; **`daemonCheckOk`**: startup **`GET …/api/v1/check`** succeeded; **`lastError`**: SSE/connect issues. |

### `providers`

Keys: provider ids from the `providers` array (e.g. **`ollama`**, **`lmstudio`**, **`nearai`**, **`nvidia`**). Each value:

| Field | Meaning |
|-------|---------|
| **`endpointType`** | Wire protocol: **`"ollama"`** or **`"openai-compat"`**. |
| **`modelDiscovery`** | Discovery method: **`"auto"`**, **`"lmstudio"`**, or **`"static"`** (mirrors config `providers[].modelDiscovery`). |
| **`models`** | Array of model name strings; empty when the provider is not in the orchestrator's **`enabledProviders`** scope or the backend is unreachable. |

### `skills`

Shared skill store on disk and lockfile state (not per-agent):

| Field | Meaning |
|-------|---------|
| **`packagesDiscovered`** | Package count on disk before per-agent **`enabledSkills`** filtering. |
| **`lockMode`** | Effective lock mode from **`config.json`** **`skills.lockMode`**: **`"strict"`** or **`"warn"`**. |
| **`lockGeneration`** | Current generation number from the profile's `skills.lock`, or **`null`** when no lockfile exists. |
| **`lockedSkills`** | Number of skills pinned in the lockfile (0 when no lockfile exists). |

### `agents`

Array of per-agent runtime rows. Orchestrator entries first (one per orchestrator in **`config.json`** order), then workers sorted by **`id`**. Each object corresponds to one **`config.json`** agent row (orchestrator or worker):

| Field | Meaning |
|-------|---------|
| **`id`**, **`role`** | Agent id; **`orchestrator`** or **`worker`**. |
| **`defaultProvider`**, **`defaultModel`** | Effective routing defaults for that row. |
| **`enabledProviders`** | Orchestrator: provider ids for discovery scope (same semantics as config). Workers: **`null`**. |
| **`enabledSkills`** | Skill package names loaded for that agent. Mirrors **`config.json`** **`agents[].enabledSkills`**. |
| **`enabledWorkers`** | Orchestrator: worker ids this orchestrator can delegate to (array or **`null`**; absent/`null` means no workers; empty array means all workers). Workers: **`null`**. |
| **`contextMode`** | **`"full"`** or **`"readOnDemand"`**. Mirrors **`config.json`** **`agents[].contextMode`**. |
| **`maxToolLoopsPerTurn`** | Orchestrator: maximum tool loops per turn (integer or **`null`**; omitted = no limit; applies globally to both orchestrator and worker turns). Workers: **`null`**. |
| **`maxDelegationsPerTurn`** | Orchestrator: optional cap on **`delegate_task`** calls per turn (integer or **`null`**). Workers: **`null`**. |
| **`maxDelegationsPerSession`** | Orchestrator: optional cap on **`delegate_task`** calls per session (integer or **`null`**). Workers: **`null`**. |
| **`maxDelegationsPerWorker`** | Orchestrator: optional per-worker delegation caps (object or **`null`**). Workers: **`null`**. |

### `agentDetail` (On-Demand Per-Agent Data)

**Protocol:** WebSocket `req` / `res` envelope; method **`"agentDetail"`**, params **`{ "agentId": "<id>" }`**.

Heavy per-agent fields that are **not** included in the polling **`status`** response to reduce payload size. Fetched on-demand by the desktop when the Agent or Tools screen is active. The handler resolves agents by checking the `orchestrator_runtimes` map first (keyed by orchestrator ID), then falling back to `worker_delegate_runtimes` (keyed by worker ID).

| Field | Meaning |
|-------|---------|
| **`id`** | Agent id (same as the requested `agentId`). |
| **`role`** | **`"orchestrator"`** or **`"worker"`**. |
| **`systemContext`** | Full system context string for that role (built at startup from agent context, optional workers roster filtered by `enabledWorkers`, and skills). Injected as `messages[0]` on every turn; not persisted in the session store. |
| **`tools`** | Pretty-printed JSON array of that agent's tool definitions, or **`null`**. Sent as a separate top-level field in the provider request; never part of the messages array. |
| **`skillsContext`** | Per-skill body (name → frontmatter-stripped body). **`null`** when no skills are loaded. |

Returns an error (`"unknown agent id"`) if the requested agent id does not match the orchestrator or any configured worker.

---

## Status Blocks And Redaction

| Block | Include | Omit |
|-------|---------|------|
| **`gateway`** | Bind, port, auth mode, protocol, max connections | Secrets |
| **`channels`** | Active vs configured, transport hints | Tokens, passwords, Matrix access tokens |
| **`providers`** | Endpoint type, model discovery method, model lists | API keys, URLs that embed credentials |
| **`sandbox`** | Mode, root count | — |
| **`agents`** | Effective defaults, per-agent runtime rows | Full raw **`config.json`** |
| **`skills`** | Disk package count, lock mode, lockfile generation, locked skill count | Full directory trees |

---

## Related Documents

- **[CONFIGURATION.md](CONFIGURATION.md)** — On-disk blocks vs **`status`** blocks.
- **[CHANNELS.md](CHANNELS.md)** — Channel runtime behavior.
- **[ORCHESTRATION.md](ORCHESTRATION.md)** — Catalog and delegation.
- **[PROVIDERS.md](PROVIDERS.md)** — Provider ids and discovery rules.
- **[PROFILES.md](PROFILES.md)** — Per-profile lockfile (`skills.lock`) and generation tracking.
- **[SKILL_FORMAT.md](SKILL_FORMAT.md)** — Skill directory layout, `SKILL.md` content, and frontmatter.
- **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)** — Skill package versioned layout, startup validation, and CLI commands.
- **[CONTEXT.md](CONTEXT.md)** — Context on every turn: system message, session history, and tool schemas.
- **`crates/lib/src/gateway/protocol.rs`** — WebSocket protocol notes.
