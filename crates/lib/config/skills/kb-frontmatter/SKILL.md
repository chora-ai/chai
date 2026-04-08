---
name: kb-frontmatter
description: Read, edit, and delete YAML frontmatter in knowledge base notes.
metadata:
  requires:
    bins: ["chai"]
generated_from:
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: moderate
---

# Knowledge Base Frontmatter

Read, edit, and delete YAML frontmatter keys in knowledge base notes. Operates
surgically on the frontmatter block without touching note body content.

All paths are relative to the knowledge base root (the active profile's sandbox
directory). Write operations are sandbox-validated.

## Skill Directives

- always use paths relative to the knowledge base root, never absolute paths
- always use `kb_frontmatter_read` to inspect frontmatter before editing
- always use `kb_frontmatter_edit` for single-key updates instead of rewriting the entire note
- never modify note body content through this skill — use `kb_write` for that
- never assume a key exists — use `kb_frontmatter_read` to verify first

## Available Tools

- `kb_frontmatter_read`
- `kb_frontmatter_edit`
- `kb_frontmatter_delete`

## Tool Instructions

### Read frontmatter

1. Call `kb_frontmatter_read` with `path` set to the note's relative path.
2. The result contains YAML key-value pairs without the `---` delimiters.
3. If the note has no frontmatter, an error is returned.

### Set a frontmatter key

1. Call `kb_frontmatter_edit` with `path`, `key`, and `value`.
2. If the key already exists, its value is replaced.
3. If the key does not exist, it is added before the closing `---`.
4. If the file has no frontmatter block, one is created at the top.

### Remove a frontmatter key

1. Call `kb_frontmatter_delete` with `path` and `key`.
2. The key and its value line are removed from the frontmatter block.
3. If the key does not exist, no changes are made.

### Update multiple keys

1. Call `kb_frontmatter_edit` once per key. Each call is a single key-value
   update — batch multiple calls for multi-field changes.

### Migrate a note's type

1. Call `kb_frontmatter_read` to inspect current frontmatter.
2. Call `kb_frontmatter_edit` to set the new `type` value.
3. Call `kb_frontmatter_delete` to remove any keys that no longer apply.

## Examples

### kb_frontmatter_read

{"path": "01-admin/AI Assistant.md"}

### kb_frontmatter_edit

{"path": "00-inbox/New Idea.md", "key": "type", "value": "inbox"}

{"path": "01-admin/AI Assistant.md", "key": "status", "value": "active"}

{"path": "03-research/World Models.md", "key": "tags", "value": "[philosophy, khora, ai]"}

### kb_frontmatter_delete

{"path": "00-inbox/old-note.md", "key": "draft"}
