---
status: stable
---

# Context at Session Start

This document describes the **context** the model receives for a turn, as implemented in `crates/lib` (gateway `server.rs`, `orchestration/workers_context.rs`, and skills loading). **Orchestrator** and **worker** turns use **separate** static strings and tool lists (per-agent **`AGENT.md`**, **`skillsEnabled`**, **`contextMode`**); see **[AGENT_ISOLATION.md](../epic/AGENT_ISOLATION.md)**. This spec is kept in sync with the code.

## Turn vs Session

- **Session messages** — One conversation: a `session_id`, its message history (user/assistant/tool), and its binding to a channel+conversation (e.g. one chat). It lasts across many user messages. `/new` creates a new session and binds it so the next message has empty history.
- **Turn** — One run of the agent for a single user message: load that session's history, build the system + messages, call the model (and the tool loop if there are tool calls), then produce one assistant reply. One user message ⇒ one turn; a session has many turns over time.

## When It Is Built

- **Skill discovery** — At gateway startup, packages are loaded from **`~/.chai/skills`** only (see **`config::default_skills_dir`** / **`skills::load_skills`**). **Enablement** is **not** global: each agent entry's **`skillsEnabled`** list selects which discovered packages apply to **that** agent. Missing or empty **`skillsEnabled`** ⇒ **no** skill tools and **no** skill-derived inlined context for that agent.
- **Orchestrator static context** — Composed once in **`run_gateway`**: **`AGENT.md`** from the orchestrator **agent context directory** (**`orchestrator_context_dir`** → **`<profileRoot>/agents/<orchestratorId>/AGENT.md`**), then optional **`## Workers`** roster from **`build_workers_context`**, then orchestrator skills via **`build_skill_context_full`** or **`build_skill_context_compact`** according to the **orchestrator** entry's **`contextMode`**. Stored in **`GatewayState.system_context`**. The gateway does **not** read **`workspace/AGENTS.md`** for any agent.
- **Worker static context** — For each **`role: worker`** entry, a **`WorkerDelegateRuntime`** is built: **`AGENT.md`** from **`worker_context_dir`** (**`<profileRoot>/agents/<workerId>/`**), worker-filtered skills, and **no** **`## Workers`** block (**`build_worker_system_context`**). Cached per worker id until restart.
- **Tools** — **Per agent** at startup: skill tools from **`tools.json`** for that agent's enabled packages; optional **`read_skill`** when that agent's **`contextMode`** is **`readOnDemand`** and at least one skill is enabled. The **orchestrator** list is merged with **`delegate_task`** via **`merge_delegate_task`** when workers exist. **Worker** lists omit **`delegate_task`**. The same prebuilt list is sent on every turn for that role.

For a **new session**, when the user sends a message, the model receives:

1. A **system message** (content = static context as below).
2. **Chat messages** = session history (for a fresh session, often a single user message).
3. **Tools** = JSON tool definitions for the LLM backend (see [§3](#3-tools)).

## 1. System Message Content

**Orchestrator** — Static string from **`build_system_context(agent_ctx, skills, context_mode, agents, skill_catalog)`**. **Worker** — Static string from **`build_worker_system_context(agent_ctx, skills, context_mode)`** (same skill builders, **no** workers roster). The static string is built once at gateway startup and sent unchanged on every turn.

### Build Order (Orchestrator)

1. **Agent context** — Contents of **`AGENT.md`** at **`<profileRoot>/agents/<orchestratorId>/AGENT.md`**. Trimmed. Omitted if missing or empty.
2. **`"\n\n"`** — Only if agent context was non-empty.
3. **Workers** — If **`config.agents.workers`** is non-empty, **`build_workers_context(agents, skill_catalog)`** appends a **`## Workers`** section: orchestrator id, short bullet list (delegate via **`delegate_task`**, complete without worker, offer workers to the user), then **`Available worker agents (id — providers — models):`** with one line per worker: **`workerId`**, **effective** provider, **effective** model (same resolution as **`delegate_task`** when **`provider`** / **`model`** are omitted—see **`effective_worker_defaults`** in **`workers_context.rs`**). Omitted entirely when there are no workers.
4. **`"\n\n"`** — Only if the workers section was non-empty.
5. **Skills** — From **`build_skill_context_full`** (**`full`**) or **`build_skill_context_compact`** (**`readOnDemand`**) using **only** packages whose names are in the **orchestrator** **`skillsEnabled`** list, or empty if that list is missing/empty or none match disk.

### Build Order (Worker)

1. **Agent context** — That worker's **`AGENT.md`** at **`<profileRoot>/agents/<workerId>/AGENT.md`**. No **`## Workers`** section.
2. **Skills** — Same builders as the orchestrator, but filtered by **that worker's** **`skillsEnabled`** and **`contextMode`**.

### Skill Context Mode

- **`full`** — Each enabled skill is inlined under a shared **`## Skills`** header with guidance bullets (call **`read_skill`**, share skills/tools, ask the user to choose), then **`### <skill name>`** per skill: optional description, then **`SKILL.md`** body with frontmatter stripped (**`strip_skill_frontmatter`**).
- **`readOnDemand`** — Same **`## Skills`** header and guidance bullets, then **`Available skills (name — description):`** with one line per skill: **`- \`<name>\` — <description>`** (no full bodies in system message). The model uses the **`read_skill`** tool to load full **`SKILL.md`** when needed.

### Skill Context — Full Mode (`build_skill_context_full`)

- If there are no skills, returns an empty string.
- Otherwise: **`## Skills`**, blank line, intro line (**`You have skills. Skills have tools.`**), blank line, then for each skill in load order: **`### `** + name, blank line, optional description + blank lines, **`--- SKILL.md (BOF) ---`**, blank line, **`strip_skill_frontmatter(content)`**, **`--- SKILL.md (EOF) ---`**, blank line between skills.

### Skill Context — Read-on-Demand Mode (`build_skill_context_compact`)

- If there are no skills, returns an empty string.
- Otherwise: **`## Skills`**, guidance bullets, **`Available skills (name — description):`**, then for each skill: **`- \`<name>\` — `** + description or **`(no description)`**.

### Workers Section (`build_workers_context`)

- Empty string if **`agents.workers`** is missing or empty.
- Otherwise: **`## Workers`**, orchestrator line (**`You are \`<orchestrator id>\` — the orchestrator agent...`**), capability bullets, **`Available worker agents (id — providers — models):`**, then per worker **`- \`<id>\` — \`<provider>\` — \`<model>\`** (effective defaults).

### `strip_skill_frontmatter(content)`

- Strips leading YAML frontmatter from **`SKILL.md`**; if multiple **`---`** blocks appear at the start, recurses until the body begins. Implemented in **`gateway/server.rs`**.

### Example Shape (Illustrative)

The **static** portion is cached at gateway startup.

#### Full mode (orchestrator or worker entry: **`contextMode`**: **`full`**)

```

<contents of AGENT.md>

## Workers

You are `hermes` — the orchestrator agent. You can:

- delegate a task to a worker agent (`delegate_task`)
- complete a task without a worker agent (use your skills)
- share available worker agents and ask the user to choose

Available worker agents (id — providers — models):

- `apollo` — `ollama` — `llama3.2:latest`

## Skills

You have skills. Skills have tools.

### notesmd

Create, read, search, update, and delete notes.

--- SKILL.md (BOF) ---

<body of notesmd/SKILL.md, frontmatter stripped>

--- SKILL.md (EOF) ---
```

*(Workers block omitted if there are no workers; **Skills** block omitted if no enabled skills.)*

#### Read-on-demand mode (orchestrator or worker entry: **`contextMode`**: **`readOnDemand`**)

```
<contents of AGENT.md>

<optional ## Workers section as above>

## Skills

You have skills. Skills have tools. You can:

- call `read_skill` when you need to read a skill

Available skills:

- `notesmd-daily` — Create, read, and update daily notes.

```

The **`read_skill`** tool (plus skill tools from **`tools.json`**) is included in the request; **`read_skill`** returns the same body full mode would have inlined (frontmatter stripped).

## 2. Chat Messages (Session History)

- Loaded from the session store for the current **`session_id`**.
- Each message has **`role`** (`user` | `assistant` | `system` | `tool`), **`content`**, and optionally **`tool_calls`** / tool metadata.
- The **system** message is built from the string above and passed into the agent/provider path as the first message before session history (see **`agent.rs`** / provider **`chat`**).

## 3. Tools

- **Skill tools** — From **`tools.json`** descriptors for **that turn's role**: only packages in that agent's **`skillsEnabled`** list (**`ToolDescriptor::to_tool_definitions()`**). Executed by the generic executor (and **`ReadOnDemandExecutor`** when that agent's **`contextMode`** is **`readOnDemand`**, which handles **`read_skill`** in-process).
- **`read_skill`** — Included only for agents using **`readOnDemand`** with at least one enabled skill. Resolves against **that** agent's enabled set and the shared discovery roots. Tool description in code: **`read a skill by name`**; parameters: **`skill_name`** (exact name from the list).
- **`delegate_task`** — Merged at the **front** of the **orchestrator** tool list via **`merge_delegate_task`** when workers exist. Worker tool lists **omit** **`delegate_task`** (nested delegation disabled). See **`orchestration/delegate.rs`** for the current schema and descriptions.

See [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md) for **`tools.json`** execution shape.

## Summary

| Source | Where defined | When loaded | What the model sees |
|--------|----------------|------------|---------------------|
| Agent context (orchestrator) | **`<profileRoot>/agents/<orchestratorId>/AGENT.md`** | Gateway startup | Trimmed file text, then optional **`## Workers`**, then orchestrator skills block. |
| Agent context (worker) | **`<profileRoot>/agents/<workerId>/AGENT.md`** | Gateway startup | Trimmed file text + worker skills only (no workers roster). |
| Workers roster | `config.json` **`agents`** (`role: worker` entries) | Composed at startup **into orchestrator static context only** | **`## Workers`** with orchestrator id, bullets, effective provider/model per **`workerId`**. |
| Skill content | **`~/.chai/skills`**; **`SKILL.md`** per package | Discovery at startup; **subset** per agent | Per agent **`skillsEnabled`**: **full** ⇒ **`## Skills`** + **`###`** bodies; **readOnDemand** ⇒ compact list + **`read_skill`**. |
| Session messages | Session store | Every turn | History for **`session_id`** (optional cap via **`agents.maxSessionMessages`**). |
| Tools | Per-agent skill descriptors + optional **`read_skill`** + orchestrator-only **`delegate_task`** | Startup | Sent on each provider request for that role. |

## Efficiency

- **Static context** — Orchestrator and each worker cache their own **`AGENT.md`** slice, orchestrator-only workers roster, and skill text.
- **`maxSessionMessages`** — When set, only the last N messages are sent to the model; full history remains in the session store.

**Inspecting the exact string:** The gateway **`status`** method returns **`agents.entries`**, an array with one object per configured agent (**`role`** **`orchestrator`** first, then **`worker`** rows). Each object's **`systemContext`** is the same static string that role would receive on a turn. Per-agent skill text slices (**`skillsContext`**, **`skillsContextFull`**, **`skillsContextBodies`**) and **`contextMode`** are on that row's **`skills`** object (see [GATEWAY_STATUS.md](GATEWAY_STATUS.md)). Chai Desktop **Context** builds a per-agent map from **`entries`** for the agent picker; orchestrator **read-on-demand** still shows skill bodies in a second column from disk.
