---
description: Read files, list directories, and search file contents (read-only).
capability_tier: minimal
model_variant_of: files
metadata:
  requires:
    bins: ["cat", "ls", "grep", "chai"]
---

# File Tools (Read-Only)

Read-only file tools for inspecting files, listing directories, and searching code. Wraps standard unix utilities (`cat`, `ls`, `grep -E`) and `chai file read-lines` through the allowlist-enforced execution model. No write or delete operations are included.

## Skill Directives

- always use paths relative to the sandbox root (`.`) — use `./` prefix for all file operations
- always set `recursive` to true when searching directories with `files_search_content` (this is the default — omit this parameter rather than setting it to false unless you intentionally want a shallow search)
- always set `line_numbers` to true when searching for code patterns
- never assume a file exists — use `files_list_dir` to verify first
- never read binary files — check file type with `files_list_dir` before reading
- prefer `files_read_lines` over `files_read_file` when you only need specific lines, to reduce context usage

## Available Tools

- `files_read_file`
- `files_read_lines`
- `files_list_dir`
- `files_search_content`

## Tool Instructions

### Read a file

1. Call `files_read_file` with `path` set to a `./`-relative file path.
2. The full file contents are returned (no line numbers).

### Read specific lines from a file

1. Call `files_read_lines` with `path` set to a `./`-relative file path, `start_line` set to the first line to read (1-indexed), and optionally `end_line` set to the last line to read.
2. When `end_line` is omitted, only `start_line` is read (single line).
3. Lines are returned with line numbers in the format `{line_number}|{content}`.
4. Use this when you only need a portion of a file — it saves context compared to reading the whole file.
5. After using `files_search_content` with `line_numbers: true` to find relevant lines, use `files_read_lines` to read context around those lines.

### List directory contents

1. Call `files_list_dir` with `path` set to a `./`-relative directory path.
2. Set `long` to true to see permissions, sizes, and dates.
3. Set `all` to true to include hidden files (dotfiles).
4. When an `AGENTS.md` file exists in the listed directory, its contents are automatically appended to the result as a context section (labeled with the filename). This is an automatic context-loading feature — it is not part of the `ls` output. The `AGENTS.md` content comes from the same directory being listed, and each path is surfaced at most once per session.

### Search for content in files

1. Call `files_search_content` with `pattern` and a `./`-relative `path`.
2. Set `recursive` to true to search all files in subdirectories.
3. Set `line_numbers` to true to include line numbers in output.
4. Set `case_insensitive` to true for case-insensitive matching.
5. Set `files_only` to true to get just the list of matching files without showing the matching lines.

The `pattern` parameter supports **extended regex** (ERE) — `|` for alternation, `+` for one-or-more, `?` for zero-or-one, `{m,n}` for repetition, and `()` for grouping all work. This is the same syntax used by `grep -E`. When no matches are found, the tool returns an empty result (not an error).

### Find files by content

1. Call `files_search_content` with `pattern` set to the content to find, a `./`-relative `path`, `recursive` to true, and `files_only` to true.
2. This returns only file paths that contain the pattern.

### Explore a codebase

1. Call `files_list_dir` on the project root to see the structure.
2. Drill into directories of interest with additional `files_list_dir` calls.
3. Use `files_search_content` to find specific functions, classes, or patterns.
4. Use `files_read_lines` to examine the lines around search results.
5. Use `files_read_file` to read entire files when needed.

## Examples

### files_read_file

{"path": "./src/main.rs"}

### files_read_lines (single line)

{"path": "./src/main.rs", "start_line": 42}

### files_read_lines (line range)

{"path": "./src/main.rs", "start_line": 20, "end_line": 30}

### files_list_dir

{"path": ".", "long": true, "all": true}

### files_search_content

{"pattern": "fn main", "path": "./src", "recursive": true, "line_numbers": true}

### files_search_content with alternation

{"pattern": "TODO|FIXME", "path": "./src", "recursive": true, "line_numbers": true}
