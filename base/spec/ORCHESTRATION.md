---
status: stable
---

# Orchestration and Delegation

This document describes how **orchestrator** and **worker** entries in **`config.json`** map to runtime behavior—especially **delegation** via the built-in **`delegate_task`** tool. For the architectural decision, see [adr/ORCHESTRATION.md](../adr/ORCHESTRATION.md).

**Former tool name:** The same built-in tool was previously named **`chai_delegate`**. Older notes or logs may still say **`chai_delegate`**; behavior and config are unchanged aside from the tool name exposed to the model.

## Roles

- **Orchestrator** — At least one entry with **`"role": "orchestrator"`**. Multiple orchestrators are supported; the first is the default. It runs the main session turn (default **`defaultProvider`** / **`defaultModel`** unless overridden per request).
- **Workers** — Zero or more entries with **`"role": "worker"`**. Each has an **`id`** used as **`workerId`** when delegating.

## Configuration Quick Reference

Canonical provider ids used in policy and catalogs: **`ollama`**, **`lms`**, **`nearai`**, **`nim`** (see [README.md](../../README.md), [PROVIDERS.md](PROVIDERS.md), and [MODELS.md](MODELS.md)). Users can configure any other `openai-compat` provider with a custom `id`.

### Orchestrator entry

| Key | Purpose |
|-----|---------|
| **`id`**, **`role`** | Identity; must include at least one **`orchestrator`**. |
| **`defaultProvider`**, **`defaultModel`** | Main session defaults. |
| **`enabledProviders`** | Which provider stacks this agent may use (discovery and routing). |
| **`enabledSkills`** | Skill package names to load for **this** agent from shared discovery roots; missing or empty ⇒ no skills for the orchestrator. |
| **`enabledWorkers`** | Optional array of worker ids this orchestrator can delegate to. Absent or `null` ⇒ all workers; present ⇒ only listed workers. Orchestrator-only — rejected on worker entries at parse time. |
| **`contextMode`** | **`full`** \| **`readOnDemand`** — how orchestrator skill text appears in system context (and whether **`read_skill`** is offered). |
| **`maxToolLoopsPerTurn`** | Maximum tool loops per turn (omitted = no limit). The loop exits naturally when the model returns no tool calls; this is a safety net against runaway loops. Applies to both orchestrator and worker (delegate) turns. When the limit is reached on the orchestrator turn, the gateway emits a **`session.tool_loop_limit`** event with the pending tool calls and includes **`loopLimitReached`** + **`pendingToolCalls`** in the `agent` RPC response, so clients can show the interrupted state. When the turn is stopped by the user, the `agent` RPC response includes **`stopped`**: **`true`** and the gateway emits a **`session.turn_stopped`** event. |
| **`maxDelegationsPerTurn`** | Cap on **`delegate_task`** calls in a single orchestrator turn. |
| **`maxDelegationsPerSession`** | Cap on **successful** delegations per persisted session (requires session id on the gateway path). |
| **`maxDelegationsPerWorker`** | Per-session caps keyed by worker id. |

### Worker entry

| Key | Purpose |
|-----|---------|
| **`id`**, **`role`** | Identity; **`id`** is **`workerId`**. |
| **`defaultProvider`**, **`defaultModel`** | Worker's single `(provider, model)` pair. Falls back to orchestrator defaults when omitted. |
| **`enabledSkills`** | Skill names for **this** worker only; missing or empty ⇒ no skills on worker turns. |
| **`contextMode`** | **`full`** \| **`readOnDemand`** for this worker's skill presentation and tools. |

Orchestrator-only fields (**`enabledProviders`**, **`enabledWorkers`**, **`maxDelegationsPerTurn`**, **`maxDelegationsPerSession`**, **`maxDelegationsPerWorker`**, **`maxToolLoopsPerTurn`**) are rejected at parse time when set on a worker entry. A worker's `defaultProvider` must be enabled at the orchestrator level via **`enabledProviders`**.

## Delegation Tool (`delegate_task`)

The orchestrator may call **`delegate_task`** to run a **subtask** on a worker's provider and model:

| Argument | Role |
|----------|------|
| **`instruction`** | Required (non-empty after trim). User text for the worker turn. Start with **`[workerId]`** to target a specific worker. |

**Worker targeting (bracket-prefix matching)**

- Every worker with a non-empty **`id`** gets an automatic delegation prefix **`[workerId]`**. The system matches **`[`** + worker ID + **`]`** at the start of the **`instruction`** (full bracket form from opening to closing bracket), injects the **`workerId`** internally, and strips the bracketed prefix from the instruction before passing it to the worker.
- When no bracket prefix matches, delegation uses the orchestrator's effective defaults (no worker selected).
- Bracket matching avoids prefix subsumption: workers named **`code`** and **`code-review`** produce prefixes **`[code]`** and **`[code-review]`**, which are unambiguous because the matcher requires the closing bracket.

**Steering semantics:** Bracket prefixes are **steering hints**, not triggers. They only operate when the orchestrator calls **`delegate_task`**; they cannot cause delegation to happen. If the orchestrator answers directly (no **`delegate_task`** call), the prefix has no effect.

## Worker Turn Behavior

- The worker receives **its own** static system string: **that worker's** **`AGENT.md`**, **that worker's** **`enabledSkills`** / **`contextMode`** skill block (no **`## Workers`** roster, no orchestrator identity copy). **`execute_delegate_task`** selects the matching **`WorkerDelegateRuntime`** by **`workerId`** (see **`gateway/server.rs`**).
- **Tool list** — Skill tools (and optional **`read_skill`**) match the worker's enabled set only. **`delegate_task`** is **not** offered (nested delegation disabled).
- **Messages** — The worker turn is **not** the main session transcript: **`execute_delegate_task`** builds **`[system?, user(instruction)]`** only (see **`delegate.rs`**). Delegation limits may still use the parent **`sessionId`** for caps.
- Implementation: **`DelegateContext.worker_runtimes`** and **`crates/lib/src/orchestration/delegate.rs`**.

### Response Flow

After a worker turn completes, the response path is:

1. The worker turn produces a `TurnResult` (text reply, tool calls, tool results).
2. `format_delegate_result` packages these into a summarized JSON string: `{"reply": "...", "worker": {"provider": "...", "model": "..."}}`. The worker's `toolCalls` and `toolResults` are **not** included — the worker's reply text synthesizes its findings. Full tool details flow to the desktop via `session.tool_call` and `session.tool_result` observability events.
3. This JSON becomes the `delegate_task` **tool result** in the orchestrator's message history.
4. The orchestrator model is called again — it sees the tool result and generates its own response.
5. The loop continues until the orchestrator produces no more tool calls. That final text is what the user receives.

The orchestrator **mediates** the worker's response — the user sees the orchestrator's synthesis, not the worker's raw text. However, the `orchestration.delegate.complete` event includes a `reply` field with the worker's text, allowing clients to display the worker's response as a distinct chat line alongside the orchestrator's final reply.

## Gateway Events

While connected to the gateway WebSocket, clients receive **`type`: `event`** frames with an **`event`** string and **`payload`**. Delegation uses:

| Event | Meaning |
|-------|---------|
| **`orchestration.delegate.start`** | Worker turn is about to run; payload includes resolved **`provider`**, **`model`**, optional **`workerId`**, **`sessionId`** when known. |
| **`orchestration.delegate.complete`** | Worker turn finished; payload includes **`provider`**, **`model`**, optional **`workerId`**, **`workerToolCalls`** count, **`workerToolResults`** count. When the worker was **not** stopped, includes **`reply`** (the worker's text response). When the worker was stopped mid-loop, **`reply`** is omitted (the content was already emitted via `session.assistant_progress`) and **`stopped`**: **`true`** is included instead. |
| **`orchestration.delegate.error`** | Resolution failed (e.g. unknown worker, provider not enabled) or the worker turn failed; payload may include **`error`**, optional **`workerId`**. |
| **`orchestration.delegate.rejected`** | Delegation not started due to a **limit**; payload includes **`reason`** (see below), optional **`maxDelegationsPerTurn`**, **`workerId`**, **`sessionId`**. |

### Turn Streaming Events

Tool calls, results, and intermediate thinking are streamed as separate events, interleaved with delegation and message events, so clients can render agent activity in real time:

| Event | Meaning |
|-------|---------|
| **`session.tool_call`** | A tool is about to execute. Payload includes **`toolName`**, **`toolArgs`**, **`index`**, **`source`** (the agent id, e.g. `"orchestrator"` or a worker id), **`sessionId`**. |
| **`session.tool_result`** | A tool execution completed. Payload includes **`toolName`**, **`toolResult`**, **`index`**, **`source`**, **`sessionId`**. |
| **`session.assistant_progress`** | Intermediate content from the model during a tool loop iteration. Payload includes **`content`**, **`iteration`**, **`sessionId`**. Emitted when the model produces non-empty text alongside tool calls; without this event, that content would be invisible since only the final iteration's content is sent as the assistant reply. |
| **`session.tool_loop_limit`** | The **`maxToolLoopsPerTurn`** limit was reached during an orchestrator turn. Payload includes **`pendingToolCalls`** (array of tool calls generated by the model but not executed) and **`sessionId`**. Worker turns do not emit this event — only the orchestrator turn faces the user. Clients should display an indication that the turn was interrupted and the user must send another message to continue. |
| **`session.turn_stopped`** | The agent turn was stopped by the user (via the `stop` WebSocket method). Payload includes **`sessionId`** and optional **`source`**. The agent finished the current tool call or model request, then paused before the next iteration. The session transcript remains valid — the user can send a new message to continue. Clients should display an indication that the turn was paused. |

### Tool Event Index Semantics

The **`index`** on tool events is a running count within the current agent turn. It resets when a new turn starts (new user message). The semantics differ between orchestrator and worker events:

- **Orchestrator events** — The index is the tool's position within the orchestrator's tool loop (across all iterations). Since the orchestrator uses **`source`: `"orchestrator"`**, orchestrator indices never collide with worker indices regardless of overlap.

- **Worker events** — Each worker delegation receives a **`tool_index_offset`** that accumulates the total tool call count from all prior delegations in the same turn. The effective index for each worker tool event is `tool_index_offset + local_index`, where `local_index` is the tool's position within that worker's own turn. This prevents index collisions between successive delegations, even when delegating to the same worker ID. If a provider error occurs mid-delegation after some tool calls were already emitted, the partial count is still accumulated into the offset so the next delegation's indices do not overlap with the failed delegation's partially-emitted events.

Clients matching `tool_result` to `tool_call` entries should search in reverse to find the most recent entry with a given index, since indices may collide across turns.

Constants and emission logic live in **`crates/lib/src/orchestration/delegate.rs`** (emitted via `DelegateObservability`). The agent loop emits them in **`crates/lib/src/agent.rs`** (`execute_turn_main`).

**`orchestration.delegate.rejected` reasons** (stable strings for clients):

- **`max_delegations_per_turn`** — **`maxDelegationsPerTurn`** exceeded in this orchestrator turn.
- **`max_delegations_per_session`** — **`maxDelegationsPerSession`** would be exceeded after a successful delegation.
- **`max_delegations_per_worker`** — Per-worker session cap would be exceeded.

## Gateway `status` — worker rows

The gateway does **not** emit a top-level **`workers`** key on **`status`**. Worker runtime is represented as **`payload.agents[]`** objects with **`role`**: **`"worker"`** (after the orchestrator row), each including **`id`**, **`defaultProvider`**, and **`defaultModel`** using the same effective **`(provider, model)`** resolution as **`delegate_task`** defaults (see **`crates/lib/src/orchestration/workers_context.rs`**). **Chai Desktop** builds an in-memory list of **`{ id, defaultProvider, defaultModel }`** from those entries for the **Status** screen under **Agents** (see **`crates/desktop/src/app/state/gateway.rs`**).

## Related Documents

| Document | Purpose |
|----------|---------|
| **[AGENTS.md](AGENTS.md)** | Per-agent workspace, **`enabledSkills`**, worker vs orchestrator system context. |
| **[adr/ORCHESTRATION.md](../adr/ORCHESTRATION.md)** | Architectural decision for the orchestrator–worker model. |
| **[PROVIDERS.md](PROVIDERS.md)** | Provider ids, configuration, API comparison. |
| **[MODELS.md](MODELS.md)** | Model ids, repository inventory, tool-fit notes. |
