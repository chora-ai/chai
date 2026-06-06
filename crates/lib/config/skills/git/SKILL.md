---
name: git
description: Inspect Git repository state, history, and diffs. Stage files, create commits, and manage branches.
metadata:
  requires:
    bins: ["git"]
generated_from:
  cli: git
  cli_version: "2.43.0"
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: moderate
---

# Git

Git repository inspection and local write operations. Provides tools for checking repository state, viewing commit history, comparing changes, listing branches, staging files, and creating commits. Network operations (push, pull) are not included in this skill.

## Skill Directives

- always check `git_status` before interpreting diffs to understand the current state
- always use specific refs (commit hashes, branch names) rather than ambiguous references
- always limit `git_log` output with `count` to avoid overwhelming context
- never assume the working directory is a Git repository — check status first
- always check `git_status` before committing to verify what will be included
- always write clear, concise commit messages that describe the change
- never push to remote — this skill only supports local operations
- always set `path` to the repository directory relative to the sandbox root (e.g. `./chai`) when the target repository is inside the sandbox — this sets the working directory so git can find the `.git` folder

## Available Tools

- `git_status`
- `git_log`
- `git_diff`
- `git_show`
- `git_branch`
- `git_add`
- `git_commit`
- `git_branch_create`

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

### Stage files

1. Call `git_add` with `files` set to the path to stage (e.g. `src/main.rs`).
2. To stage all changes, set `files` to `.`.
3. Set `path` to the repository directory if different from the sandbox root.
4. Call `git_status` to verify the files are staged.

### Commit changes

1. Call `git_status` with `path` set to the repository directory to review staged changes before committing.
2. Call `git_commit` with `message` set to a clear description of the changes and `path` set to the repository directory.
3. Call `git_log` with `count` set to `1` to verify the commit was created.

### Create a branch

1. Call `git_branch_create` with `name` set to the new branch name and `path` set to the repository directory.
2. This creates the branch and switches to it.
3. Call `git_status` to confirm you are on the new branch.

### Stage and commit a change

1. Call `git_status` with `path` set to the repository directory to see which files have been modified.
2. Call `git_add` with `files` set to the specific file or `.` for all changes and `path` set to the repository directory.
3. Call `git_status` to confirm the staged changes are correct.
4. Call `git_commit` with `message` describing the change and `path` set to the repository directory.

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

### git_add

{"path": "./chai", "files": "src/main.rs"}

### git_commit

{"path": "./chai", "message": "Add search endpoint to API"}

### git_branch_create

{"path": "./chai", "name": "feature/add-search"}
