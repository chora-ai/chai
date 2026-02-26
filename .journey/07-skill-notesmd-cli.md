# Journey: Skill NotesMD CLI

**Goal:** Confirm the **notesmd-cli** skill is loaded, the agent can call its tools (search, search-content, create, daily), and the reply reflects vault data or confirms an action.

This journey is for the binary **`notesmd-cli`** only. For the official Obsidian CLI (early access, binary `obsidian`), see [06-skill-obsidian.md](06-skill-obsidian.md).

## Prerequisites

- **`chai init`** has been run (so bundled skills exist; if you updated from an older install, ensure the `notesmd-cli` skill directory is present under your bundled or workspace skills).
- **notesmd-cli** — The `notesmd-cli` command is on your PATH (e.g. Homebrew: `brew install yakitrak/yakitrak/notesmd-cli`). Check with `which notesmd-cli`. Set a default vault if needed: `notesmd-cli set-default "{vault-name}"`.
- **Vault** is available to the CLI (see “Multiple vaults” below).
- **Ollama** is running with a model that supports **tool/function calling** (e.g. `llama3.2:latest`).
- **Gateway** will be started after the above so it sees `notesmd-cli` and loads the notesmd-cli skill.

**Multiple vaults:** The skill does not pass a vault name or path to the CLI. The CLI uses whatever it considers the default or current vault. Set a default with `notesmd-cli set-default` or configure the CLI as needed; the chai tool layer does not support a per-request vault parameter.

## Steps

1. **Confirm notesmd-cli is available**
   - Run: `which notesmd-cli`. Ensure you have at least one vault and, if needed, set a default vault.

2. **Start the gateway**
   - From repo root: `cargo run -p cli -- gateway` or `chai gateway`. Optional: `RUST_LOG=info`.
   - **Expect:** A log line like `loaded 1 skill(s) for agent context` (or more). If you see `loaded 0 skill(s)`, `notesmd-cli` is not on PATH when the gateway starts, or the bundled skills directory is missing (run `chai init`).

3. **Trigger the agent with an Obsidian-style request**
   - **Via WebSocket:** Connect and send `connect`, then an agent request (see [02-gateway-ws-agent.md](02-gateway-ws-agent.md)). Example: `{"type":"req","id":"2","method":"agent","params":{"message":"Search my vault for note names that contain 'meeting' and list them."}}`
   - **Via Telegram:** Send the same message to your bot (journey 05). Example: "Search my vault for notes about meetings."

4. **Verify the reply**
   - The model should describe search results (note names or snippets) or say it found nothing. The model uses tools `notesmd_cli_search`, `notesmd_cli_search_content`, `notesmd_cli_create`, `notesmd_cli_daily`.
   - With `RUST_LOG=debug`, tool failures appear as `agent: tool notesmd_cli_search failed: ...`.

5. **Optional: search inside note content**
   - Send: "Search inside the content of my notes for the word 'project' and show me a few lines." Expect `notesmd_cli_search_content` results.

6. **Optional: create a note (use a test path)**
   - Send: "Create a note in my vault at path Test/Chai test note with content 'Created by the chai agent.'" Check your vault for the new note.

7. **Optional: daily note**
   - Send: "Open my daily note for today" or "Create today's daily note." Expect `notesmd_cli_daily` to run.

8. **Stop the gateway** with Ctrl+C when done.

## How to verify the notesmd-cli skill was used

- **Reply content:** The model’s reply should reflect vault data or confirm an action. If the model does not call tools, try "Use the notesmd_cli search tool to…".
- **Logs:** With `RUST_LOG=debug`, tool failures appear as `agent: tool notesmd_cli_search failed: ...` (or other `notesmd_cli_*` tool names).

## Telegram message format for local models (e.g. Llama 3)

One clear intent per message; use wording that matches the tool. Examples:

| Command | What to send (Telegram) |
|--------|--------------------------|
| **Search note names** | "Search my vault for notes with 'meeting' in the name." |
| **Search inside content** | "Search inside the content of my notes for 'deadline'." |
| **Create** | "Create a note in my vault at path Test/My note with content 'Hello world'." |
| **Daily note** | "Open my daily note for today" or "Create today's daily note." |

If the model replies without using a tool, resend with "Use the notesmd_cli search tool to …". Use a model with tool/function calling (e.g. `llama3.2:latest`).

## Context size (model processing “too much” information)

Every turn the model receives the full system context (skills), full conversation history, and tool definitions. If the combined size is large, the model can be slow or fail to respond.

- **Mitigations:** Prefer a model with a larger context window (e.g. 32K+). Keep skill content concise. For long Telegram chats, starting a new chat can reduce history length.

## If something fails

- **"loaded 0 skill(s)"** — `notesmd-cli` is not on PATH when the gateway starts, or the bundled skills directory is missing. Install `notesmd-cli`, ensure it is on PATH, run `chai init` if needed, restart the gateway.
- **Reply has no vault data / model doesn’t use tools** — Use a model that supports tool/function calling. Try a more explicit message: "Use the notesmd_cli search tool to find notes containing X and list them."
- **"agent: tool notesmd_cli_search failed: ..."** — The CLI failed (vault not set, binary not found, or permission). Run `notesmd-cli set-default` if you have multiple vaults; check PATH and vault availability; see the log for the exact error.
- **Model says "I don't have direct access to your notes" or similar** — The model may not be calling the tools. (1) Confirm the skill is loaded: gateway log should show `loaded 1 skill(s)` (or more) and `notesmd-cli` must be on PATH when the gateway starts; if you use `skills.disabled`, ensure you disabled `obsidian` not `notesmd-cli`. (2) Use an explicit message with a path and content, e.g. "Create a note in my vault at path Test/Hello with content 'hello'."

## Summary

| Step              | Action |
|-------------------|--------|
| 1                 | Confirm `notesmd-cli` on PATH and vault available (set default if needed) |
| 2                 | Start gateway; check log for at least one skill loaded |
| 3                 | Send an agent message that asks to search (or create) in the vault |
| 4                 | Verify reply contains search results or action confirmation |
| 5–7 (optional)    | Try search-content, create a test note, or daily note |

**See also:** [06-skill-obsidian.md](06-skill-obsidian.md) for the official obsidian skill (early access binary `obsidian`).
