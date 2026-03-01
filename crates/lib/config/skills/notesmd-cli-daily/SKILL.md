---
name: notesmd-cli-daily
description: Create, read, and update daily notes via notesmd-cli. Use when the user asks for their daily note, daily tasks, today's note, or to create or edit a daily note.
metadata:
  requires:
    bins: ["notesmd-cli"]
---

**When the user asks for their daily note, daily tasks, today's note:**

1. Call `notesmd_cli_read_note` with `path` set to the date. If they said "today" or didn't specify a date, use today's date in YYYY-MM-DD.
2. In your very next message to the user, include the **full content** from the tool response so they see their note or task list. Do not summarize or say "here's what I found" without showing the actual text—paste the note content into your reply.

**When the user asks to create, edit, or update a daily note:**

Use `- [x] ` and `- [ ] ` for task state (lowercase x, no space inside the brackets).

1. Call `notesmd_cli_read_note` with `path` = date.
2. Call `notesmd_cli_update_daily` with the same `date`, `replace: true`, and `content` = the note from step 1 with the user's changes applied.
3. Call `notesmd_cli_read_note` again and include the full tool response in your reply.

**When the user asks you to read, print, or share a daily note:**

1. Call `notesmd_cli_daily` when the user asks to open today's note. For reading content back to the user, use `notesmd_cli_read_note` and include the content in your reply as above.
