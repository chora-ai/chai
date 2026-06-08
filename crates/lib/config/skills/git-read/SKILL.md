---
description: Inspect Git repository state, history, diffs, and branches (read-only).
capability_tier: minimal
model_variant_of: git
metadata:
  requires:
    bins: ["git"]
---

# Git (Read-Only)

Read-only Git repository inspection. Provides tools for checking repository state, viewing commit history, comparing changes, and listing branches. No staging, committing, branching, or network operations are included.

## Skill Directives

- always check `git_status` before interpreting diffs to understand the current state
- always use specific refs (commit hashes, branch names) rather than ambiguous references
- always limit `git_log` output with `count` to avoid overwhelming context
- never assume the working directory is a Git repository — check status first
- always set `path` to the repository directory relative to the sandbox root (e.g. `./chai`) when the target repository is inside the sandbox — this sets the working directory so git can find the `.git` folder

## Available Tools

- `git_status`
- `git_log`
- `git_diff`
- `git_show`
- `git_branch`

## Tool Instructions

### Check repository state

1. Call `git_status` with `path` set to the repository directory to see the current branch, staged/unstaged changes, and untracked files.

### View commit history

1. Call `git_log` with `path` set to the repository directory and `count` set to the desired number of commits.
2. Set `oneline` to true for a compact overview, or omit for full details.
3. Set `file_path` to limit history to a specific file or directory within the repository.

### Compare changes

1. Call `git_diff` with `path` set to the repository directory to see unstaged working tree changes.
2. Set `staged` to true to see changes in the index (staged for commit).
3. Set `ref` to a branch or commit to compare the working tree against it (e.g. `main` to see all changes since diverging from main).
4. Set `file_path` to limit the diff to a specific file within the repository.

### Inspect a specific commit

1. Call `git_show` with `ref` set to the commit hash, branch name, or tag.
2. Set `path` to the repository directory if different from the sandbox root.
3. The output includes the commit message, author, date, and diff.

### List branches

1. Call `git_branch` with `path` set to the repository directory to see local branches.
2. Set `all` to true to include remote-tracking branches.

### Review recent changes

1. Call `git_log` with `path` set to the repository directory, `count` set to `10`, and `oneline` to true.
2. Identify commits of interest from the summary.
3. Call `git_show` with `ref` set to a specific commit hash for details.
4. Call `git_diff` with `ref` set to compare ranges (e.g. `HEAD~5`).

## Examples

### git_status

{"path": "./chai"}

### git_log

{"path": "./chai", "count": "10", "oneline": true}

### git_diff

{"path": "./chai", "ref": "main", "file_path": "src/main.rs"}

### git_show

{"path": "./chai", "ref": "abc1234"}

### git_branch

{"path": "./chai", "all": true}
