# Epic: Orchestrators and Workers

**Summary** — Enable an orchestrator model (or agent) to plan work and delegate subtasks to worker models or agents, so different models can be used per step for capability, privacy, and cost.

**Status** — **Phased delivery for this epic is complete** (implementation phases **1–4** below). Delegation, policy, observability, desktop surfacing, merged **`orchestrationCatalog`** on gateway **`status`**, and advanced orchestrator policy (allowlists, session caps, blocked providers, instruction routes) are implemented. **Optional follow-ups** (richer catalog metadata, analytics, CLI parity) and **out-of-scope** items (see **Scope**) are tracked separately—see **Follow-ups** and **Related Work**.

**Delegation spec** — Behavior for **`delegate_task`** (built-in tool), workers, limits, and gateway surfaces is documented in **[spec/ORCHESTRATION.md](spec/ORCHESTRATION.md)**. This epic file focuses on goals, config evolution, phases, requirements, and closure notes.

## Epic Completion (What Shipped)

| Area | Delivered |
|------|-----------|
| **Delegation** | In-memory worker turns, **`delegate_task`**, provider dispatch, nested delegation disabled on workers. |
| **Config** | **`agents`** array (Option A), **`delegateAllowedModels`**, **`delegationInstructionRoutes`**, **`delegateBlockedProviders`**, **`maxDelegationsPerTurn`** / **Session** / **PerProvider**, session delegation counters on [`Session`](../crates/lib/src/session.rs). |
| **Protocol** | WebSocket **`orchestration.delegate.*`** events; **`status.orchestrationCatalog`**; **`DelegateContext`** with **`DelegateObservability`** and session wiring for policy. |
| **Clients** | Chai Desktop: chat timeline for delegation events; **Status** collapsible orchestration catalog. |

---

## Configuration Model (Today)

Before orchestration, config already separates **integration plumbing** from **agent policy**:

| Top-level key | Role |
|---------------|------|
| **`gateway`** | Bind, port, auth. |
| **`channels`** | Message delivery integrations (e.g. Telegram tokens, webhooks). |
| **`providers`** | Model API connection settings: base URLs and API keys per backend (`providers.ollama`, `providers.lms`, `providers.vllm`, `providers.nim`, `providers.openai`, `providers.hf`; see [README.md](../README.md)). Same “tier” as **`channels`**: how we connect to external systems. |
| **`agents`** | **JSON array** of entries with **`id`**, **`role`** (`orchestrator` \| `worker`), and per-entry defaults (`defaultProvider`, `defaultModel`, …). Exactly one orchestrator; **`agents`** may be omitted for a built-in default orchestrator (id `orchestrator`). See **Likely Evolution** (Option A). |
| **`skills`** | Skill roots, enabled list, context mode. |

**Rationale:** **`agents`** holds *which* provider and model the agent run uses and which provider catalogs to poll for discovery—behavior and routing. **`providers`** holds *how to reach* each stack (credentials, URLs). This split stays valid when **`agents`** becomes a list or splits into orchestrator vs workers.

---

## Goal

Move from a single default model for the entire agent loop to an **orchestrator–worker** design: an orchestrator holds the conversation and context, decides which steps to take, and can **delegate** specific subtasks to worker models or agents. Workers handle narrow, well-defined steps (e.g. a single tool call or classification); the orchestrator chooses which provider and model to use per step. This enables smaller/faster models as workers, privacy-aware routing (sensitive steps only to local/self-hosted), and cost/latency optimization.

---

## Current State

- The gateway runs the **orchestrator** turn with **`agents.defaultProvider`** / **`agents.defaultModel`** (or array form: the single **`role: orchestrator`** entry). **`delegate_task`** can delegate to **workers** (preset **`workerId`** or **`provider`** / **`model`** overrides), subject to **allowlists**, **blocked providers**, **instruction routes**, and **per-turn / per-session / per-provider** caps where configured.
- Config lists **`agents`** as an **array** (or omits it); worker entries merge into **`agents.workers`** for **`delegate_task`** resolution.
- Per-step delegation can target different providers/models; the main session model is still one orchestrator unless future multi-orchestrator selection is added.
- The gateway broadcasts **structured WebSocket events** for delegation lifecycle (`orchestration.delegate.start` / `.complete` / `.error` / `.rejected`) on the same channel as other `type: "event"` frames; see [`delegate.rs`](../crates/lib/src/orchestration/delegate.rs). **Chai Desktop** shows these as in-timeline rows in Chat ([`state/chat.rs`](../crates/desktop/src/app/state/chat.rs) listener + [`screens/chat.rs`](../crates/desktop/src/app/screens/chat.rs) UI). Other clients may still ignore unknown events.
- Gateway **`status`** exposes **`orchestrationCatalog`** (merged discovery + allowlist-only rows); see [`catalog.rs`](../crates/lib/src/orchestration/catalog.rs) and **Chai Desktop** **Status**.
- Connection settings live under top-level **`providers`**; see **Configuration Model** above.

---

## Likely Evolution of `agents` Config

The implemented shape is **Option A** (below). Alternatives were considered for documentation only.

### Option A — Single array (implemented)

```json
"agents": [
  {
    "id": "main",
    "role": "orchestrator",
    "defaultProvider": "ollama",
    "defaultModel": "llama3.2:latest",
    "enabledProviders": ["ollama", "lms"]
  },
  {
    "id": "tools",
    "role": "worker",
    "defaultProvider": "lms",
    "defaultModel": "ibm/granite-4-micro"
  }
]
```

- **Pros:** One schema, easy to add roles; discovery lists can be per entry or inherited.
- **Cons:** conventions needed (e.g. exactly one orchestrator, or mark primary by `role`).
- **Implemented:** The gateway loads this shape only (no alternate JSON object form). Worker entries populate **`agents.workers`** after parse. **`orchestratorId`** reflects the orchestrator entry’s **`id`** (default **`orchestrator`** when **`agents`** is omitted).

### Option B — Explicit orchestrator + workers

```json
"orchestrator": { "defaultProvider": "ollama", "defaultModel": "..." },
"workers": [
  { "id": "fast", "defaultProvider": "lms", "defaultModel": "..." }
]
```

- **Pros:** Very clear for users and validation.
- **Cons:** Two top-level shapes; harder to extend if more roles appear (e.g. reviewer).

### Option C — Orchestrator object + `agents` as workers only

Naming asymmetry: **`orchestrator`** is separate; **`agents`** lists only worker definitions. Documented as: the primary session agent is the orchestrator; listed **`agents`** are additional definitions used for delegation.

**Shared fields (expected across options):** per logical agent, the same concepts as today: **`defaultProvider`**, **`defaultModel`**, **`enabledProviders`** (and workspace / limits as needed). Top-level **`providers`** remains shared connection config unless a future design scopes credentials per agent (out of scope for the first orchestration slice).

---

## Implementation Phases

| Phase | Focus | Status |
|-------|--------|--------|
| **1** | **Delegation primitive (in-memory worker turn)** — [`run_turn_with_messages`](../crates/lib/src/agent.rs) in `crate::agent`, re-exported from [`crate::orchestration`](../crates/lib/src/orchestration/mod.rs). Same tool loop as session-backed `run_turn`; no `SessionStore` read/write. Caller supplies `ChatMessage` list + `Provider` + model id. | Done |
| **2** | **Registry / dispatch** — `ProviderChoice`, `resolve_provider_choice`, `ProviderClients::as_dyn` ([dispatch.rs](../crates/lib/src/orchestration/dispatch.rs)), `resolve_model` ([model.rs](../crates/lib/src/orchestration/model.rs)) in [orchestration/mod.rs](../crates/lib/src/orchestration/mod.rs); gateway uses `run_turn_dyn` ([agent.rs](../crates/lib/src/agent.rs)) with `as_dyn(provider_choice)` instead of matching on each provider inline. Optional **(provider, model)** allowlists are implemented as **`delegateAllowedModels`** (policy), not as a separate discovery API. | Done |
| **3** | **Orchestrator loop** — Built-in tool **`delegate_task`** ([delegate.rs](../crates/lib/src/orchestration/delegate.rs)): main turn receives merged tools; orchestrator may delegate with optional `workerId` (defaults/allowlist come from worker entries in `agents`) and/or `provider`, optional `model`, and `instruction`. Worker runs [`run_turn_with_messages_dyn`](../crates/lib/src/agent.rs) with the same **system context** and skill tools as the session (`delegate_task` stripped); nested `delegate_task` is disabled. Tool result JSON includes worker `reply`, `toolCalls`, `toolResults`, and resolved provider/model; **`AgentTurnResult`** merges worker tool activity into the orchestrator turn. **`maxDelegationsPerTurn`** caps delegate calls. Inbound channel and WebSocket **`agent`** both wire [`DelegateContext`](../crates/lib/src/orchestration/delegate.rs). Logs distinguish delegate targets (`workerId` when set). | Done |
| **4** | **Config & protocol** — Multiple agent definitions (`agents` array); WebSocket hooks for delegation; structured observability on the gateway event stream; desktop UI surfacing. | Done — Option A **`agents` array** + validation ([`config.rs`](../crates/lib/src/config.rs)); inbound + WebSocket **`agent`** pass [`DelegateObservability`](../crates/lib/src/orchestration/delegate.rs) and session-aware [`DelegateContext`](../crates/lib/src/orchestration/delegate.rs); desktop shows `orchestration.delegate.*` in the chat timeline; **`status.orchestrationCatalog`**; advanced policy in [`policy.rs`](../crates/lib/src/orchestration/policy.rs). |

---

## Follow-ups

Optional backlog (does not block treating this epic as **complete**):

| Item | Notes |
|------|--------|
| **Richer catalog metadata** | Per-row fields beyond `local` / `toolCapable` (e.g. max context, tags); may require provider APIs or manual config; optional export. |
| **Richer analytics / export** | Persist or forward structured delegation events (logs, metrics, tracing) beyond live WebSocket. |
| **CLI or other UIs** | Surface delegation events and/or **`orchestrationCatalog`** like Chai Desktop. |
| **Multi-turn delegation planning** | Smarter orchestration across many steps (policy / model planning); distinct from single-turn **`delegate_task`**. |

**Done (was a follow-up):** — **Delegation events in the desktop UI** — implemented via the shared WebSocket event stream ([`state/chat.rs`](../crates/desktop/src/app/state/chat.rs) parses `orchestration.delegate.*` after `session.message`); [`screens/chat.rs`](../crates/desktop/src/app/screens/chat.rs) renders compact, color-keyed lines (start / complete / error / rejected).

## Related Work (Outside This Epic)

| Topic | Where to track |
|--------|----------------|
| **New model providers** and wire-protocol work | [EPIC_API_ALIGNMENT.md](EPIC_API_ALIGNMENT.md) and [EPIC_API_ALIGNMENT_PHASE_2.md](EPIC_API_ALIGNMENT_PHASE_2.md) |
| **Human-in-the-loop approval**, **sandboxing**, **exec approval** for delegation or tools | Future dedicated epic (not part of phased closure here); config today supports **deny** via **`delegateBlockedProviders`** and caps. |

---

## Scope

- **In scope:** Delegation primitive; orchestrator–worker loop with **`delegate_task`**; dispatch and config-driven **policy** (allowlists, caps, blocks, instruction routes); **merged catalog** on **`status`**; observability (logs + WebSocket events + desktop); worker invocation via gateway paths.
- **Out of scope (by design):** Adding **new provider backends** as product work—see [EPIC_API_ALIGNMENT.md](EPIC_API_ALIGNMENT.md). **Full sandboxing**, **arbitrary code exec approval**, and **interactive human approval queues** for delegation (pause/resume, pending state, multi-user review)—these belong in a **separate security/UX epic** if prioritized; **`delegateBlockedProviders`** covers “never delegate to this provider” without an approval workflow.

---

## Requirements

- [x] **Delegation allowlist (policy slice)** — Orchestrator and per-worker **`delegateAllowedModels`** in **`config.json`**: explicit allowed `(provider, model)` pairs for **`delegate_task`** after resolution, with optional **`local`** / **`toolCapable`** hints; enforced in [`policy.rs`](../crates/lib/src/orchestration/policy.rs) from [`resolve_delegate_target`](../crates/lib/src/orchestration/delegate.rs). Empty or omitted lists allow only the effective default `(provider, model)` for that scope (orchestrator vs worker), in addition to **`enabledProviders`** and other policy.
- [x] **Multi-provider and model registry (extended)** — Gateway **`status`** includes **`orchestrationCatalog`**: merged `(provider, model)` rows from per-provider discovery plus allowlist-only pairs, with optional **`local`** / **`toolCapable`** hints when the pair matches an allowlist entry ([`catalog.rs`](../crates/lib/src/orchestration/catalog.rs)). Chai Desktop **Status** shows a collapsible list. Richer metadata (e.g. max context) remains a follow-up.
- [x] **Delegation primitive** — [`run_turn_with_messages`](../crates/lib/src/agent.rs): run a subtask with explicit messages, a `Provider`, and model id; returns `AgentTurnResult` (no session persistence). Orchestrator will supply messages and merge results.
- [x] **Orchestrator loop and tool semantics (phase 3 slice)** — `delegate_task` invokes a worker turn with chosen provider/model and instruction; worker shares orchestrator system context and skill tools; nested `delegate_task` disabled; structured JSON result merged into the orchestrator turn. Optional **multi-turn planning** is a **Follow-up**, not required for epic closure.
- [x] **Policy and config (allowlist slice)** — **`delegateAllowedModels`** constrains resolved `(provider, model)` for **`delegate_task`**; works with **`maxDelegationsPerTurn`** and **`enabledProviders`**.
- [x] **Policy and config (advanced)** — Orchestrator-only: **`delegationInstructionRoutes`** (prefix → optional **`workerId`** / **`provider`** / **`model`**), **`maxDelegationsPerSession`**, **`maxDelegationsPerProvider`** (per canonical provider id), **`delegateBlockedProviders`**. Session counters live on [`Session`](../crates/lib/src/session.rs); enforced in [`policy.rs`](../crates/lib/src/orchestration/policy.rs) / [`execute_delegate_task`](../crates/lib/src/orchestration/delegate.rs) with **`DelegateContext`** session fields (**`sessionStore`**, **`sessionId`**). **`orchestration.delegate.rejected`** reasons include **`max_delegations_per_session`** / **`max_delegations_per_provider`**. Interactive approval queues / human-in-the-loop remain out of scope (see **Scope**).
- [x] **Worker invocation path (gateway)** — Channel inbound and WebSocket **`agent`** runs use the same `run_turn_dyn` path with **`delegate_task`** and [`DelegateContext`](../crates/lib/src/orchestration/delegate.rs). A separate WebSocket-only RPC for workers alone is not required for phase 3.
- [x] **Observability (phase 3 slice)** — `log::info` / `log::warn` on delegate target (provider, model, optional `workerId`), delegation failures, and max-delegation rejection.
- [x] **Observability (gateway events)** — Structured WebSocket broadcast events for delegation (`orchestration.delegate.start`, `.complete`, `.error`, `.rejected`) with `sessionId` and resolved provider/model where applicable; see [`delegate.rs`](../crates/lib/src/orchestration/delegate.rs).
- [x] **Observability in the desktop UI** — Delegation WebSocket events appear as in-chat timeline rows in Chai Desktop ([`state/chat.rs`](../crates/desktop/src/app/state/chat.rs), [`screens/chat.rs`](../crates/desktop/src/app/screens/chat.rs)).

---

## Technical Reference

### Definitions

- **Worker** — A model (or agent) given a **narrow, well-defined subtask** (e.g. “call this tool with these arguments,” “answer this classification question,” “summarize this text”). A worker does not own the full conversation, plan multi-step flows, or choose which skills to use.
- **Orchestrator** — A model (or agent) that **plans the work**, chooses which steps to take, and **delegates** specific steps to workers. The orchestrator holds the conversation and context; it decides “for this step I’ll use the local 3B model” or “for this step I need the 70B model” or “this step stays on a privacy-safe worker.”

### Why the Distinction Matters

- **Model size and capability** — Smaller models can be good **workers**: fast, low resource, reliable at single-step tool calls or simple reasoning. Larger models are better at planning across many turns.
- **Privacy and routing** — An orchestrator can route by sensitivity: keep sensitive data on local or self-hosted workers.
- **Cost and latency** — Use a small, fast model for many simple steps and a larger (or remote) model only when needed.

### Current vs Future

| Aspect | Current implementation | Future (optional) |
|--------|------------------------|---------------------|
| **Who runs the agent loop** | Single orchestrator from `agents` (default id `orchestrator` when omitted) | Multi-orchestrator selection, if needed |
| **Who runs tools / subtasks** | Orchestrator; worker turns via **`delegate_task`** (different provider/model per delegation) | Same; possible tighter integration with external planners |
| **Model choice** | Defaults + overrides; **`delegateAllowedModels`**; **`delegationInstructionRoutes`**; **`orchestrationCatalog`** on **`status`** | Richer per-model metadata (e.g. max context); automatic routing policies beyond config |
| **Privacy / routing** | **`delegateAllowedModels`**, **`delegateBlockedProviders`**, caps, **`local`** / **`toolCapable`** hints | Human-in-the-loop approval, sandboxed execution—see **Related Work (Outside This Epic)** |

### Implementation Notes

- The existing `run_turn` and **`Provider`** trait are a starting point; a worker invocation might be a constrained `run_turn` (e.g. single tool-call step, no channel delivery, result only).
- Today `run_turn` is tied to a single default provider from config; a worker call would need to select provider and model by id from the registry.
- Top-level **`providers`** remains the right place for shared URLs/keys; per-agent **`defaultProvider`** / **`defaultModel`** (under the future **`agents`** shape) selects which **`Provider`** implementation and model id to use for that agent’s turns.
