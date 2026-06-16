---
description: Discover and validate wikilink relationships, and rename notes with automatic wikilink updates.
capability_tier: moderate
metadata:
  requires:
    bins: ["grep", "chai"]
---

## Skill Directives

- always use `kb_wikilink_broken` to validate links rather than checking manually
- never assume a wikilink target exists just because the link is present
- never rename notes without `kb_wikilink_rename` — manual rename breaks wikilinks
- never use `kb_wikilink_rename` to just move a file without link updates — it always updates links
- always specify `kb_root` when working with a KB in a subdirectory (for `kb_wikilink_outlinks`, `kb_wikilink_broken`, and `kb_wikilink_rename`)

All paths are relative to the sandbox root, matching the `files` skill. Use `./` prefix for paths in the current directory.

`kb_wikilink_backlinks` uses `note_name` (the display name, e.g. "Conventions"), not the file path.
