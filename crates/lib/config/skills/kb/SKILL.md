---
name: kb
description: Read, write, append, delete, list, and search knowledge base notes.
metadata:
  requires:
    bins: ["cat", "ls", "grep", "chai"]
generated_from:
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: moderate
---

# Knowledge Base

Read, write, append, delete, list, and search a knowledge base of markdown
notes. The knowledge base root is the active profile's sandbox directory. All
paths in tool parameters are relative to this root.

Notes are markdown files with YAML frontmatter. The knowledge base is compatible
with Obsidian but does not depend on Obsidian-specific features or configuration.

## Skill Directives

- always use paths relative to the knowledge base root, never absolute paths
- always read a note before overwriting it to avoid data loss
- always include YAML frontmatter when creating new notes
- always preserve existing frontmatter fields when updating notes
- never write notes outside the knowledge base root (the sandbox enforces this)
- prefer `kb_append` over read-then-write when adding content to the end of a note

## Available Tools

- `kb_read`
- `kb_write`
- `kb_append`
- `kb_delete`
- `kb_list`
- `kb_search`

## Tool Instructions

### Read a note

1. Call `kb_read` with `path` set to the note's relative path.

### Create a new note

1. Call `kb_list` to verify the target directory exists.
2. Call `kb_write` with `path` and `content`. Include YAML frontmatter at the
   top of the content.

### Update an existing note

1. Call `kb_read` to get the current content.
2. Call `kb_write` with the modified content. The full content must be provided
   (this is a complete overwrite, not a patch).

### Append to a note

1. Call `kb_append` with `path` and `content`. The content is appended to the
   end of the file. Use this for adding sections, log entries, or daily note
   updates without reading the full note first.

### Delete a note

1. Call `kb_delete` with `path` set to the note's relative path. Only files can
   be deleted (directories are refused).

### List directory contents

1. Call `kb_list` with `path` set to the directory. Omit `path` to list the
   knowledge base root.

### Search for content

1. Call `kb_search` with a `pattern`. Omit `path` to search all notes.
2. Use `files_only` to get just file paths without matching lines.
3. Narrow results by setting `path` to a subdirectory.

### Find notes by topic

1. Call `kb_search` with `files_only` set to `true` and a keyword pattern.
2. Call `kb_read` on relevant results to inspect content.

## Examples

### kb_read

{"path": "01-admin/AI Assistant.md"}

### kb_write

{"path": "00-inbox/New Idea.md", "content": "---\ntype: inbox\n---\n\n# New Idea\n\nContent here.\n"}

### kb_append

{"path": "01-admin/AI Assistant.md", "content": "\n## New Section\n\nAppended content.\n"}

### kb_delete

{"path": "00-inbox/old-note.md"}

### kb_list

{}

{"path": "01-admin"}

### kb_search

{"pattern": "sandbox", "files_only": true}

{"pattern": "write.*enforcement", "path": "03-research"}
