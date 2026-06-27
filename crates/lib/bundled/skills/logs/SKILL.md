---
description: Read and search chai gateway log output for diagnostic data.
capability_tier: minimal
metadata:
  requires:
    bins: ["chai"]
---

## Skill Guidelines

- Use `logs_search` to check for specific conditions like `"finish_reason"` values, `"truncated"`, or `"error"` — it returns context lines around matches.
- Log lines may contain token counts and finish reasons but not full message content; use them for self-diagnosis, not for reading conversation history.
