---
description: Read, write, replace, delete, list, and search notes.
capability_tier: full
metadata:
  requires:
    bins: ["cat", "ls", "grep", "chai"]
---

## Skill Directives

- never delete notes without confirming the action is intended
- never assume a note exists — use `notes_list` to verify first
- always read a note with `notes_read` before overwriting it with `notes_write` to avoid data loss — `notes_write` is a complete overwrite, not a patch
- when making multiple non-adjacent `notes_write_lines` edits in the same note, work from bottom to top (highest line numbers first)
- after using `notes_search` with `line_number: true` to find relevant lines, use `notes_read_lines` to read context around those lines
- use `notes_write_lines` for targeted edits; use `notes_replace` for bulk find-and-replace across a note
