# Bug: kb_daily_write/append Fail When Date Omitted

## Status

Verified ✅

## Summary

`kb_daily_write` and `kb_daily_append` fail with a CLI error when the `date` parameter is omitted. The skill descriptions say "Omit for today's note" but omitting the date causes `--path` to be missing from the `chai file write`/`chai file append` invocation, which requires it.

## Impact

- Users cannot use `kb_daily_write` or `kb_daily_append` without explicitly providing a date.
- The skill descriptions and SKILL.md instructions explicitly tell agents to omit the date for today's note, which always fails.
- `kb_daily_read` works correctly without a date — the inconsistency is confusing.

## Evidence

Tested on 2026-05-28:

### `kb_daily_write` without date ❌

```
kb_daily_write(content="---\ntype: daily\ndate: 2026-05-28\n---\n\n# Test")
→ error: exit exit status: 2: error: the following required arguments were not provided:
  --path <PATH>

Usage: chai file write --path <PATH> --content <CONTENT>
```

### `kb_daily_append` without date ❌

Same error — `--path` is missing from the `chai file append` command.

### `kb_daily_read` without date ✅ (works fine)

```
kb_daily_read()
→ returned correct content for today's date
```

### Both write tools work with explicit date ✅

```
kb_daily_write(date="2026-05-28", content=...)
→ wrote /home/ryan/.chai/profiles/testing/sandbox/00-daily/2026-05-28.md
```

## Root Cause

In `kb-daily/tools.json`, the `date` parameter is declared as `optional: true` with `kind: "flag"` and `flag: "path"`:

```json
{
  "param": "date",
  "kind": "flag",
  "flag": "path",
  "optional": true,
  "writePath": true,
  "resolveCommand": {
    "script": "resolve-daily-path",
    "args": ["$param"]
  }
}
```

When the caller omits `date`, the generic executor skips the parameter entirely — the `resolveCommand` script is never invoked, so `--path` is never added to the CLI args. Both `chai file write` and `chai file append` require `--path`, causing the error.

This differs from `kb_daily_read`, which uses `kind: "positional"` for `date`. For positional args, the resolve script still runs even with an empty/missing value, and `resolve-daily-path.sh` correctly defaults to today's date when called with no argument.

The fundamental issue: `optional: true` with `kind: "flag"` means "skip entirely when omitted," but `resolveCommand` scripts may need to run even for optional params to produce a default value.

Note: The `optional` field's doc comment in `descriptor.rs` already describes the intended behavior — "When true, a missing or null JSON parameter is omitted from argv (unless `resolveCommand` is set, in which case the resolver runs with an empty string)" — but this was only implemented for `ArgKind::Positional`, not for `ArgKind::Flag`.

## Fix

Extended the generic executor to invoke `resolveCommand` for optional `ArgKind::Flag` params when the parameter is omitted, mirroring the existing behavior for `ArgKind::Positional`. Two changes in `crates/lib/src/tools/generic.rs`:

### Change 1: `build_argv` — `ArgKind::Flag` branch

Before: when the flag param was omitted, the match arm `_ => continue` skipped it entirely.

After: a new match guard `_ if arg.optional == Some(true) && arg.resolve_command.is_some()` runs the resolver with an empty string. If the resolver produces a non-empty value, `--flag <resolved>` is added to argv. If the resolver returns empty (script failed or produced no output), the flag is skipped — this avoids passing an empty `--flag ""` to the binary which would likely be invalid.

### Change 2: `validate_write_paths` — `ArgKind::Flag` branch

Before: when the flag param was omitted, the match arm `_ => continue` skipped validation entirely.

After: same logic — when `optional == Some(true) && resolve_command.is_some()`, run the resolver with an empty string. If the resolved value is non-empty, validate it against the write sandbox and substitute the canonical path. If the resolved value is empty, skip validation.

### Why this approach

- Consistent with the already-documented intended behavior of the `optional` field.
- Mirrors the existing `ArgKind::Positional` pattern — minimal conceptual addition.
- Does not change behavior for optional flags *without* `resolveCommand` (those still skip when omitted).
- The `resolve-daily-path.sh` script already handles empty input correctly by defaulting to today's date, so no script changes needed.
- Added 3 unit tests covering: optional flag with resolveCommand (omitted → default, provided → resolved), optional flag without resolveCommand (omitted → skipped), and non-optional flag without resolveCommand (omitted → skipped).

### Files Changed

| File | Change |
|------|--------|
| `crates/lib/src/tools/generic.rs` | `build_argv` and `validate_write_paths`: optional + flag + resolveCommand now invokes resolver with empty string when param omitted |

No changes to `tools.json` or resolve scripts — the fix is entirely in the generic executor.

## Verified

Tested on 2026-05-28 after gateway restart with new session:

| Operation | Date Param | Result |
|-----------|-----------|--------|
| `kb_daily_write` | Omitted (defaults to today) | ✅ Wrote 127 bytes to `00-daily/2026-05-28.md` |
| `kb_daily_append` | Omitted (defaults to today) | ✅ Appended 84 bytes to `00-daily/2026-05-28.md` |
| `kb_daily_read` | Omitted (defaults to today) | ✅ Read back full content correctly |

All three operations work correctly without providing a date. The `resolve-daily-path.sh` script runs with empty input, defaults to today's date, and `--path` is properly added to argv. Fix confirmed.

## Related

- `BUG_KB_PATH_DOUBLING.md` — Verified fix for the path doubling bug in the same tools. Discovered during verification testing.
