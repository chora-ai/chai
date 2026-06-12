---
description: Inspect Git repository state, history, and diffs. Stage files, create commits, and manage branches.
capability_tier: moderate
metadata:
  requires:
    bins: ["git"]
---

## Skill Directives

- never assume the working directory is a Git repository — check status first
- always check `git_status` before interpreting diffs to understand the current state
- always use specific refs (commit hashes, branch names) rather than ambiguous references
- always set `count` on `git_log` to limit output — without it, the full history is returned
- always check `git_status` before committing to verify what will be included
- always write clear, concise, conventional commit messages that describe the change
- never delete the current branch — switch to another branch first

Commits on `main` and `release/*` branches are blocked. Push to these branches is also blocked. Use feature branches for all changes.

Using `ref: "main"` in `git_diff` shows all changes since diverging from main.
