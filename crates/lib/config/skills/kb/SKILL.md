---
description: Read, write, append, replace, delete, list, and search knowledge base notes.
capability_tier: moderate
metadata:
  requires:
    bins: ["cat", "ls", "grep", "chai"]
---

## Skill Directives

- never delete notes without confirming the action is intended
- never assume a note exists — use `kb_list` to verify first
- always read a note before overwriting it to avoid data loss — `kb_write` is a complete overwrite, not a patch
- use `kb_read_lines` to get the exact content at the target range first before calling `kb_write_lines`
- prefer rewriting an entire affected section as a single `kb_write_lines` call over making multiple small targeted edits to the same note
- when making multiple non-adjacent `kb_write_lines` edits in the same note, work from bottom to top (highest line numbers first)
- after using `kb_search` with `line_numbers: true` to find relevant lines, use `kb_read_lines` to read context around those lines
- prefer `kb_append` over read-then-write when adding content to the end of a note
- use `kb_replace` for bulk find-and-replace across a note; use `kb_write_lines` for targeted edits where surrounding context must be verified before replacement

All paths are relative to the sandbox root, matching the `files` skill. Use `./` prefix for paths in the current directory.

The `pattern` parameter in `kb_search` supports extended regex (ERE): `|` for alternation, `+` for one-or-more, `?` for zero-or-one, `{m,n}` for repetition, and `()` for grouping.

The `pattern` parameter in `kb_replace` is matched against the full note content with multiline mode enabled (`^` and `$` match line boundaries). The `\n` in a pattern matches a newline, enabling multi-line patterns and line deletion (e.g., matching `line_content\n` and replacing with `""` deletes the line). Capture groups from the pattern can be referenced in the replacement as `$1`–`$9`. Use `$$` for a literal `$`.
