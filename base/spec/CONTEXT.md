---
status: stable
---

# Context on Every Turn

This document describes the **context** the model receives for a turn, as implemented in `crates/lib` (gateway `server.rs`, `orchestration/workers_context.rs`, and skills loading). **Orchestrator** and **worker** turns use **separate** static strings and tool lists (per-agent **`AGENT.md`**, **`enabledSkills`**, **`contextMode`**); see **[AGENTS.md](AGENTS.md)**. This spec is kept in sync with the code.

## Turn vs Session

- **Session messages** — One conversation: a `session_id`, its message history (user/assistant/tool), and its binding to a channel+conversation (e.g. one chat). It lasts across many user messages. `/new` creates a new session and binds it so the next message has empty history.
- **Turn** — One run of the agent for a single user message: load that session's history, build the system + messages, call the model (and the tool loop if there are tool calls), then produce one assistant reply. One user message ⇒ one turn; a session has many turns over time.

## When It Is Built

- **Skill discovery** — At gateway startup, packages are loaded from **`~/.chai/skills`** only (see **`config::default_skills_dir`** / **`skills::load_skills`**). **Enablement** is **not** global: each agent entry's **`enabledSkills`** list selects which discovered packages apply to **that** agent. Missing or empty **`enabledSkills`** ⇒ **no** skill tools and **no** skill-derived inlined context for that agent.
- **Orchestrator static context** — Composed once in **`run_gateway`**: **`AGENT.md`** from the orchestrator **agent context directory** (**`orchestrator_context_dir`** → **`<profileRoot>/agents/<orchestratorId>/AGENT.md`**), then optional **`## Workers`** roster from **`build_workers_context`**, then orchestrator skills via **`build_skill_context_full`** or **`build_skill_context_compact`** according to the **orchestrator** entry's **`contextMode`**. Stored in **`GatewayState.system_context`**.
- **Worker static context** — For each **`role: worker`** entry, a **`WorkerDelegateRuntime`** is built: **`AGENT.md`** from **`worker_context_dir`** (**`<profileRoot>/agents/<workerId>/`**), worker-filtered skills, and **no** **`## Workers`** block (**`build_worker_system_context`**). Cached per worker id until restart.
- **Tools** — **Per agent** at startup: skill tools from **`tools.json`** for that agent's enabled packages; optional **`read_skill`** when that agent's **`contextMode`** is **`readOnDemand`** and at least one skill is enabled. The **orchestrator** list is merged with **`delegate_task`** via **`merge_delegate_task`** when workers exist. **Worker** lists omit **`delegate_task`**. The same prebuilt list is sent on every turn for that role.

After skill loading and config resolution, the gateway runs startup validation (lockfile verification and capability-tier checks) before accepting the configuration. See [SKILL_PACKAGES.md](SKILL_PACKAGES.md).

For a **new session**, when the user sends a message, the gateway sends the provider a request with two separate top-level fields:

1. **Messages** — An array containing:
   - A **system message** at position 0 (content = static context as below, built at startup, injected fresh every turn, never persisted in the session store).
   - **Session history** — All persisted messages for that `session_id`: `user`, `assistant` (with `tool_calls`), and `tool` (results). Loaded from the session store every turn.
2. **Tools** — A separate top-level array of JSON tool schemas for the LLM backend (see [§3](#3-tools)). Built at startup, sent unchanged on every turn. Tool schemas are never part of the messages array.

## 1. System Message Content

**Orchestrator** — Static string from **`build_system_context(agent_ctx, skills, context_mode, agents, skill_catalog)`**. **Worker** — Static string from **`build_worker_system_context(agent_ctx, skills, context_mode)`** (same skill builders, **no** workers roster). The static string is built once at gateway startup and injected as a `system`-role message at position 0 of the messages array on every turn. It is **not** persisted in the session store — it is reconstructed each turn from the cached startup string, so it never accumulates in the history.

### Build Order (Orchestrator)

1. **Agent context** — Contents of **`AGENT.md`** at **`<profileRoot>/agents/<orchestratorId>/AGENT.md`**. Trimmed. Omitted if missing or empty.
2. **`"\n\n"`** — Only if agent context was non-empty.
3. **Workers** — If **`config.agents.workers`** is non-empty, **`build_workers_context(agents, skill_catalog)`** appends a **`## Workers`** section (see [Workers Section](#workers-section-build_workers_context) below). Omitted entirely when there are no workers.
4. **`"\n\n"`** — Only if the workers section was non-empty.
5. **Skills** — From **`build_skill_context_full`** (**`full`**) or **`build_skill_context_compact`** (**`readOnDemand`**) using **only** packages whose names are in the **orchestrator** **`enabledSkills`** list, or empty if that list is missing/empty or none match disk.

### Build Order (Worker)

1. **Agent context** — That worker's **`AGENT.md`** at **`<profileRoot>/agents/<workerId>/AGENT.md`**. No **`## Workers`** section.
2. **Skills** — Same builders as the orchestrator, but filtered by **that worker's** **`enabledSkills`** and **`contextMode`**.

### Skill Context Mode

- **`full`** — Each enabled skill is inlined under a shared **`## Skills`** header with guidance bullets (call **`read_skill`**, share skills/tools, ask the user to choose), then **`### <skill name>`** per skill: optional description, then **`SKILL.md`** body with frontmatter stripped (**`strip_skill_frontmatter`**).
- **`readOnDemand`** — Same **`## Skills`** header and guidance bullets, then **`Available skills:`** with one line per skill: **`- \`<name>\` — <description>`** (no full bodies in system message). The model uses the **`read_skill`** tool to load full **`SKILL.md`** when needed.

### Skill Context — Full Mode (`build_skill_context_full`)

- If there are no skills, returns an empty string.
- Otherwise: **`## Skills`**, blank line, intro line (**`You have skills. Skills have tools.`**), blank line, then for each skill in load order: **`### `** + name, blank line, optional description + blank lines, **`--- SKILL.md (BOF) ---`**, blank line, **`strip_skill_frontmatter(content)`**, **`--- SKILL.md (EOF) ---`**, blank line between skills.

### Skill Context — Read-on-Demand Mode (`build_skill_context_compact`)

- If there are no skills, returns an empty string.
- Otherwise: **`## Skills`**, guidance bullets, **`Available skills:`**, then for each skill: **`- \`<name>\` — `** + description or **`(no description)`**.

### Workers Section (`build_workers_context`)

- Empty string if **`agents.workers`** is missing or empty.
- Otherwise:
```
## Workers

You are the orchestrator agent. You have worker agents.

You can call `delegate_task` to delegate a task to a worker agent.

The worker does not share session history — each worker turn begins with no history.

`delegate_task` calls execute sequentially — each worker turn completes before the next begins.

Only delegate a task to a worker if the worker has the relevant skills.
```
Then per worker (via **`lines_for_worker`**):
  - **`### <id>`** heading.
  - Skill descriptions from **`skill_catalog`** (**`This worker has the following skills:`** + one **`- <description>`** per enabled skill; omitted if no enabled skills).
  - Bracket prefix line (**`Start your instruction with \`[<id>]\` to delegate to this worker.`**).
  - Example (**`{ "instruction": "[<id>] Do X" }`**).

### `strip_skill_frontmatter(content)`

- Strips leading YAML frontmatter from **`SKILL.md`**; if multiple **`---`** blocks appear at the start, recurses until the body begins. Implemented in **`gateway/server.rs`**.

### Example Shape (Illustrative)

The **static** portion is cached at gateway startup.

#### Full mode (orchestrator or worker entry: **`contextMode`**: **`full`**)

```
<contents of AGENT.md>

## Workers

You are the orchestrator agent. You have worker agents.

You can call `delegate_task` to delegate a task to a worker agent.

Only delegate a task to a worker if the worker has the relevant skills.

### read-only

This worker has the following skills:

- Read files, list directories, and search file contents (read-only).

Start your instruction with `[read-only]` to delegate to this worker.

Example:

{ "instruction": "[read-only] Do X" }

## Skills

You have skills. Skills have tools.

### files

Read files, list directories, search file contents, write files, and delete files and directories.

--- SKILL.md (BOF) ---

<body of files/SKILL.md, frontmatter stripped>

--- SKILL.md (EOF) ---
```

*(Workers block omitted if there are no workers; **Skills** block omitted if no enabled skills.)*

#### Read-on-demand mode (orchestrator or worker entry: **`contextMode`**: **`readOnDemand`**)

```
<contents of AGENT.md>

<optional ## Workers section as above>

## Skills

You have skills. Skills have tools.

You can call `read_skill` to read about a skill.

Available skills:

- `files` — Read files, list directories, search file contents, write files, and delete files and directories.

```

The **`read_skill`** tool (plus skill tools from **`tools.json`**) is included in the request; **`read_skill`** returns the same body full mode would have inlined (frontmatter stripped).

## 2. Chat Messages (Session History)

- Loaded from the session store for the current **`session_id`** every turn.
- The session store persists messages with roles **`user`**, **`assistant`** (with optional **`tool_calls`**), and **`tool`** (results with **`tool_name`**). Tool calls and tool results are part of the session history and are replayed on subsequent turns.
- The **system** message is **not** persisted in the session store. It is built from the cached startup string and injected as `messages[0]` before the session history each turn (see **`agent.rs`** `run_turn_dyn` / provider **`chat`**).

### Session Persistence

Sessions are persisted to disk so they survive gateway restarts. See [PERSISTENT_SESSIONS.md](../epic/PERSISTENT_SESSIONS.md) for the full design and phased delivery plan.

- **Storage layout** — Each session is stored as a JSON file (`{session_id}.json`) under `<profileRoot>/agents/<agentId>/sessions/`. Only the orchestrator's `sessions/` directory is populated in the current architecture.
- **`Session` struct** — Derives `Serialize, Deserialize` and includes `created_at: String` and `updated_at: String` (ISO 8601 timestamps, with `#[serde(default)]` for backward compatibility).
- **Write-through** — Every mutation (`create`, `append_message_full`, `record_delegation`, `remove`) writes to disk immediately via atomic writes (`.tmp` then rename). The `updated_at` timestamp advances on every write.
- **Lazy loading** — On gateway start, `session_store.scan()` reads metadata only (id, timestamps, message count) without loading full message history. Full history is loaded on the first `get()` call for that session, keeping startup fast.
- **`SessionStore::new()` vs `with_data_dir(data_dir)`** — `new()` creates an in-memory-only store (no disk I/O, used by tests). `with_data_dir()` enables persistence to the given directory.
- **Graceful degradation** — Missing or corrupt session files are logged (warn level) and skipped. A missing `data_dir` is treated as empty (no sessions on disk).
- **Manual cleanup** — Deleting the `sessions/` directory on disk is a valid way to clear all session history. The gateway handles missing files gracefully.

## 3. Tools

Tool schemas are sent as a **separate top-level field** in the provider request — they are never part of the messages array.

- **Skill tools** — From **`tools.json`** descriptors for **that turn's role**: only packages in that agent's **`enabledSkills`** list (**`ToolDescriptor::to_tool_definitions()`**). Executed by the generic executor (and **`ReadOnDemandExecutor`** when that agent's **`contextMode`** is **`readOnDemand`**, which handles **`read_skill`** in-process).
- **`read_skill`** — Included only for agents using **`readOnDemand`** with at least one enabled skill. Resolves against **that** agent's enabled set and the shared discovery roots. Tool description in code: **`read a skill by name`**; parameters: **`skill_name`** (exact name from the list).
- **`delegate_task`** — Merged at the **front** of the **orchestrator** tool list via **`merge_delegate_task`** when workers exist. Worker tool lists **omit** **`delegate_task`** (nested delegation disabled). Parameters: **`instruction`** (required). No **`workerId`**, **`provider`**, or **`model`** parameters; worker targeting is done via bracket prefix in the instruction, and the worker always runs on its single `(defaultProvider, defaultModel)` pair (see [ORCHESTRATION.md](ORCHESTRATION.md)).

See [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md) for **`tools.json`** execution shape.

## Summary

| Source | Where defined | When loaded | What the model sees |
|--------|----------------|------------|---------------------|
| Agent context (orchestrator) | **`<profileRoot>/agents/<orchestratorId>/AGENT.md`** | Gateway startup | Trimmed file text, then optional **`## Workers`**, then orchestrator skills block. Injected as `messages[0]` (`system` role) every turn; not persisted in session store. |
| Agent context (worker) | **`<profileRoot>/agents/<workerId>/AGENT.md`** | Gateway startup | Trimmed file text + worker skills only (no workers roster). Injected as `messages[0]` (`system` role) every turn; not persisted in session store. |
| Workers roster | `config.json` **`agents`** (`role: worker` entries) | Composed at startup **into orchestrator static context only** | **`## Workers`** with intro text, per-worker skill descriptions, bracket prefix guidance, and example. |
| Skill content | **`~/.chai/skills`**; **`SKILL.md`** per package | Discovery at startup; **subset** per agent | Per agent **`enabledSkills`**: **full** ⇒ **`## Skills`** + **`###`** bodies; **readOnDemand** ⇒ compact list + **`read_skill`**. |
| Session messages | Session store | Every turn | History for **`session_id`** (user, assistant with tool_calls, tool results). |
| Tools | Per-agent skill descriptors + optional **`read_skill`** + orchestrator-only **`delegate_task`** | Startup | Sent as a separate top-level `tools` field on each provider request — never part of the messages array. |

## Efficiency

- **Static context** — Orchestrator and each worker cache their own **`AGENT.md`** slice, orchestrator-only workers roster, and skill text.

**Inspecting the exact string:** The gateway **`agentDetail`** method returns per-agent heavy data (**`systemContext`**, **`tools`**, **`skillsContext`**) on demand, given an **`agentId`**. The polling **`status`** response includes lightweight agent metadata (**`id`**, **`role`**, **`enabledSkills`**, **`contextMode`**, routing defaults, delegation limits) but omits the large fields to reduce payload size (see [GATEWAY_STATUS.md](GATEWAY_STATUS.md)). Chai Desktop fetches **`agentDetail`** when the Agent or Tools screen is active, builds a per-agent cache, and shows skill bodies in a second column when available; otherwise the desktop shows a loading placeholder or error message (no on-disk fallback).
