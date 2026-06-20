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
- when making multiple non-adjacent `files_write_lines` edits in the same file, work from bottom to top (highest line numbers first)
- after using `files_search_content` with `line_numbers: true` to find relevant lines, use `files_read_lines` to read context around those lines
- use `files_write_lines` for targeted edits; use `files_replace` for bulk find-and-replace across a file
