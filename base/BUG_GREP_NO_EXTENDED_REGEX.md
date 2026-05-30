# Bug: Pipe `|` in Search Pattern Doesn't Work as Alternation

## Status

Verified

## Summary

The `devtools_search_content` tool description said "basic regex supported", but `grep` was invoked without `-E` (extended regex). In BRE mode, the pipe character `|` is treated as a literal character, not alternation. So a pattern like `apply_side_read|side_read` searched for the literal string `"apply_side_read|side_read"` rather than matching either term.

## Impact

- Agents (and users) who expect standard regex alternation (`|`) get silently wrong results — no matches are found when matches should exist.
- The tool description claiming "basic regex supported" is misleading since most people expect `|`, `+`, `?` to work in regex.

## Root Cause

In `crates/lib/config/skills/devtools/tools.json`, the `devtools_search_content` execution spec did not pass `-E` or `--extended-regexp` to grep. The same issue existed in `crates/lib/config/skills/devtools-read/tools.json`, which also lacked `successExitCodes: [0, 1]` (that fix from BUG_GREP_EXIT_1 had only been applied to the `devtools` skill).

## Fix

Applied Option 1 from "Possible Fixes" — added `-E` as the grep subcommand, which is the simplest and most useful fix since ERE is what users and agents expect when they see "regex supported".

Changes in both `crates/lib/config/skills/devtools/tools.json` and `crates/lib/config/skills/devtools-read/tools.json`:

1. **Allowlist**: `"grep": ["", "-E"]` — allows `grep -E` through the allowlist.
2. **Subcommand**: `"subcommand": "-E"` — grep is now invoked as `grep -E [flags] pattern path`, enabling extended regex (`|`, `+`, `?`, `{m,n}`, `()`).
3. **Tool description**: Updated pattern description from "basic regex supported" to "extended regex supported".
4. **`devtools-read` only**: Added missing `"successExitCodes": [0, 1]` (was already present in `devtools`).

The execution spec now looks like:

```json
{
  "tool": "devtools_search_content",
  "binary": "grep",
  "subcommand": "-E",
  "args": [
    { "param": "recursive", "kind": "flagifboolean", "flagIfTrue": "--recursive" },
    { "param": "line_numbers", "kind": "flagifboolean", "flagIfTrue": "--line-number" },
    { "param": "case_insensitive", "kind": "flagifboolean", "flagIfTrue": "--ignore-case" },
    { "param": "files_only", "kind": "flagifboolean", "flagIfTrue": "--files-with-matches" },
    { "param": "pattern", "kind": "positional" },
    { "param": "path", "kind": "positional", "readPath": true }
  ],
  "successExitCodes": [0, 1]
}
```

The `-E` is passed as the subcommand because the allowlist executor prepends `subcommand.split_whitespace()` before `args` when building the command. Setting subcommand to `"-E"` produces the command `grep -E --recursive ... pattern path`, which is exactly what we want.

## Verification

All ERE features were tested live using `devtools_search_content` against the chai codebase:

| Test | Pattern | ERE Feature | Result |
|------|---------|-------------|--------|
| Alternation | `apply_side_read\|side_read` | `\|` | ✅ Found both terms in `crates/lib/` |
| One-or-more | `BUG_+` | `+` | ✅ Matched `BUG_` followed by one or more characters |
| Grouped alternation | `BUG_(GREP\|WRITE)` | `()` + `\|` | ✅ Matched lines mentioning BUG_GREP or BUG_WRITE |
| Character class + zero-or-more | `FEAT[A-Z_]*` | `[]` + `*` | ✅ Found FEAT references across multiple files |
| No-match (exit 1) | `nonexistent_xyz_pattern` | — | ✅ Empty result returned, no error |
| Grouped function alternation | `fn (apply_side_read\|build_argv)` | `()` + `\|` | ✅ Found both function definitions |

Both `devtools` and `devtools-read` tools.json files were inspected and confirmed to contain:
- `"grep": ["", "-E"]` in the allowlist
- `"subcommand": "-E"` in the execution spec
- `"successExitCodes": [0, 1]`
- Pattern description: "extended regex supported"

## Related Files

- `crates/lib/config/skills/devtools/tools.json` — grep execution spec (fixed, verified)
- `crates/lib/config/skills/devtools/SKILL.md` — skill instructions (updated)
- `crates/lib/config/skills/devtools-read/tools.json` — grep execution spec (fixed, verified)
- `crates/lib/config/skills/devtools-read/SKILL.md` — skill instructions (updated)
