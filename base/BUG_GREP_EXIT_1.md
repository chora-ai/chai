# Bug: grep Exit Code 1 on No Matches Surface as Tool Error

## Status

Resolved

## Summary

When `devtools_search_content` is called with a pattern that matches nothing, `grep` exits with status 1. The `GenericToolExecutor` treats any non-zero exit as an error, so the tool returns a cryptic error string instead of an empty result:

```
error: exit exit status 1:
```

## Impact

- Agents cannot distinguish between a genuine failure (bad syntax, permissions, missing path) and a simple "no matches found" result.
- The empty stderr after "exit 1:" provides no diagnostic information.
- Agents must work around this by parsing the error string, which is fragile.

## Root Cause

In `crates/lib/src/exec.rs`, `Allowlist::collect_output` returns `Err` for any non-zero exit status:

```rust
fn collect_output(output: std::process::Output) -> Result<String, String> {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        // ...
        Err(format!("exit {}: {}", output.status, msg))
    }
}
```

For `grep`, exit code 1 means "no match found" (not an error), exit code 2+ means a genuine error (bad option, missing file, etc.). The executor has no way to distinguish these.

## Fix

Added a `successExitCodes` field to `ExecutionSpec` in `descriptor.rs`, implemented `run_with_codes()` / `run_with_stdin_with_codes()` / `collect_output_with_codes()` in `exec.rs`, and set `"successExitCodes": [0, 1]` on the grep tool in `tools.json`. The original `collect_output` (exit-0-only) was replaced by `collect_output_with_codes` with an empty codes list as the default. Exit codes not in the success list (e.g. 2 for grep) still surface as tool errors.

### Files Changed

- `crates/lib/src/skills/descriptor.rs` — Added `success_exit_codes: Option<Vec<i32>>` field to `ExecutionSpec`
- `crates/lib/src/exec.rs` — Added `run_with_codes()`, `run_with_stdin_with_codes()`, `collect_output_with_codes()`. Old `run()` / `run_with_stdin()` / `collect_output()` now delegate to their `_with_codes` variants with an empty list. Removed the commented-out `collect_output` wrapper.
- `crates/lib/config/skills/devtools/tools.json` — Set `"successExitCodes": [0, 1]` on the grep execution spec.
- `crates/lib/src/tools/generic.rs` — Wire `spec.success_exit_codes` through to `run_with_codes` / `run_with_stdin_with_codes`.

## Verification

Three test scenarios executed against the live tool:

| Scenario | Pattern | Path | Result | Pass? |
|----------|---------|------|--------|-------|
| No matches (exit 1) | `zzzz_nonexistent_pattern_xyzzy` | `./chai` | Empty result (no error) | ✅ |
| Matches found (exit 0) | `fn main` | `./chai/crates/cli/src` | `main.rs:76:async fn main() {` | ✅ |
| Genuine error (exit 2) | `test_pattern` | `./nonexistent_directory_xyzzy` | Error with exit status 2 | ✅ |

### Cosmetic Note

The error format string `"exit {}: {}"` combined with `output.status`'s `Display` impl (which already produces `"exit status: N"`) produces `"exit exit status: N: ..."`. This is a pre-existing formatting wart, not introduced by this fix, but could be cleaned up by removing the redundant `"exit "` prefix.

## Related Files

- `crates/lib/src/exec.rs` — `Allowlist::collect_output_with_codes`
- `crates/lib/src/tools/generic.rs` — `GenericToolExecutor::execute`
- `crates/lib/src/skills/descriptor.rs` — `ExecutionSpec`
- `crates/lib/config/skills/devtools/tools.json` — grep execution spec
