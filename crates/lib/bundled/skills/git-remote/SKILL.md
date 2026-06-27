---
description: Git remote operations — clone, pull, push, and remote listing.
capability_tier: minimal
metadata:
  requires:
    bins: ["git"]
---

## Skill Directives

- Always verify the remote with `git_remote` before pushing
- Always specify `remote` and `set_upstream` when pushing a new branch for the first time

## Skill Guidelines

- When pushing to a shared remote, pull first to integrate remote changes
