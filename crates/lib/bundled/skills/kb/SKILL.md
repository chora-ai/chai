---
description: Read, write, replace, delete, list, and search knowledge base notes.
capability_tier: full
metadata:
  requires:
    bins: ["cat", "ls", "grep", "chai"]
---

## Skill Directives

- never delete notes without confirming the action is intended
- never assume a note exists — use `kb_list` to verify first
- always read a note with `kb_read` before overwriting it with `kb_write` to avoid data loss — `kb_write` is a complete overwrite, not a patch
- prefer rewriting an entire affected section as a single `kb_write_lines` call over making multiple small targeted edits to the same note
- when making multiple non-adjacent `kb_write_lines` edits in the same note, work from bottom to top (highest line numbers first)
- after using `kb_search` with `line_numbers: true` to find relevant lines, use `kb_read_lines` to read context around those lines
- use `kb_replace` for bulk find-and-replace across a note; use `kb_write_lines` for targeted edits where surrounding context must be verified before replacement
- use `literal: true` on `kb_replace` when the pattern contains regex metacharacters (`|`, `()`, `[]`, `.`, `*`, `+`, `?`, `{}` — common in source code, markdown tables, JSON, and URLs) that should be matched as-is

The `pattern` parameter in `kb_search` supports extended regex (ERE): `|` for alternation, `+` for one-or-more, `?` for zero-or-one, `{m,n}` for repetition, and `()` for grouping.

The `literal` parameter on `kb_replace` treats the pattern as literal text instead of regex — no regex metacharacters are interpreted. Use `literal: true` for patterns containing `|`, `()`, `[]`, `.`, `*`, `+`, `?`, `{}` that should be matched as-is. Capture groups (`$1`–`$9`) are not supported in literal mode.

The `pattern` parameter in `kb_replace` is matched against the full note content with multiline mode enabled (`^` and `$` match line boundaries). Include actual newlines in the pattern string for multi-line matches. Capture groups from the pattern can be referenced in the replacement as `$1`–`$9`. Use `$$` for a literal `$`. Use an empty string to delete matches.

The `max_replacements` parameter in `kb_replace` limits how many matches are replaced. The default is 0 (unlimited). Use `max_replacements: 1` to replace only the first match — this prevents unintended changes when the same pattern appears in multiple locations (e.g., similar sections in the same note). When `max_replacements` limits the result, the output shows "N of M match(es) replaced" instead of "M replacement(s)".

When the regex pattern matches 0 times, `kb_replace` automatically retries with a trailing-whitespace-tolerant literal search: trailing whitespace is stripped from each line of both the pattern and the note content before matching. The fallback only accepts matches that start and end at line boundaries — the pattern must match one or more complete lines, not a substring within a line. If a match is found, the note's original trailing whitespace is preserved in the replacement. When no match is found even after the fallback, the output may include a hint if the pattern would match with leading-whitespace normalization (indentation) — this indicates the pattern's indentation differs from the note content.
