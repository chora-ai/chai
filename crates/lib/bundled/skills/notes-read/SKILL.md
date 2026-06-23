---
description: Read notes, list directories, and search note contents (read-only).
capability_tier: minimal
variant_of: notes
metadata:
  requires:
    bins: ["cat", "ls", "grep", "chai"]
---

## Skill Directives

- never assume a note exists — use `notes_list` to verify first
- after using `notes_search` with `line_number: true` to find relevant lines, use `notes_read_lines` to read context around those lines
