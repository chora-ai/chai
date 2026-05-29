# Improvement: Document Side-Read Behavior in Skill Instructions

## Status

Open

## Summary

The `devtools_list_dir` tool has a `sideRead` spec that automatically appends `AGENTS.md` contents to directory listings. This behavior is not documented in the tool description or the skill instructions, making it surprising and confusing — especially when the wrong `AGENTS.md` is loaded (see BUG_LOADING_AGENTS.md).

## Suggested Improvement

Add a note to the `devtools_list_dir` tool description and/or the skill instructions explaining that:

- Directory listings may include an appended `AGENTS.md` section from the listed directory.
- The `AGENTS.md` content is loaded from the same directory the tool operates on (after the fix in BUG_LOADING_AGENTS.md).
- This is an automatic context-loading feature, not part of the `ls` output itself.

This helps agents understand the structure of the tool result and avoid misattributing the `AGENTS.md` content to the directory listing itself.

## Related Files

- `crates/lib/config/skills/devtools/tools.json` — `devtools_list_dir` tool definition
- Whichever file contains the skill instructions text rendered in the system prompt
