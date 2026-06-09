# Audit: Worker Response Summarization for Orchestrator Context

## Problem

When the orchestrator delegates a task via `delegate_task`, the worker's full response—including the reply text, all tool calls (with arguments), and all tool results—is packed into a JSON string and returned to the orchestrator as the tool result message. This means the orchestrator model sees every detail of the worker's internal execution, which:

1. **Inflates the orchestrator's context window** with worker implementation details that are often irrelevant to the orchestrator's decision-making.
2. **Increases token cost** for cloud providers, since the large tool result is part of the orchestrator's conversation history and is re-processed on every subsequent turn.
3. **Degrades orchestrator performance** when the context fills with verbose worker artifacts instead of the concise summaries the orchestrator actually needs to continue its work.

Meanwhile, the **desktop user** benefits from seeing these details (which tools the worker called, what the results were), and the observability layer already provides this via WebSocket events. These two audiences—orchestrator model and desktop user—have different information needs that are currently served by the same channel.

## Implementation Status: Complete

The changes described in this audit have been implemented. The sections below document what was done.

## Architecture

### Data Flow (After Implementation)

```
Orchestrator calls delegate_task(instruction)
    └─> execute_delegate_task(ctx, args)
            ├─> resolve_delegate_target()           // pick provider/model/worker
            ├─> emit EVENT_DELEGATE_START            // → WebSocket → desktop
            ├─> run_turn_with_messages_dyn()         // worker turn
            │       ├─> model chat (loop)
            │       ├─> emit EVENT_TOOL_CALL         // → WebSocket → desktop
            │       ├─> tool execution
            │       └─> emit EVENT_TOOL_RESULT       // → WebSocket → desktop
            ├─> emit EVENT_DELEGATE_COMPLETE         // → WebSocket → desktop
            └─> return format_delegate_result(       // ← Summarized for orchestrator
                    reply,
                    provider_id,
                    model
                )
```

### The Two Channels

| Channel | Audience | Content |
|---------|----------|---------|
| **Tool result** (returned to orchestrator model) | Orchestrator LLM | `format_delegate_result()`: reply + worker provider/model |
| **WebSocket events** (`EVENT_DELEGATE_*`, `EVENT_TOOL_CALL`, `EVENT_TOOL_RESULT`) | Desktop UI | Structured events with tool names, arguments, results, worker id, provider/model |

### Key Code Locations

| File | Role |
|------|------|
| `crates/lib/src/orchestration/delegate.rs` | `format_delegate_result()` builds the summarized JSON tool result. `execute_delegate_task()` calls it and emits observability events. |
| `crates/lib/src/agent.rs` | Orchestrator tool loop: calls `execute_delegate_task()`, pushes the result string as a `tool` message. |
| `crates/desktop/src/app/state/chat.rs` | Desktop processes `orchestration.delegate.*` events for the chat display. |

### What the Orchestrator Receives

1. **The worker's reply text** — the assistant message the worker produced (its final content after the tool loop).
2. **Worker identity** — which worker ran, on which provider/model (for the orchestrator's decision-making about retries or alternative workers).

### What the Orchestrator Does NOT Receive

1. **Individual tool call structures** (tool name + arguments JSON) — the orchestrator didn't issue these calls and doesn't need to replay them.
2. **Individual tool result strings** — these are the worker's internal artifacts. The worker's reply already synthesizes them.
3. **Raw tool output** — file contents, search results, etc. that the worker already processed.

### `format_delegate_result` (After Implementation)

```json
{
  "reply": "All edits are correctly applied...",
  "worker": {
    "provider": "openai",
    "model": "gpt-4o"
  }
}
```

The `toolCalls` and `toolResults` fields are removed from the tool result message. They continue to flow to the desktop via the existing `EVENT_TOOL_CALL` and `EVENT_TOOL_RESULT` observability events.

## Changes Made

### 1. `crates/lib/src/orchestration/delegate.rs`

- **`format_delegate_result()`** — Removed `tool_calls` and `tool_results` parameters. New signature:

  ```rust
  fn format_delegate_result(
      reply: String,
      provider_id: &str,
      model: &str,
  ) -> String
  ```

- **`execute_delegate_task()`** — Updated the call site to pass only `reply`, `provider_id`, and `model`.

- **`parse_delegate_tool_calls()`** — Removed. No longer needed since `toolCalls` is not in the result.
- **`parse_delegate_tool_results()`** — Removed. No longer needed since `toolResults` is not in the result.

- **Import cleanup** — Removed `ToolCall` from the top-level import and `ToolCallFunction` from the test module import.

- **Tests updated**:
  - `format_delegate_result_includes_reply_and_tool_calls` → replaced with `format_delegate_result_includes_reply_and_worker` (asserts `toolCalls` and `toolResults` are absent).
  - `parse_delegate_tool_calls_round_trip` — removed.
  - `parse_delegate_tool_results_round_trip` — removed.

### 2. `crates/lib/src/agent.rs`

- **Orchestrator tool loop** — Removed the code that extracted worker tool calls/results from the delegate result (`worker_tool_calls`, `worker_tool_results` variables and the `parse_delegate_tool_calls`/`parse_delegate_tool_results` calls). Removed the `executed_tool_calls.extend(worker_tool_calls)` and `executed_tool_results.extend(worker_tool_results)` lines.

- **Import cleanup** — Removed `parse_delegate_tool_calls` and `parse_delegate_tool_results` from the imports.

### 3. `crates/lib/src/orchestration/mod.rs`

- Removed `parse_delegate_tool_calls` and `parse_delegate_tool_results` from the public exports.

### 4. Observability (No Changes)

The `EVENT_DELEGATE_COMPLETE` event still includes `workerToolCalls` (count), `workerToolResults` (count), and `reply`. The `EVENT_TOOL_CALL` and `EVENT_TOOL_RESULT` events still carry full details per tool invocation. **No changes needed** — the desktop continues to receive all worker details through these events.

### 5. Desktop App (No Changes)

The desktop chat display already consumes observability events for delegation lifecycle (start/complete/error/rejected) and per-tool-call details. It does not parse the tool result JSON directly. **No changes needed.**

## Tradeoffs

### Benefits

| Benefit | Explanation |
|---------|-------------|
| **Reduced orchestrator context** | The tool result message is dramatically smaller — just the reply and worker identity, instead of potentially kilobytes of tool call arguments and output. |
| **Lower token cost** | The orchestrator's conversation history is smaller, so each subsequent API call processes fewer tokens. This is the primary cost driver for cloud providers. |
| **Better orchestrator performance** | Less noise in context means the orchestrator model can focus on its own decision-making rather than parsing worker internals. |
| **No user experience change** | The desktop continues to show full details via observability events. The user sees everything the worker did. |

### Risks

| Risk | Severity | Notes |
|------|----------|-------|
| **Orchestrator loses visibility into worker tool failures** | Medium | The worker's `reply` text typically includes summaries of what went wrong. If the worker's model fails to mention a tool error in its reply, the orchestrator won't know. Holding off on adding structured error summarization because it could confuse the orchestrator if a tool call failed but the worker successfully resolved the error. |
| **Debugging orchestrator behavior** | Low | When the orchestrator makes a bad decision after delegation, it's harder to see what the worker returned from the orchestrator's messages alone. The observability events still capture everything, and the reply text is the primary driver of orchestrator behavior. |

### What Stays the Same

- The desktop user sees **all** worker activity: every tool call, every result, every error.
- The `EVENT_DELEGATE_START`, `EVENT_DELEGATE_COMPLETE`, `EVENT_TOOL_CALL`, and `EVENT_TOOL_RESULT` events are unchanged.
- The worker's tool loop and execution are unchanged — only what the orchestrator model receives back is trimmed.

## Future Considerations

### Configurable Summary Detail

A future enhancement could add a `summaryDetail` field to `delegate_task` arguments:

- `"concise"` (default) — reply + worker identity only (the current implementation).
- `"verbose"` — reply + worker identity + tool call names (no arguments or results).
- `"full"` — the previous behavior, for cases where the orchestrator genuinely needs to inspect worker artifacts.


## Live Testing (Session 2026-06-08)

This section records observations from hands-on testing of the worker summarization feature after implementation.

### Test Scenarios

| # | Worker | Task | Tools Used | Result |
|---|--------|------|------------|--------|
| 1 | worker-1 | Fix unused variable `selected` → `_selected` in `skills.rs:253` | `files_read_lines`, `files_write_lines` | ✅ Correct fix applied |
| 2 | worker-2 | Git status of chai repo | `git_status` | ✅ Correct: branch, 9 modified, 2 untracked |
| 3 | worker-1 | Read nonexistent file | `files_list_dir` (error path) | ✅ Worker reported file doesn't exist, listed directory instead |
| 4 | worker-2 | Git log in sandbox root (wrong directory) | `git_log` (failed) | ⚠️ Worker didn't know repo was at `./chai` |
| 5 | worker-1 | Multi-step: read lines + search + list dir (3 tools) | `files_read_lines`, `files_search_content`, `files_list_dir` | ✅ All three results reported accurately |
| 6 | worker-2 | Git log at `./chai` (corrected path) | `git_log` | ✅ Correct: 3 commits shown with author/date/message |

### Before vs. After

#### Before (Hypothetical — Unsummarized)

For the multi-step test (#5), the orchestrator would have received the raw tool calls and results:

```
Tool Call: files_read_lines(path="./chai/crates/desktop/src/app/screens/skills.rs", start_line=1, end_line=10)
Tool Result: "1|use eframe::egui;\n2|\n3|use crate::app::ui::{dashboard, spacing};\n..."
Tool Call: files_search_content(pattern="skill_field_block", path="./chai/crates/desktop/src/app/screens/skills.rs", line_numbers=true)
Tool Result: "253:fn skill_field_block(ui: &mut egui::Ui, key: &str, value: &str, _selected: bool) {\n315:..."
Tool Call: files_list_dir(path="./chai/crates/desktop/src/app/screens/")
Tool Result: "chat.rs\nconfig.rs\ncontext.rs\n..."
```

Estimated overhead: ~500-800 tokens of tool call JSON + ~300-500 tokens of raw results = **~800-1300 extra tokens per delegation** that the orchestrator would re-process on every subsequent turn.

#### After (Current — Summarized)

The orchestrator received only the worker's reply text and identity:

```json
{"reply":"Here are the results from all three operations:...","worker":{"provider":"nearai","model":"zai-org/GLM-5.1-FP8"}}
```

The reply already synthesizes the three tool results into a concise, structured format. No redundant raw tool artifacts.

**Token savings per delegation**: approximately 800-1300 tokens. Over a session with 10-20 delegations, this compounds to **8,000-26,000 tokens** saved in cumulative context processing.

### Cost Efficiency

**Yes, this is meaningfully more cost-efficient** without degrading orchestrator abilities:

1. **The orchestrator never needed the raw tool data.** In every test, the worker's reply contained all the information the orchestrator needed to make decisions. The raw `files_read_lines` output, `files_search_content` matches, and `files_list_dir` entries are implementation details that the worker already digested.

2. **Cumulative savings are significant.** Token savings aren't one-time — every saved token is a token the orchestrator doesn't re-process on each subsequent API call. A 1,000-token saving on turn 3 is still saving 1,000 tokens on turns 4, 5, 6, etc.

3. **No observed quality degradation.** The orchestrator successfully:
   - Verified the worker-1 fix by independently reading the file afterward.
   - Dispatched a follow-up correction to worker-2 when it failed to find the git repo.
   - Assessed error-handling behavior from the worker's reply alone.

### Tradeoffs Observed in Practice

#### Worker Error Visibility

Test #3 (nonexistent file) confirmed the **medium-severity risk** documented above: the worker encountered an error but self-resolved it (listed the directory instead). The orchestrator only saw the worker's reply explaining this, not the underlying tool error. In this case, the worker's reply was accurate and complete. **The risk is real but manageable** — worker models generally include error context in their replies.

#### Worker Capability Discovery

Test #4 revealed a practical issue: worker-2 didn't know the git repo was at `./chai` rather than the sandbox root. This isn't a summarization problem per se, but it highlights that **the orchestrator needs to provide sufficient context in the instruction** for the worker to succeed. The summarization didn't obscure this — the worker clearly reported the failure in its reply.

#### Multi-Step Task Summarization

Test #5 (3-tool task) is the most important result. A worker performing 3 tool calls produced a reply that synthesized all three results. The orchestrator got exactly what it needed without 3 sets of tool call/result JSON. **This is the strongest case for summarization**: the more tools a worker uses, the greater the savings, and the worker's synthesis is typically better than raw tool output for orchestrator decision-making.

### Additional Observations

#### Worker Identity Is Useful

The `worker.provider` and `worker.model` fields in the summarized result are valuable. Knowing which model executed the task helps the orchestrator reason about reliability (e.g., a weaker model might need verification). This was a good inclusion in the summary format.

#### Worker Reply Quality Matters More Now

Since the reply is the sole channel for information flow back to the orchestrator, the **quality of the worker's reply text** is now the critical path. Workers that produce terse or incomplete replies will starve the orchestrator. This is an emerging property that should be monitored — if workers from certain providers/models produce poor summaries, the orchestrator's decisions will degrade without any diagnostic trail in its context.

Recommendation: the **skill directives** for workers should include guidance like "always summarize your findings fully in your reply, including any errors encountered and how they were resolved." This ensures the reply is self-contained even without the tool artifacts.

#### No `git stash` for worker-2

Worker-2 correctly identified that `git stash list` isn't among its available tools. This is a tool gap, not a summarization issue. The worker was transparent about the limitation in its reply.

#### Orchestrator Verification Is Still Possible

After worker-1 fixed the variable, the orchestrator independently verified the fix by reading lines 251-255 of the file. This confirms that **summarization doesn't prevent verification** — the orchestrator can always use its own tools to double-check worker output when it matters.
