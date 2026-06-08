# Journey: Skill — Files

**Goal:** Confirm the **files** skill is loaded, the agent can call its tools to read, write, patch, search, and delete files, and the write sandbox enforces path boundaries.

**Background:** [Skills](../guides/06-skills.md) · [Write Sandbox](../guides/07-sandbox.md)

This journey covers the **`files`** skill (full read + write + delete). The read-only variant **`files-read`** provides the same read tools (`files_read_file`, `files_read_lines`, `files_list_dir`, `files_search_content`) without the write/delete surface. See [Skill Variants](../guides/06-skills.md#skill-variants) for guidance on choosing between them.

## Prerequisites

- **Setup complete** — You have installed chai, run `chai init`, and verified Ollama is available (see [00-setup-init.md](00-setup-init.md)).
- **Ollama** running with a model that supports **tool/function calling** (e.g. `llama3.2:3b`).
- **`files` skill enabled** — In `~/.chai/profiles/<active>/config.json`, add `"files"` to the `skillsEnabled` array on the orchestrator agent:
  ```json
  {
    "agents": [
      {
        "id": "orchestrator",
        "role": "orchestrator",
        "skillsEnabled": ["files"]
      }
    ]
  }
  ```
- **Gateway** will be started after the above so it loads the files skill.

## Steps

1. **Confirm the files skill is enabled**

   - Check your config: `cat ~/.chai/profiles/assistant/config.json`
   - **Expect:** `skillsEnabled` includes `"files"`.

2. **Start the gateway**
   - `chai gateway` (or `cargo run -p cli -- gateway`). Optional: `RUST_LOG=info`.
   - **Expect:** A log line like `loaded 1 skill(s) for agent context` (or more). If you see `loaded 0 skill(s)`, the skill is not in `skillsEnabled` or the config was not saved correctly.

3. **Read a file**
   - Send an agent message: "List the files in the current directory, then read the config.json file."
   - **Expect:** The agent uses `files_list_dir` and `files_read_file` to list the sandbox and read the file. The reply includes the directory listing and file contents.

4. **Search for content**
   - Send: "Search for the word 'agent' in all files in this directory and show me the matches with line numbers."
   - **Expect:** The agent uses `files_search_content` with `line_numbers: true`. The reply shows matching lines with line numbers.

5. **Write a file**
   - Send: "Create a file called test-note.md with the content '# Test Note\n\nHello from the files skill.'"
   - **Expect:** The agent uses `files_write_file`. Verify the file exists: `cat ~/.chai/profiles/assistant/sandbox/test-note.md`.
   - **Expect:** The file contains the heading and body text.

6. **Patch a file (write specific lines)**
   - Send: "In test-note.md, replace the line 'Hello from the files skill.' with 'Updated by the files skill.'"
   - **Expect:** The agent uses `files_read_lines` to get `original_content`, then `files_write_lines` to patch the file. The reply confirms the change. Verify: `cat ~/.chai/profiles/assistant/sandbox/test-note.md` should show "Updated by the files skill."

7. **Delete a file**
   - Send: "Delete the file test-note.md."
   - **Expect:** The agent uses `files_delete_file`. Verify: `ls ~/.chai/profiles/assistant/sandbox/test-note.md` should fail (file not found).

8. **Verify sandbox enforcement (optional)**
   - Send: "Write a file at /tmp/outside-sandbox.txt with content 'test'."
   - **Expect:** The tool call is rejected — the path is outside the sandbox root. The agent reports the rejection (path not in writable roots).

9. **Stop the gateway** with Ctrl+C when done.

## How to Verify the Files Skill Was Used

- **Reply content:** The model's reply should reflect actual file data or confirm actions. If the model does not call tools, try a more explicit message: "Use the files_list_dir tool to list the current directory."
- **Logs:** With `RUST_LOG=debug`, tool calls and their results are visible. Tool failures appear as `agent: tool files_write_file failed: ...` (or other `files_*` tool names).
- **Filesystem:** Write and delete operations can be verified by checking the sandbox directory directly.

## Context Size

Every turn the model receives the full system context (skills), full conversation history, and tool definitions. If the combined size is large, the model can be slow or fail to respond.

- **Mitigations:** Prefer a model with a larger context window (e.g. 32K+). Keep skill content concise. For long chats, type `/new` to start a fresh session.

## If Something Fails

- **"loaded 0 skill(s)"** — The `files` skill is not in `skillsEnabled` on the orchestrator agent. Edit `config.json` to add it, then restart the gateway.
- **Agent does not use tools** — Use a model that supports tool/function calling. Try a more explicit message: "Use the files_search_content tool to find files containing 'config' and show me the results."
- **"agent: tool files_write_file failed: path not in writable roots"** — The file path is outside the sandbox. All write operations target the sandbox directory (`~/.chai/profiles/<active>/sandbox/`). Use relative paths; the skill's directives instruct the model to use `./`-relative paths.
- **"agent: tool files_write_lines failed: original_content mismatch"** — The file changed between the read and the write. The agent should re-read and retry; this is expected behavior for the verification mechanism.
- **File not found after write** — The file may have been written inside the sandbox but you checked the wrong path. Check `~/.chai/profiles/<active>/sandbox/` for the file.
- **Agent writes to wrong path** — The model may have used an absolute path. The skill's directives instruct relative paths, but models vary in compliance. Check the sandbox root for the file.

## Summary

| Step | Action | Expected Outcome |
|------|--------|-------------------|
| 1 | Confirm `files` in `skillsEnabled` | Config includes the skill |
| 2 | `chai gateway` | At least 1 skill loaded |
| 3 | "List files, read config.json" | Agent lists directory and reads file |
| 4 | "Search for 'agent'" | Agent returns matches with line numbers |
| 5 | "Create test-note.md" | File created in sandbox |
| 6 | "Patch test-note.md" | Line replaced; file updated |
| 7 | "Delete test-note.md" | File removed |
| 8 | "Write to /tmp/…" (optional) | Rejected — sandbox enforcement |
| 9 | Ctrl+C | Gateway stops |

**Next:** [06 — Skill: Knowledge Base](06-skill-kb.md) · [07 — Skill: Skills](07-skill-skills.md)
