---
status: accepted
---

# Orchestration Model

Orchestrator–worker delegation with a single `agents` array in `config.json`, where the orchestrator holds the conversation and delegates subtasks to workers with independent providers, models, and skill configuration.

## Context

Before orchestration, the gateway ran a single default model for the entire agent loop with no way to use different models for different steps. There was no mechanism to delegate narrow subtasks to smaller or faster models, route sensitive steps to local or self-hosted workers, or optimize cost and latency by matching model capability to step complexity. A single provider and model handled everything — planning, tool calls, and responses — regardless of whether a smaller model could handle a specific step.

## Decision

Chai uses an **orchestrator–worker** model:

- The **orchestrator** holds the conversation and context, plans work, and can **delegate** specific subtasks to workers via the built-in `delegate_task` tool. The orchestrator decides which provider and model to use per delegation step.
- **Workers** handle narrow, well-defined subtasks. Each worker has its own provider and model defaults, its own skill configuration, and its own system context. Workers do not see the orchestrator's identity, the worker roster, or the `delegate_task` tool (nested delegation is disabled).
- Agent definitions live in a single **`agents` array** in `config.json`. Each entry has an `id`, a `role` (`"orchestrator"` or `"worker"`), and fields for provider/model defaults, skill enablement, and delegation policy. Exactly one orchestrator is required; zero or more workers may be defined.
- **`providers`** (connection plumbing: base URLs, API keys) and **`agents`** (routing: which provider/model per role, which skills, delegation policy) are separate top-level config concerns. `providers` describes how to reach each backend; `agents` describes which backend each role uses.
- Delegation policy is config-driven: allowlists (`delegateAllowedModels`), caps (`maxDelegationsPerTurn`, `maxDelegationsPerSession`, `maxDelegationsPerProvider`), blocked providers, and instruction routing (`delegationInstructionRoutes`). No interactive human approval queue — the policy is enforced mechanically.

## Alternatives Considered

| Alternative | Why not |
|-------------|---------|
| **Single model for everything** (prior state) | Cannot delegate to smaller/faster models. No privacy-aware routing. No cost/latency optimization per step. |
| **Explicit `orchestrator` + `workers` keys** (Option B — `{"orchestrator": {...}, "workers": [...]}`) | Very clear for users and validation, but two top-level shapes make it harder to extend if more roles appear or multiple orchestrators are allowed. The single array (Option A) is more flexible with a small convention cost ("exactly one orchestrator"). |
| **`orchestrator` object + `agents` as workers only** (Option C) | Naming asymmetry: orchestrator is special-cased, agents lists only workers. The unified array treats all roles with the same schema. |
| **Interactive human approval for delegation** | Significantly more complex UX and state management. Deny/caps policy covers the most common case (preventing unwanted delegations) without a pending-approval workflow. Can be added as a separate feature if needed. |
| **Hot-reloadable orchestration config** | The restart requirement for config changes is simple and auditable. Dynamic reconfiguration of delegation policy mid-session introduces consistency issues. |

## Consequences

- **Different models per step.** Smaller models can serve as fast, low-resource workers for single-step tasks. Larger models handle planning and complex reasoning.
- **Privacy-aware routing.** The orchestrator can route sensitive steps to local or self-hosted workers while using remote models for less sensitive work.
- **Config-driven policy.** All delegation constraints are expressed in `config.json` and enforced at runtime. No runtime approval workflow — the policy is the contract.
- **Single array shape.** The `agents` array is the sole config representation for agent definitions. There is no alternate object form. This keeps validation and parsing simple.
- **Workers are isolated.** Each worker receives its own system context and tool list. No orchestrator identity or `delegate_task` leaks into worker turns.

## References

- [spec/ORCHESTRATION.md](../spec/ORCHESTRATION.md) — Behavioral contract for delegation, policy, and events.
- [spec/AGENTS.md](../spec/AGENTS.md) — Per-agent context directories, skill configuration, and tool lists.
- [spec/CONFIGURATION.md](../spec/CONFIGURATION.md) — `config.json` blocks and agent entries.
- [spec/PROVIDERS.md](../spec/PROVIDERS.md) — Provider configuration and discovery.
