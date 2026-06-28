---
description: Discover and validate wikilink relationships, and rename notes with automatic wikilink updates.
capability_tier: moderate
metadata:
  requires:
    bins: ["grep", "chai"]
---

## Skill Directives

- Always specify `scope` when working with notes in a subdirectory
- Never assume a wikilink target exists just because the link is present
- Never rename notes without `notes_wikilink_rename` — manual rename breaks wikilinks

## Skill Guidelines

- Use `notes_wikilink_find_broken` to validate links rather than checking manually
