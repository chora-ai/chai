---
description: Read files, list directories, and search file contents (read-only).
capability_tier: minimal
variant_of: files
metadata:
  requires:
    bins: ["cat", "ls", "grep", "chai"]
---

## Skill Directives

- never assume a file exists — use `files_list` to verify first
- never read binary files — check file type with `files_list` before reading
- after using `files_search` with `line_number: true` to find relevant lines, use `files_read_lines` to read context around those lines
