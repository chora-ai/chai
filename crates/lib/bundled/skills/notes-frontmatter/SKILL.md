---
description: Read, edit, and delete YAML frontmatter in notes.
capability_tier: moderate
metadata:
  requires:
    bins: ["chai"]
---

## Skill Guidelines

- When the note has no frontmatter, `notes_frontmatter_read` returns an error.
- When the note has no frontmatter, `notes_frontmatter_edit` creates a frontmatter block.
