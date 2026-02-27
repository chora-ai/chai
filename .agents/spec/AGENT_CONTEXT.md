# Agent Context at Session Start

This document describes the **exact** context provided to the model when processing a message (e.g. the first message after `/new` in Telegram). It is derived from the code in `crates/lib`.

## Turn vs Session

- **Session messages** — One conversation: a `session_id`, its message history (user/assistant/tool), and its binding to a channel+conversation (e.g. one Telegram chat). It lasts across many user messages. `/new` creates a new session and binds it so the next message has empty history.
- **Turn** — One run of the agent for a single user message: load that session's history, build the system + messages, call the model (and the tool loop if there are tool calls), then produce one assistant reply. One user message ⇒ one turn; a session has many turns over time.

## When It Is Built

- **Agent context (AGENTS.md) and skills** are loaded once when the gateway starts (`gateway/server.rs`: `run_gateway`). They are stored in `GatewayState` and not reloaded until the gateway restarts.
- **System context string** is built on every turn: `build_system_context(agent_ctx, skills, context_mode)` (see Skill context mode below).
- **Tools** are built at startup from skills that have a `tools.json` descriptor; when `skills.contextMode` is `readOnDemand`, a `read_skill` tool is prepended. Same list every turn.

So for a **new session** (e.g. after `/new`), when the user sends their first message, the model receives:

1. A **system message** (content = the string below).
2. **Chat messages** = the session history. For a fresh session this is a single `user` message (e.g. `"hello"`).
3. **Tools** = the JSON tool definitions passed to the Ollama API (see below).

## 1. System Message Content

The system message is built by `build_system_context(agent_ctx, skills, context_mode)` in `gateway/server.rs`. The **skill context** depends on **`skills.contextMode`** in config.

### Build Order

1. **Agent context** — Raw contents of `AGENTS.md` from the workspace directory (e.g. `~/.chai/workspace/AGENTS.md`), trimmed. If the file is missing or empty, this part is omitted.
2. **Newline** — `"\n\n"` (only if agent context was non-empty).
3. **Skill context** — From the configured context mode (see below).

### Skill Context Mode

- **`full`** (default): Full SKILL.md for every loaded skill (intro line, then for each skill: `## name`, description, and body after frontmatter strip). Best for few skills and smaller local models.
- **`readOnDemand`**: A compact list only: intro instructing the model to use the **`read_skill`** tool to load a skill’s full SKILL.md when it clearly applies; then a bullet list of skill names and descriptions. The model must call `read_skill(skill_name)` to get full docs before using that skill’s tools. Keeps the system prompt small and scales to many skills.

### Skill Context — Full Mode (`build_skill_context_full`)

- If there are no skills, this is an empty string.
- Otherwise:
  - A single intro line:  
    `"You have access to the following skills. Use them when relevant.\n\n"`
  - For **each** loaded skill (order = merged order from `load_skills`: config dir skills, then extra dirs; later overwrites earlier by name):
    - `"## "` + skill `name` (e.g. `notesmd-cli`) + `"\n"`
    - If the skill has a non-empty `description` (from SKILL.md frontmatter): that string + `"\n\n"`
    - **Skill body**: `strip_skill_frontmatter(skill.content)` + `"\n\n"`

### Skill Context — Read-on-Demand Mode (`build_skill_context_compact`)

- If there are no skills, this is an empty string.
- Otherwise: an intro line instructing the model to use the `read_skill` tool when a skill clearly applies, then `## Available skills` and a bullet list of **name**: description for each loaded skill.

### `strip_skill_frontmatter(content)`

- Removes the first YAML frontmatter block from the skill's raw `SKILL.md` content.
- Logic: find the first `---`; then find `\n---`; the returned string is everything **after** that second `---` (and the newline), trimmed. So the body starts with the first line after the closing `---` (e.g. `# notesmd-cli` or `The following guidelines...`).
- If there is no second `---`, the whole content is returned unchanged.

### Example Shape (Concrete)

Assume workspace has `AGENTS.md` and one skill `notesmd-cli` with frontmatter and body. The system message is built from real file contents only; the model never sees anything other than the contents of the files:

```
<AGENTS.md>

You have access to the following skills. Use them when relevant.

## notesmd-cli
Create, read, update, and search notes when the user asks.

<SKILL.md for each skill - excluding YAML frontmatter>

```

## 2. Chat Messages (Session History)

- Loaded from the session store for the current `session_id`.
- Each message has: `role` (`"user"` | `"assistant"` | `"system"` | `"tool"`), `content`, and optionally `tool_calls` / `tool_name`.
- **System message is inserted at index 0** in `agent/agent.rs` before calling Ollama. So the array sent to Ollama is:

  `[ { role: "system", content: "<system context string>" }, ...session_messages ]`

- For a **new session** after `/new`, `session_messages` contains only the one new user message (e.g. `role: "user", content: "hello"`).

## 3. Tools (Ollama API)

- **Source**: Tool list is built at gateway startup from skills that have a **`tools.json`** descriptor. Each descriptor’s `tools` array is converted to Ollama `ToolDefinition`s via `ToolDescriptor::to_tool_definitions()`. When **`skills.contextMode`** is **`readOnDemand`** and there are loaded skills, a **`read_skill(skill_name)`** tool definition is prepended so the model can load a skill’s full SKILL.md on demand.
- **Shape** sent to Ollama (from `llm/ollama.rs`): each tool is a JSON object:

  ```json
  {
    "type": "function",
    "function": {
      "name": "<tool_name>",
      "description": "<optional string>",
      "parameters": { <JSON schema object> }
    }
  }
  ```

- **Execution**: A single **generic executor** builds argv from each tool’s execution spec in `tools.json` (positional, flag, flagifboolean) and runs via the descriptor’s allowlist (`exec::Allowlist::run()`). Param resolution (`resolveCommand`) may use a script from the skill’s `scripts/` dir when `skills.allowScripts` is true, or an allowlisted command. When context mode is readOnDemand, a **ReadOnDemandExecutor** wraps it: it handles `read_skill` in-process (returns that skill’s SKILL.md content) and delegates all other tool names to the generic executor. See [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md).

## Summary

| Source | Where defined | When loaded | What the model sees |
|--------|----------------|------------|---------------------|
| Agent context | `workspace_dir/AGENTS.md` | Gateway startup | Raw file content, then `\n\n`, then skill context. |
| Skill content | `skills/<name>/SKILL.md` (from config dir, `skills.directory`, or `skills.extraDirs`) | Gateway startup | Full mode: `## <name>\n` + description + body (frontmatter stripped). ReadOnDemand: compact list only; full content via `read_skill` tool. |
| System message | — | Every turn | `agent_ctx + "\n\n" + skill_context`, where skill_context depends on `skills.contextMode` (full vs compact). Inserted as first message. |
| Session messages | Session store | Every turn | All messages for that session (e.g. one user message after `/new`). |
| Tools | Skills’ `tools.json` (and built-in `read_skill` when contextMode is readOnDemand) | Startup (list fixed) | `Vec<ToolDefinition>` from descriptors + optional read_skill; sent in the Ollama chat request as `tools`. |

## What Is Sent Every Turn

- **Session messages** — Loaded from the session store on every turn (`store.get(session_id)` in `run_turn`). The model always sees the current conversation history.
- **System message** — The string is built every turn via `build_system_context(agent_ctx, skills)`. The inputs (`agent_ctx` and `skills`) are not re-read from disk; they were loaded at gateway startup and live in `GatewayState`. So the system text is recomputed each turn from in-memory data. Changes to `AGENTS.md` or `SKILL.md` on disk take effect only after a gateway restart.
- **Tools** — The list is built once at startup from loaded skills’ `tools.json` descriptors (and optional `read_skill` when context mode is readOnDemand). Same list every turn; no disk read per turn.

## What Might Be More Efficient

**Why each is sent every turn**

- **System message** — The API is stateless. Ollama doesn't remember the system prompt between requests, so we have to send it on every call. Omitting it on later turns would make the model "forget" the rules.
- **Session messages** — The model needs conversation history to respond in context. We send the full history so it doesn't lose earlier context. The only way to reduce tokens is to send less history (e.g. sliding window or summarization), which can degrade quality.
- **Tools** — With Ollama's chat API, tool definitions are part of the request. There's no "tools already sent" state; each `/api/chat` call is independent. So if we want the model to be able to call tools on that turn, we have to send the tool list every time.

**What can be made more efficient**

- **System** — We already trimmed AGENTS.md and the skill. We could cache the built system string (e.g. in `GatewayState`) and reuse it each turn instead of calling `build_system_context` every time. Same bytes sent, less work per turn.
- **Session** — We could add an optional history limit (e.g. last N messages or last N tokens) for very long chats. That saves tokens and cost but can weaken the model's ability on long conversations. So it's a tradeoff, not a free win.
- **Tools** — No way to avoid sending them each request with the current API; the payload is small (a few KB of JSON).

**Summary**

- System + session + tools are all "necessary" every turn in the sense that the API and behavior we want require them.
- We can improve efficiency by: (1) caching the system string, and (2) optionally capping session length for long chats, with the understanding that (2) may reduce quality in those long sessions.
- Implementation options: build the system string once at startup (or when config is reloaded) and reuse it each turn; and/or add an optional session-history cap (e.g. in `agent.rs` or the gateway).

To see the **exact** system string your gateway sends, add a temporary log in `gateway/server.rs` where `build_system_context` is called (e.g. in `process_inbound_message`), and log `system_context` before `run_turn`. The tool list is fixed at startup from skills’ `tools.json` and (when readOnDemand) the built-in `read_skill` definition.
