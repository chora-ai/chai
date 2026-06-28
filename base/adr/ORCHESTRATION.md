---
status: accepted
---

# Orchestration Model

Orchestrator–worker delegation with a single `agents` array in `config.json`, where the orchestrator holds the conversation and delegates subtasks to workers with independent providers, models, and skill configuration.

## Context

Before orchestration, the gateway ran a single default model for the entire agent loop with no way to use different models for different steps. There was no mechanism to delegate narrow subtasks to smaller or faster models, route sensitive steps to local or self-hosted workers, or optimize cost and latency by matching model capability to step complexity. A single provider and model handled everything — planning, tool calls, and responses — regardless of whether a smaller model could handle a specific step.

## Decision

Chai uses an **orchestrator–worker** model:

- The **orchestrator** holds the conversation and context, plans work, and can **delegate** specific subtasks to workers via the built-in `delegate_task` tool. The orchestrator uses its own `defaultProvider` and `defaultModel` for planning.
- **Workers** handle narrow, well-defined subtasks. Each worker has a single `defaultProvider` / `defaultModel` pair and its own skill configuration. Workers do not see the orchestrator's identity, the worker roster, or the `delegate_task` tool (nested delegation is disabled).
- Agent definitions live in a single **`agents` array** in `config.json`. Each entry has an `id`, a `role` (`"orchestrator"` or `"worker"`), and fields for provider/model defaults and skill enablement. At least one orchestrator is required (multiple supported); zero or more workers may be defined. Each orchestrator can optionally restrict which workers it delegates to via `enabledWorkers`.
- **`providers`** (connection plumbing: base URLs, API keys) and **`agents`** (routing: which provider/model per role, which skills) are separate top-level config concerns. `providers` describes how to reach each backend; `agents` describes which backend each role uses.
- Delegation policy is config-driven: caps (`maxDelegationsPerTurn`, `maxDelegationsPerSession`, `maxDelegationsPerWorker`) and bracket-prefix worker targeting. No interactive human approval queue — the policy is enforced mechanically.

### Workers Use a Single Provider and Model

Each worker has exactly one `(provider, model)` pair — defined by its `defaultProvider` and `defaultModel` (falling back to the orchestrator's defaults when omitted). There is no mechanism to override the provider or model per delegation call; the bracket prefix `[workerId]` is the sole routing mechanism.

When different models are needed for the same kind of work, the operator defines separate workers rather than attaching multiple pairs to one worker. For example, `code-fast` (local model) and `code-powerful` (cloud model) are two workers, not one worker with two pairs.

This design keeps the delegation path simple: the orchestrator picks a worker by its bracket prefix, and that worker always runs on its single configured pair. There are no override parameters, no allowlists to maintain, and no ambiguity about which model will handle a given delegation.

### Provider Access Is Controlled by `enabledProviders` Only

Which providers are available at all — for discovery, for the orchestrator, and for workers — is determined by `agents.enabledProviders` on the orchestrator entry. Workers do not have their own `enabledProviders` field; a worker's provider is its `defaultProvider`, which must already be an enabled provider at the orchestrator level. This avoids contradictory configurations where a provider is enabled globally but blocked or restricted for delegation.

## Alternatives Considered

| Alternative | Why not |
|-------------|---------|
| **Single model for everything** (prior state) | Cannot delegate to smaller/faster models. No privacy-aware routing. No cost/latency optimization per step. |
| **Explicit `orchestrator` + `workers` keys** (Option B — `{"orchestrator": {...}, "workers": [...]}`) | Very clear for users and validation, but two top-level shapes make it harder to extend if more roles appear or multiple orchestrators are allowed. The single array (Option A) is more flexible — the "exactly one orchestrator" constraint has since been relaxed to "at least one" (see [epic/MULTI_ORCHESTRATOR.md](../epic/MULTI_ORCHESTRATOR.md)), confirming Option A's extensibility advantage. |
| **`orchestrator` object + `agents` as workers only** (Option C) | Naming asymmetry: orchestrator is special-cased, agents lists only workers. The unified array treats all roles with the same schema. |
| **Multi-pair workers** (one worker with multiple `(provider, model)` pairs) | Creates two routing mechanisms (bracket prefix for defaults, `provider`/`model` override parameters for alternatives) and requires allowlists (`delegateAllowedModels`) to constrain which pairs a worker may use. Separate workers per model are simpler to configure, reason about, and render in context — and they eliminate ambiguity about which model a delegation will use. |
| **Provider blocklists for delegation** (`delegateBlockedProviders`) | Redundant with `enabledProviders`. If a provider should not be targeted by delegation, the operator omits it from `enabledProviders`. Enabling a provider and then blocking it for delegation is contradictory configuration with no valid use case. |
| **Interactive human approval for delegation** | Significantly more complex UX and state management. Deny/caps policy covers the most common case (preventing unwanted delegations) without a pending-approval workflow. Can be added as a separate feature if needed. |
| **Hot-reloadable orchestration config** | The restart requirement for config changes is simple and auditable. Dynamic reconfiguration of delegation policy mid-session introduces consistency issues. |

## Consequences

- **Different models per step.** Smaller models can serve as fast, low-resource workers for single-step tasks. Larger models handle planning and complex reasoning.
- **Privacy-aware routing.** The orchestrator can route sensitive steps to local or self-hosted workers while using remote models for less sensitive work.
- **Config-driven policy.** All delegation constraints are expressed in `config.json` and enforced at runtime. No runtime approval workflow — the policy is the contract.
- **Single array shape.** The `agents` array is the sole config representation for agent definitions. There is no alternate object form. This keeps validation and parsing simple.
- **Workers are isolated.** Each worker receives its own system context and tool list. No orchestrator identity or `delegate_task` leaks into worker turns.
- **Single routing mechanism.** The bracket prefix `[workerId]` is the only way to target a worker. There are no `provider`/`model` override parameters on `delegate_task`, no allowlists to maintain, and no `(default)` markings in context. The worker's pair is always its `defaultProvider`/`defaultModel`.
- **No contradictory config.** A worker's provider must be enabled at the orchestrator level via `enabledProviders`. There is no separate blocklist or allowlist that could conflict with the enablement decision.

## References

- [spec/ORCHESTRATION.md](../spec/ORCHESTRATION.md) — Behavioral contract for delegation, policy, and events.
- [spec/AGENTS.md](../spec/AGENTS.md) — Per-agent context directories, skill configuration, and tool lists.
- [spec/CONFIGURATION.md](../spec/CONFIGURATION.md) — `config.json` blocks and agent entries.
- [spec/PROVIDERS.md](../spec/PROVIDERS.md) — Provider configuration and discovery.
