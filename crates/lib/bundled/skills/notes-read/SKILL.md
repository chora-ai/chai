---
description: Read notes, list directories, and search note contents (read-only).
capability_tier: minimal
variant_of: notes
metadata:
  requires:
    bins: ["cat", "ls", "grep", "chai"]
---

## Skill Directives

- Never assume a note exists — use `notes_list` to verify first
