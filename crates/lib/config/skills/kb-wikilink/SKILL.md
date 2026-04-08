---
name: kb-wikilink
description: Discover and validate wikilink relationships between knowledge base notes.
metadata:
  requires:
    bins: ["grep"]
generated_from:
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: moderate
---

# Knowledge Base Wikilinks

Discover relationships between knowledge base notes through backlinks, tag
search, outgoing link extraction, and broken link detection. Operates on
markdown files with `[[wikilink]]` syntax and YAML frontmatter tags.

All paths are relative to the knowledge base root (the active profile's sandbox
directory). This skill is read-only.

## Skill Directives

- always use paths relative to the knowledge base root, never absolute paths
- always verify that reported links resolve to actual notes before acting on them
- always use `kb_wikilink_broken` to validate links rather than checking manually
- never modify note files — this skill is read-only
- never assume a wikilink target exists just because the link is present

## Available Tools

- `kb_wikilink_backlinks`
- `kb_wikilink_outlinks`
- `kb_wikilink_by_tag`
- `kb_wikilink_broken`

## Tool Instructions

### Find backlinks to a note

1. Call `kb_wikilink_backlinks` with `note_name` set to the note's display name
   (e.g. "Conventions", not the file path).
2. Optionally set `path` to a subdirectory to narrow the search.
3. Each result line shows a file path and the line containing the wikilink.

### Extract outgoing links from a note

1. Call `kb_wikilink_outlinks` with `path` set to the note's relative path.
2. Results show one wikilink target per line (note names without brackets).

### Find notes by tag

1. Call `kb_wikilink_by_tag` with `tag` (with or without `#` prefix).
2. Optionally set `path` to a subdirectory to narrow the search.
3. Results show files containing the tag in their frontmatter or body.

### Check for broken links in a note

1. Call `kb_wikilink_broken` with `path` set to the note's relative path.
2. The result lists wikilink targets that do not resolve to existing notes.
3. An empty result means all links are valid.

### Map note relationships

1. Call `kb_wikilink_outlinks` on the note to get its outgoing links.
2. Call `kb_wikilink_backlinks` with the note's name to get incoming links.
3. Combine the results to show the note's position in the knowledge graph.

### Audit a directory for broken links

1. For each note in the directory (use `kb_list` from the `kb` skill), call
   `kb_wikilink_broken` to check for broken links.
2. Collect and report all broken links grouped by source note.

## Examples

### kb_wikilink_backlinks

{"note_name": "Conventions"}

{"note_name": "AI Assistant", "path": "01-admin"}

### kb_wikilink_outlinks

{"path": "01-admin/AI Assistant.md"}

### kb_wikilink_by_tag

{"tag": "agentic-systems"}

{"tag": "#specification-engineering", "path": "03-research"}

### kb_wikilink_broken

{"path": "01-admin/AI Assistant.md"}
