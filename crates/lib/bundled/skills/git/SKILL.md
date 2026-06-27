---
description: Inspect Git repository state, history, and diffs. Stage files, create commits, manage branches, merge, rebase, cherry-pick, and reset.
capability_tier: moderate
metadata:
  requires:
    bins: ["git"]
---

## Skill Guidelines

- When interpreting diffs on an uncommitted working tree, check `git_status` first
- When the exact target matters, prefer specific refs (commit hashes, branch names) over relative refs (HEAD~N)
