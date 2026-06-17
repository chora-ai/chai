---
description: Read files, list directories, search file contents, write files, replace patterns, and delete files and directories.
capability_tier: full
metadata:
  requires:
    bins: ["cat", "ls", "grep", "chai"]
---

## Skill Directives

- never delete files without confirming the action is intended
- never assume a file exists — use `files_list_dir` to verify first
- never read binary files — check file type with `files_list_dir` before reading
- always read a file with `files_read_file` before overwriting it with `files_write_file`
- prefer rewriting an entire affected section as a single `files_write_lines` call over making multiple small targeted edits to the same file
- when making multiple non-adjacent `files_write_lines` edits in the same file, work from bottom to top (highest line numbers first)
- after using `files_search_content` with `line_numbers: true` to find relevant lines, use `files_read_lines` to read context around those lines
- use `files_replace` for bulk find-and-replace across a file; use `files_write_lines` for targeted edits where surrounding context must be verified before replacement
- use `literal: true` on `files_replace` when the pattern contains regex metacharacters (`|`, `()`, `[]`, `.`, `*`, `+`, `?`, `{}` — common in source code, markdown tables, JSON, and URLs) that should be matched as-is
