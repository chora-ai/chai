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

- When making targeted edits to large notes, use `notes_write_lines` instead of `notes_write`
- When making multiple `notes_write_lines` edits in the same note, work from bottom to top
- When using `notes_replace`, use `max_replacements: 1` to replace only the first match
