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

- When making an edit to a note, use `notes_write` with `start_line` and `original_content`
- When making multiple edits in the same note with `notes_write`, work from bottom to top
- When using `notes_replace` with a common pattern, use `dry_run: true` to preview changes
