# Journey: Skill Obsidian (official CLI)

**Goal:** Confirm the **obsidian** skill is loaded (official Obsidian CLI, early access), the agent can call its tools (search, search-content, create), and the reply reflects vault data or confirms an action.

This journey is for the binary **`obsidian`** only. For **`notesmd-cli`**, see [06-skill-notesmd-cli.md](06-skill-notesmd-cli.md).

## Prerequisites

- **`chai init`** has been run (so the skills directory exists).
- **Official Obsidian CLI** — `obsidian` is on your PATH (early access; enable in Obsidian Settings → Command line interface). See [Obsidian CLI — early access](https://help.obsidian.md/cli). Check with `which obsidian`.
- **Vault** is available to the CLI (see "Multiple vaults" below).
- **Ollama** is running with a model that supports **tool/function calling** (e.g. `llama3.2:latest`).
- **Gateway** will be started after the above so it sees `obsidian` and loads the obsidian skill.

**Multiple vaults:** The skill does not pass a vault name or path to the CLI. The CLI uses whatever it considers the default or current vault. Configure the CLI's default or targeting as needed; the chai tool layer does not support a per-request vault parameter.

## Steps

1. **Confirm the official Obsidian CLI is available**
   - Run: `which obsidian`. Ensure you have at least one vault. Note: running `obsidian search` or `obsidian vault` from a terminal may open the Obsidian app or vault picker instead of returning CLI output; the gateway will still invoke the CLI when the agent uses the tools.

2. **Start the gateway**
   - From repo root: `cargo run -p cli -- gateway` or `chai gateway`. Optional: `RUST_LOG=info`.
   - **Expect:** A log line like `loaded 1 skill(s) for agent context` (or more). If you see `loaded 0 skill(s)`, `obsidian` is not on PATH when the gateway starts, or the skills directory is missing (run `chai init`).

3. **Trigger the agent with an Obsidian-style request**
   - **Via WebSocket:** Connect and send `connect`, then an agent request (see [02-gateway-ws-agent.md](02-gateway-ws-agent.md)). Example: `{"type":"req","id":"2","method":"agent","params":{"message":"Search my Obsidian vault for note names that contain 'meeting' and list them."}}`
   - **Via Telegram:** Send the same message to your bot (journey 05). Example: "Search my vault for notes about meetings."

4. **Verify the reply**
   - The model should describe search results (note names or snippets) or say it found nothing. The model uses tools `obsidian_search`, `obsidian_search_content`, `obsidian_create`, etc.
   - With `RUST_LOG=debug`, tool failures appear as `agent: tool obsidian_search failed: ...`.

5. **Optional: search inside note content**
   - Send: "Search inside the content of my notes for the word 'project' and show me a few lines." Expect `obsidian_search_content` results.

6. **Optional: create a note (use a test path)**
   - Send: "Create a note in my vault at path Test/Chai test note with content 'Created by the chai agent.'" Check your vault for the new note.

7. **Stop the gateway** with Ctrl+C when done.

## How to verify the obsidian skill was used

- **Reply content:** The model's reply should reflect vault data or confirm an action. If the model does not call tools, try "Use your Obsidian search tool to…".
- **Logs:** With `RUST_LOG=debug`, tool failures appear as `agent: tool obsidian_search failed: ...` (or other `obsidian_*` tool names).

## Telegram message format for local models (e.g. Llama 3)

One clear intent per message; use wording that matches the tool. Examples:

| Command | What to send (Telegram) |
|--------|--------------------------|
| **Search note names** | "Search my vault for notes with 'meeting' in the name." |
| **Search inside content** | "Search inside the content of my notes for 'deadline'." |
| **Create** | "Create a note in my vault at path Test/My note with content 'Hello world'." |

If the model replies without using a tool, resend with "Use your Obsidian search tool to …". Use a model with tool/function calling (e.g. `llama3.2:latest`).

## Context size (model processing "too much" information)

Every turn the model receives the full system context (skills), full conversation history, and tool definitions. If the combined size is large, the model can be slow or fail to respond.

- **Mitigations:** Prefer a model with a larger context window (e.g. 32K+). Keep skill content concise. For long Telegram chats, starting a new chat can reduce history length.

## If something fails

- **"loaded 0 skill(s)"** — `obsidian` is not on PATH when the gateway starts, or the skills directory is missing. Install and enable the official CLI, ensure it is on PATH, run `chai init` if needed, restart the gateway.
- **Reply has no vault data / model doesn't use tools** — Use a model that supports tool/function calling. Try a more explicit message: "Use your Obsidian search tool to find notes containing X and list them."
- **"agent: tool obsidian_search failed: ..."** — The CLI failed (vault not targeted, binary not found, or permission). Check PATH and vault availability; see the log for the exact error.
- **Create note fails** — The official CLI create command may require the Obsidian app. On headless servers, use search-only for verification if needed.

## Summary

| Step              | Action |
|-------------------|--------|
| 1                 | Confirm `obsidian` on PATH and vault available |
| 2                 | Start gateway; check log for at least one skill loaded |
| 3                 | Send an agent message that asks to search (or create) in the vault |
| 4                 | Verify reply contains search results or action confirmation |
| 5–6 (optional)    | Try search-content or create a test note |

**See also:** [06-skill-notesmd-cli.md](06-skill-notesmd-cli.md) for the notesmd-cli skill ([yakitrak/notesmd-cli](https://github.com/yakitrak/notesmd-cli)).
