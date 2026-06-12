---
description: Git remote operations — clone, pull, push, and remote listing.
capability_tier: minimal
metadata:
  requires:
    bins: ["git"]
---

## Skill Directives

- always clone repositories into the sandbox directory
- always verify the remote with `git_remote` before pushing
- always pull before pushing to avoid conflicts
- always specify `remote` and `set_upstream` when pushing a new branch for the first time
