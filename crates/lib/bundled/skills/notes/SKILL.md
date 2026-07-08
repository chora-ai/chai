---
description: Read, write, edit, replace, delete, list, and search notes.
capability_tier: full
metadata:
  requires:
    bins: ["ls", "grep", "chai"]
---

## Skill Directives

- Never assume a note exists — use `notes_list` to verify first

## Skill Guidelines

- When making multiple edits in the same note with `notes_edit`, work from bottom to top
