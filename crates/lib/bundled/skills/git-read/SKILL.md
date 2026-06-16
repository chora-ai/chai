---
description: Inspect Git repository state, history, diffs, and branches (read-only).
capability_tier: minimal
variant_of: git
metadata:
  requires:
    bins: ["git"]
---

## Skill Directives

- always check `git_status` before interpreting diffs to understand the current state
- always use specific refs (commit hashes, branch names) rather than ambiguous references

Using `ref: "main"` in `git_diff` shows all changes since diverging from main.
