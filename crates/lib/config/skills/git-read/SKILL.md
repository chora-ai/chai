---
name: git-read
description: Inspect Git repository state, history, diffs, and branches (read-only).
metadata:
  requires:
    bins: ["git"]
generated_from:
  cli: git
  cli_version: "2.43.0"
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: minimal
model_variant_of: git
---

# Git (Read-Only)

Read-only Git repository inspection. Provides tools for checking repository
state, viewing commit history, comparing changes, and listing branches. No
staging, committing, branching, or network operations are included.

Do not enable alongside the `git` or `git-remote` skills — this is a read-only
subset intended for worker agents that only need to inspect repositories.

## Skill Directives

- always check `git_status` before interpreting diffs to understand the current state
- always use specific refs (commit hashes, branch names) rather than ambiguous references
- always limit `git_log` output with `count` to avoid overwhelming context
- never assume the working directory is a Git repository — check status first

## Available Tools

- `git_status`
- `git_log`
- `git_diff`
- `git_show`
- `git_branch`

## Tool Instructions

### Check repository state

1. Call `git_status` to see the current branch, staged/unstaged changes, and
   untracked files.

### View commit history

1. Call `git_log` with `count` set to the desired number of commits.
2. Set `oneline` to true for a compact overview, or omit for full details.
3. Set `path` to limit history to a specific file or directory.

### Compare changes

1. Call `git_diff` with no parameters to see unstaged working tree changes.
2. Set `staged` to true to see changes in the index (staged for commit).
3. Set `ref` to a branch or commit to compare the working tree against it
   (e.g. `main` to see all changes since diverging from main).
4. Set `path` to limit the diff to a specific file.

### Inspect a specific commit

1. Call `git_show` with `ref` set to the commit hash, branch name, or tag.
2. The output includes the commit message, author, date, and diff.

### List branches

1. Call `git_branch` to see local branches.
2. Set `all` to true to include remote-tracking branches.

### Review recent changes

1. Call `git_log` with `count` set to `10` and `oneline` to true.
2. Identify commits of interest from the summary.
3. Call `git_show` with `ref` set to a specific commit hash for details.
4. Call `git_diff` with `ref` set to compare ranges (e.g. `HEAD~5`).

## Examples

### git_status

{}

### git_log

{"count": "10", "oneline": true}

### git_diff

{"ref": "main", "path": "src/main.rs"}

### git_show

{"ref": "abc1234"}

### git_branch

{"all": true}
