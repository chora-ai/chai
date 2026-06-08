---
description: Full Git operations including clone, pull, push, branching, staging, and committing.
capability_tier: full
model_variant_of: git
metadata:
  requires:
    bins: ["git"]
---

# Git (Remote)

Full Git operations for open-source contribution and remote collaboration. Includes all local operations (inspect, stage, commit, branch) plus network operations (clone, pull, push). Clone targets are validated against the write sandbox. Do not enable alongside the `git` skill — this skill is a superset.

## Skill Directives

- always check `git_status` before interpreting diffs to understand the current state
- always use specific refs (commit hashes, branch names) rather than ambiguous references
- always limit `git_log` output with `count` to avoid overwhelming context
- never assume the working directory is a Git repository — check status first
- always set `path` to the repository directory relative to the sandbox root (e.g. `./chai`) when the target repository is inside the sandbox — this sets the working directory so git can find the `.git` folder
- always check `git_status` before committing to verify what will be included
- always write clear, concise commit messages that describe the change
- always clone repositories into the sandbox directory
- always verify the remote with `git_remote` before pushing
- always create a feature branch before making changes — never commit directly to main
- always pull before pushing to avoid conflicts

## Available Tools

- `git_status`
- `git_log`
- `git_diff`
- `git_show`
- `git_branch`
- `git_add`
- `git_commit`
- `git_branch_create`
- `git_clone`
- `git_pull`
- `git_push`
- `git_remote`

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

### Create a branch

1. Call `git_branch_create` with `name` set to the new branch name and `path` set to the repository directory.
2. This creates the branch and switches to it.
3. Call `git_status` to confirm you are on the new branch.

### Stage files

1. Call `git_add` with `files` set to the path to stage (e.g. `src/main.rs`).
2. To stage all changes, set `files` to `.`.
3. Set `path` to the repository directory if different from the sandbox root.
4. Call `git_status` to verify the files are staged.

### Commit changes

1. Call `git_status` with `path` set to the repository directory to review staged changes before committing.
2. Call `git_commit` with `message` set to a clear description of the changes and `path` set to the repository directory.
3. Call `git_log` with `count` set to `1` to verify the commit was created.

### Clone a repository

1. Call `git_clone` with `url` set to the repository URL and `directory` set to a directory name (e.g. `my-repo`) or absolute path. Relative names are automatically resolved to the sandbox directory.
2. Call `git_status` to verify the clone succeeded.
3. Call `git_remote` to confirm the remote configuration.

### Pull changes from remote

1. Call `git_pull` to pull from the tracking remote.
2. To pull from a specific remote, set `remote` (e.g. `upstream`) and optionally `branch` (e.g. `main`).
3. Call `git_log` with `path` set to the repository directory and `count` set to `5` to see what was pulled.

### Push changes to remote

1. Call `git_status` with `path` set to the repository directory to verify all changes are committed.
2. Call `git_push` to push to the tracking remote.
3. For a new branch with no upstream, set `remote` to `origin`, `branch` to the branch name, and `set_upstream` to true.

### List remotes

1. Call `git_remote` with `path` set to the repository directory to see configured remotes and their URLs.

### Contribute to an open-source project

1. Call `git_clone` with the fork URL and a sandbox directory name.
2. Call `git_remote` with `path` set to the cloned repo to verify the remote setup.
3. Call `git_branch_create` with a descriptive branch name and `path` set to the cloned repo.
4. Make changes using other skills (e.g. `files_write_file`).
5. Call `git_add` to stage the changes.
6. Call `git_commit` with a clear commit message.
7. Call `git_push` with `remote` set to `origin`, `branch` set to the branch name, and `set_upstream` set to true.

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

### git_clone

{"url": "https://github.com/user/repo.git", "directory": "repo"}

### git_pull

{"path": "./repo", "remote": "upstream", "branch": "main"}

### git_push

{"path": "./repo", "remote": "origin", "branch": "feature/add-search", "set_upstream": true}

### git_remote

{"path": "./chai"}
