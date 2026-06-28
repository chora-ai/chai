---
description: Read, write, replace, delete, list, and search files.
capability_tier: full
metadata:
  requires:
    bins: ["ls", "grep", "chai"]
---

## Skill Directives

- Never assume a file exists — use `files_list` to verify first
- Never read binary files — check file type with `files_list` before reading

## Skill Guidelines

- When making targeted edits to large files, use `files_write_lines` instead of `files_write`
- When making multiple `files_write_lines` edits in the same file, work from bottom to top
- When using `files_replace`, use `max_replacements: 1` to replace only the first match
