---
name: notesmd
description: Create, read, search, update, and delete notes.
metadata:
  requires:
    bins: ["notesmd-cli"]
generated_from:
  cli: notesmd-cli
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: moderate
---

# NotesMD

General-purpose note management through `notesmd-cli`. Provides full CRUD
operations on vault notes including content search, frontmatter inspection, and
frontmatter editing.

## Skill Directives

- always use note paths relative to the vault root (e.g. `01-admin/AI Assistant.md`)
- always use markdown format for `content` when creating or updating notes
- always include YAML frontmatter when creating new notes
- always read a note before updating it to avoid overwriting content
- always confirm before deleting a note
- never use `notesmd_create` with `replace: true` without reading the note first

## Available Tools

- `notesmd_search`
- `notesmd_read`
- `notesmd_create`
- `notesmd_list`
- `notesmd_delete`
- `notesmd_frontmatter_read`
- `notesmd_frontmatter_edit`

## Tool Instructions

### Search for notes

1. Call `notesmd_search` with `query` set to the search term.
2. Review the matching notes returned.
3. Call `notesmd_read` on specific results to examine their content.

### Read a note

1. Call `notesmd_read` with `path` set to the note path relative to the vault root.
2. To include linked mentions (backlinks), set `mentions` to true.

### Create a new note

1. Call `notesmd_list` to verify the target directory exists.
2. Call `notesmd_create` with `path`, `content` (including YAML frontmatter),
   and `replace` set to true.
3. Call `notesmd_read` to verify the note was created correctly.

### Update an existing note

1. Call `notesmd_read` to get the current content.
2. Apply changes to the content.
3. Call `notesmd_create` with `path`, the updated `content`, and `replace` set
   to true.
4. Call `notesmd_read` to verify the update.

### Append to an existing note

1. Call `notesmd_create` with `path`, the new `content` to append, and `replace`
   set to false.

### Delete a note

1. Confirm the deletion with the user.
2. Call `notesmd_delete` with `path` set to the note path.

### List vault contents

1. Call `notesmd_list` with `path` set to a directory (or omit for vault root).
2. Review the files and folders returned.

### Read note frontmatter

1. Call `notesmd_frontmatter_read` with `note` set to the note name or path.
2. Review the YAML frontmatter fields.

### Edit note frontmatter

1. Call `notesmd_frontmatter_read` to see the current frontmatter.
2. Call `notesmd_frontmatter_edit` with `note`, `key`, and `value`.
3. Call `notesmd_frontmatter_read` to verify the change.

## Examples

### notesmd_search

{"query": "agentic systems"}

### notesmd_read

{"path": "01-admin/AI Assistant.md", "mentions": true}

### notesmd_create

{"path": "00-inbox/New Note.md", "content": "---\ntitle: New Note\nstatus: draft\n---\n\n# New Note\n\nContent here.", "replace": true}

### notesmd_list

{"path": "01-admin"}

### notesmd_delete

{"path": "00-inbox/Old Note.md"}

### notesmd_frontmatter_read

{"note": "Conventions"}

### notesmd_frontmatter_edit

{"note": "Conventions", "key": "status", "value": "done"}
