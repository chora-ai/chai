---
description: Read and search chai gateway log output for diagnostic data (finish_reason, token usage, errors).
capability_tier: minimal
metadata:
  requires:
    bins: ["chai"]
---

## Skill Directives

- use `logs_search` to check for specific conditions like `"finish_reason"` values, `"truncated"`, or `"error"` — it returns context lines around matches
- use `logs_recent` with `level: "warn"` or `level: "error"` to focus on problems rather than routine output
- log lines may contain token counts and finish reasons but not full message content; use them for self-diagnosis, not for reading conversation history

Logs come from the gateway's in-memory ring buffer — only gateway diagnostic output is included (desktop app logs are separate and not accessible).
