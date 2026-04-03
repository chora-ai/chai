---
status: proposed
---

# Epic: Agent Isolation (Per-Agent Workspace and Skills)

**Summary** — Give each logical agent (orchestrator and each worker) its **own workspace directory** under a **profile-local** **`agents/<agentId>/`** tree (see **Profiles and agent directories** under **Design**), **distinct system context** (no shared orchestrator prompt on workers), and **per-agent skill policy** (**`skillsEnabled`**, **`contextMode`**, and related fields on each agent entry). Remove **global** skill enablement from config in favor of agent-scoped configuration. **Backwards compatibility is not a goal** for this proof-of-concept: prefer a clean schema and implementation over preserving the previous top-level **`skills`** shape.

**Status** — **Proposed.** Implements the isolation phases that follow **[ORCHESTRATION.md](ORCHESTRATION.md)** delegation work; see **Relationship to Orchestration** below.

## Problem Statement

Today the gateway builds **one** static system context and **one** skill tool set at startup: shared **`AGENTS.md`**, a single **`skills.enabled`** list, and one **`skills.contextMode`**. **Worker** turns reuse that same preamble—including copy that describes the **orchestrator** role—and the same tools, minus **`delegate_task`**. There is no first-class place on disk for per-agent instructions, and no way to give a small worker model only the skills it needs without giving it the full set.

## Goal

- Each **agent id** has a **default workspace root** at **`<profileRoot>/agents/<agentId>/`**, where **`profileRoot`** is the active runtime profile directory when profiles are used (see **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)**), or **`~/.chai`** for a single implicit layout (predictable layout including **`AGENTS.md`**).
- Each agent entry in the top-level **`agents`** array declares **its own** skill policy (**which** skills are enabled, **how** they appear in context, etc.).
- **Orchestrator** and **worker** turns each receive **correct role-specific** system text (workers are not told they are the orchestrator).
- **No** accumulating legacy config: dropping the old top-level **`skills`** block for enablement/context mode is acceptable; update **`spec/CONTEXT.md`**, **`spec/ORCHESTRATION.md`**, and user-facing docs when implementation lands.

## Current State

- Static context is built in **`gateway/server.rs`** into **`GatewayState.system_context_static`**; workers receive that full string in **`DelegateContext`** ([`delegate.rs`](../../crates/lib/src/orchestration/delegate.rs)).
- **`build_workers_context`** injects orchestrator-only instructions into the **same** string every agent sees ([`workers_context.rs`](../../crates/lib/src/orchestration/workers_context.rs)).
- Skill loading and tools are driven by top-level **`skills`** in config ([`config.rs`](../../crates/lib/src/config.rs), gateway startup).
- **`ORCHESTRATION.md`** treats shared worker/orchestrator context as **shipped** behavior; this epic **supersedes** that assumption for context and skills (delegation mechanics remain).

## Relationship to Orchestration

**[ORCHESTRATION.md](ORCHESTRATION.md)** delivered **`delegate_task`**, provider dispatch, policy, events, and the **`agents`** array with **`role: orchestrator` \| `worker`**. This epic **does not** redo that work; it **extends** the same config entry shape with workspace and skill fields, and changes **how** the gateway composes context and tools **per role**. When this epic is **in progress**, treat orchestration as **continued** for isolation concerns; the orchestration spec should be updated so **`spec/ORCHESTRATION.md`** “Worker Turn Behavior” matches the new design.

## Scope

### In Scope

- Filesystem layout: **`<profileRoot>/agents/<agentId>/`** as the **default** workspace for each agent ( **`profileRoot`** = active profile subtree when **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** is in effect; otherwise **`~/.chai`** ); **`chai init`** scaffolds **`agents/<defaultOrchestratorId>/`** under that root with a starter **`AGENTS.md`**.
- **Optional** per-agent **`workspace`** override (absolute or root-relative path). When omitted, resolve to **`<profileRoot>/agents/<id>/`**.
- **Per-agent** fields (names illustrative; finalize in implementation): e.g. **`skillsEnabled`**, **`skillContextMode`** (or reuse existing enum names: **`full`** \| **`readOnDemand`**).
- **Remove** top-level skill **policy** (**enablement**, **context mode**) from config; **skill package discovery paths** may remain in a **minimal** top-level key (e.g. **`skillPaths`** with **`directory`** / **`extraDirs`**) that does **not** duplicate per-agent policy, or default to **`~/.chai/skills`** in code with optional extra dirs—see **Design**.
- Gateway: build **per-agent** static context and **per-agent** tool lists (and executor scope) at startup; **`execute_delegate_task`** selects the **worker** agent’s bundle by **`workerId`**.
- Prompt split: orchestrator system text includes **delegation** + worker roster; worker system text is **worker-specific** (identity, allowed tools, optional short roster if needed) and **excludes** nested **`delegate_task`**.
- Update internal specs listed under **Related Epics and Docs** when behavior changes.

### Out of Scope

- **OS-level** sandboxing (containers, VMs); see orchestration epic **Scope**.
- **Hot reload** of per-agent context or skill lists without gateway restart (restart remains the simple contract).
- **Skill package revisions, lockfiles, pins** — **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)**; this epic assumes packages on disk under shared roots, filtered per agent.
- **Implementing named runtime profiles** end-to-end — tracked in **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)**; this epic **defines** default **`agents/<id>/`** paths **relative to the profile root** so the two designs compose without a global **`~/.chai/agents/`** tree when profiles are active.

## Dependencies

- Delegation **primitive** and **`agents`** array (**[ORCHESTRATION.md](ORCHESTRATION.md)**) — already implemented.
- Skill directory layout and **`tools.json`** — **[spec/SKILL_FORMAT.md](../spec/SKILL_FORMAT.md)**, **[spec/TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md)**.

## Design

### Profiles and agent directories

**[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** puts **trust-sensitive** state (config, pairing, **`workspace/`**, secrets) **inside** **`~/.chai/profiles/<name>/`**. Per-agent **`AGENTS.md`** files are **natural-language policy** for that profile’s orchestrator and workers, so they belong **in the same trust boundary**, not under a single global **`~/.chai/agents/`** that every profile would share.

| Layout | Where default **`agents/<agentId>/`** resolves |
|--------|-----------------------------------------------|
| **Runtime profiles active** | **`~/.chai/profiles/<activeProfile>/agents/<agentId>/`** (sibling to **`workspace/`**, **`matrix/`**, etc.). |
| **Single config / no profile indirection** | **`~/.chai/agents/<agentId>/`** ( **`profileRoot` = `~/.chai`** ). |

**Skill packages** stay **shared** at **`~/.chai/skills/`** (plus **`extraDirs`**): same rule as the profiles epic—profiles differ by **composition** (per-agent **`skillsEnabled`** in config), not by copying skill trees into each profile.

**Relationship to legacy `workspace/AGENTS.md`:** The profiles epic already used **`profiles/<name>/workspace/AGENTS.md`** as a single orchestrator scratch context. After agent isolation, **orchestrator** instructions should live under **`agents/<orchestratorId>/AGENTS.md`** (or the optional **`workspace`** override on that entry). Whether **`workspace/AGENTS.md`** remains as an alias, a symlink, or is removed is an implementation detail; the **authoritative** model is **per-agent dirs under the profile**.

### Layout Under `~/.chai`

Illustrative with **named profiles** (exact names may follow **`chai init`** conventions):

```text
~/.chai/
├── profiles/
│   └── assistant/                    # example active profile
│       ├── config.json
│       ├── workspace/              # optional; see note above
│       │   └── AGENTS.md
│       └── agents/
│           ├── orchestrator/       # default id or configured orchestrator id
│           │   └── AGENTS.md
│           └── fast-worker/
│               └── AGENTS.md
├── skills/                         # shared across profiles
│   └── <skill-name>/
└── active -> profiles/assistant/   # when using profiles
```

**Without** profiles, the same structure may appear flatter:

```text
~/.chai/
├── config.json
├── agents/
│   ├── orchestrator/
│   │   └── AGENTS.md
│   └── fast-worker/
│       └── AGENTS.md
└── skills/
    └── <skill-name>/
```

- **Agent workspaces** live under **`<profileRoot>/agents/<agentId>/`**.
- **Skill packages** remain a **shared store** under **`~/.chai/skills/`** (plus optional **extraDirs** from config); **per-agent** config selects **which** packages apply and **how** they are surfaced.

### Config Schema Direction

- **Top-level `agents`**: array of objects; each object includes **`id`**, **`role`**, provider/model defaults, delegation policy fields as today, plus:
  - **`workspace`**: optional path; default **`<profileRoot>/agents/<id>/`** (see **Profiles and agent directories**).
  - **`skillsEnabled`**: list of skill names (required for explicit policy; empty list = no skills for that agent).
  - **`skillContextMode`** (or **`contextMode`**): **`full`** \| **`readOnDemand`** for that agent’s skill inlined vs compact + **`read_skill`** behavior.
- **Remove** the old top-level **`skills.enabled`** and **`skills.contextMode`**.
- **Skill discovery paths**: avoid duplicating “which skills exist on disk” per agent unless needed later. Prefer one small top-level **`skillPaths`** (or env-only defaults) for **`directory`** + **`extraDirs`**, with **no** enablement there; **only** agent entries choose subsets. If the PoC hardcodes **`~/.chai/skills`**, document that and add **`skillPaths`** when a second root is needed.

### Tooling and Executor

- Build **per-agent** **`ToolDefinition`** lists from enabled skills for **that** agent only.
- **`read_skill`** (when used) resolves against the **same** agent’s enabled set and skill roots.
- **`delegate_task`** remains on the **orchestrator** tool list only; worker lists **omit** it (unchanged rule).

### Gateway Status and Desktop

- **`status.systemContext`** (and Desktop **Context**) today assume a **single** orchestrator string; decide whether to expose **orchestrator-only**, **per-agent** map, or **document** that worker context is not shown until UX is defined (**Open Questions**).

## Requirements

- [ ] **Directory layout** — Document and implement **`<profileRoot>/agents/<agentId>/`** as the default workspace (**`profileRoot`** = active profile dir or **`~/.chai`**); **`chai init`** creates the default orchestrator subdirectory and **`AGENTS.md`** stub in the right tree.
- [ ] **Per-agent workspace resolution** — Optional **`workspace`** on each agent entry; default path rule as above; resolver must know **profile root** when **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** is implemented.
- [ ] **Per-agent skill policy** — Each agent entry declares **`skillsEnabled`** and skill **context mode**; remove global enablement/context mode from the old **`skills`** block.
- [ ] **Skill discovery paths** — Define minimal top-level or hardcoded **`~/.chai/skills`** (+ optional **`extraDirs`**) without per-agent duplication in the first slice.
- [ ] **Static context** — Build and cache **per-agent** system preamble (orchestrator vs each worker) at startup; include **date line** per turn as today.
- [ ] **Worker prompt** — Workers **do not** receive orchestrator-only **`## Agents`** copy; worker identity and constraints are explicit.
- [ ] **Tools** — Register **per-agent** skill tools and scoped executor; orchestrator merges **`delegate_task`**; workers do not.
- [ ] **Delegation path** — **`DelegateContext`** (or successor) passes the **resolved worker agent id** bundle into **`run_turn_with_messages_dyn`**.
- [ ] **Specs** — Update **[spec/CONTEXT.md](../spec/CONTEXT.md)** and **[spec/ORCHESTRATION.md](../spec/ORCHESTRATION.md)**; align **[spec/CONFIGURATION.md](../spec/CONFIGURATION.md)** if present.

## Technical Reference

| Topic | Code / doc area |
|--------|------------------|
| Delegation worker turn | [`crates/lib/src/orchestration/delegate.rs`](../../crates/lib/src/orchestration/delegate.rs) |
| Worker roster text (orchestrator-only after split) | [`crates/lib/src/orchestration/workers_context.rs`](../../crates/lib/src/orchestration/workers_context.rs) |
| Gateway state, static context | [`crates/lib/src/gateway/server.rs`](../../crates/lib/src/gateway/server.rs) |
| Config parsing | [`crates/lib/src/config.rs`](../../crates/lib/src/config.rs) |
| Agent turn / tools | [`crates/lib/src/agent.rs`](../../crates/lib/src/agent.rs) |

## Phases

| Phase | Focus | Status |
|-------|--------|--------|
| **1** | **Layout + init** — **`<profileRoot>/agents/<id>/`** convention ( **`profileRoot`** from active profile or **`~/.chai`** ); default orchestrator folder on init; optional **`workspace`** override in schema. Load **`AGENTS.md`** from the **orchestrator** agent dir for the main session turn; align with profile **`workspace/`** story (see **Profiles and agent directories**). | Pending |
| **2** | **Prompt split** — Orchestrator static text = workspace context + **`## Agents`** roster + orchestrator skill block; worker static text = worker workspace **`AGENTS.md`** + worker skill block + **no** delegation/orchestrator identity copy. | Pending |
| **3** | **Per-agent skills** — Config: per-agent **`skillsEnabled`** + **`skillContextMode`**; remove top-level skill policy; build per-agent skill context + tool lists + scoped **`read_skill`** at startup; wire **`execute_delegate_task`** to the correct worker bundle. | Pending |
| **4** | **Cleanup + docs** — Remove dead code paths from the old global skill wiring; update specs and README; gateway **`status`** / Desktop decisions from **Open Questions**. | Pending |

## Open Questions

- **`workspace/AGENTS.md` vs `agents/<orchestratorId>/AGENTS.md`** — Deprecation, migration, or merge strategy when both exist under a profile.
- **`skillPaths` vs hardcoded roots** — Whether the first implementation uses only **`~/.chai/skills`** in code or introduces **`skillPaths`** immediately.
- **`status.systemContext`** — Single orchestrator string vs structured **per-agent** preview for debugging.
- **Empty `skillsEnabled`** — Treat as **no skills** (explicit); avoid implicit “inherit all” to keep behavior predictable for a PoC.

## Implementation order (with related epics)

When implementing **profiles**, **agent isolation**, and **skill packages** together, use this sequence (same as **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)**):

1. **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** — **First.** **`profileRoot`** and active profile so default **`agents/<agentId>/`** resolves under **`~/.chai/profiles/<name>/`**.
2. **This epic** — **Second.** Per-agent workspaces, per-agent skill policy, prompts, and delegation wiring.
3. **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)** — **Third.** Revisions, lockfiles, and pins; depends on profile-aware config and stable per-agent skill naming.

**Note:** A PoC may set **`profileRoot = ~/.chai`** until profiles exist; expect a one-time move of **`agents/`** into the profile tree when **[RUNTIME_PROFILES](RUNTIME_PROFILES.md)** lands.

## Related Epics and Docs

| Topic | Where |
|--------|--------|
| Delegation **`delegate_task`**, policy, phases 1–4 | [ORCHESTRATION.md](ORCHESTRATION.md) |
| Runtime profiles ( **`profileRoot`**, trust boundaries) | [RUNTIME_PROFILES.md](RUNTIME_PROFILES.md) — **`agents/<id>/`** is **inside** each profile; **`skills/`** stays at **`~/.chai`** root. |
| Skill pins and lockfiles (future) | [SKILL_PACKAGES.md](SKILL_PACKAGES.md) |
| Context assembly (update after implementation) | [spec/CONTEXT.md](../spec/CONTEXT.md) |
| Worker turn behavior (update after implementation) | [spec/ORCHESTRATION.md](../spec/ORCHESTRATION.md) |
| Skill format | [spec/SKILL_FORMAT.md](../spec/SKILL_FORMAT.md) |
