---
name: files
description: Read files, list directories, search file contents, write files, and delete files and directories.
metadata:
  requires:
    bins: ["cat", "ls", "grep", "chai"]
generated_from:
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: full
---

# File Tools

File tools for inspecting, writing, and deleting files, listing directories, and searching code. Wraps standard unix utilities (`cat`, `ls`, `grep`) and `chai file` subcommands through the allowlist-enforced execution model. Write and delete tools require a configured write sandbox — paths are validated against writable roots before execution.

## Skill Directives

- always use paths relative to the sandbox root (`.`) — use `./` prefix for all file operations
- always set `recursive` to true when searching directories with `files_search_content` (this is the default — omit this parameter rather than setting it to false unless you intentionally want a shallow search)
- always set `line_numbers` to true when searching for code patterns
- never assume a file exists — use `files_list_dir` to verify first
- never read binary files — check file type with `files_list_dir` before reading
- always read a file with `files_read_file` before overwriting it with `files_write_file`
- never write to or delete paths outside the configured sandbox
- always verify a file exists with `files_list_dir` before deleting it
- never delete files without confirming the action is intended
- prefer `files_read_lines` over `files_read_file` when you only need specific lines, to reduce context usage
- prefer `files_write_lines` over `files_write_file` for targeted edits to large files
- always verify a directory is empty with `files_list_dir` before deleting it with `files_delete_dir`
- always re-read or re-search a file to get fresh line numbers before each `files_write_lines` call — line numbers shift after any edit that adds or removes lines
- prefer rewriting an entire affected section (e.g. a struct + impl block) as a single `files_write_lines` call over making multiple small targeted edits to the same file
- when making multiple non-adjacent `files_write_lines` edits in the same file, work from bottom to top (highest line numbers first)

## Available Tools

- `files_read_file`
- `files_read_lines`
- `files_list_dir`
- `files_search_content`
- `files_write_file`
- `files_write_lines`
- `files_append`
- `files_delete_file`
- `files_delete_dir`

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

### Write a file

1. Call `files_write_file` with `path` set to a `./`-relative file path and `content` set to the full file content.
2. The file is created if it does not exist, or overwritten if it does.
3. The parent directory must already exist.

### Write specific lines to a file

1. Call `files_write_lines` with `path`, `start_line`, and `content`.
2. Set `end_line` to replace a multi-line range. When omitted, only `start_line` is replaced.
3. Lines outside `[start_line, end_line]` are preserved unchanged.
4. The replacement content can expand (more lines), contract (fewer lines), or delete (empty content) the range.
5. Use this for targeted edits to large files instead of reading and rewriting the entire file.

**Caution: line numbers change after each edit.** A `files_write_lines` call that adds or removes lines shifts all subsequent line numbers in the file. When making multiple edits to the same file:
- Always verify the exact content at your target lines immediately before each `files_write_lines` call by reading or searching — never assume line numbers from a prior read are still accurate.
- When making several non-adjacent edits, work from bottom to top (highest line numbers first) so earlier edits don't shift the line numbers of later targets.
- When deleting multiple fields or functions, prefer rewriting the entire affected section as a single `files_write_lines` call rather than making many small single-line deletions.

### Append to a file

1. Call `files_append` with `path` set to a `./`-relative file path and `content` set to the content to append.
2. The file is created if it does not exist, or the content is appended to the end if it does.
3. Parent directories are created automatically if they do not exist.
4. Use this instead of `files_write_file` when adding to an existing file without reading and rewriting the entire content.

### Update an existing file

For small files or full rewrites:
1. Call `files_read_file` to get the current content.
2. Apply changes to the content.
3. Call `files_write_file` with `path` and the updated `content`.
4. Call `files_read_file` to verify the change.

For targeted edits to large files:
1. Call `files_search_content` with `line_numbers: true` to find the lines to change.
2. Call `files_read_lines` to read the lines around the change (for context).
3. Call `files_write_lines` with the replacement content for just those lines.
4. Call `files_read_lines` to verify the change.

**Important: preventing stale line number errors.** After any `files_write_lines` call that changes the line count (adds or removes lines), all line numbers below the edit shift. If you need to make another edit to the same file, you must re-read or re-search to get fresh line numbers before the next call — do not reuse line numbers from a previous read. Alternately, rewrite the entire affected section (e.g. a full struct + impl block) as a single `files_write_lines` call, which eliminates the stale-line-number problem entirely.

### Delete a file

1. Call `files_list_dir` to verify the file exists.
2. Call `files_delete_file` with `path` set to a `./`-relative file path.
3. The file is deleted. Directories cannot be deleted with this tool.

### Delete an empty directory

1. Call `files_list_dir` to verify the directory exists and is empty.
2. Call `files_delete_dir` with `path` set to a `./`-relative directory path.
3. The directory is deleted only if it is empty. Non-empty directories and files are refused.
4. To delete a directory with contents, first delete all files and subdirectories inside it, then delete the empty directory.

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

### files_write_file

{"path": "./src/config.rs", "content": "pub struct Config {\n    pub port: u16,\n}"}

### files_write_lines (replace single line)

{"path": "./src/config.rs", "start_line": 5, "content": "    pub host: String,"}

### files_write_lines (replace line range)

{"path": "./src/config.rs", "start_line": 3, "end_line": 5, "content": "    pub name: String,\n    pub port: u16,\n    pub host: String,"}

### files_write_lines (delete lines by replacing with empty content)

{"path": "./src/config.rs", "start_line": 8, "end_line": 10, "content": ""}

### files_append

{"path": "./src/log.txt", "content": "entry added at runtime\n"}

### files_delete_file

{"path": "./src/old-config.rs"}

### files_delete_dir

{"path": "./src/obsolete-module"}
