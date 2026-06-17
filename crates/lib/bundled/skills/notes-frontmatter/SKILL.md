---
description: Read, edit, and delete YAML frontmatter in notes.
capability_tier: moderate
metadata:
  requires:
    bins: ["chai"]
---

## Skill Directives

- always use `notes_frontmatter_edit` for single-key updates instead of rewriting the entire note
- never modify note body content through this skill

## Skill Guidelines

If the note has no frontmatter, `notes_frontmatter_read` returns an error. `notes_frontmatter_edit` creates a frontmatter block if the file has none.
