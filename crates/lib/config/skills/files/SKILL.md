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
- always set `line_numbers` to true when searching for code patterns
- never assume a file exists — use `files_list_dir` to verify first
- never read binary files — check file type with `files_list_dir` before reading
- always read a file with `files_read_file` before overwriting it with `files_write_file`
- never delete files without confirming the action is intended
- prefer `files_read_lines` over `files_read_file` when you only need specific lines, to reduce context usage
- prefer `files_write_lines` over `files_write_file` for targeted edits to large files
- always provide `original_content` when calling `files_write_lines` — use `files_read_lines` to get the exact content at the target range first
- prefer rewriting an entire affected section (e.g. a struct + impl block) as a single `files_write_lines` call over making multiple small targeted edits to the same file
- when making multiple non-adjacent `files_write_lines` edits in the same file, work from bottom to top (highest line numbers first)

## Tool Instructions

### Read a file

1. Call `files_read_file` with `path` set to a `./`-relative file path.
2. The full file contents are returned (no line numbers).

### Read specific lines from a file

1. Call `files_read_lines` with `path` set to a `./`-relative file path, `start_line` set to the first line to read (1-indexed), and optionally `end_line` set to the last line to read.
2. When `end_line` is omitted, only `start_line` is read (single line).
3. Lines are returned with line numbers in the format `{line_number}|{content}`.
4. After using `files_search_content` with `line_numbers: true` to find relevant lines, use `files_read_lines` to read context around those lines.

### List directory contents

1. Call `files_list_dir` with `path` set to a `./`-relative directory path.
2. Set `long` to true to see permissions, sizes, and dates.
3. Set `all` to true to include hidden files (dotfiles).
4. When an `AGENTS.md` file exists in the listed directory, its contents are automatically appended to the result as a context section — this is not part of the `ls` output but an automatic context-loading feature.

### Search for content in files

1. Call `files_search_content` with `pattern` and a `./`-relative `path`.
2. Set `recursive` to true to search all files in subdirectories (default).
3. Set `line_numbers` to true to include line numbers in output.
4. Set `case_insensitive` to true for case-insensitive matching.
5. Set `files_only` to true to get just the list of matching files without showing the matching lines.

The `pattern` parameter supports **extended regex** (ERE) — `|` for alternation, `+` for one-or-more, `?` for zero-or-one, `{m,n}` for repetition, and `()` for grouping all work. When no matches are found, the tool returns an empty result (not an error).

### Write a file

1. Call `files_write_file` with `path` set to a `./`-relative file path and `content` set to the full file content.
2. The file is created if it does not exist, or overwritten if it does.
3. Parent directories are created automatically if they do not exist.

### Write specific lines to a file

1. Use `files_read_lines` to read the exact content at the target range — this becomes the `original_content` parameter.
2. Call `files_write_lines` with `path`, `start_line`, `original_content`, and `content`.
3. Set `end_line` to replace a multi-line range. When omitted, only `start_line` is replaced.
4. The tool verifies `original_content` matches the file before applying the patch. If it doesn't match, the edit is rejected — re-read the file and retry with fresh content.
5. Lines outside `[start_line, end_line]` are preserved unchanged.
6. The replacement content can expand (more lines), contract (fewer lines), or delete (empty content) the range.
7. The tool returns a diff showing removed lines (`-` prefix), added lines (`+` prefix), and 3 lines of context before and after the change.

**Line numbers shift after edits.** When making multiple edits, work from bottom to top and always re-read to get fresh line numbers and `original_content` before the next edit.

### Delete a file

1. Call `files_delete_file` with `path` set to a `./`-relative file path.
2. The file is deleted. Directories cannot be deleted with this tool.

### Delete an empty directory

1. Call `files_delete_dir` with `path` set to a `./`-relative directory path.
2. The directory is deleted only if it is empty. Non-empty directories and files are refused.
3. To delete a directory with contents, first delete all files and subdirectories inside it, then delete the empty directory.

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

### files_write_file

{"path": "./src/config.rs", "content": "pub struct Config {\n    pub port: u16,\n}"}

### files_write_lines (replace single line)

{"path": "./src/config.rs", "start_line": 5, "original_content": "    pub name: String,", "content": "    pub host: String,"}

### files_write_lines (replace line range)

{"path": "./src/config.rs", "start_line": 3, "end_line": 5, "original_content": "    pub name: String,\n    pub port: u16,\n    pub host: String,", "content": "    pub name: String,\n    pub port: u16,\n    pub host: String,\n    pub active: bool,"}

### files_write_lines (delete lines by replacing with empty content)

{"path": "./src/config.rs", "start_line": 8, "end_line": 10, "original_content": "    // deprecated\n    pub legacy: bool,\n    pub old_field: i32,", "content": ""}

### files_delete_file

{"path": "./src/old-config.rs"}

### files_delete_dir

{"path": "./src/obsolete-module"}
