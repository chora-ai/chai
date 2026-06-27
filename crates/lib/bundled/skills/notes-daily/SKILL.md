---
description: Read, write, and append to daily notes with date-based path resolution.
capability_tier: minimal
variant_of: notes
metadata:
  requires:
    bins: ["cat", "chai"]
---

## Skill Directives

- Always specify `scope` when working with notes in a subdirectory
- Never construct daily note paths manually — the resolver handles path construction

## Skill Guidelines

Daily notes are stored in a configurable folder. The folder is resolved in order:

1. `.notes-daily.conf` in the notes directory (format: `folder=custom-directory`)
2. Default: `daily`

When notes are in a subdirectory of the sandbox, specify `scope` to point the resolver to the right directory. When omitted, the notes directory defaults to the sandbox root.
