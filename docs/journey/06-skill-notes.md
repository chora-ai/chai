# Journey: Skill — Notes

**Goal:** Confirm the **notes** skill is loaded, the agent can call its tools to create, read, search, append to, list, and delete notes, and the write sandbox enforces the boundary.

**Background:** [Skills](../guides/06-skills.md) · [Write Sandbox](../guides/07-sandbox.md)

This journey covers the **`notes`** skill (notes CRUD). The root is the active profile's sandbox directory — the same location the `files` skill operates on, but the `notes` tools are designed for markdown and include frontmatter conventions.

**Extended skills:** The `notes` skill has three companion skills that add specialized capabilities:
- **`notes-daily`** — Read, write, and append to daily notes with date-based path resolution.
- **`notes-frontmatter`** — Read, edit, and delete YAML frontmatter keys without touching note body content.
- **`notes-wikilink`** — Discover relationships between notes via backlinks, outlinks, tag search, and broken link detection, and rename notes with automatic wikilink updates.

These are separate skill packages, enabled independently in `skillsEnabled`. The core `notes` journey below focuses on the `notes` skill; the extended skills are mentioned where relevant.

## Prerequisites

- **Setup complete** — You have installed chai, run `chai init`, and verified Ollama is available (see [00-setup-init.md](00-setup-init.md)).
- **Ollama** running with a model that supports **tool/function calling** (e.g. `llama3.2:3b`).
- **`notes` skill enabled** — In `~/.chai/profiles/<active>/config.json`, add `"notes"` to the `skillsEnabled` array:
  ```json
  {
    "agents": [
      {
        "id": "orchestrator",
        "role": "orchestrator",
        "skillsEnabled": ["notes"]
      }
    ]
  }
  ```
  Add `"notes-daily"`, `"notes-frontmatter"`, or `"notes-wikilink"` as needed for the extended steps.
- **Gateway** will be started after the above so it loads the notes skill.

## Steps

1. **Confirm the notes skill is enabled**
   - Check your config: `cat ~/.chai/profiles/assistant/config.json`
   - **Expect:** `skillsEnabled` includes `"notes"`.

2. **Start the gateway**
   - `chai gateway` (or `cargo run -p cli -- gateway`). Optional: `RUST_LOG=info`.
   - **Expect:** A log line like `loaded 1 skill(s) for agent context` (or more — add 1 per enabled skill). If you see `loaded 0 skill(s)`, the skill is not in `skillsEnabled` or the config was not saved correctly.

3. **List the directory**
   - Send an agent message: "List the contents of the root directory."
   - **Expect:** The agent uses `notes_list` and describes the directory contents (may include `AGENTS.md`, `README.md`, or other seeded files).

4. **Create a note**
   - Send: "Create a note at path test-note.md with content '---\ntype: test\n---\n\n# Test Note\n\nThis is a test note from the notes skill.'"
   - **Expect:** The agent uses `notes_write`. Verify: `cat ~/.chai/profiles/assistant/sandbox/test-note.md` should contain the frontmatter and body.

5. **Read the note**
   - Send: "Read the note at test-note.md."
   - **Expect:** The agent uses `notes_read` and returns the full content including frontmatter.

6. **Search for content**
   - Send: "Search all notes for the word 'test' and show me the results."
   - **Expect:** The agent uses `notes_search` and returns matching lines with line numbers.

7. **Append to the note**
   - Send: "Append a new section to test-note.md with the content '\n## Added Section\n\nThis was appended later.'"
   - **Expect:** The agent uses `notes_append`. Verify: the file now includes the added section at the end.

8. **Bulk find-and-replace**
   - First, create a note with repeated patterns: Send: "Create a note at versions.md with content '# Versions\n\nrelease = \"1.0.0\"\nrelease = \"2.0.0\"\n'."
   - Then send: "Use notes_replace to replace all occurrences of `release = \"(\d+)\.(\d+)\.(\d+)\"` with `release = \"$1.$2.99\"` in versions.md."
   - **Expect:** The agent uses `notes_replace` with capture groups. Both release lines are updated in a single call. The diff shows both changes. Verify: both lines in the note should now end in `.99`.

9. **Delete the note**
   - Send: "Delete the note at test-note.md."
   - **Expect:** The agent uses `notes_delete`. Verify: `ls ~/.chai/profiles/assistant/sandbox/test-note.md` should fail (file not found).

10. **Stop the gateway** with Ctrl+C when done.

## Extended: notes-daily (optional)

If you enabled `notes-daily`, try these additional steps after step 8:

- **Create today's daily note:** "Create today's daily note with a tasks section."
  - **Expect:** The agent uses `notes_daily_write`. The note is stored in the configured daily folder (default `00-daily/`).
- **Append to today's daily note:** "Add an insight to today's daily note: discovered the notes skill works."
  - **Expect:** The agent uses `notes_daily_append`.
- **Read a past daily note:** "Read the daily note for 2025-01-01."
  - **Expect:** The agent uses `notes_daily_read` with a date parameter. If no note exists for that date, the tool returns an error.

## Extended: notes-frontmatter (optional)

If you enabled `notes-frontmatter`, try these after creating a note with frontmatter:

- **Read frontmatter:** "Read the frontmatter of the note at test-note.md."
  - **Expect:** The agent uses `notes_frontmatter_read` and returns the YAML key-value pairs.
- **Edit a frontmatter key:** "Set the frontmatter key 'status' to 'active' in test-note.md."
  - **Expect:** The agent uses `notes_frontmatter_edit`. The note's frontmatter now includes `status: active` and the body is unchanged.
- **Delete a frontmatter key:** "Remove the frontmatter key 'type' from test-note.md."
  - **Expect:** The agent uses `notes_frontmatter_delete`. The key is removed; other frontmatter and the body are preserved.

## Extended: notes-wikilink (optional)

If you enabled `notes-wikilink` (and have notes with `[[wikilink]]` syntax):

- **Find backlinks:** "Find all notes that link to 'Conventions'."
  - **Expect:** The agent uses `notes_wikilink_backlinks` with `note_name`.
- **Check for broken links:** "Check for broken wikilinks in the note at 01-admin/AI Assistant.md."
  - **Expect:** The agent uses `notes_wikilink_broken`. An empty result means all links resolve.
- **Find notes by tag:** "Find all notes tagged with 'agentic-systems'."
  - **Expect:** The agent uses `notes_wikilink_by_tag`.
- **Rename a note with link updates:** "Rename the note from '00-inbox/Old Name.md' to '03-research/New Name.md' and update all wikilinks."
  - **Expect:** The agent uses `notes_wikilink_rename`. The file moves and all `[[Old Name]]` references are updated.

## How to Verify the notes Skill Was Used

- **Reply content:** The model's reply should reflect actual note data or confirm actions. If the model does not call tools, try a more explicit message: "Use the notes_list tool to list the root directory."
- **Logs:** With `RUST_LOG=debug`, tool calls and results are visible. Tool failures appear as `agent: tool notes_write failed: ...` (or other `notes_*` tool names).
- **Filesystem:** Write and delete operations can be verified by checking the sandbox directory directly.

## Context Size

Every turn the model receives the full system context (skills), full conversation history, and tool definitions. If the combined size is large, the model can be slow or fail to respond.

- **Mitigations:** Prefer a model with a larger context window (e.g. 32K+). Keep skill content concise. For long chats, type `/new` to start a fresh session.

## If Something Fails

- **"loaded 0 skill(s)"** — The `notes` skill is not in `skillsEnabled` on the orchestrator agent. Edit `config.json` to add it, then restart the gateway.
- **Agent does not use tools** — Use a model that supports tool/function calling. Try a more explicit message: "Use the notes_search tool to search all notes for 'test'."
- **"agent: tool notes_write failed: path not in writable roots"** — The note path resolved outside the sandbox. The notes skill resolves paths relative to the root directory (the sandbox directory). Ensure you are not requesting an absolute path.
- **Note not found after write** — The note may be in the sandbox directory under a different path than expected. Check `~/.chai/profiles/<active>/sandbox/` for the file.
- **notes-daily returns error** — The daily notes folder may not exist. The resolver will create the file but the parent directory must exist. Create `00-daily/` in the sandbox if needed.
- **notes-frontmatter error on a note without frontmatter** — Some operations (like `notes_frontmatter_read`) require the note to have existing frontmatter. Create frontmatter first with `notes_write`, then use frontmatter tools to edit it.
- **notes-wikilink finds no results** — This is expected if no notes in the directory use `[[wikilink]]` syntax. Create a few notes with wikilinks to test backlinks and broken link detection.

## Summary

| Step | Action | Expected Outcome |
|------|--------|-------------------|
| 1 | Confirm `notes` in `skillsEnabled` | Config includes the skill |
| 2 | `chai gateway` | At least 1 skill loaded |
| 3 | "List the files" | Agent lists directory contents |
| 4 | "Create a note" | Note created in sandbox |
| 5 | "Read the note" | Agent returns full content |
| 6 | "Search for 'test'" | Agent returns matches |
| 7 | "Append to the note" | Section added at end |
| 8 | "Bulk replace in versions.md" | Both release lines updated via `notes_replace` |
| 9 | "Delete the note" | Note removed |
| 10 | Ctrl+C | Gateway stops |

**See also:** [05 — Skill: Files](05-skill-files.md) · [07 — Skill: Skills](07-skill-skills.md)