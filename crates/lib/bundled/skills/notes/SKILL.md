---
description: Read, write, replace, delete, list, and search notes.
capability_tier: full
metadata:
  requires:
    bins: ["ls", "grep", "chai"]
---

## Skill Directives

- Never assume a note exists — use `notes_list` to verify first

## Skill Guidelines

- When making an edit to a large note, use `notes_write_lines` instead of `notes_write`
- When making multiple `notes_write_lines` edits in the same note, work from bottom to top
- When using `notes_replace` with common patterns, use `dry_run: true` to preview changes
