# Journey: Skill — Skills

**Goal:** Confirm the **skills** skill is loaded, the agent can inspect, validate, and manage skill packages, and can create a minimal custom skill from scratch.

**Background:** [Skills](../guides/06-skills.md)

This journey covers the **`skills`** skill (full read + write + delete for skill packages). The read-only variant **`skills-read`** provides the same inspection tools (`skills_list`, `skills_read`, `skills_validate`) without the write, init, or delete surface. See [Skill Variants](../guides/06-skills.md#skill-variants) for guidance on choosing between them.

**Related skill:** The **`skills-design`** skill is a context-only skill (no callable tools) that provides design principles for authoring quality skills — tools over inference, surface reduction, SKILL.md sizing. It is automatically loaded into the agent's context when enabled. Consider enabling it alongside `skills` when the agent is creating or modifying skills.

## Prerequisites

- **Setup complete** — You have installed chai, run `chai init`, and verified Ollama is available (see [00-setup-init.md](00-setup-init.md)).
- **Ollama** running with a model that supports **tool/function calling** (e.g. `llama3.2:3b`).
- **`skills` skill enabled** — In `~/.chai/profiles/<active>/config.json`, add `"skills"` to the `enabledSkills` array:
  ```json
  {
    "agents": [
      {
        "id": "orchestrator",
        "role": "orchestrator",
        "enabledSkills": ["skills"]
      }
    ]
  }
  ```
  Optionally add `"skills-design"` to have design principles available in the agent's context.
- **Gateway** will be started after the above so it loads the skills skill.

## Steps

1. **Confirm the skills skill is enabled**
   - Check your config: `cat ~/.chai/profiles/assistant/config.json`
   - **Expect:** `enabledSkills` includes `"skills"`.

2. **Start the gateway**
   - `chai gateway` (or `cargo run -p cli -- gateway`). Optional: `RUST_LOG=info`.
   - **Expect:** A log line like `loaded 1 skill(s) for agent context` (or more). If you see `loaded 0 skill(s)`, the skill is not in `enabledSkills` or the config was not saved correctly.

3. **List installed skills**
   - Send an agent message: "List all installed skills."
   - **Expect:** The agent uses `skills_list` and returns a table showing each skill's name, SKILL.md status, tools.json/allowlist.json/execution.json status, and tool count. Bundled skills like `files`, `notes`, `git`, etc. should appear.

4. **Read a skill's definition**
   - Send: "Read the SKILL.md for the 'notes' skill."
   - **Expect:** The agent uses `skills_read` with `file: "skill_md"` and returns the full content of the notes skill's SKILL.md.
   - Then: "Read the tools.json, allowlist.json, and execution.json for the 'notes' skill."
   - **Expect:** The agent uses `skills_read` with `file: "tools_json"`, `file: "allowlist_json"`, and `file: "execution_json"` and returns the tool definitions, security grants, and execution mapping from the three separate files.

5. **Validate a skill**
   - Send: "Validate the 'notes' skill."
   - **Expect:** The agent uses `skills_validate` and reports the result. A conformant skill shows "PASS"; errors or warnings are listed if found.

6. **Preview a tool's execution mapping**
   - Send: "Preview what the files_write tool would execute with args '{\"path\": \"test.txt\", \"content\": \"hello\"}'."
   - **Expect:** The agent uses `skills_dry_run` with `tool: "files_write"` and the args, and returns a JSON result showing the argv mapping, sandbox validation status, and resolved parameter values. The `binary` should be `"chai"` and `subcommand` should be `"file write"`.

7. **Discover a CLI interface**
   - Send: "Discover the interface of the 'git' binary."
   - **Expect:** The agent uses `skills_discover` with `binary: "git"` and returns the subcommands available. The discover tool runs the binary's `--help` output.

8. **Create a custom skill**
   - Send: "Initialize a new skill called 'test-skill' with the description 'A test skill for the journey.'"
   - **Expect:** The agent uses `skills_init` and reports success.
   - Verify: `chai skill list` should now show `test-skill`.

9. **Write the skill's tool descriptor files**
   - Send: "Write the tool descriptor files for 'test-skill': tools.json with a single tool called 'test-skill_echo' with a 'message' parameter, allowlist.json allowing the 'echo' binary with an empty subcommand, and execution.json mapping the tool to echo with a positional argument for the message parameter."
   - **Expect:** The agent uses `skills_write_tools_json`, `skills_write_allowlist_json`, and `skills_write_execution_json` and reports success for each.
   - Then: "Validate the 'test-skill' skill."
   - **Expect:** The agent uses `skills_validate` and reports "PASS" or lists any issues to fix.

10. **Write the skill's SKILL.md**
   - Send: "Write a SKILL.md for 'test-skill' with frontmatter description 'A test skill for the journey.', capability_tier 'minimal', and metadata.requires.bins set to ['echo']. The body should have a heading 'Test Skill' and list the 'test-skill_echo' tool."
   - **Expect:** The agent uses `skills_write_skill_md` and reports success.

11. **Verify the custom skill loads**
    - Stop the gateway (Ctrl+C), then restart it: `chai gateway`.
    - **Expect:** The gateway starts, and `test-skill` is included in the loaded skill count (if it is also added to `enabledSkills`). Alternatively, confirm the skill exists: `chai skill list` should show `test-skill`.

12. **Clean up: delete the test skill**
    - Send: "Delete the 'test-skill' skill."
    - **Expect:** The agent uses `skills_delete` and reports success.
    - Verify: `chai skill list` should no longer show `test-skill`.

13. **Stop the gateway** with Ctrl+C when done.

## How to Verify the Skills Skill Was Used

- **Reply content:** The model's reply should reflect actual skill metadata or confirm actions. If the model does not call tools, try a more explicit message: "Use the skills_list tool to show all installed skills."
- **Logs:** With `RUST_LOG=debug`, tool calls and results are visible. Tool failures appear as `agent: tool skills_validate failed: ...` (or other `skills_*` tool names).
- **CLI verification:** Use `chai skill list`, `chai skill read`, and `chai skill validate` from a terminal to cross-check what the agent reports.

## Context Size

Every turn the model receives the full system context (AGENT.md, workers roster, skills), the session history (including tool calls and results), and tool schemas (sent as a separate field from the messages). If the combined size is large, the model can be slow or fail to respond.

- **Mitigations:** Prefer a model with a larger context window (e.g. 32K+). Keep skill content concise. For long chats, type `/new` to start a fresh session.

## If Something Fails

- **"loaded 0 skill(s)"** — The `skills` skill is not in `enabledSkills` on the orchestrator agent. Edit `config.json` to add it, then restart the gateway.
- **Agent does not use tools** — Use a model that supports tool/function calling. Try a more explicit message: "Use the skills_list tool to show all installed skills."
- **`skills_init` fails with "already exists"** — A skill with that name already exists. Choose a different name or delete the existing one first.
- **`skills_validate` reports errors** — The tool descriptor files may have structural issues or cross-file inconsistencies. The agent (or you) can use `skills_read` with `file: "tools_json"`, `file: "allowlist_json"`, or `file: "execution_json"` to inspect the content and identify the problem. Fix the JSON and write again.
- **`skills_write_tools_json` reports JSON parse error** — The content is not valid JSON. This can happen when the model's output is malformed. Re-send the request with the correct JSON structure.
- **Custom skill not loaded after restart** — The skill was created but not added to `enabledSkills` in `config.json`. Add it to the array and restart the gateway.
- **`skills_delete` fails** — The skill name may be wrong. Use `skills_list` first to confirm the exact directory name.
- **Agent deletes a bundled skill** — The `skills_delete` tool rejects bundled skill names via `denyPattern`. If a bundled skill is somehow removed, re-running `chai init` will restore the bundled skill snapshots.

## Summary

| Step | Action | Expected Outcome |
|------|--------|-------------------|
| 1 | Confirm `skills` in `enabledSkills` | Config includes the skill |
| 2 | `chai gateway` | At least 1 skill loaded |
| 3 | "List installed skills" | Agent returns skill inventory |
| 4 | "Read notes SKILL.md and tool descriptor files" | Agent returns skill definitions |
| 5 | "Validate the notes skill" | Agent reports PASS or lists issues |
| 6 | "Preview files_write execution mapping" | Agent returns argv, sandbox validation, resolved params |
| 7 | "Discover the git binary" | Agent returns git subcommands |
| 8 | "Init test-skill" | Skill directory created |
| 9 | "Write tool descriptor files for test-skill" | Tool descriptor files written and validated |
| 10 | "Write SKILL.md for test-skill" | Skill instructions written |
| 11 | Restart gateway | Custom skill loads (if in enabledSkills) |
| 12 | "Delete test-skill" | Skill removed |
| 13 | Ctrl+C | Gateway stops |

**See also:** [05 — Skill: Files](05-skill-files.md) · [06 — Skill: Notes](06-skill-notes.md)
