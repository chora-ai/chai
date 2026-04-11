---
status: complete
---

# Epic: Agent Isolation (Per-Agent Context Directories and Skills)

**Summary** — Give each logical agent (orchestrator and each worker) its **own agent context directory** under **`~/.chai/profiles/<profile>/agents/<agentId>/`** ( **`profileRoot`** = active profile directory per **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** ), **distinct system context** (no shared orchestrator preamble on workers), and **per-agent skill configuration** (**`skillsEnabled`**, **`contextMode`** on each agent entry). Remove **global** skill enablement and global skill context mode from config in favor of agent-scoped fields. Chai has **no** compatibility contract: **no** shims, **no** fallback paths, **no** migration from **`workspace/AGENTS.md`**—only **`agents/<id>/AGENTS.md`** at that fixed path holds on-disk agent context (**no** per-entry directory override). **Implemented:** config, gateway, delegation, **`chai init`**, **README**, desktop **Config** / **Context** / **Skills**, and internal specs aligned with this behavior.

**Status** — **Complete** for planned phases. Runtime behavior, **README**, **`status.agents.entries`**, and internal specs match **Decisions (Shipped)**. **Follow-ups (Non-Blocking)** below are **fully shipped** (entries kept as a record).

## Problem Statement

**Previously**, the gateway built **one** static system context and **one** skill tool set at startup: shared **`AGENTS.md`**, a single **`skills.enabled`** list, and one **`skills.contextMode`**. **Worker** turns reused that same preamble—including copy that describes the **orchestrator** role—and the same tools, minus **`delegate_task`**. There was no first-class place on disk for per-agent instructions, and no way to give a small worker model only the skills it needs without giving it the full set.

## Goal

- Each **agent id** uses **`<profileRoot>/agents/<agentId>/`** for **`AGENTS.md`**, where **`profileRoot`** is always the active runtime profile directory (**`~/.chai/profiles/<name>/`** resolved via **`~/.chai/active`**, **`CHAI_PROFILE`**, or CLI — see **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)**).
- Each agent entry in the top-level **`agents`** array declares **its own** skill configuration (**which** skills are enabled, **`contextMode`** for skill text).
- **Orchestrator** and **worker** turns each receive **correct role-specific** system text (workers are not told they are the orchestrator).
- **No** accumulating legacy config: drop top-level **`skills.enabled`** and **`skills.contextMode`**. Skill packages are discovered only from **`~/.chai/skills`** (no top-level **`skills`** config block).

## Current State

- **Config** ([`config.rs`](../../crates/lib/src/config.rs)): JSON **`agents`** array entries (orchestrator + workers) support **`skillsEnabled`**, **`contextMode`**. There is **no** top-level **`skills`** object; discovery uses **`~/.chai/skills`** only. **`orchestrator_context_dir`** / **`worker_context_dir`** resolve **`<profileRoot>/agents/<id>/`** (fixed rule).
- **Gateway** ([`gateway/server.rs`](../../crates/lib/src/gateway/server.rs)): loads skill packages from disk; **filters** by orchestrator **`skillsEnabled`** for the main turn and builds orchestrator tools/executor; builds **`WorkerDelegateRuntime`** **per worker** (worker **`AGENTS.md`**, worker skills, tools, executor—**no** **`## Workers`** block). **`GatewayState.system_context_static`** is **orchestrator-only** (orchestrator **`AGENTS.md`** + **`build_workers_context`** + orchestrator skill text).
- **`build_workers_context`** ([`workers_context.rs`](../../crates/lib/src/orchestration/workers_context.rs)): included **only** in the orchestrator static string, **not** in worker delegate bundles.
- **Delegation** ([`delegate.rs`](../../crates/lib/src/orchestration/delegate.rs)): **`DelegateContext`** supplies orchestrator fields plus **`worker_runtimes`**; **`execute_delegate_task`** uses the **worker** bundle when **`workerId`** is set, otherwise orchestrator skill tools for **delegate** without **`workerId`**.
- **`chai init`** ([`init.rs`](../../crates/lib/src/init.rs)): writes **`profiles/<name>/agents/orchestrator/AGENTS.md`**; does **not** create **`workspace/AGENTS.md`**.
- **Desktop** (**`crates/desktop/`**): **Config** shows the orchestrator **agent context directory** (**`orchestrator_context_dir`**); **Context** / **Skills** use orchestrator fields and **`status.agents.entries`** for per-agent previews.
- **Docs**: **README** and internal **specs** describe per-agent **`AGENTS.md`** paths, skills, and **`status`** fields.

## Scope

### In Scope

- Filesystem layout: **`<profileRoot>/agents/<agentId>/`** for each agent’s **`AGENTS.md`**; **`chai init`** scaffolds **`agents/orchestrator/AGENTS.md`** (default orchestrator id **`orchestrator`**) under **each** default profile; operators add **`agents/<workerId>/AGENTS.md`** when they define worker entries in config.
- **Per-agent** fields: **`skillsEnabled`** (array of skill names), **`contextMode`**: **`full`** \| **`readOnDemand`** (same semantics as the former global skill context mode, but per agent).
- **Skill root**: fixed **`~/.chai/skills`**; **remove** any top-level **`skills`** JSON block (**`directory`**, **`extraDirs`**, **`enabled`**, **`contextMode`**).
- Gateway: build **per-agent** static context and **per-agent** tool lists (and executor scope) at startup; **`execute_delegate_task`** selects the **worker** agent’s bundle by **`workerId`**.
- Prompt split: orchestrator system text includes **delegation** + worker roster + orchestrator skills; worker system text is **worker-specific** and **excludes** nested **`delegate_task`** and orchestrator identity copy.
- Update internal specs listed under **Related Epics and Docs** when behavior changes.

### Out of Scope

- **OS-level** sandboxing (containers, VMs); see orchestration epic **Scope**.
- **Hot reload** of per-agent context or skill lists without gateway restart (restart remains the contract).
- **Skill package revisions, lockfiles, pins** — **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)**; this epic assumes packages on disk under shared roots, filtered per agent.
- **Flat `~/.chai/config.json`** or **`~/.chai/agents/`** without a profile parent — **not supported**; same rule as **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** (**`profileRoot`** is always a profile directory).
- **Legacy orchestrator paths** — **No** fallback to **`workspace/AGENTS.md`**, **no** dual locations, **no** “warn and load old path.” Operators who still have content only under **`workspace/`** move it manually to **`agents/<orchestratorId>/AGENTS.md`**.

## Dependencies

- Delegation **primitive** and **`agents`** array (**[ORCHESTRATION.md](ORCHESTRATION.md)**) — implemented.
- **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** — **`profileRoot`**, **`ChaiPaths`**, init layout — **complete**; this epic layers on **`agents/<id>/`** under each profile.
- Skill directory layout and **`tools.json`** — **[spec/SKILL_FORMAT.md](../spec/SKILL_FORMAT.md)**, **[spec/TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md)**.

## Design

### Profiles and agent directories

**[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** keeps **trust-sensitive** state inside **`~/.chai/profiles/<name>/`**. Per-agent **`AGENTS.md`** files supply **on-disk agent context** for that profile’s orchestrator and workers, so they live **inside** the profile: **`~/.chai/profiles/<name>/agents/<agentId>/`**.

| Topic | Resolution |
|-------|------------|
| **Default `agents/<agentId>/`** | **`~/.chai/profiles/<activeProfile>/agents/<agentId>/`** (under the profile tree alongside **`config.json`**, **`matrix/`**, pairing/device state, etc.). |
| **Skill packages on disk** | Shared **`~/.chai/skills/`** only; per-agent **`skillsEnabled`** chooses subsets. |

### Decisions (Shipped)

| Question | Decision |
|----------|----------|
| **Orchestrator context on disk** | **Only** **`agents/<orchestratorId>/AGENTS.md`**. **`workspace/AGENTS.md`** is **not** read by the gateway for any agent; **`chai init`** does **not** create it. Optional: keep an empty profile **`workspace/`** (or omit it) for user misc files—out of scope for gateway behavior. |
| **Skill discovery shape** | Single root **`~/.chai/skills`**; **no** configurable discovery paths in JSON. |
| **`status.agents.entries` and Desktop Context** | Each row includes **`systemContext`** for that agent. Desktop **Context** builds the agent dropdown from **`entries`**: orchestrator two-column (**read-on-demand**) vs worker single column (see **[GATEWAY_STATUS.md](../spec/GATEWAY_STATUS.md)**). |
| **Empty or missing `skillsEnabled`** | **Explicit:** missing or empty **`skillsEnabled`** ⇒ **no** skill tools and **no** skill-derived inlined context for that agent. **No** implicit “inherit from old global list.” Operators must set lists per agent. |

### Example Layout Under `~/.chai`

```text
~/.chai/
├── profiles/
│   ├── assistant/                    # default profile; `active`
│   │   ├── agents/
│   │   │   ├── orchestrator/         # example orchestrator id
│   │   │   │   └── AGENTS.md
│   │   │   └── worker/               # example worker id
│   │   │       └── AGENTS.md
│   │   ├── .env
│   │   ├── config.json
│   │   ├── device.json
│   │   ├── device_token
│   │   └── paired.json
│   └── developer/                    # default profile
│       ├── agents/
│       │   ├── orchestrator/         # example orchestrator id
│       │   │   └── AGENTS.md
│       │   └── worker/               # example worker id
│       │       └── AGENTS.md
│       ├── .env
│       ├── config.json
│       ├── device.json
│       ├── device_token
│       └── paired.json
├── skills/                           # shared skill packages
│   └── <skill-name>/
└── active -> profiles/assistant/
```

- **Agent context directories** live under **`<profileRoot>/agents/<agentId>/`**.
- **Skill discovery** uses **`~/.chai/skills`** only; **enablement** is **only** per-agent **`skillsEnabled`** in **`config.json`**.

### Config Schema Direction

- **Top-level `agents`**: each object includes **`id`**, **`role`**, provider/model and delegation fields as today, plus:
  - **`skillsEnabled`**: string array; empty or omitted ⇒ no skills for that agent.
  - **`contextMode`**: **`full`** \| **`readOnDemand`** for that agent’s skill presentation.
- **Top-level `skills`**: **omitted** — not part of config; packages load from **`~/.chai/skills`** only.

### Tooling and Executor

- Build **per-agent** **`ToolDefinition`** lists from enabled skills for **that** agent only.
- **`read_skill`** resolves against the **same** agent’s enabled set and packages under **`~/.chai/skills`**.
- **`delegate_task`** remains on the **orchestrator** tool list only; worker lists **omit** it.

### Gateway Status and Desktop

**`status.agents.entries`** lists orchestrator + workers; each **`systemContext`** matches what **`WorkerDelegateRuntime`** / orchestrator state would send on a turn. The desktop **Context** screen selects an agent when multiple entries exist; worker rows omit the orchestrator **`## Workers`** block.

## Requirements

- [x] **Directory layout** — **`<profileRoot>/agents/<agentId>/`**; **`chai init`** creates **`agents/orchestrator/AGENTS.md`** for each default profile.
- [x] **Orchestrator `AGENTS.md` only under agent dirs** — Gateway reads **`agents/<orchestratorId>/AGENTS.md`**; **`workspace/AGENTS.md`** is not loaded; **`chai init`** does not create it.
- [x] **Per-agent context directory** — Fixed **`<profileRoot>/agents/<id>/`**; **`ChaiPaths.profile_dir`** as **`profileRoot`**.
- [x] **Per-agent skill configuration** — **`skillsEnabled`** and **`contextMode`** on agent entries; top-level **`skills.enabled`** / **`skills.contextMode`** removed from schema.
- [x] **Skill discovery paths** — **`~/.chai/skills`** only; no config overrides.
- [x] **Static context** — Per-agent static preamble at startup (orchestrator + per-worker bundles); per-turn header lines (**`TODAYS_DATE=`**, **`WORKERS_ENABLED=`** / **`SKILLS_ENABLED=`**) unchanged in behavior (see **[spec/CONTEXT.md](../spec/CONTEXT.md)**).
- [x] **Worker prompt** — Worker bundles exclude **`## Workers`** / orchestrator identity; **`build_workers_context`** is orchestrator-only.
- [x] **Tools** — Per-agent skill tools and executor; orchestrator merges **`delegate_task`**; workers do not.
- [x] **Delegation path** — **`DelegateContext.worker_runtimes`**; **`execute_delegate_task`** uses the worker bundle when **`workerId`** is set.
- [x] **Status API** — **`status.agents.entries`** carries per-agent **`systemContext`** (see **[GATEWAY_STATUS.md](../spec/GATEWAY_STATUS.md)**).
- [x] **Specs** — **[spec/CONTEXT.md](../spec/CONTEXT.md)**, **[spec/ORCHESTRATION.md](../spec/ORCHESTRATION.md)**, **[spec/CONFIGURATION.md](../spec/CONFIGURATION.md)** aligned; **[spec/SKILL_FORMAT.md](../spec/SKILL_FORMAT.md)** and **[spec/GATEWAY_STATUS.md](../spec/GATEWAY_STATUS.md)** updated for per-agent enablement and status shape notes.

## Technical Reference

| Topic | Code / doc area |
|--------|----------------|
| Delegation worker turn | [`crates/lib/src/orchestration/delegate.rs`](../../crates/lib/src/orchestration/delegate.rs) |
| Worker roster text (orchestrator-only after split) | [`crates/lib/src/orchestration/workers_context.rs`](../../crates/lib/src/orchestration/workers_context.rs) |
| Gateway state, static context | [`crates/lib/src/gateway/server.rs`](../../crates/lib/src/gateway/server.rs) |
| Config parsing; **`orchestrator_context_dir`** / **`worker_context_dir`** (path join in private **`agent_context_dir`**) | [`crates/lib/src/config.rs`](../../crates/lib/src/config.rs) |
| Agent turn / tools | [`crates/lib/src/agent.rs`](../../crates/lib/src/agent.rs) |
| Profile roots | [`crates/lib/src/profile.rs`](../../crates/lib/src/profile.rs) |
| **`AGENTS.md` loader** | [`crates/lib/src/agent_ctx.rs`](../../crates/lib/src/agent_ctx.rs) |
| Init scaffolding | [`crates/lib/src/init.rs`](../../crates/lib/src/init.rs) |
| Desktop (orchestrator summary) | [`crates/desktop/src/app/screens/config.rs`](../../crates/desktop/src/app/screens/config.rs), [`context.rs`](../../crates/desktop/src/app/screens/context.rs), [`skills.rs`](../../crates/desktop/src/app/screens/skills.rs) |

## Phases

| Phase | Focus | Status |
|-------|--------|--------|
| **1** | **Layout + init** — **`agents/<id>/`** under each profile; init scaffolds **`agents/orchestrator/AGENTS.md`**; orchestrator loads from **`agents/<orchId>/AGENTS.md`**; no **`workspace/AGENTS.md`**. | Done |
| **2** | **Prompt split** — Orchestrator static text = orchestrator **`AGENTS.md`** dir + **`## Workers`** roster + orchestrator skills; worker static text = worker **`AGENTS.md`** + worker skills + **no** **`delegate_task`** / orchestrator copy. | Done |
| **3** | **Per-agent skills** — Config: per-agent **`skillsEnabled`** + **`contextMode`**; **no** top-level **`skills`** block; per-agent tool lists + **`read_skill`**; **`execute_delegate_task`** uses worker bundle. | Done |
| **4** | **Cleanup + docs** — Dead global skill paths removed; **README** and **`status.agents.entries`** documented; **internal specs** aligned (**`spec/CONTEXT.md`**, **`ORCHESTRATION.md`**, **`CONFIGURATION.md`**, **`SKILL_FORMAT.md`**, **`GATEWAY_STATUS.md`**). | Done |

## Follow-ups (Non-Blocking)

*Completed during this epic; nothing open here.*

- [x] **Desktop Context — per worker** — **`status.agents.entries`**, desktop agent selector on **Context**, worker preview from gateway (see **[GATEWAY_STATUS.md](../spec/GATEWAY_STATUS.md)**).
- [x] **Structured `status` / gateway** — Per-agent rows live under **`agents.entries`** (see **GATEWAY_STATUS**).

## Related Epics and Docs

**Implementation order** (with **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)**): runtime profiles **(complete)** → **this epic** → **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)** (lockfiles / pins).

| Topic | Where |
|--------|--------|
| Delegation **`delegate_task`**, delegation policy | [ORCHESTRATION.md](ORCHESTRATION.md) |
| Runtime profiles (**`profileRoot`**) | [RUNTIME_PROFILES.md](RUNTIME_PROFILES.md) |
| Skill pins (future) | [SKILL_PACKAGES.md](SKILL_PACKAGES.md) |
| Context assembly | [spec/CONTEXT.md](../spec/CONTEXT.md) |
| Worker turn behavior | [spec/ORCHESTRATION.md](../spec/ORCHESTRATION.md) |
| Skill format | [spec/SKILL_FORMAT.md](../spec/SKILL_FORMAT.md) |
