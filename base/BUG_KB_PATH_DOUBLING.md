# Bug: KB Skill Path Resolution Doubles Sandbox Root

## Status

Resolved

## Summary

The `kb_write`, `kb_append`, `kb_daily_write`, and `kb_daily_append` tools wrote files to incorrect paths where the sandbox root directory appeared twice, separated by a double slash. The corresponding read tools (`kb_read`, `kb_daily_read`) used different path resolution and could not find the files that were written, making the write-then-read cycle broken. The `kb_daily_write` also produced a double `.md` extension.

## Impact

- Files written via `kb_write`/`kb_append`/`kb_daily_write`/`kb_daily_append` ended up at unexpected doubled paths, making them inaccessible via `kb_read`/`kb_daily_read`.
- The written files still existed on disk but at the wrong location, creating orphaned files.
- Read-only kb operations (`kb_read`, `kb_list`, `kb_search`, `kb_daily_read`) worked correctly on files that already existed at the expected paths.
- Also affected: `kb_wikilink_write`, `kb_frontmatter`, `kb_wikilink` (same `resolve-kb-path.sh` script), and `notesmd-daily` (similar `resolve-daily-path.sh` pattern).

## Evidence

Tested on 2026-05-28. Writing via the kb tools produced these results:

### `kb_write`

```
kb_write(path="00-inbox/escape-test-kb.md", content=...)
→ wrote /home/ryan/.chai/active/sandbox//home/ryan/.chai/profiles/developer/sandbox/00-inbox/escape-test-kb.md
```

The file was written to the active sandbox directory with the **developer profile sandbox path concatenated as a subdirectory**, creating a nonsense doubled path.

### `kb_daily_write`

```
kb_daily_write(date="2026-05-28", content=...)
→ wrote /home/ryan/.chai/active/sandbox/00-daily//home/ryan/.chai/profiles/developer/sandbox/00-daily/2026-05-28.md.md
```

Two problems here:
1. Same sandbox path doubling as `kb_write`.
2. Double `.md` extension (`2026-05-28.md.md`) — the daily note resolver appended `.md` to a filename that already had it.

### Reads fail

```
kb_read(path="00-inbox/escape-test-kb.md")       → error: No such file or directory
kb_daily_read(date="2026-05-28")                  → error: No such file or directory
```

Both read tools looked at the correct (non-doubled) paths, but the files were written to the wrong location.

### Workaround

Files written by the kb tools could be read via `devtools_read_file` using the full doubled path. For example:

```
devtools_read_file(path="/home/ryan/.chai/active/sandbox//home/ryan/.chai/profiles/developer/sandbox/00-inbox/escape-test-kb.md")
```

## Root Cause

The `resolveCommand` scripts (`resolve-kb-path.sh`, `resolve-daily-path.sh`) converted relative KB paths to absolute paths by prepending `$HOME/.chai/active/sandbox/`. This is correct on the first pass. However, the `generic.rs` executor runs these scripts **twice** for `writePath` parameters:

1. **First pass** — `validate_write_paths()` calls `transform_param_value()` → `resolve_value()` → runs the resolve script. For input `"00-inbox/test.md"`, the script outputs `/home/ryan/.chai/active/sandbox/00-inbox/test.md`. Then `WriteSandbox::validate()` canonicalizes this (following the `~/.chai/active` symlink) to `/home/ryan/.chai/profiles/developer/sandbox/00-inbox/test.md`. `substitute_canonical_paths()` replaces the param value in args with this canonical path.

2. **Second pass** — `build_argv()` calls `transform_param_value()` → `resolve_value()` → runs the resolve script **again**, this time on the already-resolved canonical path `/home/ryan/.chai/profiles/developer/sandbox/00-inbox/test.md`. The script blindly prepends `$kb_root/`, producing the doubled path: `/home/ryan/.chai/active/sandbox//home/ryan/.chai/profiles/developer/sandbox/00-inbox/test.md`.

For daily notes, the same double-resolution caused two problems: the sandbox root doubling, and the `.md` extension being appended again (producing `2026-05-28.md.md`).

The devtools skills (`devtools_write_file`, `devtools_delete_file`) did **not** have this bug because their `writePath` parameters do not use `resolveCommand` — the path goes directly through `WriteSandbox::validate` → canonical substitution → `build_argv` with no script re-resolution.

## Fix

Made all `resolveCommand` scripts **idempotent**: if the input is already an absolute path (starts with `/`), return it unchanged. This way the second pass through `build_argv` → `resolve_value` → script re-invocation is a no-op on the already-resolved canonical path.

### Files changed

| File | Change |
|------|--------|
| `skills/kb/scripts/resolve-kb-path.sh` | Added `case "$path" in /*) echo "$path"; exit 0 ;; esac` guard |
| `skills/kb-daily/scripts/resolve-daily-path.sh` | Added `case "$date" in /*) echo "$date"; exit 0 ;; esac` guard |
| `skills/kb-wikilink-write/scripts/resolve-kb-path.sh` | Same guard added |
| `skills/kb-wikilink/scripts/resolve-kb-path.sh` | Same guard added |
| `skills/kb-frontmatter/scripts/resolve-kb-path.sh` | Same guard added |
| `skills/notesmd-daily/scripts/resolve-daily-path.sh` | Same guard added |

The `git-remote/scripts/resolve-clone-path.sh` already had this idempotent check and needed no changes.

### Why not fix in `generic.rs`?

An alternative fix would be to skip `resolveCommand` in `build_argv` when a parameter value has already been substituted by `substitute_canonical_paths`. However:
- The resolve scripts are the simpler, more localized fix.
- Making scripts idempotent is a good defensive practice regardless — it protects against any future code path that might re-invoke them.
- The `generic.rs` code path is shared by devtools (which works correctly) and would need careful conditional logic.

## Verified

Requires gateway restart and new session to test. After fix, the expected flow is:

1. `kb_write(path="00-inbox/test.md")` → `resolve-kb-path.sh` outputs `/home/ryan/.chai/active/sandbox/00-inbox/test.md` → `WriteSandbox::validate` canonicalizes → `substitute_canonical_paths` replaces path → `build_argv` calls `resolve-kb-path.sh` again on the absolute path → script returns it unchanged → `chai file write --path <canonical-path>` writes to correct location.

2. `kb_daily_write(date="2026-05-28")` → `resolve-daily-path.sh` outputs absolute path with `.md` → canonicalized → substituted → second script call returns unchanged → correct path, single `.md` extension.

## Related

- `BUG_WRITE_TOOL_ESCAPES.md` — Resolved double-decode bug in the same tools.
- `BUG_SKILLGEN_MULTILINE_FLAG.md` — Separate multiline content issue in skillgen tools.
