---
name: kb-daily
description: Read, write, and append to daily notes with date-based path resolution.
metadata:
  requires:
    bins: ["cat", "chai"]
generated_from:
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: minimal
---

# Knowledge Base Daily Notes

Read, write, and append to daily notes with automatic date-based path
resolution. Daily notes are stored in a configurable folder within the knowledge
base (default: `00-daily/`).

The daily notes folder is configured via `.kb-daily.conf` in the knowledge base
root. If no configuration exists, the default folder `00-daily` is used.

## Skill Directives

- always use YYYY-MM-DD format for dates (e.g. '2026-04-06')
- always omit the date parameter to target today's note
- always use `kb_daily_append` to add content to an existing daily note
- always use `kb_daily_write` only for creating new daily notes or full rewrites
- never construct daily note paths manually — the resolver handles path construction

## Available Tools

- `kb_daily_read`
- `kb_daily_write`
- `kb_daily_append`

## Tool Instructions

### Read today's daily note

1. Call `kb_daily_read` with no parameters.

### Read a specific date's daily note

1. Call `kb_daily_read` with `date` set to the target date.

### Create today's daily note

1. Call `kb_daily_write` with `content` including YAML frontmatter and the
   note body. Omit `date` to target today.

### Add content to today's daily note

1. Call `kb_daily_append` with `content` set to the text to add. Omit `date`
   to target today. The content is appended to the end of the file.

### Add insights from a session

1. Call `kb_daily_append` with a formatted insights section. This avoids
   reading and rewriting the full daily note.

## Configuration

The daily notes folder is read from `.kb-daily.conf` in the knowledge base root:

```
folder=00-daily
```

If this file does not exist, the default folder `00-daily` is used.

## Examples

### kb_daily_read

{}

{"date": "2026-04-06"}

### kb_daily_write

{"content": "---\ntype: daily\ndate: 2026-04-06\n---\n\n# 2026-04-06\n\n## Tasks\n\n- [ ] Review inbox\n"}

{"date": "2026-04-05", "content": "---\ntype: daily\ndate: 2026-04-05\n---\n\n# 2026-04-05\n\n## Notes\n\nBackfilled daily note.\n"}

### kb_daily_append

{"content": "\n## Insights\n\n- Discovered connection between X and Y\n"}

{"date": "2026-04-05", "content": "\n## Evening Review\n\nCompleted all planned tasks.\n"}
