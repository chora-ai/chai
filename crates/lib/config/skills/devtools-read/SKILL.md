---
name: devtools-read
description: Read files, list directories, and search file contents (read-only).
metadata:
  requires:
    bins: ["cat", "ls", "grep"]
generated_from:
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: minimal
model_variant_of: devtools
---

# Developer Tools (Read-Only)

Read-only developer tools for inspecting files, listing directories, and
searching code. Wraps standard unix utilities (`cat`, `ls`, `grep`) through the
allowlist-enforced execution model. No write or delete operations are included.

Do not enable alongside the `devtools` skill — this is a read-only subset
intended for worker agents that only need inspection capabilities.

## Skill Directives

- always use absolute paths or paths relative to the working directory
- always set `recursive` to true when searching directories with `devtools_search_content`
- always set `line_numbers` to true when searching for code patterns
- never assume a file exists — use `devtools_list_dir` to verify first
- never read binary files — check file type with `devtools_list_dir` before reading

## Available Tools

- `devtools_read_file`
- `devtools_list_dir`
- `devtools_search_content`

## Tool Instructions

### Read a file

1. Call `devtools_read_file` with `path` set to the file path.
2. The full file contents are returned.

### List directory contents

1. Call `devtools_list_dir` with `path` set to the directory.
2. Set `long` to true to see permissions, sizes, and dates.
3. Set `all` to true to include hidden files (dotfiles).

### Search for content in files

1. Call `devtools_search_content` with `pattern` and `path`.
2. Set `recursive` to true to search all files in subdirectories.
3. Set `line_numbers` to true to include line numbers in output.
4. Set `case_insensitive` to true for case-insensitive matching.
5. Set `files_only` to true to get just the list of matching files
   without showing the matching lines.

### Find files by content

1. Call `devtools_search_content` with `pattern` set to the content to
   find, `path` to the search root, `recursive` to true, and `files_only`
   to true.
2. This returns only file paths that contain the pattern.

### Explore a codebase

1. Call `devtools_list_dir` on the project root to see the structure.
2. Drill into directories of interest with additional `devtools_list_dir` calls.
3. Use `devtools_search_content` to find specific functions, classes, or patterns.
4. Use `devtools_read_file` to examine files identified by search.

## Examples

### devtools_read_file

{"path": "/home/user/project/src/main.rs"}

### devtools_list_dir

{"path": "/home/user/project", "long": true, "all": true}

### devtools_search_content

{"pattern": "fn main", "path": "/home/user/project/src", "recursive": true, "line_numbers": true}
