# Bug: Pipe `|` in Search Pattern Doesn't Work as Alternation

## Status

Open

## Summary

The `devtools_search_content` tool description says "basic regex supported", but `grep` is invoked without `-E` (extended regex). In BRE mode, the pipe character `|` is treated as a literal character, not alternation. So a pattern like `apply_side_read|side_read` searches for the literal string `"apply_side_read|side_read"` rather than matching either term.

## Impact

- Agents (and users) who expect standard regex alternation (`|`) get silently wrong results — no matches are found when matches should exist.
- The tool description claiming "basic regex supported" is misleading since most people expect `|`, `+`, `?` to work in regex.

## Root Cause

In `crates/lib/config/skills/devtools/tools.json`, the `devtools_search_content` execution spec does not pass `-E` or `--extended-regexp` to grep:

```json
{
  "tool": "devtools_search_content",
  "binary": "grep",
  "subcommand": "",
  "args": [
    { "param": "recursive", "kind": "flagifboolean", "flagIfTrue": "--recursive" },
    { "param": "line_numbers", "kind": "flagifboolean", "flagIfTrue": "--line-number" },
    { "param": "case_insensitive", "kind": "flagifboolean", "flagIfTrue": "--ignore-case" },
    { "param": "files_only", "kind": "flagifboolean", "flagIfTrue": "--files-with-matches" },
    { "param": "pattern", "kind": "positional" },
    { "param": "path", "kind": "positional", "readPath": true }
  ]
}
```

## Possible Fixes

1. **Add `-E` as a fixed argument** to the grep invocation in `tools.json`. This enables extended regex, making `|`, `+`, `?`, `{` work as operators.
2. **Add a new boolean parameter** (e.g. `extended_regex` / `regex`) that passes `-E` when true.
3. **Update the tool description** to clarify that only BRE is supported and alternation is not available (minimal fix, but disappointing).

Option 1 is the simplest and most useful — ERE is what users and agents expect when they see "regex supported".

## Related Files

- `crates/lib/config/skills/devtools/tools.json` — grep execution spec
