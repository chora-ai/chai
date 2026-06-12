---
description: Inspect Git repository state, history, diffs, and branches (read-only).
capability_tier: minimal
variant_of: git
metadata:
  requires:
    bins: ["git"]
---

## Skill Directives

- never assume the working directory is a Git repository — check status first
- always check `git_status` before interpreting diffs to understand the current state
- always use specific refs (commit hashes, branch names) rather than ambiguous references
- always set `count` on `git_log` to limit output — without it, the full history is returned

Using `ref: "main"` in `git_diff` shows all changes since diverging from main.
