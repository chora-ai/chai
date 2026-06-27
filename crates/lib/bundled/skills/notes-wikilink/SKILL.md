---
description: Discover and validate wikilink relationships, and rename notes with automatic wikilink updates.
capability_tier: moderate
metadata:
  requires:
    bins: ["grep", "chai"]
---

## Skill Directives

- Never assume a wikilink target exists just because the link is present
- Never rename notes without `notes_wikilink_rename` — manual rename breaks wikilinks
- Always specify `scope` when working with notes in a subdirectory (for `notes_wikilink_find_outlinks`, `notes_wikilink_find_broken`, and `notes_wikilink_rename`)

## Skill Guidelines

- Use `notes_wikilink_find_broken` to validate links rather than checking manually.
- `notes_wikilink_rename` always updates wikilinks — do not use it for simple file moves that don't need link updates.
