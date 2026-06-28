---
description: Read files, list directories, and search file contents (read-only).
capability_tier: minimal
variant_of: files
metadata:
  requires:
    bins: ["ls", "grep", "chai"]
---

## Skill Directives

- Never assume a file exists — use `files_list` to verify first
- Never read binary files — check file type with `files_list` before reading
