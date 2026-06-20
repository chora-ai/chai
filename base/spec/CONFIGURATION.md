---
status: stable
---

# Configuration File

This document specifies **on-disk** **`config.json`** at the level of top-level blocks and responsibilities, as consumed by the Chai runtime. It matches the implementation in **`crates/lib/src/config.rs`** and pairs with **[GATEWAY_STATUS.md](GATEWAY_STATUS.md)** for cross-checking. Full field lists, serde names, and types live in **`config.rs`** and **`README.md`**.

## Purpose

The file expresses operator intent: bind address, channels, providers, agent layout, skills settings, and delegation policy. Environment variables and helpers in **`config.rs`** resolve effective values at runtime.

### Relationship To Gateway Status

| Artifact | Role | See |
|----------|------|-----|
| **`config.json`** | Operator intent. | This document; **`config.rs`**; **`README.md`** |
| **`status` WebSocket payload** | Runtime snapshot (sanitized). | [GATEWAY_STATUS.md](GATEWAY_STATUS.md) |

The desktop app aligns **config** top-level blocks with **`status`** payload blocks in this order: **`gateway`**, **`channels`**, **`providers`**, **`sandbox`**, **`agents`**, **`skills`**. Per-agent **`enabledSkills`** and **`contextMode`** in **`config.json`** correspond to the matching row in **`status.agents[]`** (**`enabledSkills`**, **`contextMode`**, and the skill context fields on that object). The top-level **`skills`** block in **`config.json`** corresponds to the top-level **`skills`** block in the **`status`** payload.

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
  "providers": [ ],
  "sandbox": {
    "mode": "strict"
  },
  "agents": [ ],
  "skills": {
    "lockMode": "strict"
  }
}
```

**`providers`** may be omitted when defaults or environment suffice. **`agents`** is an array in the file. The **`skills`** block holds shared skill package settings; per-agent **`enabledSkills`** and **`contextMode`** live on orchestrator and worker entries inside **`agents`**. **`skills.lockMode`** controls gateway startup behavior for the lockfile — in strict mode (the default), the lockfile acts as a complete manifest: the gateway refuses to start if the lockfile is missing, any enabled skill has no lock entry (unpinned), or any pinned skill's active version does not match its locked hash (see [PROFILES.md](PROFILES.md)).

---

## Configuration Blocks

Counterpart to the status blocks table in [GATEWAY_STATUS.md](GATEWAY_STATUS.md).

| Block | Holds (summary) | Notes |
|-------|-----------------|-------|
| **`gateway`** | Listen **`bind`**, **`port`**; **`auth.mode`** (**`none`** \| **`token`**) and optional **`token`** (WebSocket connect). | Token may be overridden by **`CHAI_GATEWAY_TOKEN`**. Loopback-only semantics for **`none`** auth. |
| **`sandbox`** | **`mode`** (**`"strict"`** (default) \| **`"current"`** \| **`"unsafe"`**) — how the gateway handles a missing sandbox directory. | **`mode`** defaults to `"strict"`: gateway refuses to start without a sandbox directory. `"current"`: use CWD as the sole writable root when the sandbox directory is missing. `"unsafe"`: start without a sandbox; CWD confinement and path validation are disabled. |
| **`channels`** | Telegram (bot token, webhook), Matrix (homeserver, credentials, room allowlist, store path, …), Signal (HTTP daemon URL, account). | Fields have **`resolve_*`** overrides (see **`config.rs`** and **`README.md`**). Matrix requires the `matrix` Cargo feature (experimental); Signal requires the `signal` Cargo feature (experimental). Config fields for a disabled channel are accepted but have no effect. |
| **`providers`** | Per-backend entries: **`ollama`**, **`lms`**, **`nearai`**, **`nim`** — plus any other `"openai-compat"` server with a `baseUrl` and `apiKey`. | Model API endpoints; not chat surfaces. Omitted when defaults or env suffice. |
| **`agents`** | Orchestrator + workers: ids, roles, **`defaultProvider`** / **`defaultModel`**, **`enabledProviders`** (orchestrator-only; discovery scope), **`enabledSkills`** (package names under the resolved skills root), **`contextMode`** (**`full`** \| **`readOnDemand`**), **`maxToolLoopsPerTurn`** (orchestrator-only; omitted = no limit; applies globally to both orchestrator and worker turns), delegation caps (orchestrator-only: **`maxDelegationsPerTurn`**, **`maxDelegationsPerSession`**, **`maxDelegationsPerWorker`**). On-disk **`AGENT.md`** for each entry is **`<profileRoot>/agents/<id>/AGENT.md`**. | Exactly one orchestrator; workers use **`role: worker`**. Each worker has a single **`(defaultProvider, defaultModel)`** pair — no override parameters or session/delegation caps. Orchestrator-only fields set on a worker entry are rejected at parse time. Omit **`agents`** for the built-in default orchestrator only. Missing or empty **`enabledSkills`** on an entry means no skills for that agent. Skill packages are loaded from the shared discovery root (see **`README.md`**). |
| **`skills`** | **`lockMode`** (**`"strict"`** (default) \| **`"warn"`**) — how the gateway handles the per-profile `skills.lock` at startup. | `"strict"` (default) treats the lockfile as a complete manifest: refuses to start when the lockfile is missing, any enabled skill has no lock entry (unpinned), or any pinned skill's active version does not match its locked hash. `"warn"` logs warnings on mismatches, allows unpinned skills, and skips verification when no lockfile is present. See [PROFILES.md](PROFILES.md). |

## Environment Overrides

Effective configuration combines the file with **`config.rs`** resolution: **`resolve_gateway_token`**, **`resolve_telegram_token`**, **`resolve_telegram_webhook_secret`**, **`resolve_matrix_homeserver`**, **`resolve_matrix_access_token`**, **`resolve_matrix_user`**, **`resolve_matrix_password`**, **`resolve_matrix_user_id`**, **`resolve_matrix_device_id`**, **`resolve_matrix_room_allowlist`**, **`matrix_channel_configured`**, provider API keys. New overrides are implemented in **`config.rs`** and documented in **`README.md`**. **`status`** reflects effective runtime values, not which source supplied a given secret.

### `.env` File

If a `.env` file exists in the profile directory (e.g. `~/.chai/profiles/assistant/.env`), it is loaded at startup — before logger initialization — so that environment-driven configuration like `RUST_LOG` takes effect. Variables from `.env` are set in the process environment only if they are not already set — shell environment variables always take precedence. This enables `apiKey` values using the `<VAR_NAME>` syntax to resolve against profile-local secrets without hardcoding them in `config.json`, and also supports runtime environment variables like `RUST_LOG`, `CHAI_BIN`, and channel tokens (e.g. `TELEGRAM_BOT_TOKEN`). See [PROVIDERS.md](PROVIDERS.md) for the full `apiKey` resolution semantics.

---

## Desktop Configuration

The desktop app reads a separate `~/.chai/desktop.json` file for client-side settings (appearance, log buffer size) that are machine-local and not tied to any profile. This file is documented in [DESKTOP.md](DESKTOP.md). No `config.json` schema changes are needed — `desktop.json` is purely additive.

## Related Documents

- **[GATEWAY_STATUS.md](GATEWAY_STATUS.md)** — Runtime **`status`** payload and alignment with these blocks.
- **[CHANNELS.md](CHANNELS.md)** — Channel config and runtime behavior.
- **[ORCHESTRATION.md](ORCHESTRATION.md)** — Delegation policy and worker semantics.
- **[PROVIDERS.md](PROVIDERS.md)** — Provider ids and discovery vs **`agents.enabledProviders`**.
- **[CONTEXT.md](CONTEXT.md)** — Per-agent **`contextMode`** and **`enabledSkills`** in context and tools.
- **[PROFILES.md](PROFILES.md)** — **`profileRoot`**, **`<profileRoot>/agents/<id>/`**, and profile directory structure.
