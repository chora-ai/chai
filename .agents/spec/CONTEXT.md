# Context at Session Start

This document describes the **context** the model receives for a turn, as implemented in `crates/lib` (gateway `server.rs`, `orchestration/workers_context.rs`, and skills loading). It is kept in sync with the code.

## Turn vs Session

- **Session messages** — One conversation: a `session_id`, its message history (user/assistant/tool), and its binding to a channel+conversation (e.g. one chat). It lasts across many user messages. `/new` creates a new session and binds it so the next message has empty history.
- **Turn** — One run of the agent for a single user message: load that session's history, build the system + messages, call the model (and the tool loop if there are tool calls), then produce one assistant reply. One user message ⇒ one turn; a session has many turns over time.

## When It Is Built

- **Static system context** — Composed once when the gateway starts (`run_gateway` in `gateway/server.rs`): `AGENTS.md` from the workspace, optional **workers** roster from **`build_workers_context`** (`orchestration/workers_context.rs`), and **skills** from **`build_skill_context_full`** or **`build_skill_context_compact`** depending on **`skills.contextMode`**. Stored in **`GatewayState.system_context_static`** and not reloaded until the gateway restarts.
- **Skills loaded** — Only skills whose names appear in **`skills.enabled`** in config (opt-in). Each skill needs a directory under the resolved skills root; descriptor tools come from **`tools.json`** where present.
- **Per turn** — **`build_system_context_for_today`** prepends **`TODAY'S DATE: YYYY-MM-DD`** to that cached static string so the model always sees the current date.
- **Tools** — Built at startup: skill tools from descriptors, optional **`read_skill`** when **`readOnDemand`**, and **`delegate_task`** merged in via **`merge_delegate_task`** (prepended when not already present). Same list every turn.

For a **new session**, when the user sends a message, the model receives:

1. A **system message** (content = date line + static context as below).
2. **Chat messages** = session history (for a fresh session, often a single user message).
3. **Tools** = JSON tool definitions for the LLM backend (see [§3](#3-tools)).

## 1. System Message Content

The static portion is built by **`build_system_context_static(agent_ctx, skills, context_mode, agents)`** in **`gateway/server.rs`**. The final system string for the provider is **`build_system_context_for_today(&static)`**.

### Build Order

1. **Agent context** — Contents of **`AGENTS.md`** from the workspace directory (e.g. `~/.chai/workspace/AGENTS.md`), trimmed. Omitted if missing or empty.
2. **`"\n\n"`** — Only if agent context was non-empty.
3. **Workers** — If **`config.agents.workers`** is non-empty, **`build_workers_context(agents)`** appends a **`## Agents`** section: orchestrator id, short bullet list (delegate via **`delegate_task`**, complete without worker, offer workers to the user), then **`Available worker agents (id — providers — models):`** with one line per worker: **`workerId`**, **effective** provider, **effective** model (same resolution as **`delegate_task`** when **`provider`** / **`model`** are omitted—see **`effective_worker_defaults`** in **`workers_context.rs`**). Omitted entirely when there are no workers.
4. **`"\n\n"`** — Only if the workers section was non-empty.
5. **Skills** — From **`build_skill_context_full`** (**`full`**) or **`build_skill_context_compact`** (**`readOnDemand`**), or empty if no enabled skills loaded.

### Skill Context Mode

- **`full`** — Each enabled skill is inlined under a shared **`## Skills`** header with guidance bullets (call **`read_skill`**, share skills/tools, ask the user to choose), then **`### <skill name>`** per skill: optional description, then **`SKILL.md`** body with frontmatter stripped (**`strip_skill_frontmatter`**).
- **`readOnDemand`** — Same **`## Skills`** header and guidance bullets, then **`Available skills (name — description):`** with one line per skill: **`- \`<name>\` — <description>`** (no full bodies in system message). The model uses the **`read_skill`** tool to load full **`SKILL.md`** when needed.

### Skill Context — Full Mode (`build_skill_context_full`)

- If there are no skills, returns an empty string.
- Otherwise: **`## Skills`**, blank line, three bullet lines of guidance (same intent as read-on-demand), blank line, then for each skill in load order: **`### `** + name, blank line, optional description + blank lines, **`strip_skill_frontmatter(content)`**, blank line between skills.

### Skill Context — Read-on-Demand Mode (`build_skill_context_compact`)

- If there are no skills, returns an empty string.
- Otherwise: **`## Skills`**, guidance bullets, **`Available skills (name — description):`**, then for each skill: **`- \`<name>\` — `** + description or **`(no description)`**.

### Workers Section (`build_workers_context`)

- Empty string if **`agents.workers`** is missing or empty.
- Otherwise: **`## Agents`**, orchestrator line (**`You are \`<orchestrator id>\` — the orchestrator agent...`**), capability bullets, **`Available worker agents (id — providers — models):`**, then per worker **`- \`<id>\` — \`<provider>\` — \`<model>\`** (effective defaults).

### `strip_skill_frontmatter(content)`

- Strips leading YAML frontmatter from **`SKILL.md`**; if multiple **`---`** blocks appear at the start, recurses until the body begins. Implemented in **`gateway/server.rs`**.

### Example Shape (Illustrative)

The **static** portion is cached at gateway startup; each turn prepends **`TODAY'S DATE: YYYY-MM-DD`**.

#### Full mode (`skills.contextMode`: `full`)

```
TODAY'S DATE: 2025-03-21

<contents of AGENTS.md>

## Agents

You are `hermes` — the orchestrator agent. You can:

- delegate a task to a worker agent (`delegate_task`)
- complete a task without a worker agent (use your skills)
- share available worker agents and ask the user to choose

Available worker agents (id — providers — models):

- `apollo` — `ollama` — `llama3.2:latest`

## Skills

You have skills. Skills have tools. You can:

- call `read_skill` when you need to use a skill
- share available skills and ask the user to choose

### notesmd

Create, read, search, update, and delete notes.

<body of notesmd/SKILL.md, frontmatter stripped>

```

*(Workers block omitted if there are no workers; **Skills** block omitted if no enabled skills.)*

#### Read-on-demand mode (`skills.contextMode`: `readOnDemand`)

```
TODAY'S DATE: 2025-03-21

<contents of AGENTS.md>

<optional ## Agents section as above>

## Skills

You have skills. Skills have tools. You can:

- call `read_skill` when you need to use a skill
- share available skills and ask the user to choose

Available skills (name — description):

- `notesmd-daily` — Create, read, and update daily notes.

```

The **`read_skill`** tool (plus skill tools from **`tools.json`**) is included in the request; **`read_skill`** returns the same body full mode would have inlined (frontmatter stripped).

## 2. Chat Messages (Session History)

- Loaded from the session store for the current **`session_id`**.
- Each message has **`role`** (`user` | `assistant` | `system` | `tool`), **`content`**, and optionally **`tool_calls`** / tool metadata.
- The **system** message is built from the string above and passed into the agent/provider path as the first message before session history (see **`agent.rs`** / provider **`chat`**).

## 3. Tools

- **Skill tools** — From **`tools.json`** descriptors for enabled skills (**`ToolDescriptor::to_tool_definitions()`**). Executed by the generic executor (and **`ReadOnDemandExecutor`** when **`readOnDemand`**, which handles **`read_skill`** in-process).
- **`read_skill`** — Prepended only when **`readOnDemand`** and there is at least one loaded skill. Tool description in code: **`read a skill by name`** Parameters: **`skill_name`** (exact name from the list).
- **`delegate_task`** — Merged at the **front** of the tool list via **`merge_delegate_task`** when the gateway builds tools (orchestrator path). Worker turns do not expose **`delegate_task`** (nested delegation disabled). See **`orchestration/delegate.rs`** for the current schema and descriptions.

See [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md) for **`tools.json`** execution shape.

## Summary

| Source | Where defined | When loaded | What the model sees |
|--------|----------------|------------|---------------------|
| Agent context | `workspace/AGENTS.md` | Gateway startup | Trimmed file text, then optional workers block, then skills block. |
| Workers | `config.json` **`agents`** (`role: worker` entries) | Composed at startup into static context | **`## Agents`** section with orchestrator id, bullets, and effective provider/model per **`workerId`**. |
| Skill content | `skills/<name>/SKILL.md` | Gateway startup (gated by **`skills.enabled`**) | **full**: **`## Skills`** + per-skill **`###`** sections with bodies. **readOnDemand**: compact list + **`read_skill`**. |
| Date line | — | Every turn | **`TODAY'S DATE: YYYY-MM-DD`** prepended to cached static context. |
| Session messages | Session store | Every turn | History for **`session_id`** (optional cap via **`agents.maxSessionMessages`**). |
| Tools | Skill descriptors + **`read_skill`** (read-on-demand) + **`delegate_task`** | Startup | Sent on each provider request. |

## Efficiency

- **Static context** — **`AGENTS.md`**, workers string, and skills string are built once; only the date line changes each turn.
- **`maxSessionMessages`** — When set, only the last N messages are sent to the model; full history remains in the session store.

**Inspecting the exact string:** Chai Desktop **Context** screen shows **`status.systemContext`** from the running gateway (same string the model receives, including the date prefix). You can also temporarily log **`build_system_context_for_today`** output in **`gateway/server.rs`** where the agent turn runs.
