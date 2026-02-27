---
name: notesmd-cli
description: Create, read, update, and search notes when the user asks.
homepage: https://github.com/yakitrak/notesmd-cli
metadata:
  requires:
    bins: ["notesmd-cli"]
---

# notesmd-cli

The following guidelines are to be followed when using the tool `notesmd-cli`:

- **When to use the tool:** Only when the user asks to create, update, read, or search notes. Do **not** use it for greetings, thanks, praise, or other conversational questions or comments.
- **How to use the tool:** Call the tool (no need to share the structure of the call unless the user asks). Follow the protocols below based on the description of when to use it ("When asked to...").

## When asked to do something

**When asked to search a note or note content:**

1. Do nothing, reply that this functionality needs to be further tested.

**When asked to read a note or note content:**

Read the note and return the content.

1. Call `notesmd_cli_read_note` to retrieve the note.
2. Use the **full** tool response when adding note content to a reply message.

**When asked to create or edit/update a daily note:**

Read the note, make the update, and then read the note again to return the latest response. Use `[x]` (lowercase x, no space) for a complete item and `[ ]` for incomplete—never `[X ]` or `[X]`.

1. Call `notesmd_cli_read_note` to retrieve the note.
  - Include `path` with today's date (e.g. `2026-02-25`)
2. Call `notesmd_cli_update_daily` using the same date provided in step 1
  - Include `replace` with the boolean `true`
  - Include `content` with the note from step 1 but with the requested changes applied
3. Call `notesmd_cli_read_note` again to retrieve the note.
  - Use the **full** tool response when adding note content to a reply message.

**When asked to create or edit/update a note that is not a daily note:**

1. Do nothing, reply that this functionality needs to be further tested.

---

End of notesmd-cli guidelines.
