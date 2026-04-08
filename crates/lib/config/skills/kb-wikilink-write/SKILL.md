---
name: kb-wikilink-write
description: Rename knowledge base notes with automatic wikilink updates.
metadata:
  requires:
    bins: ["chai"]
generated_from:
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: moderate
---

# Knowledge Base Wikilink Write

Rename knowledge base notes and automatically update all wikilinks that
reference them. Handles both plain links (`[[Note Name]]`) and aliased links
(`[[Note Name|display text]]`).

All paths are relative to the knowledge base root (the active profile's sandbox
directory). Write operations are sandbox-validated.

## Skill Directives

- always use paths relative to the knowledge base root, never absolute paths
- always verify the source note exists before renaming (use `kb_read` from the `kb` skill)
- always verify the destination does not already exist
- never rename notes without this tool — manual rename breaks wikilinks
- never use this tool to just move a file without link updates — it always updates links

## Available Tools

- `kb_wikilink_rename`

## Tool Instructions

### Rename a note

1. Call `kb_wikilink_rename` with `from` set to the current path and `to` set
   to the new path. Both are relative to the knowledge base root.
2. The tool renames the file and updates all `[[old name]]` and
   `[[old name|alias]]` wikilinks across the entire knowledge base.
3. The output reports the rename and the number of files with updated links.

### Move a note to a different directory

1. Call `kb_wikilink_rename` with `from` as the current path and `to` as the
   path in the new directory. Wikilinks use note names (not paths), so links
   are only updated if the file name changes.

### Rename and reorganize

1. Call `kb_wikilink_rename` for each note that needs to move or be renamed.
2. Check for broken links afterward using `kb_wikilink_broken` from the
   `kb-wikilink` skill.

## Examples

### kb_wikilink_rename

{"from": "00-inbox/Raw Idea.md", "to": "03-research/Refined Concept.md"}

{"from": "01-admin/Old Project Name.md", "to": "01-admin/New Project Name.md"}
