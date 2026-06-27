---
description: Read, edit, and delete YAML frontmatter in notes.
capability_tier: moderate
metadata:
  requires:
    bins: ["chai"]
---

## Skill Guidelines

- Prefer `notes_frontmatter_edit` for single-key updates instead of rewriting the entire note.

If the note has no frontmatter, `notes_frontmatter_read` returns an error. `notes_frontmatter_edit` creates a frontmatter block if the file has none.
