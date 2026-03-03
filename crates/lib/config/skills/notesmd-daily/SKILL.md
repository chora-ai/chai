---
name: notesmd-daily
description: Create, read, and update daily notes.
metadata:
  requires:
    bins: ["notesmd-cli"]
---

**When the user asks for their daily note, daily tasks, today's note:**

1. Call `notesmd_daily_read` with `path` set to the date. If the user said "today" or didn't specify a date, use today's date in YYYY-MM-DD format.
2. Include the **full content** from the tool response in your reply.

**When the user asks to create, edit, or update a daily note:**

Use `- [x] ` and `- [ ] ` for tasks, agendas, and checklists (lowercase x, no whitespace inside the brackets).

1. Call `notesmd_daily_read` with `path` set to the date.
2. Modify the full content returned from the tool call with the changes requested by the user.
2. Call `notesmd_cli_update_daily` with `date` set to the date, `replace` set to `true`, and `content` set the the modified content from step 2.
3. Call `notesmd_daily_read` again and include the **full content** from the tool repsonse in your reply.
