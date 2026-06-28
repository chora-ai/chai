---
status: in-progress
---

# Epic: Multiple Orchestrator Configuration

**Summary** — Allow users to configure multiple orchestrator agents in a single profile and switch between them from the desktop chat screen, enabling different orchestrator roles (e.g., developer, reviewer) that share the same workers and sandbox while maintaining separate agent context.

**Status** — **In progress.** Phase 1 (config layer) delivered. Phases 2–4 remain.

## Problem Statement

Today, the `agents` array in `config.json` enforces **exactly one orchestrator**. All conversations go through that single orchestrator. When a user wants different orchestrator behaviors — for example, a developer orchestrator that writes code and a reviewer orchestrator that audits it — they must either:

1. **Switch profiles** — Create separate profiles (e.g., `developer`, `reviewer`) with duplicated workers, providers, and sandbox configuration. This is heavy: the user maintains two full profiles, each with their own `config.json`, workers, and skill lockfiles. Workers and sandbox directories are duplicated, not shared.
2. **Manually edit `AGENT.md`** — Change the orchestrator's instructions between sessions, which is fragile and loses the previous configuration.
3. **Use a single orchestrator with broad instructions** — Ask one orchestrator to handle multiple roles, which produces lower-quality output than an agent with focused context.

The core tension: **profile switching changes everything** (config, workers, sandbox, skills, sessions), but the user only wants to change the orchestrator's context and role while keeping the same workers, sandbox, and providers.

### The Sandbox Sharing Problem

Profiles isolate sandboxes: `~/.chai/profiles/developer/sandbox/` and `~/.chai/profiles/reviewer/sandbox/` are separate directories. But a reviewer orchestrator needs to read the same files the developer orchestrator wrote. With separate profiles, the reviewer has no access to the developer's sandbox — the user would need to copy or symlink files between profiles.

What the user really wants: a **shared sandbox** where a developer orchestrator writes code, then a reviewer orchestrator reads and critiques it, all within the same profile and filesystem context.

## Goal

Allow users to configure multiple orchestrator agents within a single profile, switch between them from the desktop chat screen (similar to the profile ComboBox), and have each orchestrator use its own `AGENT.md` context while sharing the same worker definitions, sandbox, and providers. Each orchestrator can optionally filter which workers it has access to via `enabledWorkers`, so that a reviewer orchestrator sees only the workers it needs while a developer orchestrator sees all.

The initial implementation supports **sequential use** — the user selects an orchestrator before starting a session, and switching orchestrators starts a new session. A future implementation could support **simultaneous use** (split screen, read-only second orchestrator), but that is explicitly out of scope for now.

## Current State

### Configuration Enforcement

The `agents` array in `config.json` is parsed by `agents_from_array()` in `crates/lib/src/config.rs`. After Phase 1, the function enforces:

- **At least one orchestrator**: Multiple entries with `role: "orchestrator"` are accepted. No orchestrator at all produces the error `"agents array must include at least one entry with role \"orchestrator\""`.
- **OrchestratorConfig struct**: After parsing, each orchestrator's fields are stored in an `OrchestratorConfig` struct within `AgentsConfig.orchestrators: Vec<OrchestratorConfig>`. Workers go into `workers: Option<Vec<WorkerConfig>>`. Accessor methods (`default_orchestrator()`, `orchestrator(id)`, `orchestrator_ids()`) provide ergonomic access.
- **`enabledWorkers`**: Optional `Vec<String>` on `OrchestratorConfig`. Rejected on worker entries (orchestrator-only). Unknown worker IDs produce a validation error.

**Before Phase 1**: The function enforced exactly one orchestrator and flattened the orchestrator's fields into top-level `AgentsConfig` fields.

### Gateway Runtime

At startup (`crates/lib/src/gateway/server.rs`, lines 777–910), the gateway builds:

- **One system context** for the orchestrator (from `AGENT.md` + workers roster + skills).
- **One tool list** for the orchestrator (skill tools + `delegate_task` if workers exist).
- **One tool executor** for the orchestrator.
- **Per-worker runtimes** (`WorkerDelegateRuntime`) with each worker's own system context, tools, and executor.

All of this is stored in `GatewayState` and referenced when the `agent` RPC is handled. The `agent` RPC has no parameter for selecting which orchestrator to use — it always uses the single configured orchestrator.

### Desktop Chat Screen

The chat screen (`crates/desktop/src/app/screens/chat.rs`) has:

- **Provider ComboBox** — selects among enabled providers.
- **Model ComboBox** — selects among models for the chosen provider.
- **No orchestrator selector** — chat always targets the single orchestrator.

The Agent and Tools screens have a shared `dashboard_agent_id` ComboBox that iterates all agents from `status.agents`, appending ` — orchestrator` or ` — worker` suffixes. This pattern is the natural starting point for a chat-screen orchestrator ComboBox.

### Sessions

Sessions are agent-agnostic — they store message history and delegation counters, not the orchestrator identity. The orchestrator context is bound to the turn at invocation time via `DelegateContext`, not stored in the session. This means switching orchestrators mid-session would require either:

1. Starting a new session (clean, simple — the initial implementation).
2. Injecting the new orchestrator's system context into the existing session (complex, risks confusing the model with a mid-conversation role change).

### ADR Context

The ORCHESTRATION ADR (`base/adr/ORCHESTRATION.md`) explicitly chose the single `agents` array (Option A) over separate `orchestrator` + `workers` keys (Option B) partly because it is *"more flexible if more roles appear or multiple orchestrators are allowed in the future."* This epic is the intended extension path.

## Scope

### In Scope

- **Multiple orchestrator entries** in the `agents` array — relax the "exactly one" constraint to "at least one."
- **Orchestrator ComboBox on the chat screen** — filtered to orchestrator agents only, similar to the existing Agent/Tools ComboBox pattern.
- **Disabled during active session** — the ComboBox is interactive only when no session is active; switching orchestrators starts a new session.
- **Active/inactive labeling** — agents in the Agent/Tools ComboBoxes show "active" or "inactive" based on the selected orchestrator.
- **Shared workers** — all orchestrators in a profile share the same worker definitions, with optional per-orchestrator filtering via `enabledWorkers` (absent = all workers; present = listed workers only).
- **Shared sandbox and providers** — orchestrators differ only in their agent context (`AGENT.md`), `enabledSkills`, `enabledWorkers`, `contextMode`, `defaultProvider`/`defaultModel`, and delegation policy. Workers, sandbox, providers, and channels are profile-level resources.

### Out of Scope

- **Simultaneous orchestrator sessions** (split screen, concurrent runs) — significant complexity around shared git directories, read/write directories, and session management. Could be a future epic.
- **Per-orchestrator worker definitions** — each orchestrator could define its own workers with different `defaultProvider`/`defaultModel`/`enabledSkills`. This adds significant configuration complexity and is not needed for the initial use case. Per-orchestrator worker *visibility* is handled by `enabledWorkers` (a subset filter on shared workers), which is in scope.
- **Hot-switching mid-session** — changing the orchestrator while a session is active. Too disruptive to the conversation; starting a new session is cleaner.

## Design

### Configuration Schema Change

#### Current: Exactly One Orchestrator

```json
{
  "agents": [
    {
      "id": "developer",
      "role": "orchestrator",
      "defaultProvider": "ollama",
      "defaultModel": "qwen3:32b",
      "enabledProviders": ["ollama", "lms"],
      "enabledSkills": ["files", "git-read", "git"],
      "contextMode": "readOnDemand",
      "maxToolLoopsPerTurn": 50,
      "maxDelegationsPerTurn": 3
    },
    {
      "id": "engineer",
      "role": "worker",
      "defaultProvider": "lms",
      "defaultModel": "qwen3-30b-a3b"
    }
  ]
}
```

#### Proposed: Multiple Orchestrators

```json
{
  "agents": [
    {
      "id": "developer",
      "role": "orchestrator",
      "defaultProvider": "ollama",
      "defaultModel": "qwen3:32b",
      "enabledProviders": ["ollama", "lms"],
      "enabledSkills": ["files", "git-read", "git"],
      "enabledWorkers": ["engineer", "reader"],
      "contextMode": "readOnDemand",
      "maxToolLoopsPerTurn": 50,
      "maxDelegationsPerTurn": 3
    },
    {
      "id": "reviewer",
      "role": "orchestrator",
      "defaultProvider": "ollama",
      "defaultModel": "qwen3:32b",
      "enabledProviders": ["ollama", "lms"],
      "enabledSkills": ["files", "git-read"],
      "enabledWorkers": ["reader"],
      "contextMode": "full",
      "maxToolLoopsPerTurn": 30
    },
    {
      "id": "engineer",
      "role": "worker",
      "defaultProvider": "lms",
      "defaultModel": "qwen3-30b-a3b"
    },
    {
      "id": "reader",
      "role": "worker",
      "defaultProvider": "ollama",
      "defaultModel": "qwen3-30b-a3b"
    }
  ]
}
```

Key observations:
- Each orchestrator has its own `defaultProvider`, `defaultModel`, `enabledSkills`, `contextMode`, and delegation policy.
- `enabledProviders` is per-orchestrator — this already makes sense since it controls which providers are available for the orchestrator's main session and for its workers.
- `enabledWorkers` is per-orchestrator — absent means all workers; present means only the listed workers. The developer orchestrator can delegate to both `engineer` and `reader`; the reviewer can only delegate to `reader`.
- Workers are defined once and shared — both orchestrators reference the same `reader` worker definition.

### AgentsConfig Refactoring

The current `AgentsConfig` struct flattens the single orchestrator into top-level fields. With multiple orchestrators, this must change. The refactoring has two dimensions: the on-disk format and the internal representation.

#### On-Disk Format: Unified Array (Decision Confirmed)

**Decision: Keep the unified `agents` array.** This is consistent with the ORCHESTRATION ADR's Option A choice. The on-disk format is a flat array with `role` discriminator — no separate `orchestrators` key. Only the internal representation and validation rules change.

#### Internal Representation: `Vec<OrchestratorConfig>` with Accessor Methods

Three approaches were evaluated:

| Approach | Structure | Assessment |
|----------|-----------|------------|
| Direct field access | `Vec<OrchestratorConfig>`, consumers index directly | Right data model, poor ergonomics (`agents.orchestrators[0]` is verbose and fragile) |
| **Accessor methods** | `Vec<OrchestratorConfig>` + `default_orchestrator()` / `orchestrator(id)` | ✅ **Chosen** — normalized data + ergonomic access |
| Denormalized | Keep flat fields + add `Vec<OrchestratorConfig>` | Rejected — two sources of truth, consistency risk |

**Decision: `Vec<OrchestratorConfig>` with accessor methods.**

Principles that drove the decision:

1. **Normalized data model** — Each piece of data stored exactly once. Denormalization creates consistency risks and maintenance burden.
2. **Symmetric types** — `OrchestratorConfig` and `WorkerConfig` are parallel types. The current asymmetry (orchestrator fields promoted to the parent, worker fields nested) exists only because there was exactly one orchestrator.
3. **Ergonomic access for the common case** — Most code paths operate on a specific orchestrator (the active one). The API should make it easy to get "the orchestrator for this turn" without unwrapping a Vec.
4. **Explicit over implicit** — Which orchestrator is being used should be explicit at every call site, not inherited from flat top-level fields. The current pattern of reading `agents.default_provider` and getting "the orchestrator's default provider" is implicit — it works because there's only one.
5. **On-disk format stability** — The `agents` array with `role` discriminator is clean. The refactoring changes internal representation, not the on-disk format.

The accessor methods provide the ergonomics of flat-field access for the common case (`agents.default_orchestrator().default_provider`) while making the "which orchestrator?" question explicit. The denormalized approach (keeping flat fields + adding the Vec) was rejected because the active orchestrator's data would be stored in two places, and every future consumer would need to decide which source to read — a consistent bug surface.

**Why not prioritize internal API backward compatibility**: `AgentsConfig` is a library-internal type, not a public API. The on-disk format is unchanged, so user configs are unaffected. The breaking change is confined to internal consumers that must be updated regardless of which approach is chosen.

```rust
pub struct AgentsConfig {
    pub orchestrators: Vec<OrchestratorConfig>,  // at least one
    pub workers: Option<Vec<WorkerConfig>>,
}

impl AgentsConfig {
    /// The default (first) orchestrator. Always present (validation ensures ≥1).
    pub fn default_orchestrator(&self) -> &OrchestratorConfig {
        &self.orchestrators[0]
    }

    /// Look up an orchestrator by ID. Returns the default if `id` is None.
    pub fn orchestrator(&self, id: Option<&str>) -> Result<&OrchestratorConfig, String> {
        match id {
            None => Ok(self.default_orchestrator()),
            Some(id) => self.orchestrators.iter().find(|o| o.id == id)
                .ok_or_else(|| format!("unknown orchestrator id: {id}")),
        }
    }

    pub fn orchestrator_ids(&self) -> impl Iterator<Item = &str> {
        self.orchestrators.iter().map(|o| o.id.as_str())
    }
}

pub struct OrchestratorConfig {
    pub id: String,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub enabled_providers: Option<Vec<String>>,
    pub enabled_skills: Option<Vec<String>>,
    pub enabled_workers: Option<Vec<String>>,
    pub context_mode: Option<SkillContextMode>,
    pub max_tool_loops_per_turn: Option<u32>,
    pub max_delegations_per_turn: Option<usize>,
    pub max_delegations_per_session: Option<usize>,
    pub max_delegations_per_worker: Option<HashMap<String, usize>>,
}
```

### Gateway State Changes

Today, `GatewayState` holds:

- `system_context: Option<String>` — the single orchestrator's system prompt.
- `tools_list: Vec<ToolDefinition>` — the single orchestrator's tool list.
- `tool_executor: Arc<dyn ToolExecutor>` — the single orchestrator's tool executor.

With multiple orchestrators, these become per-orchestrator:

```rust
pub struct GatewayState {
    pub orchestrator_runtimes: HashMap<String, OrchestratorRuntime>,
    // ... other fields unchanged
}

pub struct OrchestratorRuntime {
    pub system_context: Option<String>,
    pub tools_list: Vec<ToolDefinition>,
    pub tool_executor: Arc<dyn ToolExecutor>,
}
```

The gateway would build an `OrchestratorRuntime` for each orchestrator at startup (same logic as today, just repeated per orchestrator).

### Agent RPC Extension

The `agent` RPC currently has no parameter to select which agent handles the turn. The simplest extension:

```json
{
  "method": "agent",
  "params": {
    "message": "...",
    "sessionId": "...",
    "provider": "...",
    "model": "...",
    "agentId": "reviewer"
  }
}
```

When `agentId` is omitted, the first (default) orchestrator is used. When provided, the gateway looks up the matching `OrchestratorRuntime` and uses its system context, tools, and executor for the turn. The `agentId` must reference an agent with role `orchestrator`; the gateway rejects IDs that refer to workers or don't exist.

### Desktop Chat Screen: Orchestrator ComboBox

A new ComboBox on the chat screen, positioned alongside the existing Provider and Model ComboBoxes:

```
[ /new ] [ /help ] [ Orchestrator ▾ ] [ Provider ▾ ] [ Model ▾ ] [ Stop ] [ Send ]
```

**Behavior**:
- Populated from `status.agents` filtered to `role === "orchestrator"`.
- Selected orchestrator determines which provider/model defaults are used (and filters the Provider/Model ComboBoxes accordingly).
- **Disabled during active session** — `chat_turn_receiver.is_some()` or `chat_session_id.is_some()`.
- Switching orchestrators while no session is active is a no-op for the current session (there isn't one). The next message will use the selected orchestrator.
- When only one orchestrator is configured, the ComboBox is still visible but disabled — no selection to make.

### Agent/Tools Screen: Active/Inactive Labeling

The existing `dashboard_agent_id` ComboBox on the Agent and Tools screens currently labels orchestrator entries with ` — orchestrator`. This would change to:

- **Active orchestrator**: `developer — orchestrator (active)`
- **Inactive orchestrator**: `reviewer — orchestrator (inactive)`
- **Workers**: unchanged (`engineer — worker`)

"Active" means the orchestrator currently selected on the chat screen. This gives the user a clear view of which orchestrator will handle the next message.

### Workers Roster in System Context

`build_workers_context()` generates the `## Workers` section in the orchestrator's system prompt. Today, it iterates all workers unconditionally. With `enabledWorkers`, it must filter the roster to only include workers in the orchestrator's `enabledWorkers` list (when set). When `enabledWorkers` is absent, all workers are included — identical to today's behavior.

The orchestrator's `AGENT.md` can still include role-specific instructions about how to use workers. For example, the developer orchestrator might instruct: *"Delegate implementation tasks to the engineer worker."* The reviewer orchestrator might instruct: *"Delegate file reads to the reader worker, but do not delegate any write operations."*

### Shared Workers: Design Decision

**Decision: All orchestrators share the same worker definitions, with optional per-orchestrator filtering via `enabledWorkers`.**

Rationale:
- Workers are profile-level resources, just like providers and the sandbox. They are defined once in `config.json` and available to all orchestrators.
- Per-orchestrator worker *definitions* (separate `WorkerConfig` entries per orchestrator) would add significant configuration complexity (which workers belong to which orchestrator? what if two orchestrators need the same worker with different models?) without a clear use case.
- However, per-orchestrator worker *visibility* is useful: a reviewer orchestrator should not see or delegate to workers it doesn't need. `enabledWorkers` provides this as an optional subset filter on the shared worker pool — the same pattern as `enabledProviders` (filter on provider pool) and `enabledSkills` (filter on skill catalog).

#### `enabledWorkers` Design

`enabledWorkers` is an optional field on the orchestrator entry in `config.json`:

```json
{
  "id": "reviewer",
  "role": "orchestrator",
  "enabledWorkers": ["reader"],
  ...
}
```

**Semantics**:
- **Absent or `null`** — all profile workers are available (default; backward compatible).
- **Present** — only workers with matching IDs are visible and delegatable.

**Enforcement at two layers**:
1. **System prompt** — `build_workers_context()` filters the `## Workers` roster to only include workers in the orchestrator's `enabledWorkers` (when set). The model never sees excluded workers and therefore never attempts to delegate to them.
2. **Delegation** — `resolve_delegate_target()` rejects delegation to a worker not in the orchestrator's `enabledWorkers` (when set), mirroring the existing `enabledProviders` check. This is a safety net for edge cases where the model attempts to delegate by ID without the worker being in its roster.

**Why this is not per-orchestrator worker definitions**: `enabledWorkers` is a *subset filter*, not a *separate definition*. Workers are defined once in the `agents` array with their `defaultProvider`, `defaultModel`, `enabledSkills`, and `contextMode`. Different orchestrators select different subsets of the same workers — no duplication, no configuration explosion. If an orchestrator needs the "same" worker with a different model, that is a genuinely separate worker definition and should be a separate entry in the `agents` array.

**Why delegation-time filtering, not startup-time filtering**: All worker runtimes are still built at startup (they're the same regardless of which orchestrator delegates to them). Filtering happens at the system-prompt and delegation layers. This avoids complicating the `worker_delegate_runtimes` HashMap with per-orchestrator views and mirrors how `enabledProviders` works (all providers are still configured; only discovery and delegation are filtered).

**Interaction with `enabledProviders`**: The two filters compose naturally. If orchestrator A has `enabledWorkers: ["engineer"]` and `enabledProviders: ["ollama"]`, but the engineer worker uses `defaultProvider: "lms"`, the existing `enabledProviders` check in `resolve_delegate_target()` catches this. No special interaction logic is needed.

**Validation**: Referenced worker IDs must exist in the profile's `agents` array. Unknown IDs produce a validation error at config load time, matching the validation pattern for `enabledProviders` and `enabledSkills`.

### Status API Changes

The `status.agents` array already includes all agents with their `role`. For multiple orchestrators, the payload would naturally include multiple entries with `role: "orchestrator"`. No schema change is needed.

The desktop currently resolves a single `orchestrator_id` from the first orchestrator entry. This would change to tracking all orchestrator IDs and which one is "active" (selected for chat). The active orchestrator could be stored as client-side state in the desktop app, or the gateway could track it as part of the session.

**Decision: Client-side state.** The desktop tracks `active_orchestrator_id` locally. The gateway doesn't need to know which orchestrator is "active" — it just needs to know which orchestrator to use when the `agent` RPC arrives, which is communicated via the `agentId` parameter.

### Profile vs. Orchestrator Switching

| Aspect | Profile Switch | Orchestrator Switch |
|--------|---------------|-------------------|
| Scope | Everything: config, workers, sandbox, providers, channels, skills | Agent context, skills, worker visibility, provider/model defaults, delegation policy |
| Workers | Different per profile | Shared (with optional `enabledWorkers` filter) |
| Sandbox | Isolated per profile | Shared |
| Gateway restart | Required | Not required |
| Session | All sessions lost | New session (old sessions remain accessible in history) |
| Configuration | Separate `config.json` files | Same `config.json`, different orchestrator entries |

Orchestrator switching is a **lighter-weight alternative** to profile switching when the user only wants to change the agent's role, not the entire environment.

## Requirements

- [x] **Multiple orchestrator entries** — The `agents` array accepts multiple entries with `role: "orchestrator"`. Validation requires at least one (not exactly one).
- [x] **Backward-compatible defaults** — When `agents` is omitted, the default remains a single orchestrator with id `"orchestrator"`.
- [x] **`AgentsConfig` refactored** — Replace flat top-level orchestrator fields with `Vec<OrchestratorConfig>`. On-disk format unchanged.
- [x] **`enabledWorkers` on `OrchestratorConfig`** — Optional `Vec<String>`; when absent, all profile workers are available; when present, only listed workers are visible and delegatable. Follows the same pattern as `enabledProviders` and `enabledSkills`.
- [x] **`enabledWorkers` validation** — Referenced worker IDs must exist in the profile's `agents` array. Unknown IDs produce a validation error.
- [ ] **`enabledWorkers` system prompt filtering** — `build_workers_context()` only includes workers in the orchestrator's `enabledWorkers` (when set) in the `## Workers` roster.
- [ ] **`enabledWorkers` delegation enforcement** — `resolve_delegate_target()` rejects delegation to a worker not in the orchestrator's `enabledWorkers` (when set), mirroring the `enabledProviders` check.
- [ ] **Per-orchestrator runtime** — Gateway builds `OrchestratorRuntime` (system context, tools, executor) for each orchestrator at startup.
- [ ] **`agent` RPC `agentId` parameter** — Optional; when omitted, the first orchestrator is used.
- [ ] **Orchestrator ComboBox on chat screen** — Filtered to orchestrators; disabled during active session; switches start a new session.
- [ ] **Provider/Model ComboBox cascade** — Switching orchestrators updates the Provider and Model ComboBoxes to reflect the new orchestrator's defaults.
- [ ] **Active/inactive labeling** — Agent and Tools screen ComboBoxes show `(active)` / `(inactive)` for orchestrators.
- [ ] **`enabledProviders` enforcement** — Worker delegation is rejected when the worker's provider is not in the requesting orchestrator's `enabledProviders`.
- [ ] **Single-orchestrator backward compatibility** — When only one orchestrator is configured, desktop and CLI behavior is identical to today (ComboBox hidden or disabled; `--agent` flag has no effect).
- [ ] **CLI `--agent` flag** — The `agent` CLI command accepts an optional `--agent <id>` flag that selects which orchestrator to use, equivalent to the desktop ComboBox selection.
- [ ] **Spec updates** — Update `spec/AGENTS.md`, `spec/ORCHESTRATION.md`, `spec/CONFIGURATION.md`, `spec/DESKTOP.md`, `spec/CLI.md` to reflect multiple orchestrators, `enabledWorkers`, and the `--agent` flag.
- [ ] **ADR update** — Update `adr/ORCHESTRATION.md` to note that the "exactly one" constraint has been relaxed.

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Config layer: relax validation, refactor `AgentsConfig`, add `OrchestratorConfig` type (including `enabledWorkers`) | **Complete** |
| 2 | Gateway runtime: per-orchestrator `OrchestratorRuntime`, `agent` RPC `agentId` parameter, `enabledWorkers` system prompt filtering and delegation enforcement, `enabledProviders` enforcement for shared workers | Not started |
| 3 | Desktop UI + CLI: orchestrator ComboBox on chat screen, `--agent` CLI flag, active/inactive labeling, provider/model cascade | Not started |
| 4 | Spec and ADR updates: document new behavior in all affected specs | Not started |

## Open Questions

### 1. What Happens When Switching Orchestrators With an Active Session?

**Proposed answer**: The ComboBox is disabled during an active session. The user must start a new session (`/new`) before switching. This is the simplest and most predictable behavior.

If the user has an active session with orchestrator A, starts a new session, and then selects orchestrator B, the new session uses orchestrator B. The previous session (with orchestrator A) remains in the session history.

### 2. Should Each Orchestrator Have Its Own Provider/Model Defaults?

**Proposed answer**: Yes. Each orchestrator entry already supports `defaultProvider` and `defaultModel`. When the user switches orchestrators on the chat screen, the Provider and Model ComboBoxes should update to reflect the new orchestrator's defaults (if configured), while still allowing per-turn overrides.

### 3. How Does `enabledProviders` Interact With Shared Workers?

**Proposed answer**: Each orchestrator's `enabledProviders` controls which providers are available for its own turns and for worker delegations it initiates. A worker with `defaultProvider: "lms"` can only be delegated to by orchestrators that have `"lms"` in their `enabledProviders`. The gateway should emit `orchestration.delegate.error` when an orchestrator attempts to delegate to a worker whose provider is not in the orchestrator's `enabledProviders`.

### 4. Should the Orchestrator ID Be Stored in the Session?

**Proposed answer**: Not in the initial implementation. Sessions are agent-agnostic and adding orchestrator identity would complicate the session model. Instead, the orchestrator used for a session is implicit from the session history — the system context is reconstructed each turn. If we later want to display "which orchestrator created this session" in the UI, we can add it as session metadata without changing the gateway's session model.

### 5. What About the Default When `agents` Is Omitted?

**Proposed answer**: When the `agents` array is omitted (or `null`), the default remains a single orchestrator with id `"orchestrator"` — backward compatible. When an `agents` array is provided, it must contain at least one orchestrator (no longer "exactly one").

### 6. How Does This Affect Channel-Bound Sessions?

**Proposed answer**: Channel messages (Telegram, etc.) currently go to the single orchestrator. With multiple orchestrators, channels need a way to select which orchestrator handles incoming messages. The simplest approach: channels always use the **first** (default) orchestrator. Channel-specific orchestrator routing is a separate feature.

## Follow-ups

- **Simultaneous orchestrator sessions** — Split-screen mode with two orchestrators running concurrently. Requires investigation into shared git directory access and desktop UI layout.
- **Per-orchestrator channel routing** — Allow channels to specify which orchestrator handles their messages.

## Related Epics and Docs

| Document | Purpose |
|----------|---------|
| [adr/ORCHESTRATION.md](../adr/ORCHESTRATION.md) | Architectural decision for the orchestrator–worker model (Option A was chosen partly for this extension) |
| [spec/AGENTS.md](../spec/AGENTS.md) | Per-agent context directories, skill configuration, and tool lists |
| [spec/ORCHESTRATION.md](../spec/ORCHESTRATION.md) | Delegation semantics, worker turn behavior, `delegate_task` |
| [spec/CONFIGURATION.md](../spec/CONFIGURATION.md) | `config.json` agent entries and fields |
| [spec/DESKTOP.md](../spec/DESKTOP.md) | Desktop chat screen, Agent/Tools screens, ComboBox patterns |
| [spec/PROFILES.md](../spec/PROFILES.md) | Profile directory structure and switching |
| [epic/PARALLEL_WORKFLOWS.md](PARALLEL_WORKFLOWS.md) | Parallel delegation (orthogonal but complementary) |
