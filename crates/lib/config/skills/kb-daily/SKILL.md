---
description: Read, write, and append to daily notes with date-based path resolution.
capability_tier: minimal
variant_of: kb
metadata:
  requires:
    bins: ["cat", "chai"]
---

Daily notes are stored in a configurable folder within the knowledge base. The folder is resolved in order:

1. `.kb-daily.conf` in the KB directory (format: `folder=00-daily`)
2. Default: `daily`

When the KB is in a subdirectory of the sandbox, specify `kb_root` to point the resolver to the right directory. When omitted, the KB directory defaults to the sandbox root.

## Skill Directives

- always use `kb_daily_append` to add content to an existing daily note
- always use `kb_daily_write` only for creating new daily notes or full rewrites
- never construct daily note paths manually — the resolver handles path construction
- always specify `kb_root` when working with a KB in a subdirectory
