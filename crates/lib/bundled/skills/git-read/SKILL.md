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
- when `git_diff` output is truncated, use `git_diff_lines` with the `start_line` shown in the truncation notice to read the remaining lines
- when `git_show` output is truncated, use `git_show_lines` with the `start_line` shown in the truncation notice to read the remaining lines
