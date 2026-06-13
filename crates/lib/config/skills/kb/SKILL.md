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

The `pattern` parameter in `kb_replace` is matched against the full file content with multiline mode enabled (`^` and `$` match line boundaries). The `\n` in a pattern matches a newline, enabling multi-line patterns and line deletion (e.g., matching `line_content\n` and replacing with `""` deletes the line). Capture groups from the pattern can be referenced in the replacement as `$1`–`$9`. Use `$$` for a literal `$`. The `\n`, `\t`, and `\\` escape sequences in the `replacement` parameter are processed as newline, tab, and literal backslash respectively, consistent with how `\n` works in the pattern.

The `max_replacements` parameter in `kb_replace` limits how many matches are replaced. The default is 0 (unlimited). Use `max_replacements: 1` to replace only the first match — this prevents unintended changes when the same pattern appears in multiple locations (e.g., boilerplate code in sibling functions). When `max_replacements` limits the result, the output shows "N of M match(es) replaced" instead of "M replacement(s)".

When the regex pattern matches 0 times, `kb_replace` automatically retries with a trailing-whitespace-tolerant literal search: the pattern's escape sequences (`\n`, `\t`, `\\`) are first processed to match the regex engine's interpretation, then trailing whitespace is stripped from each line of both the pattern and the file content before matching. If a match is found, the file's original trailing whitespace is preserved in the replacement. This handles the common case where the LLM drops trailing whitespace when copying content from file reads into the pattern parameter.
