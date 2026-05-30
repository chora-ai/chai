# Feature: Line-Level Read and Write Operations

## Status

Verified

## Summary

The `devtools_read_file` and `devtools_write_file` tools currently operate on entire files. This makes it expensive (in context window space) to read or update a small portion of a large file. Adding line-range support lets agents read only the lines they need and replace specific line ranges without rewriting the entire file.

## Problem

- **Read waste**: Reading a 1500-line file to inspect lines 200–210 loads 1490 unnecessary lines into the agent's context, wasting tokens and making it harder to focus.
- **Write impossible for large files**: `devtools_write_file` must write the entire file. For files exceeding practical size limits (e.g. >30KB), this becomes impossible — the agent can't hold the full content and send it back in a single write.
- **Current workaround**: The "Update an existing file" workflow requires reading the whole file into context, mentally splicing the change, and writing the entire file back. This is error-prone and context-expensive.

## Proposed Solution

### Two New Tools

Add two new tools to the `devtools` skill:

- **`devtools_read_lines`** — Read a range of lines from a file with line numbers. Uses `chai file read-lines` under the hood.
- **`devtools_write_lines`** — Replace a range of lines in a file without rewriting the entire file. Uses `chai file patch` under the hood.

Keeping these as separate tools (rather than adding optional params to the existing tools) is consistent with the one-tool-one-execution-spec architecture: each tool name maps to exactly one `ExecutionSpec` in `tools.json`.

### Two New CLI Subcommands

- **`chai file read-lines --path <path> --start-line <n> --end-line <n>`** — Read lines `[start_line, end_line]` (1-indexed, inclusive) with line numbers in format `{line_number}|{content}`.
- **`chai file patch --path <path> --start-line <n> --end-line <n>`** — Replace lines `[start_line, end_line]` with content from stdin/`--content`. Lines outside the range are preserved.

### Line Number Format

For `devtools_read_lines`, lines are prefixed with line numbers in the format `{line_number}|{content}`. This is consistent with `grep --line-number` output (which uses `:`) but uses `|` to make programmatic parsing easier and visually distinct from grep output.

Full-file reads via `devtools_read_file` continue to omit line numbers (backward compatible).

## Design Decisions

### Why two new tools instead of optional parameters?

The generic executor maps one tool name → one `ExecutionSpec`. Conditional execution paths (e.g., "use `cat` if no line range, use `chai file read-lines` if line range present") are not supported by the current architecture. Two separate tools is the cleanest approach.

### Why `chai file patch` instead of `sed -i`?

- `sed -i` has portability issues (different syntax on BSD vs GNU).
- `chai file patch` integrates with the existing write sandbox validation — the `--path` flag uses the same `writePath` pipeline.
- More control over edge cases (empty replacement content, appending at end of file).
- Keeps the Rust implementation testable and consistent with `chai file write`/`delete`.

### Why `chai file read-lines` instead of `sed -n`?

- Using `sed` would require adding it to the allowlist with `-n` subcommand, then building the `start,endp` argument dynamically via a resolve script — complex and fragile.
- `chai file read-lines` is simpler, testable, and consistent with the file subcommand family.
- The line number formatting (`{n}|{content}`) is handled in Rust, not by post-processing `sed` output.

### Why `|` separator in line number format?

- `grep --line-number` uses `{n}:{content}`. But `:` appears frequently in file content (URLs, timestamps, Rust paths).
- `|` is rare in most code and makes line-number/content boundaries unambiguous.
- The separator is only used for `devtools_read_lines` output — `devtools_search_content` continues to use grep's `:` format.

## Files to Change

### Rust (source tree — manual application required)

- **`crates/cli/src/main.rs`** — Add `FileCmd::Patch` and `FileCmd::ReadLines` variants; add `patch_string()` helper function and its unit tests.

### Skill Config (source tree — manual application required)

- **`crates/lib/config/skills/devtools/tools.json`** — Add `devtools_read_lines` and `devtools_write_lines` tool definitions, add `"read-lines"` and `"patch"` to the `chai` allowlist, add execution specs for both new tools.
- **`crates/lib/config/skills/devtools/SKILL.md`** — Document line-range read and write workflows.
- **`crates/lib/config/skills/devtools-read/tools.json`** — Add `devtools_read_lines` tool definition, add `"read-lines"` to the `chai` allowlist, add execution spec.

## Implementation Details

### `patch_string` Function

The core patching logic is extracted into a `patch_string()` function for testability:

1. Split file content into lines.
2. Preserve lines before `start_line - 1`.
3. Insert replacement content (ensuring trailing newline).
4. Preserve lines after `end_line`.
5. Handle edge cases: `end_line` exceeds file length (clamp), `start_line` exceeds file length (no-op), no trailing newline (preserve).

### Unit Tests

- Replace single line
- Replace range with more lines (expansion)
- Replace range with fewer lines (contraction)
- Delete range (empty replacement)
- Patch at start/end of file
- Preserve no-trailing-newline files
- `end_line` exceeds file length (clamped)
- `start_line` exceeds file length (no-op)

## Reference Implementations

The exact source changes are documented in `/sandbox/impl/`:
- `main.rs.patch` — Diff-style reference for CLI changes
- `devtools-tools.json` — Complete updated tools.json
- `devtools-read-tools.json` — Complete updated tools.json
- `devtools-SKILL.md` — Complete updated SKILL.md
- `devtools-read-SKILL.md` — Complete updated SKILL.md

## Verification

All tools tested and working as expected in the live sandbox environment:

| Test | Tool | Result |
|------|------|--------|
| Read single line | `devtools_read_lines` (no `end_line`) | ✅ Returns one line with `{n}\|{content}` format |
| Read line range | `devtools_read_lines` (with `end_line`) | ✅ Returns range with line numbers |
| Replace single line | `devtools_write_lines` (no `end_line`) | ✅ Replaces just that line |
| Replace range (expansion) | `devtools_write_lines` (2 lines → 3) | ✅ Lines expand correctly |
| Replace range (contraction) | `devtools_write_lines` (3 lines → 1) | ✅ Lines contract correctly |
| Delete lines (empty content) | `devtools_write_lines` (empty replacement) | ✅ Lines removed, rest preserved |

The "Files to Change" have all been applied:
- `crates/cli/src/main.rs` — `FileCmd::Patch` and `FileCmd::ReadLines` variants, `patch_string()` function and unit tests added.
- `crates/lib/config/skills/devtools/tools.json` — Both tool definitions, allowlist entries, and execution specs added.
- `crates/lib/config/skills/devtools/SKILL.md` — Line-range read and write workflows documented.
- `crates/lib/config/skills/devtools-read/tools.json` — `devtools_read_lines` tool, allowlist entry, and execution spec added.
- `crates/lib/config/skills/devtools-read/SKILL.md` — Line-range read workflow documented.
