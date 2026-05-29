---
status: stable
---

# Configuration File

This document specifies **on-disk** **`config.json`** at the level of top-level blocks and responsibilities, as consumed by the Chai runtime. It matches the implementation in **`crates/lib/src/config.rs`** and pairs with **[GATEWAY_STATUS.md](GATEWAY_STATUS.md)** for cross-checking. Full field lists, serde names, and types live in **`config.rs`** and **`README.md`**.

## Purpose

The file expresses operator intent: bind address, channels, providers, agent layout, skills enablement, and delegation policy. Environment variables and helpers in **`config.rs`** resolve effective values at runtime.

### Relationship To Gateway Status

| Artifact | Role | See |
|----------|------|-----|
| **`config.json`** | Operator intent. | This document; **`config.rs`**; **`README.md`** |
| **`status` WebSocket payload** | Runtime snapshot (sanitized). | [GATEWAY_STATUS.md](GATEWAY_STATUS.md) |

The desktop app aligns **config** top-level blocks with **`status`** payload blocks in this order: **`clock`**, **`gateway`**, **`channels`**, **`providers`**, **`agents`**, **`skillPackages`**. Per-agent **`skillsEnabled`** and **`contextMode`** in **`config.json`** correspond to the matching row in **`status.agents.entries`** (**`skills.enabledSkills`**, **`skills.contextMode`**, and the skill context fields on that object). **`config.json`** has **no** top-level **`skills`** key.

---

## Source Of Truth

- **Types and serde names:** **`crates/lib/src/config.rs`** (`#[serde(rename_all = "camelCase")]` on **`Config`** and nested structs).
- **Paths and environment overrides:** **`README.md`** and **`resolve_*`** helpers in **`config.rs`**.
- **Agents in the file:** **`agents`** is a **JSON array** of **`{ id, role, … }`**; it deserializes into **`AgentsConfig`** in code.

This spec describes **blocks and policy**, not every optional key.

---

## Top-Level Shape

```json
{
  "gateway": { },
  "channels": { },
  "providers": { },
  "agents": [ ]
}
```

**`providers`** may be omitted when defaults or environment suffice. **`agents`** is an array in the file. There is **no** top-level **`skills`** object; per-agent **`skillsEnabled`** and **`contextMode`** live on orchestrator and worker entries inside **`agents`**.

---

## Configuration Blocks

Counterpart to the status blocks table in [GATEWAY_STATUS.md](GATEWAY_STATUS.md).

| Block | Holds (summary) | Notes |
|-------|-----------------|--------|
| **`gateway`** | Listen **`bind`**, **`port`**; **`auth.mode`** (**`none`** \| **`token`**) and optional **`token`** (WebSocket connect). | Token may be overridden by **`CHAI_GATEWAY_TOKEN`**. Loopback-only semantics for **`none`** auth. |
| **`channels`** | Telegram (bot token, webhook), Matrix (homeserver, credentials, room allowlist, store path, …), Signal (HTTP daemon URL, account). | Fields have **`resolve_*`** overrides (see **`config.rs`** and **`README.md`**). |
| **`providers`** | Per-backend entries: **`ollama`**, **`lms`**, **`nim`**, **`vllm`**, **`openai`**, **`hf`** — base URLs and API keys where applicable. | Model API endpoints; not chat surfaces. Omitted when defaults or env suffice. |
| **`agents`** | Orchestrator + workers: ids, roles, **`defaultProvider`** / **`defaultModel`**, **`enabledProviders`** (discovery scope), **`skillsEnabled`** (package names under the resolved skills root), **`contextMode`** (**`full`** \| **`readOnDemand`**), session caps, delegation policy (workers, allowlists, caps, routes, blocked providers). On-disk **`AGENTS.md`** for each entry is **`<profileRoot>/agents/<id>/AGENTS.md`**. | Exactly one orchestrator; workers use **`role: worker`**. Omit **`agents`** for the built-in default orchestrator only. Missing or empty **`skillsEnabled`** on an entry means no skills for that agent. Skill packages are loaded from the shared discovery root (see **`README.md`**); there is **no** top-level **`skills`** block. |

---

## Environment Overrides

Effective configuration combines the file with **`config.rs`** resolution: **`resolve_gateway_token`**, **`resolve_telegram_token`**, **`resolve_matrix_room_allowlist`**, provider keys (NIM, OpenAI, HF, …). New overrides are implemented in **`config.rs`** and documented in **`README.md`**. **`status`** reflects effective runtime values, not which source supplied a given secret.

---

## Related Documents

- **[GATEWAY_STATUS.md](GATEWAY_STATUS.md)** — Runtime **`status`** payload and alignment with these blocks.
- **[CHANNELS.md](CHANNELS.md)** — Channel config and runtime behavior.
- **[ORCHESTRATION.md](ORCHESTRATION.md)** — Delegation policy and worker semantics.
- **[PROVIDERS.md](PROVIDERS.md)** — Provider ids and discovery vs **`agents.enabledProviders`**.
- **[CONTEXT.md](CONTEXT.md)** — Per-agent **`contextMode`** and **`skillsEnabled`** in context and tools.
- **[RUNTIME_PROFILES.md](../epic/RUNTIME_PROFILES.md)** — **`profileRoot`** and **`<profileRoot>/agents/<id>/`**.
