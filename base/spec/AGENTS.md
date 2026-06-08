---
status: stable
---

# Agents

This document specifies how **logical agents** (orchestrator and workers) are configured, how their context directories and skill lists work, and how the gateway builds per-agent system text and tool lists. For the architectural decision, see [adr/AGENT_ISOLATION.md](../adr/AGENT_ISOLATION.md). For delegation semantics and worker turn behavior, see [ORCHESTRATION.md](ORCHESTRATION.md).

## Purpose

Each logical agent has its own identity, context directory, and skill configuration. The gateway builds separate system context strings and separate tool lists for each agent at startup. This spec describes the per-agent model: how agents are defined, how their resources are organized on disk, and how the gateway uses that configuration.

## Agent Entries

Agents are defined in `config.json` as an `agents` array. Each entry is an object with an `id`, a `role`, and additional fields.

### Orchestrator

Exactly one entry with `"role": "orchestrator"`. The orchestrator runs the main session turn.

| Field | Purpose |
|-------|---------|
| `id` | Agent identifier (e.g., `"orchestrator"`) |
| `role` | Must be `"orchestrator"` |
| `defaultProvider`, `defaultModel` | Main session defaults |
| `enabledProviders` | Which provider stacks this agent may use (discovery and routing scope) |
| `skillsEnabled` | Array of skill package names to load for this agent from `~/.chai/skills/`. Missing or empty ⇒ no skills. |
| `contextMode` | `full` \| `readOnDemand` — how this agent's skill text appears in system context |
| `maxSessionMessages` | When set and > 0, only the last N messages are sent per turn |
| `maxToolLoopIterations` | Maximum LLM round-trips per turn (default 100). Safety net against runaway loops. Applies to both orchestrator and worker turns. When reached on the orchestrator turn, the gateway emits a `session.tool_loop_limit` event (see [ORCHESTRATION.md](ORCHESTRATION.md)). |
| `maxDelegationsPerTurn`, `maxDelegationsPerSession`, `maxDelegationsPerProvider` | Delegation caps |

### Workers

Zero or more entries with `"role": "worker"`. Each has an `id` used as `workerId` when delegating.

| Field | Purpose |
|-------|---------|
| `id` | Worker identifier — used as `workerId` in `delegate_task` |
| `role` | Must be `"worker"` |
| `defaultProvider`, `defaultModel` | Worker's single `(provider, model)` pair. Falls back to orchestrator defaults when omitted. |
| `skillsEnabled` | Skill package names for this worker only. Missing or empty ⇒ no skills. |
| `contextMode` | `full` \| `readOnDemand` for this worker's skill presentation |

A worker's `defaultProvider` must be enabled at the orchestrator level via `enabledProviders`. Workers do not have their own `enabledProviders` field; the worker's single provider is its `defaultProvider`, which must already be an enabled provider at the orchestrator level.

## Agent Context Directories

Each agent has its own on-disk context directory under the active profile:

```
<profileRoot>/agents/<agentId>/AGENT.md
```

- The gateway reads `AGENT.md` for each agent at startup and includes its contents in that agent's system context string.
- `chai init` creates `agents/orchestrator/AGENT.md` for each default profile.
- When workers are defined in `config.json`, operators add `agents/<workerId>/AGENT.md` with worker-specific instructions.

## Per-Agent Skill Configuration

Skill enablement and presentation mode are configured **per agent** in `config.json`:

| Field | Behavior |
|-------|----------|
| `skillsEnabled` | Array of skill package names (e.g., `["files", "git-read"]`). The agent receives tools and context only for listed packages. Missing or empty ⇒ no skill tools and no skill-derived context for that agent. |
| `contextMode` | `full` — each enabled skill's full `SKILL.md` body (frontmatter stripped) is inlined under `## Skills` in the system context. `readOnDemand` — a compact skill list is inlined, and a `read_skill` tool is offered so the model can load full `SKILL.md` content on demand. |

There is **no** top-level `skills` object in `config.json`. Skill packages are discovered from `~/.chai/skills/` only (no configurable discovery paths). Per-agent `skillsEnabled` selects which discovered packages apply to each agent.

## Per-Agent System Context

The gateway builds **separate** static system context strings for each agent at startup. The static string is cached and sent unchanged on every turn for that agent.

### Orchestrator Build Order

1. **Agent context** — Contents of `<profileRoot>/agents/<orchestratorId>/AGENT.md`. Trimmed. Omitted if missing or empty.
2. **Workers roster** — If any workers are defined, a `## Workers` section is rendered by `build_workers_context` (see [CONTEXT.md](CONTEXT.md)). Omitted when there are no workers.
3. **Skills** — From enabled packages for the orchestrator: either `full` (inlined bodies) or `readOnDemand` (compact list + `read_skill` tool).

### Worker Build Order

1. **Agent context** — That worker's `<profileRoot>/agents/<workerId>/AGENT.md`. Trimmed.
2. **Skills** — Same builders as the orchestrator, but filtered by that worker's `skillsEnabled` and `contextMode`.

**Workers do not receive:** the `## Workers` roster, orchestrator identity, or `delegate_task` instructions.

### Skill Context Modes

See [CONTEXT.md](CONTEXT.md) for the full build-order details, separator rules, and text format.

## Per-Agent Tool Lists

Tool lists are built **per agent** at startup from that agent's enabled skill packages:

| Tool source | Orchestrator | Worker |
|-------------|-------------|--------|
| Skill tools from `tools.json` | Only packages in orchestrator's `skillsEnabled` | Only packages in worker's `skillsEnabled` |
| `read_skill` | Included when orchestrator's `contextMode` is `readOnDemand` and at least one skill is enabled | Included when worker's `contextMode` is `readOnDemand` and at least one skill is enabled |
| `delegate_task` | Merged at the **front** of the orchestrator tool list when workers exist | **Not offered** (nested delegation disabled) |

The same prebuilt list is sent on every turn for that agent.

## Skill Discovery and the Shared Store

- **Discovery root:** `~/.chai/skills/` only (no config override).
- **Loading:** At gateway startup, `load_skills` discovers all packages containing `SKILL.md`.
- **Filtering:** Each agent's `skillsEnabled` list selects which discovered packages apply to that agent. An agent with an empty `skillsEnabled` receives no skill tools and no skill context.
- **No per-profile skill trees:** Skill packages are not duplicated under profile directories. Profiles differ by per-agent enablement, not by separate package stores.

## Delegation and Worker Runtimes

When the orchestrator calls `delegate_task` with a bracket prefix `[workerId]` in the instruction:

- The gateway matches the bracket prefix, injects `workerId`, strips the prefix from the instruction, and selects the matching `WorkerDelegateRuntime` by `workerId`.
- The worker turn runs on the worker's single `(defaultProvider, defaultModel)` pair.
- The worker turn receives the worker's own system context, tools, and executor (no orchestrator context or tools leak through).
- The worker turn message structure is `[system?, user(instruction)]` — not the main session transcript.

See [ORCHESTRATION.md](ORCHESTRATION.md) for delegation semantics, policy, limits, and event streaming.

## Status API

`status.agents.entries` provides per-agent runtime rows. Each entry includes:

| Field | Meaning |
|-------|---------|
| `id`, `role` | Agent identity |
| `contextDirectory` | Absolute path to `<profile>/agents/<id>/` |
| `defaultProvider`, `defaultModel` | Effective routing defaults |
| `systemContext` | Full system context string for that agent |
| `tools` | Pretty-printed JSON array of that agent's tool definitions |
| `skills` | Per-agent skill runtime (`enabledSkills`, `contextMode`, skill context fields) |

See [GATEWAY_STATUS.md](GATEWAY_STATUS.md) for the full payload specification.

## Related Documents

| Document | Purpose |
|----------|---------|
| [adr/AGENT_ISOLATION.md](../adr/AGENT_ISOLATION.md) | Architectural decision for per-agent isolation |
| [CONTEXT.md](CONTEXT.md) | System context assembly, skill context modes, and build order |
| [ORCHESTRATION.md](ORCHESTRATION.md) | Delegation, worker turn behavior, `delegate_task` |
| [CONFIGURATION.md](CONFIGURATION.md) | `config.json` agent entries and fields |
| [PROFILES.md](PROFILES.md) | Profile directory structure (`agents/<id>/` under profiles) |
| [SKILL_FORMAT.md](SKILL_FORMAT.md) | Skill package layout and `tools.json` |
| [GATEWAY_STATUS.md](GATEWAY_STATUS.md) | `status.agents.entries` payload |
