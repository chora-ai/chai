---
description: Read files, list directories, search file contents, write files, and delete files and directories.
capability_tier: full
metadata:
  requires:
    bins: ["cat", "ls", "grep", "chai"]
---

## Skill Directives

- never delete files without confirming the action is intended
- never assume a file exists — use `files_list_dir` to verify first
- never read binary files — check file type with `files_list_dir` before reading
- always set `line_numbers` to true when searching for code patterns
- always read a file with `files_read_file` before overwriting it with `files_write_file`
- use `files_read_lines` to get the exact content at the target range first before calling `files_write_lines`
- prefer rewriting an entire affected section as a single `files_write_lines` call over making multiple small targeted edits to the same file
- when making multiple non-adjacent `files_write_lines` edits in the same file, work from bottom to top (highest line numbers first)
- after using `files_search_content` with `line_numbers: true` to find relevant lines, use `files_read_lines` to read context around those lines

The `pattern` parameter in `files_search_content` supports extended regex (ERE): `|` for alternation, `+` for one-or-more, `?` for zero-or-one, `{m,n}` for repetition, and `()` for grouping.
