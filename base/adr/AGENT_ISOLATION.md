---
status: accepted
---

# Agent Isolation

Per-agent context directories, per-agent skill configuration, and separate system context for orchestrator and worker agents.

## Context

Before agent isolation, the gateway built one static system context and one skill tool set at startup, shared by the orchestrator and all workers. The orchestrator's system text (including identity and role descriptions) was reused on worker turns — workers received copy describing the orchestrator's role. Skill enablement was global: a single `skills.enabled` list and one `skills.contextMode` applied to every agent. There was no first-class on-disk location for per-agent instructions; the only context file was `workspace/AGENTS.md` shared by all agents.

This meant workers could not have their own instructions, could not be given a subset of skills appropriate to their model size or task, and could not avoid receiving orchestrator-specific context that was irrelevant or confusing for a delegated subtask.

## Decision

Each logical agent (orchestrator and each worker) has its **own agent context directory** under `<profileRoot>/agents/<agentId>/`, with `AGENT.md` as the on-disk context file:

- **Per-agent context directories.** The gateway reads `AGENT.md` from `<profileRoot>/agents/<agentId>/` for each agent. The file `workspace/AGENTS.md` is not read by the gateway for any agent. `chai init` creates `agents/orchestrator/AGENT.md` for each default profile and does not create `workspace/AGENTS.md`.
- **Per-agent skill configuration.** Each agent entry in `config.json` declares its own `enabledSkills` (array of skill package names) and `contextMode` (`full` | `readOnDemand`). There is no top-level `skills` object in `config.json`. Missing or empty `enabledSkills` on an agent means no skill tools and no skill context for that agent.
- **Separate system context.** The orchestrator's system text includes `AGENT.md`, the worker roster (`## Workers`), and orchestrator skills. Each worker's system text includes only that worker's `AGENT.md` and worker-specific skills — no orchestrator identity, no `delegate_task`, no worker roster.
- **Per-agent tool lists.** Tool lists are built from each agent's enabled skills. The orchestrator list merges `delegate_task` when workers exist. Worker lists omit `delegate_task` (nested delegation disabled).
- **Skill discovery is shared.** Packages load from `~/.chai/skills` only. Per-agent `enabledSkills` selects subsets.

## Alternatives Considered

| Alternative | Why not |
|-------------|---------|
| **Global `skills.enabled` and `skills.contextMode`** (prior state) | Cannot give a small worker model only the skills it needs. All agents receive the same tool set and skill context regardless of their role or capabilities. |
| **`workspace/AGENTS.md` as orchestrator context** (prior state) | Single shared file cannot hold separate instructions for orchestrator and workers. No per-agent location on disk. Dual locations create confusion about which file is authoritative. |
| **Per-entry directory override** (configurable agent context paths) | Adds config surface for no practical benefit. The fixed rule `<profileRoot>/agents/<id>/AGENT.md` is simple, predictable, and aligned with the profile layout. |
| **Workers inheriting orchestrator context** | Workers receive irrelevant orchestrator identity and `delegate_task` instructions. Confuses the model about its role and capabilities. |
| **Top-level `skills` config block with directory overrides** | Concepts that belong per-agent (which skills, how to present them) were globally scoped. Mixing global and per-agent config creates ambiguous precedence. |

## Consequences

- **Agents are fully independent in context and tools.** Each agent's system prompt and tool set are tailored to its role. Workers are never told they are the orchestrator.
- **Explicit skill enablement.** Operators must set `enabledSkills` per agent — there is no implicit inheritance from a global list. Empty means no skills, which is a valid and useful configuration for workers.
- **Skill packages are shared, composition is per-agent.** All agents draw from the same `~/.chai/skills` store. Profiles and agents differ by enablement, not by duplicated package trees.

## References

- [spec/AGENTS.md](../spec/AGENTS.md) — Behavioral contract for the per-agent model.
- [spec/CONTEXT.md](../spec/CONTEXT.md) — System context assembly and skill presentation modes.
- [spec/ORCHESTRATION.md](../spec/ORCHESTRATION.md) — Delegation, worker turn behavior, and tool lists.
- [spec/CONFIGURATION.md](../spec/CONFIGURATION.md) — Agent entries and per-agent fields in `config.json`.
- [adr/RUNTIME_PROFILES.md](RUNTIME_PROFILES.md) — Profile layout (`profileRoot`, `agents/<id>/` under profiles).
