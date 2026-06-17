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
- prefer rewriting an entire affected section as a single `notes_write_lines` call over making multiple small targeted edits to the same note
- when making multiple non-adjacent `notes_write_lines` edits in the same note, work from bottom to top (highest line numbers first)
- after using `notes_search` with `line_numbers: true` to find relevant lines, use `notes_read_lines` to read context around those lines
- use `notes_replace` for bulk find-and-replace across a note; use `notes_write_lines` for targeted edits where surrounding context must be verified before replacement
- use `literal: true` on `notes_replace` when the pattern contains regex metacharacters (`|`, `()`, `[]`, `.`, `*`, `+`, `?`, `{}` — common in source code, markdown tables, JSON, and URLs) that should be matched as-is
