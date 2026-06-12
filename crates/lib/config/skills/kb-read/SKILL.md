---
description: Read knowledge base notes, list directories, and search note contents (read-only).
capability_tier: minimal
variant_of: kb
metadata:
  requires:
    bins: ["cat", "ls", "grep", "chai"]
---

## Skill Directives

- never assume a note exists — use `kb_list` to verify first
- after using `kb_search` with `line_numbers: true` to find relevant lines, use `kb_read_lines` to read context around those lines

All paths are relative to the sandbox root, matching the `files` skill. Use `./` prefix for paths in the current directory.

The `pattern` parameter in `kb_search` supports extended regex (ERE): `|` for alternation, `+` for one-or-more, `?` for zero-or-one, `{m,n}` for repetition, and `()` for grouping.
