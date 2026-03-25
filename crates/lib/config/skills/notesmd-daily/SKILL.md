---
name: notesmd-daily
description: Create, read, and update daily notes.
metadata:
  requires:
    bins: ["notesmd-cli"]
---

#### Skill Directives

- always follow tool instructions step-by-step
- always use YYYY-MM-DD format for today's date
- always use markdown for `content` when calling tools
- always return `content` from calling tools in code blocks
- always use `- [x] ` or `- [ ] ` for checklist items

#### Available Tools

- `notesmd_daily_read`
- `notesmd_daily_update`

#### Tool Instructions

To read a daily note:

1. Call `notesmd_daily_read` with `path` set to today's date.
2. Return the content from the tool call.

To create a daily note:

1. Call `notesmd_daily_read` with `path` set to today's date. If the note exists, do not proceed and return the content.
2. Call `notesmd_update_daily` with `date` set to today's date, `replace` set to true, and `content` set to the new content in markdown format.
3. Call `notesmd_daily_read` again with `path` set to today's date.
4. Return the content from the tool call.

To update a daily note:

1. Call `notesmd_daily_read` with `path` set to today's date.
2. Starting from the tool call's content, apply or append the new content.
3. Call `notesmd_update_daily` with `date` set to the date, `replace` set to `true`, and `content` set to the updated content in markdown format.
4. Call `notesmd_daily_read` again with `path` set to today's date.
5. Return the content from the tool call.
