---
description: Inspect Git repository state, history, diffs, and branches (read-only).
capability_tier: minimal
variant_of: git
metadata:
  requires:
    bins: ["git"]
---

## Skill Guidelines

- When interpreting diffs on an uncommitted working tree, check `git_status` first
- When the exact target matters, prefer specific refs (commit hashes, branch names) over relative refs (HEAD~N)
