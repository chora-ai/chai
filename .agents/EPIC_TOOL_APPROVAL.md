# Epic: Tool Call Approval (Draft Proposal)

**Summary** — Optional **human-in-the-loop** approval before executing tool calls from the assistant, so operators can prevent unintended or harmful actions. Approval would be **configurable** (default: current behavior — execute tools immediately). This document is a **draft proposal**: requirements, implications, performance concerns, and open decisions. It does **not** commit the project to implementation.

**Status** — **Draft.** Not scheduled. Review against [VISION.md](../VISION.md) (long-term security goals) and [EPIC_ORCHESTRATION.md](EPIC_ORCHESTRATION.md).

## Goal

- **Safety** — Reduce risk from mistaken or over-eager tool use (file changes, shell, outbound actions, delegation) when the operator wants a review step.
- **Operator control** — Clear opt-in: users who trust their setup or need low friction keep **auto-execute**; users who want gates enable **approval**.
- **Consistent semantics** — Whatever behavior is chosen should be understandable: what is pending, what was denied, and how the model is informed.

## Current State (Baseline)

- **Execution model** — In **`crates/lib/src/agent.rs`**, after the model returns **`tool_calls`**, the runtime invokes **`ToolExecutor::execute`** for each call **immediately** (synchronous `execute`), appends **`tool`** role messages, and continues the tool loop (up to **`MAX_TOOL_LOOP`**) without user input.
- **Gateway** — **`GenericToolExecutor`** is built from skills; **`ReadOnDemandExecutor`** wraps file reads ([`gateway/server.rs`](../crates/lib/src/gateway/server.rs)). Channels (**Telegram**, **Matrix**, **Signal**) funnel **`InboundMessage`** into the same session/agent path; there is **no** pending-approval state today.
- **Delegation** — **`delegate_task`** can run worker turns with their own tool loop; approval policy would need an explicit story (inherit orchestrator policy, or separate rules).

## How a Turn Would Change (Conceptual)

Understanding this is important for judging complexity and impact on small models.

### Today: one continuous tool loop per “agent turn”

Roughly:

1. Build messages (history + new user message).
2. Call the model (stream or not).
3. If the response includes **`tool_calls`**: for **each** call, run the executor → append **`tool`** messages → go to step 2.
4. Repeat until no **`tool_calls`** or iteration cap.
5. Return final text and accumulated tool metadata.

From the user’s perspective this is often **one** reply turn, but internally it may involve **multiple** model invocations (one per tool round). Small models already struggle with multi-step tool use; each extra model call adds failure modes.

### With approval: the loop must pause

Approval means **no tool runs until the user consents**. The runtime cannot complete step 3 the same way: after the model asks for tools, execution must **stop** until approval arrives (or deny/timeout policy applies).

Two implementation families:

**A. Split-turn (persist and resume)**  
- **Phase 1** — Model returns assistant message + **`tool_calls`**. Persist **pending approval** (session id, conversation id, serialized message list or continuation handle, ordered tool calls, channel metadata). Send the user a **summary** (tool names + arguments, possibly redacted) and **approve/deny** affordances. **Do not** append **`tool`** messages yet (or append only after resolution, depending on transcript rules — see open questions).  
- **Phase 2** — On approve: execute **approved** calls only, append **`tool`** messages, **resume** the tool loop (next model call). On deny: append synthetic **`tool`** results indicating denial (or abort — policy), then optionally one model call to recover.  
- **Effect on “one turn”** — The **chat turn** splits across **time** and often **multiple inbound events** (user message → later “approve” callback). The **LLM** still does multiple rounds **after** approval, similar to today; the **new** cost is **latency** (human) and **state management**, not an extra model call *solely* for approval unless you add a dedicated “confirmation” model step (not assumed here).

**B. Async barrier inside the tool loop**  
- Keep a single long-lived async task that **awaits** a channel-specific future when a tool would run.  
- Simpler to reason about as “one turn,” but holds **memory and tasks** open until the user responds; needs **timeouts**, **cancellation**, and **crash recovery** (what if the gateway restarts while waiting?).  
- Often converges with **A** on disk/network for durability.

In both cases, **the model is not “re-prompted” for approval** unless you explicitly add that (e.g. a second small call to summarize risk — that **would** add tokens and latency). The default epic assumption is: **human reviews structured tool names/args**, not an extra LLM judge.

### What changes for the model transcript

- **Approved path** — Same as today after tools run: **`assistant` (with tool_calls) → `tool` results → …**  
- **Denied path** — The model still needs **`tool`** messages for calls it proposed, or a clear abort. Common pattern: one **`tool`** result per denied call: e.g. `error: user denied execution` (wording should be consistent and logged per [AGENTS.md](../AGENTS.md) style). The next model call may “waste” context recovering politely; **small models** may loop or apologize instead of progressing — a **product risk**, not just a performance metric.  
- **Partial approval** — If only some calls are approved, the transcript must remain **valid** for the chat API (tool results matching tool_calls order — exact rules depend on provider; must be specified in a future spec).

### Interruption nuance

“Interrupting a turn” here means **interrupting the synchronous progression** of the tool loop, **not** necessarily canceling an in-flight **streaming** assistant message. Policy choices:

- Finish streaming the assistant message, **then** prompt for approval before any tool runs (typical).
- Or cancel stream on first tool delta (unusual; not proposed).

## Requirements (Draft)

### Functional

1. **Configuration** — Global or per-agent policy: **`auto`** (default) vs **`approve_before_execute`** (names TBD). Optional future: per-tool or per-risk-tier rules.
2. **Pending state** — Durable or recoverable enough for restarts: session/conversation identity, pending **`tool_calls`**, snapshot of messages required to resume (or opaque checkpoint), expiry time.
3. **User experience** — Show **what** will execute (tool name + JSON args; redaction for secrets). **Approve**, **deny**, optional **approve all in this batch** for power users.
4. **Denial semantics** — Deterministic outcome for denied tools; model receives coherent **`tool`** outcomes so the next step is valid.
5. **Channels** — Either **desktop-first** (native or in-app UI) with other channels unchanged, or **full parity** (Telegram inline keyboards, Matrix patterns, Signal constraints) — **explicit scope decision** (see open questions).
6. **Delegation** — Define behavior for **`delegate_task`** and nested worker **`tool_calls`** (likely same policy as parent unless configured otherwise).
7. **Timeouts** — If the user never responds: deny, cancel, or leave pending (default should be documented; blocking forever is usually bad for automation).

### Non-functional

- **Security** — Pending approvals must not leak across sessions/conversations; validate approve/deny tokens (cryptographic nonce or server-side mapping).
- **Observability** — Structured logs for pending, approved, denied (lowercase messages per project conventions).
- **Testing** — Unit tests for state machine; integration tests for resume after deny/approve.

## Performance and Usability Implications

### Latency

- **Human response time dominates** when approval is on: seconds to hours. For interactive chat, perceived “slowness” is expected; for **headless** or **channel** workflows, this may be unacceptable unless async/background approval is acceptable.
- **No inherent extra model round-trip** for the approval step itself (unless a separate LLM-based risk summary is added — optional and costly).

### Token and context pressure (small / local models)

- **Denied tools** — The model may consume tokens explaining failure and replanning; weak models may spiral. Mitigations: short, consistent denial strings; optional **system** hint that denial is not an error requiring apology loops (adds a few tokens once per session, not per tool).
- **Split turns** — Do **not** by themselves duplicate the full history for each leg; storage efficiency matters if checkpoints are naive copies.
- **Many tool calls in one batch** — Large **`tool_calls`** payloads increase **assistant** message size; approval UI must summarize without blowing context when replaying — mostly a **UX** issue.

### CPU and memory

- **Pending state** — Bounded storage per pending approval; cleanup on timeout.
- **Async barrier** — Risk of **held** requests and **connection** timeouts on HTTP/WebSocket clients if the gateway blocks waiting for approval; may require **async job** model + client polling or notifications.

### Risk: “unusable” on small models

The epic acknowledges a **product risk**: approval does not fix **tool-calling reliability**. It adds **human judgment** before side effects. If the baseline model **already** fails to call tools correctly, operators may see:

- More **frustration** (approve → still wrong behavior after execution).
- **Deny** loops where the model keeps proposing bad calls.

So approval is a **safety gate**, not a substitute for **better models**, **narrower tool sets**, or **sandboxing**. In practice, the experience can diverge from IDE-centric agents (e.g. Claude Code): those systems are usually optimized for a single integrated UI/session and richer previews, while a Chai-style approval flow would likely require split-turn persistence/resume and may force additional recovery steps after denial—both of which can be rough on smaller local models. Document clearly so expectations stay realistic.

## Tradeoffs (Summary)

| Aspect | Auto-execute (today) | Approval |
|--------|----------------------|----------|
| Safety | Relies on model + tools design | Human veto before execution |
| Latency | Model + tool time only | Adds human wait; may add deny/retry model rounds |
| Complexity | Lower | State machine, UX per surface, failure modes |
| Small models | Tool loop fragility only | Same + possible deny/recovery churn |

## Related Work (Claude Code)

Claude Code-like agents are typically tightly integrated with a single development surface (IDE/terminal). When they implement safety confirmations, they’re usually inline and closely coupled to the same session context the operator sees (e.g. interactive diffs/previews, immediate “apply” actions), which keeps the “approve → execute → feedback” loop short and avoids (or minimizes) split-turn persistence.

Chai’s approval mechanism would need to generalize across its gateway/session model and potentially across multiple channels (Telegram, Matrix, Signal). That makes the UX more dependent on durable “pending approval” state and robust resume semantics. This difference is the main reason approval could become more restrictive (or feel unusable) with smaller local models: the model may already struggle with multi-step tool planning, and the approval workflow can add additional recovery complexity when denial occurs.

## Phases (Illustrative — Not Committed)

| Phase | Scope |
|-------|--------|
| 1. Design | ADR: split-turn vs async barrier; transcript rules; config schema; desktop vs all channels. |
| 2. Core (gateway + lib) | Pending store; resume API; policy hook in tool loop; tests. |
| 3. Desktop | UI for pending approvals; settings toggle. |
| 4. Channels (optional) | Telegram/Matrix/Signal UX for approve/deny if desired. |
| 5. Hardening | Timeouts, metrics, crash recovery, delegation semantics. |

## Non-Goals (Draft)

- **LLM-as-judge** for every tool call by default (optional future experiment).
- **Replacing** workspace sandboxing, allowlists, or least-privilege tool design.
- **Guaranteed** improvement in model reasoning — approval only gates **execution**.

## Open Questions

- **Scope** — Desktop-only first vs parity across **Telegram**, **Matrix**, **Signal** from day one?
- **Transcript** — Whether **`assistant` messages with tool_calls** are persisted **before** approval (visible in history) or only after approval (cleaner UX, harder replay).
- **Partial deny** — Single tool in a batch denied: allow partial execution and synthetic results for the rest?
- **`delegate_task`** — Block worker tools until orchestrator’s user approves, or separate policy?
- **Gateway API** — WebSocket/event for “pending approval” so UIs stay in sync without polling?

## Related Documents

- [VISION.md](../VISION.md) — Long-term security and privacy direction.
- [EPIC_ORCHESTRATION.md](EPIC_ORCHESTRATION.md) — Delegation and agent policy.
- [EPIC_MSG_CHANNELS.md](EPIC_MSG_CHANNELS.md) — Channel surfaces and shared inbound path.
- [AGENTS.md](../AGENTS.md) — Repository guidelines (logging style, architecture).
- Implementation touchpoints (when/if built): **`crates/lib/src/agent.rs`** (`ToolExecutor`, tool loop), **`crates/lib/src/gateway/server.rs`**, channel **`InboundMessage`** handling, **`crates/desktop`** for UI.
