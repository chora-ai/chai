# Configuration File

**Status:** Draft — describes the **on-disk** `config.json` shape at the level of top-level blocks and responsibilities. **Authoritative field lists and types** remain in **`crates/lib/src/config.rs`** (serde) and user **`README.md`**; update this document when behavior or grouping changes in a way operators care about.

## Purpose

This document is the **specification for configuration** as consumed by the Chai runtime: what each top-level block is for, how it relates to environment overrides, and how it pairs with **[GATEWAY_STATUS.md](GATEWAY_STATUS.md)** for cross-checking.

**Process:** Keep **this spec** and **Gateway Status** updated together while the nested status payload and dashboard cross-checks are still being defined. When implementation direction stabilizes, trim draft notices and align examples with the wire format and `config.json` samples in `README.md`.

### Relationship To Gateway Status

| Artifact | Role | See |
|----------|------|-----|
| **`config.json`** | Operator intent: what to bind, which channels and providers to use, agent layout, skills paths, etc. | This document; `lib::config`; `README` |
| **`status` WebSocket payload** | Runtime snapshot: what the gateway is actually using (sanitized). | [GATEWAY_STATUS.md](GATEWAY_STATUS.md) |

Cross-checking in the desktop app (and elsewhere) uses **the same top-level names** as this file: `gateway`, `channels`, `providers`, `agents`, `skills`. The **[GATEWAY_STATUS.md](GATEWAY_STATUS.md)** payload is **preferred nested** under those keys so **configured** vs **live** can be compared without a translation layer.

---

## Source Of Truth

- **Types and serde names:** `crates/lib/src/config.rs` (`#[serde(rename_all = "camelCase")]` on `Config` and nested structs).
- **User-facing paths and env vars:** `README.md` (and module docs on `resolve_*` helpers in `config.rs`).
- **Agents array wire format:** `agents` is a **JSON array** of `{ id, role, … }` in the file; it deserializes into the flattened `AgentsConfig` struct in code.

Do not duplicate every optional field here; add prose only when it clarifies **block boundaries** or **policy** (e.g. delegation, discovery).

---

## Top-Level Shape

```json
{
  "gateway": { },
  "channels": { },
  "providers": { },
  "agents": [ ],
  "skills": { }
}
```

(`agents` is an array in the file; `providers` may be omitted.)

---

## Configuration Blocks

The table below describes **what each block holds in the configuration context**. It is the counterpart to the “status blocks” table in [GATEWAY_STATUS.md](GATEWAY_STATUS.md) (what status should report vs avoid).

| Block | Holds (summary) | Notes |
|-------|-----------------|--------|
| **`gateway`** | Listen `bind`, `port`; `auth.mode` (`none` \| `token`) and optional `token` (WebSocket connect). | Token may be overridden by `CHAI_GATEWAY_TOKEN`. Loopback-only semantics for `none` auth. |
| **`channels`** | Telegram (bot token, webhook), Matrix (homeserver, credentials, room allowlist, store path, …), Signal (HTTP daemon URL, account). | Many fields have env overrides (see `resolve_*` in `config.rs` and `README`). |
| **`providers`** | Optional per-backend entries: `ollama`, `lms`, `nim`, `vllm`, `openai`, `hf` — base URLs and API keys where applicable. | Sibling to `channels`: model API endpoints, not chat surfaces. Omitted when defaults/env suffice. |
| **`agents`** | Orchestrator + worker definitions: ids, roles, `defaultProvider` / `defaultModel`, `enabledProviders` (discovery scope), workspace, session caps, delegation policy (workers, allowlists, caps, routes, blocked providers). | Exactly one orchestrator; worker entries are `role: worker`. Omit `agents` key for built-in default orchestrator only. |
| **`skills`** | Skill root `directory`, `extraDirs`, `enabled` name list, `contextMode` (`full` \| `readOnDemand`). | Paths resolved relative to config file location where documented. |

---

## Environment Overrides

Configuration is not only the file: **`config.rs`** defines `resolve_gateway_token`, `resolve_telegram_token`, `resolve_matrix_room_allowlist`, provider keys (e.g. NIM, OpenAI, HF), and more. Prefer documenting new overrides in **`README.md`** and mentioning them here only when they affect **cross-checking** (e.g. “status shows effective token presence, not which source supplied it”).

---

## Related Documents

- **[GATEWAY_STATUS.md](GATEWAY_STATUS.md)** — Runtime `status` payload, redaction, and alignment with these blocks.
- **[CHANNELS.md](CHANNELS.md)** — How channel config maps to runtime behavior.
- **[ORCHESTRATION.md](ORCHESTRATION.md)** — Delegation policy and worker semantics.
- **[PROVIDERS.md](PROVIDERS.md)** — Provider ids and discovery interaction with `agents.enabledProviders`.
- **[CONTEXT.md](CONTEXT.md)** — Skills context modes vs `skills.contextMode`.
