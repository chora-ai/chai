---
name: notelink
description: Discover note relationships through backlinks, tags, and outgoing wikilinks.
metadata:
  requires:
    bins: ["grep"]
generated_from:
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: moderate
---

# Note Linking

Discover relationships between knowledge base notes through backlinks, tag
search, and outgoing link extraction. Operates on markdown files with
`[[wikilink]]` syntax and YAML frontmatter tags.

## Skill Directives

- always use absolute paths for the vault root and note files
- always verify that reported links resolve to actual files before acting on them
- never modify note files — this skill is read-only
- never assume a wikilink target exists just because the link is present

## Available Tools

- `notelink_backlinks`
- `notelink_by_tag`
- `notelink_outlinks`

## Tool Instructions

### Find backlinks to a note

1. Call `notelink_backlinks` with `note_name` set to the note's display name
   (e.g. "Conventions", not the file path) and `path` set to the vault root.
2. Review the results — each line shows a file path and the line containing
   the wikilink.

### Find notes by tag

1. Call `notelink_by_tag` with `tag` set to the tag value and `path` set to
   the vault root.
2. Results show files containing the tag in their frontmatter or body.

### Extract outgoing links from a note

1. Call `notelink_outlinks` with `file` set to the absolute path of the note.
2. Results show one wikilink target per line (note names without brackets).

### Check for broken links

1. Call `notelink_outlinks` to get all wikilink targets from a note.
2. For each target, verify it resolves to an existing file using a file listing
   or search tool from another skill.
3. Report any targets that do not resolve.

### Map note relationships

1. Call `notelink_outlinks` on the note of interest to get its outgoing links.
2. Call `notelink_backlinks` with the note's name to get incoming links.
3. Combine the results to show the note's position in the knowledge graph.

## Examples

### notelink_backlinks

{"note_name": "Conventions", "path": "/home/user/vault"}

### notelink_by_tag

{"tag": "agentic-systems", "path": "/home/user/vault"}

### notelink_outlinks

{"file": "/home/user/vault/01-admin/AI Assistant.md"}
