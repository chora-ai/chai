---
status: draft
---

# Epic: Parallel Workflows

**Summary** — Enable the orchestrator agent to run multiple `delegate_task` calls in parallel, so that an orchestrator with one worker can process several subtasks concurrently and an orchestrator with multiple workers can run them all simultaneously. Today, `delegate_task` calls in the orchestrator's tool loop are processed **sequentially** — each worker turn blocks until completion before the next tool call is handled. This epic redesigns the tool loop to support concurrent delegation, adds a `maxParallelWorkflows` configuration value (default **`3`**), and addresses the UX challenges of presenting multiple concurrent workflows to the user in the desktop application and CLI.

**Status** — **Proposed (not implemented).** All delegation is sequential; the orchestrator awaits each worker turn before proceeding.

## Problem Statement

When the orchestrator model returns multiple `delegate_task` tool calls in a single response (or across loop iterations), the current tool loop processes them **one at a time**. In **`execute_turn_main`** (`crates/lib/src/agent.rs`), the `for (idx, call) in last_tool_calls.iter().enumerate()` loop awaits each `execute_delegate_task` call sequentially. This means:

1. **Wasted latency** — If the orchestrator delegates to three workers in one response, the total wall-clock time is the **sum** of all three worker turns, not the **maximum**. With parallel execution, the total would be bounded by the slowest worker.
2. **Underutilized providers** — Workers targeting different providers (e.g., one on `ollama`, one on `nim`) could run simultaneously, but the sequential loop forces them to wait on each other.
3. **Same-worker serialization** — Even when delegating to the same worker multiple times (e.g., an orchestrator with one worker handling three independent file reads), the calls are serialized. Parallel execution would allow up to `maxParallelWorkflows` concurrent turns on the same worker's provider.
4. **No orchestration during delegation** — While a worker turn is in progress, the orchestrator is **blocked**. It cannot make its own tool calls, emit progress, or start additional delegations. The entire agent turn is a single blocking `await`.

## Goal

- **Parallel delegation** — When the model returns multiple `delegate_task` calls in one loop iteration, execute them concurrently (up to `maxParallelWorkflows`).
- **Same-worker parallelism** — An orchestrator with one worker agent should be able to run multiple workflows in parallel on that worker. The `maxParallelWorkflows` limit applies to total concurrent delegations, not per-worker.
- **Multi-worker parallelism** — An orchestrator with multiple workers should be able to run all of them in parallel when the model targets different workers simultaneously.
- **Configurable concurrency limit** — A `maxParallelWorkflows` field on the orchestrator configuration with a safe default of **`3`**. This bounds resource usage (provider API rate limits, gateway memory, concurrent tool execution).
- **Clear user experience** — Users must be able to follow what is happening when multiple workflows run concurrently: which workers are active, what tools they are calling, and what results they are producing.
- **Backward compatibility** — When `maxParallelWorkflows` is **`1`**, behavior is identical to today's sequential execution.

## Current State

### Tool Loop Architecture

**`execute_turn_main`** (`crates/lib/src/agent.rs`) — The orchestrator's tool loop:

```
loop {
    1. Call the model with current messages
    2. If no tool_calls → break (turn complete)
    3. For each tool_call in response:
       a. If delegate_task → await execute_delegate_task()  ← BLOCKS HERE
       b. Else → execute via ToolExecutor
       c. Append tool result to messages
    4. Continue loop (call model again with updated messages)
}
```

Tool calls within a single model response are iterated with a **synchronous `for` loop**. Each `delegate_task` call is `await`ed, blocking the entire loop. The orchestrator cannot proceed until the worker turn completes.

### Worker Turn Execution

**`execute_delegate_task`** (`crates/lib/src/orchestration/delegate.rs`) — Resolves the target provider/model, builds worker messages (`[system?, user(instruction)]`), and calls **`run_turn_with_messages_dyn`**, which runs `execute_turn_worker`. The worker's own tool loop runs to completion (may involve multiple LLM calls and tool executions), then returns the result. The orchestrator is blocked for the entire duration.

### Observability Events

Events are broadcast via **`DelegateObservability`** through a `broadcast::Sender<String>` channel. Currently:

| Event | Timing | Payload |
|-------|--------|---------|
| `orchestration.delegate.start` | Before worker turn | `provider`, `model`, `workerId?` |
| `orchestration.delegate.complete` | After worker turn succeeds | `provider`, `model`, `workerId?`, `workerToolCalls`, `workerToolResults`, `reply` |
| `orchestration.delegate.error` | After worker turn fails | `error`, `provider?`, `model?`, `workerId?` |
| `orchestration.delegate.rejected` | Policy limit hit | `reason`, `maxDelegationsPerTurn?`, `workerId?` |
| `session.tool_call` | Before each worker tool execution | `toolName`, `toolArgs`, `index`, `source` |
| `session.tool_result` | After each worker tool execution | `toolName`, `toolResult`, `index`, `source` |

The `source` field distinguishes orchestrator vs. worker events. The `index` field (with `tool_index_offset`) prevents index collisions between orchestrator and worker tool calls.

### Desktop Rendering

The desktop (`crates/desktop/src/app/screens/chat.rs`) renders:

- **Orchestrator tool calls** — Green-bordered (`70, 90, 70`) collapsible frames with 🔧 headers
- **Worker tool calls** — Blue-bordered (`70, 70, 90`) collapsible frames with 🔧 headers
- **Delegation events** — Italic text with accent color (blue=start, green=complete, amber=rejected, red=error)
- **Worker replies** — Blue-bordered chat lines with a small worker id label

All events are appended to a **single flat timeline** (`session_messages[session_id]`). There is no grouping, nesting, or parallel column layout.

### Configuration

**`AgentsConfig`** (`crates/lib/src/config.rs`) — Current delegation-related fields:

| Field | Purpose |
|-------|---------|
| `max_delegations_per_turn` | Cap on `delegate_task` calls in one orchestrator turn |
| `max_delegations_per_session` | Cap on successful delegations per session |
| `max_delegations_per_provider` | Per-provider per-session caps |

There is no `maxParallelWorkflows` or equivalent field.

### Concurrency Primitives

The gateway uses `tokio::spawn` for model discovery and channel tasks, `broadcast` channels for event distribution, and `mpsc` for inbound message queuing. **Notably absent**: `JoinSet`, `FuturesUnordered`, `Semaphore`, `oneshot` — none of the primitives needed for structured concurrent task execution are in use today.

## Scope

### In Scope

- **Concurrent delegation** — When the model returns multiple `delegate_task` calls in one loop iteration, execute them concurrently using `tokio::JoinSet` or equivalent.
- **`maxParallelWorkflows` config** — New field on `AgentsConfig` with default **`3`**. Bounds the number of concurrent `delegate_task` executions.
- **Same-worker parallelism** — Multiple concurrent delegations to the same worker are allowed, bounded by `maxParallelWorkflows`. The worker's `WorkerDelegateRuntime` is `Arc`-wrapped and safe to share across concurrent tasks (tools_list is `Clone`, tool_executor is `Arc<dyn ToolExecutor>`).
- **Orchestrator tool calls during delegation** — Whether the orchestrator can make its own tool calls while workers are running. This is a significant design decision (see Design section).
- **Event ordering for concurrent delegations** — Observability events from parallel workers must be distinguishable and correctly attributed so the desktop can render them coherently.
- **Desktop UX for parallel workflows** — Presenting multiple concurrent worker activities to the user in a way that is understandable.
- **CLI rendering** — Presenting concurrent worker events in a terminal-friendly format.

### Out of Scope

- **Streaming worker responses** — Workers today do not stream their output to the desktop in real-time (they emit `session.tool_call`/`tool_result` events, but the final reply only appears in `orchestration.delegate.complete`). Streaming worker output is a separate concern.
- **Nested parallel delegation** — Workers cannot use `delegate_task` (this is already enforced). No change needed.
- **Cross-turn parallelism** — Parallelism is scoped to tool calls within a single orchestrator turn. Users cannot send a second message while the first turn's workers are still running (the gateway processes one `agent` RPC at a time per session).
- **Priority or cancellation** — No mechanism to cancel a running worker or prioritize one delegation over another.
- **Worker resource isolation** — No rate limiting per worker, no per-worker concurrency limits (the single `maxParallelWorkflows` limit applies globally).

## Design

### Core Change: From Sequential to Concurrent Delegation

The fundamental change is in `execute_turn_main` (`crates/lib/src/agent.rs`). Today, tool calls are processed sequentially:

```rust
for (idx, call) in last_tool_calls.iter().enumerate() {
    let result = if name == DELEGATE_TASK_TOOL_NAME {
        execute_delegate_task(ctx, args).await  // BLOCKS
    } else {
        tool_executor.execute(name, args, session_id)
    };
    messages.push(tool_result_msg(result));
}
```

The redesigned loop must separate `delegate_task` calls from regular tool calls, then execute delegations concurrently:

```rust
// Phase 1: Execute non-delegate tool calls sequentially
// Phase 2: Execute delegate_task calls concurrently (up to maxParallelWorkflows)
// Phase 3: Collect results, append to messages, continue loop
```

#### Detailed Loop Redesign

For each loop iteration:

1. **Classify tool calls** — Partition the model's `tool_calls` into:
   - `regular_calls: Vec<(usize, ToolCall)>` — non-delegate tool calls
   - `delegate_calls: Vec<(usize, ToolCall)>` — `delegate_task` calls

2. **Execute regular tool calls** — Sequential, as today. These are typically fast (file reads, etc.) and may have side effects that ordering depends on.

3. **Execute delegate calls concurrently** — Using `tokio::JoinSet`:
   - Apply `maxParallelWorkflows` cap: if there are more delegate calls than the cap, execute the first N concurrently and queue the rest.
   - Apply `maxDelegationsPerTurn` cap: count delegate calls against the per-turn limit *before* launching (reject the excess with `orchestration.delegate.rejected`).
   - For each launched delegation, emit `orchestration.delegate.start`.
   - Each delegation runs `execute_delegate_task` in a spawned task.
   - Collect results as they complete (not necessarily in launch order).

4. **Merge results** — Append all tool results to `messages` in a deterministic order (original call index order, not completion order). This ensures the message history is consistent regardless of which worker finishes first.

5. **Continue the loop** — Re-call the model with the updated messages.

#### Concurrency Limiting

A `tokio::sync::Semaphore` initialized with `maxParallelWorkflows` permits controls how many delegations can run at once:

```rust
let semaphore = Arc::new(Semaphore::new(max_parallel_workflows));
let mut join_set = JoinSet::new();

for (original_idx, call) in delegate_calls {
    let permit = semaphore.clone().acquire_owned().await;
    let ctx = ctx.clone(); // DelegateContext needs Clone
    join_set.spawn(async move {
        let result = execute_delegate_task(&ctx, &call.function.arguments).await;
        drop(permit); // Release semaphore permit
        (original_idx, call, result)
    });
}
```

If `maxParallelWorkflows` is **`1`**, the semaphore allows only one delegation at a time — equivalent to today's sequential behavior.

#### `DelegateContext` Must Be `Clone`

Currently, `DelegateContext` derives `Clone` but holds references (`&'a ProviderClients`, `&'a AgentsConfig`, etc.) that cannot be moved across `tokio::spawn` boundaries (they are not `'static`). For concurrent delegation, the context must be converted to owned types or `Arc`-wrapped:

| Field | Current Type | Proposed Type |
|-------|-------------|---------------|
| `clients` | `&'a ProviderClients` | `Arc<ProviderClients>` |
| `providers` | `&'a ProvidersConfig` | `Arc<ProvidersConfig>` |
| `agents` | `&'a AgentsConfig` | `Arc<AgentsConfig>` |
| `orchestrator_system_context` | `Option<&'a str>` | `Option<Arc<String>>` |
| `orchestrator_worker_tools` | `Option<Vec<ToolDefinition>>` | `Option<Arc<Vec<ToolDefinition>>>` |
| `orchestrator_tool_executor` | `Option<&'a dyn ToolExecutor>` | `Option<Arc<dyn ToolExecutor>>` |
| `worker_runtimes` | `Option<&'a HashMap<String, WorkerDelegateRuntime>>` | `Option<Arc<HashMap<String, WorkerDelegateRuntime>>>` |
| `observability` | `Option<DelegateObservability>` | `Option<DelegateObservability>` (already `Clone`) |
| `session_store` | `Option<&'a SessionStore>` | `Option<Arc<SessionStore>>` (already `Arc`-wrappable) |
| `session_id` | `Option<&'a str>` | `Option<String>` |

This is a significant refactor but aligns with how `GatewayState` already stores these types (most are already `Arc`-wrapped in the gateway). The gateway constructs `DelegateContext` from `GatewayState` fields — the conversion from `Arc` to references (current) or directly using `Arc` (proposed) is straightforward.

### Should the Orchestrator Make Tool Calls While Workers Are Running?

This is the central design question of the epic. There are two approaches:

#### Approach A: Orchestrator Is Blocked During Delegation (Simpler)

The orchestrator waits for all concurrent delegations to complete before continuing the tool loop. This is the **minimal change** from today's behavior: instead of awaiting each delegation one at a time, it awaits all of them concurrently.

**How it works:**

1. Model returns `tool_calls` containing some `delegate_task` calls and some regular calls.
2. Execute regular calls first (sequential, fast).
3. Launch all `delegate_task` calls concurrently.
4. **Block**: await all results.
5. Append all results to messages, continue the loop.
6. The model sees all results at once in the next iteration.

**Pros:**
- Simple to implement — the tool loop structure is unchanged except for replacing sequential `await` with `JoinSet` collection.
- Message history is always consistent — the model receives all tool results in one batch.
- Event ordering is straightforward — delegation events are bracketed by `start`/`complete` pairs within a single loop iteration.

**Cons:**
- The orchestrator is idle while workers run — it cannot do its own work (file reads, analysis) in parallel with delegations.
- No opportunity for the orchestrator to react to partial results (e.g., "worker A finished, start a new delegation based on its output while worker B is still running").

**User experience:**
- In the desktop, the timeline shows: orchestrator activity → delegation start events (multiple) → worker tool calls interleaved by source → delegation complete events (as they finish) → orchestrator continues.
- Multiple workers' tool calls appear **interleaved** in the timeline. The `source` field distinguishes them, and the blue border color groups all worker activity visually.

#### Approach B: Orchestrator Continues While Workers Run (More Complex)

The orchestrator is free to make its own tool calls and even call the model again while worker delegations are in flight. This is a **fundamental architectural change** — the tool loop becomes an event-driven system rather than a simple request-response loop.

**How it works:**

1. Model returns `tool_calls` containing some `delegate_task` calls and some regular calls.
2. Launch `delegate_task` calls as background tasks (do not await immediately).
3. Execute regular tool calls normally.
4. Append regular results to messages.
5. **Call the model again** with the partial results (delegations still in flight).
6. As delegation results arrive, inject them as tool results into the message history and trigger a re-evaluation.
7. The orchestrator may call the model multiple times during a single "turn", each time with more delegation results available.

**Pros:**
- Maximum concurrency — the orchestrator can do its own work while workers are running.
- More responsive — the user sees orchestrator activity during long-running delegations.

**Cons:**
- **Complex message history management** — The message list is mutated concurrently: delegation results arrive asynchronously while the orchestrator may be mid-LLM-call. This creates race conditions in message ordering.
- **Provider API constraints** — Most provider APIs expect a coherent message history. Injecting results mid-conversation requires careful sequencing to avoid invalid message sequences (e.g., a `tool` result without a matching `assistant` tool_call in the same position).
- **Token waste** — Calling the model with partial results may lead to suboptimal responses if the model doesn't yet have all the information it needs.
- **Event ordering chaos** — Multiple concurrent model calls produce interleaved `assistant_progress` events, making the timeline confusing for users.
- **Small model fragility** — Weaker models may struggle with the concept that some tool results are still pending, leading to confusion or hallucinated results.

**User experience:**
- In the desktop, the timeline would show orchestrator and worker activities **truly interleaved**: orchestrator makes a tool call → worker A makes a tool call → orchestrator emits progress → worker B completes → orchestrator continues.
- This is significantly harder to follow. The user would need a way to visually separate the orchestrator's timeline from each worker's timeline.

#### Recommendation

**Approach A is recommended for the initial implementation.** It delivers the core value (parallel delegation) with minimal architectural risk. Approach B introduces fundamental complexity in message history management and provider API compliance that is better addressed as a follow-up once the parallel execution primitives are proven.

Approach A can be enhanced incrementally:
- **Phase 1**: Concurrent delegation with blocked orchestrator (Approach A).
- **Phase 2 (future)**: Investigate orchestrator-continues mode (Approach B) as a separate design, potentially gated behind a config flag.

### Event Attribution for Concurrent Delegations

When multiple delegations run concurrently, their `session.tool_call` and `session.tool_result` events will interleave on the broadcast channel. The desktop needs to attribute each event to the correct worker.

#### Current Attribution

- `source` field — The worker id (e.g., `"engineer"`, `"researcher"`) or `"worker"` for unnamed delegations.
- `index` field — Tool call index within the turn, with `tool_index_offset` to avoid collisions.

#### Problem With Concurrent Delegations

When two workers run in parallel, each emits `session.tool_call` events with their own `index` sequence starting from their `tool_index_offset`. But if both workers emit events simultaneously, the indices may interleave in confusing ways:

```
session.tool_call  index=0  source="engineer"   toolName="read_file"
session.tool_call  index=1  source="researcher"  toolName="git_log"
session.tool_result index=0 source="engineer"   toolName="read_file"
session.tool_call  index=2  source="researcher"  toolName="read_file"
session.tool_result index=1 source="researcher"  toolName="git_log"
session.tool_result index=2 source="researcher"  toolName="read_file"
```

The `source` field already disambiguates these events. The desktop currently matches `tool_call` to `tool_result` by `(index, toolName, source)` — this continues to work with concurrent delegations because each worker's `DelegateObservability` has its own `tool_index_offset` and `source`.

However, there is a new concern: `tool_index_offset` is currently set to **`0`** for all workers (the orchestrator's offset is applied separately). With concurrent delegations, each worker's observability should use a distinct offset range to prevent index collisions across workers.

**Proposed solution**: Before launching concurrent delegations, assign each worker a non-overlapping index range:

```
Worker 1: tool_index_offset = orchestrator_tool_count
Worker 2: tool_index_offset = orchestrator_tool_count + estimated_worker_1_tool_count
Worker 3: tool_index_offset = orchestrator_tool_count + estimated_worker_1_tool_count + estimated_worker_2_tool_count
```

Since worker tool counts are not known in advance, a simpler approach is to use **large stride offsets** (e.g., `worker_index * 1000`) or switch to a composite key `(source, index_within_source)` for matching. The latter is cleaner:

**Option 1: Composite key matching** — The desktop matches tool_call → tool_result by `(source, index)` instead of just `index`. This is already effectively how it works (the code checks `source` in addition to `index`), so no change may be needed.

**Option 2: Delegation-scoped event prefix** — Add a `delegationId` field to all events emitted during a delegation, uniquely identifying which delegation the event belongs to. This is the most robust solution but requires changes to `DelegateObservability`, event payloads, and desktop event processing.

**Option 3: Per-worker offset reservation** — Reserve index ranges for each concurrent delegation before launching. Simpler than delegation IDs but requires estimating max tool calls per worker.

**Recommendation**: **Option 1 (composite key matching)** for the initial implementation. The desktop already uses `source` in its matching logic. If this proves insufficient (e.g., two concurrent delegations to the same worker produce ambiguous indices), escalate to **Option 2 (delegationId)**.

### Desktop UX for Parallel Workflows

Presenting multiple concurrent worker activities is a significant UX challenge. Three approaches:

#### UX Approach 1: Flat Timeline With Source Labels (Minimal Change)

Keep the current single-timeline layout. Worker events from different sources are interleaved by arrival time. The `source` label and blue border color distinguish workers from the orchestrator and from each other.

**Rendering:**
```
┌─ 🤖 orchestrator ──────────────────────────┐
│ I'll investigate these in parallel.         │
│ 🔧 delegate_task  [engineer] ✅             │
│ 🔧 delegate_task  [researcher] ✅           │
└──────────────────────────────────────────────┘
┌─ 👷 engineer ───────────────────────────────┐
│ 🔧 read_file  ✅                            │
│ 🔧 git_log    ✅                            │
│ I found the relevant code in...             │
└──────────────────────────────────────────────┘
┌─ 👷 researcher ─────────────────────────────┐
│ 🔧 search_content  ✅                       │
│ The configuration is in...                  │
└──────────────────────────────────────────────┘
```

**Pros:**
- Minimal UI changes — only need to ensure source labels are visible and tool call headers show the worker name.
- Matches the existing rendering model.
- Events appear in real-time as they arrive.

**Cons:**
- With truly concurrent workers, events from different workers interleave unpredictably, which can be confusing.
- Hard to visually group "all events from worker A" vs "all events from worker B" when they are interleaved.
- No visual indicator of which workers are still running vs completed.

#### UX Approach 2: Grouped/Nested Timeline

Group all events from each delegation into a collapsible section, nested under the delegation start event. Events within a group are ordered by arrival time but groups can be expanded/collapsed independently.

**Rendering:**
```
┌─ 🤖 orchestrator ──────────────────────────┐
│ I'll investigate these in parallel.         │
│ ▼ 🔧 delegate_task  [engineer] ✅           │
│   ┌─ 👷 engineer ─────────────────────────┐ │
│   │ 🔧 read_file  ✅                      │ │
│   │ 🔧 git_log    ✅                      │ │
│   │ I found the relevant code in...       │ │
│   └────────────────────────────────────────┘ │
│ ▼ 🔧 delegate_task  [researcher] ✅         │
│   ┌─ 👷 researcher ───────────────────────┐ │
│   │ 🔧 search_content  ✅                 │ │
│   │ The configuration is in...            │ │
│   └────────────────────────────────────────┘ │
└──────────────────────────────────────────────┘
```

**Pros:**
- Clear visual grouping — all events from one worker are together.
- Collapsible — users can collapse workers they don't care about.
- Arrival-time ordering within each group is intuitive.

**Cons:**
- Requires `delegationId` (or equivalent) to group events correctly.
- More complex rendering — nested collapsible sections within the chat timeline.
- Events from one worker may arrive after the next orchestrator message, making nesting awkward.
- Does not naturally show "workers running in parallel" — the groups appear sequentially in the timeline.

#### UX Approach 3: Multi-Column or Tabbed View

Dedicate a separate visual lane, column, or tab to each active worker. The orchestrator has its own lane, and each concurrent delegation gets its own.

**Rendering (columns):**
```
┌─ orchestrator ──┐ ┌─ engineer ──────┐ ┌─ researcher ───┐
│ Delegating...    │ │ 🔧 read_file ✅ │ │ 🔧 search ✅    │
│                  │ │ 🔧 git_log  ✅  │ │ Found config... │
│                  │ │ Found code...   │ │                 │
└──────────────────┘ └─────────────────┘ └─────────────────┘
```

**Pros:**
- Clearest visualization of parallel activity.
- Each worker's timeline is independent and easy to follow.
- Intuitive for users who understand concurrent workflows.

**Cons:**
- **Major UI redesign** — The current chat view is a single-column scroll. Multi-column would require a fundamentally different layout.
- Does not work well on narrow screens or with many workers.
- Merging the columns back into a single conversation flow when the orchestrator synthesizes results is awkward.
- The `chai chat` CLI cannot render columns — would need a different approach for terminal output.

**Recommendation**: Start with **UX Approach 1 (flat timeline with source labels)** as the initial implementation. It requires minimal changes and leverages the existing `source` field. Add visual enhancements (worker name in tool call headers, subtle grouping) without changing the fundamental layout. If user feedback demands better organization, escalate to **UX Approach 2 (grouped/nested)** in a follow-up. **UX Approach 3 (multi-column)** is out of scope for this epic but should be noted as a future possibility.

### CLI Rendering for Parallel Workflows

In `chai chat`, concurrent worker events will interleave in the terminal. Options:

**Option A: Interleaved output** — Print events as they arrive, prefixed with the source:
```
[engineer] 🔧 read_file(src/main.rs)
[researcher] 🔧 search_content("session", src/)
[engineer] ✅ read_file → found 42 lines
[researcher] ✅ search_content → 3 matches
[engineer] I found the relevant code...
[researcher] The session module is in...
```

**Option B: Buffered output** — Collect all delegation results, then print them grouped by worker after all complete:
```
⚙ Running 2 delegations in parallel...

── engineer ──
🔧 read_file(src/main.rs) ✅
🔧 git_log ✅
I found the relevant code...

── researcher ──
🔧 search_content("session", src/) ✅
The session module is in...
```

**Recommendation**: **Option A (interleaved)** for the initial implementation — it shows progress in real-time and is simpler to implement. **Option B** could be a future enhancement for users who prefer clean output over real-time feedback.

### `maxParallelWorkflows` Configuration

**New field on `AgentsConfig`:**

| Field | Type | Default | Config Key |
|-------|------|---------|------------|
| `max_parallel_workflows` | `Option<usize>` | `3` | `maxParallelWorkflows` |

**Semantics:**
- Controls how many `delegate_task` calls can execute concurrently in a single loop iteration.
- Does **not** limit the total number of delegations per turn (`maxDelegationsPerTurn` still applies).
- When `maxParallelWorkflows` is **`1`**, behavior is identical to today's sequential execution.
- When the model returns more `delegate_task` calls than the limit, the excess calls are queued and executed as slots become available (semaphore behavior). This is preferred over rejection because the model intended all of them to run.

**Interaction with `maxDelegationsPerTurn`:**
- `maxDelegationsPerTurn` is checked **before** launching any delegations. If the model returns 5 `delegate_task` calls and `maxDelegationsPerTurn` is 3, the first 3 are launched (up to `maxParallelWorkflows` at a time) and the remaining 2 are rejected with `orchestration.delegate.rejected` (reason: `max_delegations_per_turn`).
- `maxParallelWorkflows` controls concurrency; `maxDelegationsPerTurn` controls total count. They are independent caps.

**Default value rationale:**
- **`3`** is a safe default that provides meaningful parallelism without overwhelming most provider APIs.
- For local providers (Ollama, LM Studio) with limited concurrent request capacity, the provider itself will queue or reject excess requests — the `maxParallelWorkflows` cap prevents the gateway from sending more than the provider can handle.
- For cloud providers with higher rate limits, users can increase the value.

**System context implication:**
- Today, **`build_workers_context`** (`crates/lib/src/orchestration/workers_context.rs`) includes the line "`delegate_task` calls execute sequentially — each worker turn completes before the next begins." This line must be removed or made dynamic when this epic is implemented.
- When `maxParallelWorkflows` is **`1`**, the "sequentially" wording should remain. When it is greater than **`1`**, the context should ommit the not about sequential execution (i.e., "`delegate_task` calls execute sequentially — each worker turn completes before the next begins.").
- This is a small but important change — the system context is the orchestrator's primary source of truth about its own capabilities, and an incorrect claim about sequential execution would cause the orchestrator to plan suboptimally.

### Session Consistency

When delegations run concurrently, the orchestrator's message history must remain consistent. Each delegation result becomes a `tool` role message in the orchestrator's transcript. With parallel execution:

1. **All delegation results are appended after all delegations complete** (Approach A). The model sees them as a batch in the next iteration.
2. **Result ordering** — Results are inserted in the original call order (by the `original_idx` assigned when classifying tool calls), not in completion order. This ensures the model can correlate tool calls with tool results by position.
3. **Session persistence** — The `SessionStore` is updated with tool results after all delegations complete. Since `SessionStore` uses `RwLock`, concurrent writes from parallel delegations would conflict. With Approach A, this is not an issue because writes happen after collection. If Approach B is pursued later, session writes would need to be serialized.

### Provider Client Thread Safety

`ProviderClients` wraps `HashMap<String, Box<dyn Provider>>`. Each `Provider` implementation must be safe to call concurrently from multiple tokio tasks. The existing `Provider` trait requires `Send + Sync`, so this should already be safe. However, specific provider implementations (especially Ollama with HTTP clients) should be audited for concurrent request handling.

## Requirements

### Functional

- [ ] **Concurrent delegation** — When the model returns multiple `delegate_task` calls in one loop iteration, execute them concurrently (up to `maxParallelWorkflows`).
- [ ] **`maxParallelWorkflows` config** — New field on `AgentsConfig` with default **`3`**, configurable via `config.json` as `maxParallelWorkflows`.
- [ ] **Same-worker parallelism** — Multiple concurrent delegations to the same worker are allowed, bounded by `maxParallelWorkflows`.
- [ ] **Sequential fallback** — When `maxParallelWorkflows` is **`1`**, behavior is identical to today's sequential execution.
- [ ] **Dynamic system context** — The workers roster in `build_workers_context` reflects the current `maxParallelWorkflows` value: "sequentially" when **`1`**, "concurrently, up to `maxParallelWorkflows` at a time" when greater than **`1`**.
- [ ] **Semaphore enforcement** — When more delegate_task calls are issued than `maxParallelWorkflows`, excess calls wait for a slot (not rejected).
- [ ] **`maxDelegationsPerTurn` interaction** — Per-turn delegation cap is checked before launching; excess calls are rejected (not queued).
- [ ] **Deterministic result ordering** — Delegation results are appended to the message history in original call order, not completion order.
- [ ] **`DelegateContext` owned types** — `DelegateContext` is refactored to use `Arc`-wrapped owned types so it can be cloned into `tokio::spawn` tasks.
- [ ] **Event attribution** — All observability events from concurrent delegations carry correct `source` and `index` fields so the desktop can attribute them correctly.
- [ ] **Desktop source labels** — Worker tool calls in the chat timeline show the worker name clearly (in the 🔧 header or as a label).
- [ ] **CLI source prefixes** — `chai chat` prints concurrent worker events with source prefixes (e.g., `[engineer] 🔧 read_file`).

### Non-functional

- [ ] **Thread safety** — `Provider` implementations are safe for concurrent use. Audit and document.
- [ ] **No session corruption** — Concurrent delegations do not corrupt the `SessionStore`. All writes are serialized (Approach A guarantees this).
- [ ] **Bounded concurrency** — The number of concurrent tokio tasks is bounded by `maxParallelWorkflows` at all times.
- [ ] **Observability** — Structured logging for parallel delegation: log when delegations are launched concurrently, when they complete, and any errors.

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1. Core infrastructure | Refactor `DelegateContext` to owned types; add `maxParallelWorkflows` config; redesign `execute_turn_main` for concurrent delegation with `JoinSet`/`Semaphore`; deterministic result ordering; event attribution | Not started |
| 2. Desktop rendering | Source labels on worker tool calls; verify composite-key matching handles concurrent events; visual polish for interleaved worker events | Not started |
| 3. CLI rendering | Source-prefixed output for concurrent worker events | Not started |
| 4. Testing and hardening | Integration tests for concurrent delegations; provider thread safety audit; stress tests with high concurrency; verify session consistency | Not started |
| 5. UX enhancement (optional) | Grouped/nested timeline for worker events; `delegationId` event field for robust attribution; collapsible worker sections | Not started |

## Open Questions

- **Should the orchestrator continue while workers run?** Approach A (blocked orchestrator) is recommended for the initial implementation, but Approach B (orchestrator continues) offers more responsiveness at the cost of significant complexity. Is the blocked-orchestrator experience acceptable for the first release?
- **Event index management for concurrent workers** — Should each concurrent delegation get a unique `delegationId` for robust event attribution, or is the current `(source, index)` composite key sufficient? The answer depends on whether concurrent delegations to the same worker produce ambiguous indices.
- **Provider concurrency limits** — Should `maxParallelWorkflows` be aware of per-provider concurrency limits (e.g., Ollama's default of 1 concurrent request)? Or should the gateway rely on the provider to queue/reject excess requests? A provider-aware limit would be more efficient but adds configuration complexity.
- **Desktop grouping UX** — Is the flat timeline with source labels (UX Approach 1) sufficient, or do users need grouped/nested views from day one? This should be validated with user feedback after Phase 2.
- **Interruption and cancellation** — When a delegation is running in parallel and the user interrupts the turn (e.g., closes the chat or sends a new message), should running delegations be cancelled? How? This is not in scope for this epic but the design should not preclude it.
- **Streaming orchestrator output during delegation** — Should the orchestrator's intermediate text (from `assistant_progress`) be visible while workers are running? With Approach A, the orchestrator does not call the model again until all delegations complete, so there is no intermediate output. With Approach B, there would be. Should `assistant_progress` events be emitted during the wait phase?

## Follow-ups

### Orchestrator-Continues Mode (Approach B)

Allow the orchestrator to make tool calls and call the model while worker delegations are in flight. This would require:
- A fundamentally different tool loop architecture (event-driven rather than request-response).
- Careful management of message history to avoid invalid sequences.
- A more sophisticated desktop UI to show interleaved orchestrator and worker activities.

### Per-Provider Concurrency Awareness

Add per-provider `maxConcurrentRequests` configuration so the gateway can respect provider rate limits when launching parallel delegations. This would work with `maxParallelWorkflows` as a global cap, with the per-provider limit being an additional constraint.

### Delegation Cancellation

Allow the user or orchestrator to cancel a running delegation. This would require:
- Cancellation tokens passed to worker turns.
- A UI affordance (e.g., "Cancel" button on in-progress delegations).
- Cleanup of partial results in the session store.

### Streaming Worker Output

Stream worker `assistant_progress` events to the desktop in real-time, rather than only showing tool_call/tool_result events. This would make concurrent delegations more visible and responsive.

## Related Epics and Docs

- [ORCHESTRATION.md](../spec/ORCHESTRATION.md) — Behavioral specification for orchestration and delegation.
- [ORCHESTRATION.md](../adr/ORCHESTRATION.md) — ADR for the orchestrator–worker model and design decisions.
- [CONTEXT.md](../spec/CONTEXT.md) — System context build order, worker roster, skill context modes.
- [DESKTOP.md](../spec/DESKTOP.md) — Desktop chat screen rendering, worker reply display, delegation events.
- [SESSIONS.md](../spec/SESSIONS.md) — Session persistence spec; session store refactoring may overlap with `DelegateContext` changes.
- Implementation touchpoints: **`crates/lib/src/agent.rs`** (tool loop redesign), **`crates/lib/src/orchestration/delegate.rs`** (`DelegateContext`, `DelegateObservability`, `execute_delegate_task`), **`crates/lib/src/config.rs`** (`AgentsConfig`, `maxParallelWorkflows`), **`crates/lib/src/gateway/server.rs`** (`DelegateContext` construction, event subscription), **`crates/desktop/src/app/screens/chat.rs`** (worker event rendering), **`crates/desktop/src/app/state/chat.rs`** (event processing and attribution).
