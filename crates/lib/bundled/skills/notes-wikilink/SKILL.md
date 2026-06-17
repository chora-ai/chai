---
description: Discover and validate wikilink relationships, and rename notes with automatic wikilink updates.
capability_tier: moderate
metadata:
  requires:
    bins: ["grep", "chai"]
---

## Skill Directives

- always use `notes_wikilink_broken` to validate links rather than checking manually
- never assume a wikilink target exists just because the link is present
- never rename notes without `notes_wikilink_rename` — manual rename breaks wikilinks
- never use `notes_wikilink_rename` to just move a file without link updates — it always updates links
- always specify `root` when working with notes in a subdirectory (for `notes_wikilink_outlinks`, `notes_wikilink_broken`, and `notes_wikilink_rename`)
