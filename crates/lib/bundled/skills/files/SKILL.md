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
- always set `line_numbers` to true when searching for code patterns
- always read a file with `files_read_file` before overwriting it with `files_write_file`
- use `files_read_lines` to get the exact content at the target range first before calling `files_write_lines`
- prefer rewriting an entire affected section as a single `files_write_lines` call over making multiple small targeted edits to the same file
- when making multiple non-adjacent `files_write_lines` edits in the same file, work from bottom to top (highest line numbers first)
- after using `files_search_content` with `line_numbers: true` to find relevant lines, use `files_read_lines` to read context around those lines
- use `files_replace` for bulk find-and-replace across a file; use `files_write_lines` for targeted edits where surrounding context must be verified before replacement
- prefer `files_write_lines` over `files_replace` when the content to match contains regex metacharacters (especially `|`, `()`, `[]`, `.`, `*`, `+`, `?` — common in markdown tables, code blocks, and URLs). The escaping burden makes `files_replace` error-prone for such content. Use `files_replace` when the pattern is simple text or intentionally uses regex features

The `pattern` parameter in `files_search_content` supports extended regex (ERE): `|` for alternation, `+` for one-or-more, `?` for zero-or-one, `{m,n}` for repetition, and `()` for grouping.

The `pattern` parameter in `files_replace` is matched against the full file content with multiline mode enabled (`^` and `$` match line boundaries). The `\n` in a pattern matches a newline, enabling multi-line patterns and line deletion (e.g., matching `line_content\n` and replacing with `""` deletes the line). Capture groups from the pattern can be referenced in the replacement as `$1`–`$9`. Use `$$` for a literal `$`. The `\n`, `\t`, and `\\` escape sequences in the `replacement` parameter are processed as newline, tab, and literal backslash respectively, consistent with how `\n` works in the pattern.

The `max_replacements` parameter in `files_replace` limits how many matches are replaced. The default is 0 (unlimited). Use `max_replacements: 1` to replace only the first match — this prevents unintended changes when the same pattern appears in multiple locations (e.g., boilerplate code in sibling functions). When `max_replacements` limits the result, the output shows "N of M match(es) replaced" instead of "M replacement(s)".

When the regex pattern matches 0 times, `files_replace` automatically retries with a trailing-whitespace-tolerant literal search: the pattern's escape sequences (`\n`, `\t`, `\\`) are first processed to match the regex engine's interpretation, then trailing whitespace is stripped from each line of both the pattern and the file content before matching. If a match is found, the file's original trailing whitespace is preserved in the replacement. This handles the common case where the LLM drops trailing whitespace when copying content from file reads into the pattern parameter.
