---
description: Read, edit, and delete YAML frontmatter in knowledge base notes.
capability_tier: moderate
metadata:
  requires:
    bins: ["chai"]
---

## Skill Directives

- always use `kb_frontmatter_read` to inspect frontmatter before editing
- always use `kb_frontmatter_edit` for single-key updates instead of rewriting the entire note
- never modify note body content through this skill

All paths are relative to the sandbox root, matching the `files` skill. Use `./` prefix for paths in the current directory.

If the note has no frontmatter, `kb_frontmatter_read` returns an error. `kb_frontmatter_edit` creates a frontmatter block if the file has none.
