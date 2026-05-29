# Bug: Side-Read Loads AGENTS.md From Wrong Directory

## Status

**Resolved** — fix applied, rebuilt, and verified.

## Summary

When `devtools_list_dir` is called with a relative path (e.g. `"."`), the `sideRead` feature appends the wrong `AGENTS.md` file to the tool result. Instead of loading `AGENTS.md` from the directory that was listed, it loads `AGENTS.md` from the gateway process's current working directory.

## Reproduction

1. Sandbox root directory contains `AGENTS.md` (sandbox-focused content) and a `chai` symlink pointing to a git repository that also has its own `AGENTS.md` (project-focused content).
2. Call `devtools_list_dir` with `path = "."`.
3. The `ls` output correctly shows the sandbox root contents (AGENTS.md, chai symlink).
4. The appended `--- AGENTS.md ---` section contains the **chai project's** AGENTS.md content instead of the **sandbox root's** AGENTS.md.

## Root Cause

In `crates/lib/src/tools/generic.rs`, the `execute` method called `apply_side_read` with the **original** `args` instead of the **resolved** `effective_args`:

```rust
// Before fix (line ~77 in execute()):
if let Some(ref sr) = spec.side_read {
    Ok(apply_side_read(sr, args, &result, session_id, &self.side_read_seen))
} else {
    Ok(result)
}
```

The `effective_args` object contains canonical (absolute, symlink-resolved) paths substituted by `substitute_canonical_paths`, while `args` retains the original raw values (e.g. `"."`, `"./chai"`).

### How the mismatch happens

1. `validate_write_paths` resolves the `path` param through the sandbox and produces canonical absolute paths in `canonical_paths`.
2. `substitute_canonical_paths` builds `effective_args` with these canonical paths — this is used for `build_argv` and for setting the `ls` binary's CWD.
3. The `ls` child process runs with the correct CWD (the canonical sandbox root) and produces the correct listing.
4. `apply_side_read` received the **original** `args`, extracted the raw `path` value (`"."`), and constructed `Path::new(".").join("AGENTS.md")`.
5. `std::fs::read_to_string("./AGENTS.md")` resolved `"."` relative to the **gateway process's current working directory** — not relative to the sandbox root where `ls` ran.
6. This resulted in reading the wrong `AGENTS.md` file (the chai project's instead of the sandbox root's).

## Fix Applied

Changed the `apply_side_read` call in `execute()` to use `effective_args` instead of `args`:

```rust
// After fix:
// Use effective_args (with canonical paths) so that apply_side_read
// locates the file relative to the resolved directory the tool
// operated on, not relative to the gateway process's CWD.
if let Some(ref sr) = spec.side_read {
    Ok(apply_side_read(sr, effective_args, &result, session_id, &self.side_read_seen))
} else {
    Ok(result)
}
```

This ensures `apply_side_read` uses the canonical absolute path for the `path` param, so it reads `AGENTS.md` from the same directory that the tool operated on.

## Verification

After rebuilding, tested with two scenarios:

1. `devtools_list_dir` with `path = "."` — the `--- AGENTS.md ---` section showed the **sandbox root's** AGENTS.md (environment context and active work tracking). ✅
2. `devtools_list_dir` with `path = "./chai"` — the `--- AGENTS.md ---` section showed the **chai project's** AGENTS.md (architecture overview, code style guidelines, crate descriptions). ✅

Both cases now correctly load AGENTS.md from the directory being listed.

### Considerations

- **`oncePerSession` dedup key**: `apply_side_read` uses `path_str` to build the dedup key (`format!("{}/{}", path_str, sr.filename)`). Switching to `effective_args` means the canonical absolute path is used for the key instead of the raw relative path. This is actually an improvement — it correctly distinguishes between different directories even if the same relative path string is used in separate calls.
- **Falling through to `else` branch**: When `canonical_paths` is empty, `effective_args` is set to `args` (`let effective_args = if canonical_paths.is_empty() { args } else { ... }`), so the `else` branch (no `sideRead`) is unaffected. When `effective_args` is the resolved args, it's a local binding (`resolved_args`) that lives long enough for the `apply_side_read` call.
- **Backward compatibility**: Tools without `readPath`/`writePath` params won't have `canonical_paths`, so `effective_args` equals `args` and behavior is unchanged.

## Related Files

- `crates/lib/src/tools/generic.rs` — contains `execute()` and `apply_side_read()`
- `crates/lib/src/skills/descriptor.rs` — defines `SideReadSpec`
- `crates/lib/config/skills/devtools/tools.json` — defines the `sideRead` spec for `devtools_list_dir`
