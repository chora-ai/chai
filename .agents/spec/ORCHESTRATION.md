---
status: stable
---

# Orchestration and Delegation

This document describes how **orchestrator** and **worker** entries in **`config.json`** map to runtime behavior—especially **delegation** via the built-in **`delegate_task`** tool. For goals, phased implementation history, and optional backlog, see **[EPIC_ORCHESTRATION.md](../EPIC_ORCHESTRATION.md)**.

**Former tool name:** The same built-in tool was previously named **`chai_delegate`**. Older notes or logs may still say **`chai_delegate`**; behavior and config are unchanged aside from the tool name exposed to the model.

## Roles

- **Orchestrator** — Exactly one entry with **`"role": "orchestrator"`**. It runs the main session turn (default **`defaultProvider`** / **`defaultModel`** unless overridden per request).
- **Workers** — Zero or more entries with **`"role": "worker"`**. Each has an **`id`** used as **`workerId`** when delegating.

## Configuration Quick Reference

Canonical provider ids used in policy and catalogs: **`ollama`**, **`lms`**, **`vllm`**, **`nim`**, **`openai`**, **`hf`** (see [README.md](../../README.md), [PROVIDERS.md](PROVIDERS.md), and [MODELS.md](MODELS.md)).

### Orchestrator entry

| Key | Purpose |
|-----|---------|
| **`id`**, **`role`** | Identity; must include exactly one **`orchestrator`**. |
| **`defaultProvider`**, **`defaultModel`** | Main session defaults. |
| **`enabledProviders`** | Which provider stacks this agent may use (discovery and routing). |
| **`delegateAllowedModels`** | Optional allowlist of **`{ "provider", "model" }`**; optional **`local`**, **`toolCapable`** hints. When **non-empty**, every resolved **`delegate_task`** target (without **`workerId`**) must match a pair. When **omitted** or **empty**, only the orchestrator effective default **`provider`** / **`model`** is allowed for those delegations. |
| **`maxDelegationsPerTurn`** | Cap on **`delegate_task`** calls in a single orchestrator turn. |
| **`maxDelegationsPerSession`** | Cap on **successful** delegations per persisted session (requires session id on the gateway path). |
| **`maxDelegationsPerProvider`** | Per-session caps keyed by canonical provider id. |
| **`delegateBlockedProviders`** | Hard deny: **`delegate_task`** cannot target these providers. |
| **`delegationInstructionRoutes`** | Prefix-based defaults for **`instruction`**; see below. |

### Worker entry

| Key | Purpose |
|-----|---------|
| **`id`**, **`role`**, **`defaultProvider`**, **`defaultModel`**, **`enabledProviders`** | Same ideas as orchestrator; **`id`** is **`workerId`**. |
| **`delegateAllowedModels`** | When non-empty, narrows targets for delegations that use this **`workerId`**. When omitted or empty, only that worker's effective default **`provider`** / **`model`** is allowed. |

## Delegation Tool (`delegate_task`; formerly `chai_delegate`)

The orchestrator may call **`delegate_task`** to run a **subtask** on another provider and/or model:

| Argument | Role |
|----------|------|
| **`instruction`** | Required (non-empty after trim). User text for the worker turn. |
| **`workerId`** | Optional. Must match a **`role: worker`** entry; that worker’s defaults and allowlists apply unless overridden. |
| **`provider`** / **`model`** | Optional overrides for backend and model id after resolution. |

**Instruction routing (`delegationInstructionRoutes`)**

- Array of **`{ "instructionPrefix", "workerId"?, "provider"?, "model"? }`** on the **orchestrator** entry only.
- The first entry whose **`instructionPrefix`** matches the start of **`instruction`** (after trim) supplies any missing **`workerId`** / **`provider`** / **`model`** fields before resolution.

## Allowlists (`delegateAllowedModels`)

- On the **orchestrator** entry, when **non-empty**, every successful **`delegate_task`** resolution must match one **`{ "provider", "model" }`** pair exactly (after defaulting). Optional **`local`** and **`toolCapable`** are hints for catalog and UX only.
- On a **worker** entry, **`delegateAllowedModels`** narrows delegations that use that **`workerId`**. When **omitted** or **empty**, only that worker's resolved default **`provider`** / **`model`** is allowed.
- Empty or omitted orchestrator **`delegateAllowedModels`** restricts delegations **without** **`workerId`** to the orchestrator default pair only (in addition to **`enabledProviders`** and other policy).

## Worker Turn Behavior

- The worker receives the **same system context** as the main session (e.g. **`AGENTS.md`**, skills) and the **same skill tools** as the orchestrator.
- **`delegate_task`** is **not** offered on the worker turn (nested delegation is disabled).
- Implementation: gateway and WebSocket **`agent`** runs use **`DelegateContext`**; see **`crates/lib/src/orchestration/delegate.rs`**.

## Gateway Events

While connected to the gateway WebSocket, clients receive **`type`: `event`** frames with an **`event`** string and **`payload`**. Delegation uses:

| Event | Meaning |
|-------|---------|
| **`orchestration.delegate.start`** | Worker turn is about to run; payload includes resolved **`provider`**, **`model`**, optional **`workerId`**, **`sessionId`** when known. |
| **`orchestration.delegate.complete`** | Worker turn finished successfully. |
| **`orchestration.delegate.error`** | Resolution failed (e.g. unknown worker, allowlist) or the worker turn failed; payload may include **`error`**, optional **`workerId`**. |
| **`orchestration.delegate.rejected`** | Delegation not started due to a **limit**; payload includes **`reason`** (see below), optional **`maxDelegationsPerTurn`**, **`workerId`**, **`sessionId`**. |

**`orchestration.delegate.rejected` reasons** (stable strings for clients):

- **`max_delegations_per_turn`** — **`maxDelegationsPerTurn`** exceeded in this orchestrator turn.
- **`max_delegations_per_session`** — **`maxDelegationsPerSession`** would be exceeded after a successful delegation.
- **`max_delegations_per_provider`** — Per-provider session cap would be exceeded.

Constants and emission logic live in **`crates/lib/src/orchestration/delegate.rs`**. **Chai Desktop** renders these in the chat timeline (**`state/chat.rs`**, **`screens/chat.rs`**).

## Gateway `status` — `orchestrationCatalog`

The **`status`** WebSocket method returns **`orchestrationCatalog`**: a merged array of **`{ provider, model, discovered, local?, toolCapable? }`** built from per-provider discovery plus any **`delegateAllowedModels`** pairs not present in discovery (**`discovered: false`**). Hints attach when the pair matches an allowlist entry. See **`crates/lib/src/orchestration/catalog.rs`**.

## Gateway `status` — `workers`

The **`status`** payload includes **`workers`**: an array of **`{ id, defaultProvider, defaultModel }`** for each non-empty worker id in config, using the same effective **`(provider, model)`** resolution as **`delegate_task`** when **`provider`** / **`model`** are omitted (see **`crates/lib/src/orchestration/workers_context.rs`**). Empty when no workers are configured. **Chai Desktop** **Status** lists these under **Agents**.

## Out of Scope for This Spec

Interactive **human approval** queues, **sandboxing**, and **arbitrary exec approval** are not described here; configuration supports **deny** and **caps** instead. New **provider backends** are tracked under **[EPIC_API_ALIGNMENT.md](../EPIC_API_ALIGNMENT.md)**.

## Related Documents

| Document | Purpose |
|----------|---------|
| **[EPIC_ORCHESTRATION.md](../EPIC_ORCHESTRATION.md)** | Epic: goals, config evolution, implementation phases, requirements checklist, closure, follow-ups. |
| **[PROVIDERS.md](PROVIDERS.md)** | Provider ids, configuration, API comparison. |
| **[MODELS.md](MODELS.md)** | Model ids, repository inventory, tool-fit notes. |
