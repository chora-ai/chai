---
description: Read, write, edit, replace, delete, list, and search files.
capability_tier: full
metadata:
  requires:
    bins: ["ls", "grep", "chai"]
---

## Skill Directives

- Never assume a file exists — use `files_list` to verify first
- Never read binary files — check file type with `files_list` before reading

## Skill Guidelines

- When making multiple edits in the same file with `files_edit`, work from bottom to top
