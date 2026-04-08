---
name: devtools
description: Read files, list directories, search file contents, write files, and delete files.
metadata:
  requires:
    bins: ["cat", "ls", "grep", "chai"]
generated_from:
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: full
---

# Developer Tools

Developer tools for inspecting, writing, and deleting files, listing
directories, and searching code. Wraps standard unix utilities (`cat`, `ls`,
`grep`) and `chai file` subcommands through the allowlist-enforced execution
model. Write and delete tools require a configured write sandbox — paths are
validated against writable roots before execution.

## Skill Directives

- always use absolute paths or paths relative to the working directory
- always set `recursive` to true when searching directories with `devtools_search_content`
- always set `line_numbers` to true when searching for code patterns
- never assume a file exists — use `devtools_list_dir` to verify first
- never read binary files — check file type with `devtools_list_dir` before reading
- always read a file with `devtools_read_file` before overwriting it with `devtools_write_file`
- always use absolute paths when writing files
- never write to or delete paths outside the configured sandbox
- always verify a file exists with `devtools_list_dir` before deleting it
- never delete files without confirming the action is intended

## Available Tools

- `devtools_read_file`
- `devtools_list_dir`
- `devtools_search_content`
- `devtools_write_file`
- `devtools_delete_file`

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

### Write a file

1. Call `devtools_write_file` with `path` set to the absolute file path and
   `content` set to the full file content.
2. The file is created if it does not exist, or overwritten if it does.
3. The parent directory must already exist.

### Update an existing file

1. Call `devtools_read_file` to get the current content.
2. Apply changes to the content.
3. Call `devtools_write_file` with `path` and the updated `content`.
4. Call `devtools_read_file` to verify the change.

### Delete a file

1. Call `devtools_list_dir` to verify the file exists.
2. Call `devtools_delete_file` with `path` set to the absolute file path.
3. The file is deleted. Directories cannot be deleted with this tool.

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

### devtools_write_file

{"path": "/home/user/project/src/config.rs", "content": "pub struct Config {\n    pub port: u16,\n}"}

### devtools_delete_file

{"path": "/home/user/project/src/old-config.rs"}
