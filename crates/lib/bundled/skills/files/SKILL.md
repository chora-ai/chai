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

- When making an edit to a file, use `files_write` with `start_line` and `original_content`
- When making multiple edits in the same file with `files_write`, work from bottom to top
- When using `files_replace` with a common pattern, use `dry_run: true` to preview changes
