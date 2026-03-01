# Epic: Orchestrators and Workers

**Summary** — Enable an orchestrator model (or agent) to plan work and delegate subtasks to worker models or agents, so different models can be used per step for capability, privacy, and cost.

**Status** — Proposed (not implemented).

---

## Goal

Move from a single default model for the entire agent loop to an **orchestrator–worker** design: an orchestrator holds the conversation and context, decides which steps to take, and can **delegate** specific subtasks to worker models or agents. Workers handle narrow, well-defined steps (e.g. a single tool call or classification); the orchestrator chooses which backend and model to use per step. This enables smaller/faster models as workers, privacy-aware routing (sensitive steps only to local/self-hosted), and cost/latency optimization.

## Current State

- The gateway uses a **single default model** for the whole agent loop: that model sees the conversation and skill context, decides when to call tools, and produces the reply.
- There is no notion of “orchestrator” vs “worker”; one model does everything.
- Backend and model are chosen once via config (**`agents.defaultBackend`**, **`agents.default_model`**); no per-step or per-delegation selection.

## Scope

- **In scope:** Multi-backend/model registry or catalog; delegation primitive (“run this subtask with that model”); orchestrator loop that can call the orchestrator then invoke workers and feed results back; policy/config for which models are allowed for which work; worker invocation path; observability (who did what, with which model).
- **Out of scope:** Implementing additional LLM backends (see [EPIC_API_ALIGNMENT.md](EPIC_API_ALIGNMENT.md)); full sandboxing or exec-approval flows.

## Requirements

- [ ] **Multi-backend and model registry** — Register or discover multiple backends and the models each exposes; catalog of (backend, model id) pairs the orchestrator is allowed to use, with optional metadata (e.g. "local", "tool-capable", "max context"); config or API to define which backends/models are available for orchestrator vs workers.
- [ ] **Delegation primitive** — Ability to issue “run this subtask with backend X, model Y” and get back a result (content, tool_calls, finish reason) in a form the orchestrator can consume (e.g. tool result or message); clear input/output contract for delegated calls.
- [ ] **Orchestrator loop and tool semantics** — Agent loop that calls the orchestrator for the next action (reply, delegate, clarify); when the action is “delegate,” invoke the worker with chosen backend/model and subtask, then feed result back and repeat; tool semantics for delegation (e.g. “delegate to worker” as a tool or first-class action).
- [ ] **Policy and config** — Policy or config so the orchestrator (or gateway) knows which models are allowed for which work (e.g. sensitive data only to local/self-hosted; worker for tool X must use backend Y); optional budgets or guards (max delegations per turn, allowed backends per session, approval hooks).
- [ ] **Worker invocation path** — Given (backend, model id, subtask), the gateway can look up the right client, build the worker request (messages, tools), call the backend, and return the result without session persistence or channel delivery.
- [ ] **Observability** — Logs or events that distinguish orchestrator decisions from worker invocations and results, including which model was used.

## Technical Reference

### Definitions

- **Worker** — A model (or agent) given a **narrow, well-defined subtask** (e.g. “call this tool with these arguments,” “answer this classification question,” “summarize this text”). A worker does not own the full conversation, plan multi-step flows, or choose which skills to use.
- **Orchestrator** — A model (or agent) that **plans the work**, chooses which steps to take, and **delegates** specific steps to workers. The orchestrator holds the conversation and context; it decides “for this step I’ll use the local 3B model” or “for this step I need the 70B model” or “this step stays on a privacy-safe worker.”

### Why the Distinction Matters

- **Model size and capability** — Smaller models (e.g. 3B, tool-trained like IBM Granite-4-micro) can be good **workers**: fast, low resource, reliable at single-step tool calls or simple reasoning. They are less reliable as the **only** model for long, multi-step flows. Larger models (7B–70B+) are better at planning and deciding when to call which tool across many turns.
- **Privacy and routing** — An orchestrator can route by sensitivity: keep sensitive data on local or self-hosted workers, and send only low-sensitivity or capability-heavy subtasks to a third-party API.
- **Cost and latency** — Use a small, fast model for many simple steps and a larger (or remote) model only when the task needs it; the orchestrator chooses “which model for this step.”

### Current vs Future

| Aspect | Current implementation | Future (orchestrator + workers) |
|--------|------------------------|----------------------------------|
| **Who runs the agent loop** | One model (`agents.default_model`) | Orchestrator model (or agent) |
| **Who runs tools / subtasks** | Same model | Can be workers: different model(s) per step or per task type |
| **Model choice** | Single backend + model id in config | Orchestrator chooses backend and model per step (or per delegation) |
| **Privacy / routing** | All traffic to one backend | Orchestrator can route sensitive steps to local/self-hosted only |

### Implementation Notes

- The existing `run_turn` and **`LlmBackend`** trait are a starting point; a worker invocation might be a constrained `run_turn` (e.g. single tool-call step, no channel delivery, result only).
- Today `run_turn` is tied to a single backend from config; a worker call would need to select backend and model by id from the registry.
- The current single-model design keeps the implementation simple and leaves the LLM layer (backends, `run_turn`, config) in a shape where adding a registry, a delegation primitive, and an orchestrator loop is a natural next step rather than a rewrite.
