---
name: obsidian-daily
description: Create daily notes via the official Obsidian CLI. Use when the user asks to create today's daily note or a daily note for a date.
metadata:
  requires:
    bins: ["obsidian"]
---

# obsidian-daily

Only when the user asks to create a daily note:

Call `obsidian_create` with `path` set to the daily note path (e.g. `Daily/2026-02-25` or `2026-02-25` depending on vault layout) and optional `content`. Use today's date in YYYY-MM-DD format when the user says "today" or "my daily note."

Read and update of daily notes are not covered by this skill; use the full obsidian skill if the CLI supports it.
