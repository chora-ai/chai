---
description: Inspect Git repository state, history, and diffs. Stage files, create commits, manage branches, merge, rebase, cherry-pick, and reset.
capability_tier: moderate
metadata:
  requires:
    bins: ["git"]
---

## Skill Directives

- always check `git_status` before interpreting diffs to understand the current state
- always use specific refs (commit hashes, branch names) rather than ambiguous references
- always write clear, concise, conventional commit messages that describe the change
