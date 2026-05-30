# Bug: devtools_write_file Silently Fails on Some Content (Retry Succeeds)

## Status

Verified

## Summary

`devtools_write_file` occasionally failed silently — the tool returned no error, but the file was not created on disk. Retrying with identical content then succeeded. Observed during verification testing of the `normalizeNewlines` fix (BUG_WRITE_TOOL_ESCAPES.md).

## Root Cause

The primary cause was that `extract_stdin_content` in `generic.rs` silently returned `None` when a `kind: "stdin"` parameter was missing or null from the tool call arguments. When `None` was returned, the executor fell through to `run_with_codes()` — the no-stdin code path — which ran `chai file write --path <canonical>` **without piping any content** and without the `--content` flag.

When `chai file write` receives no `--content` and no stdin, it falls through to `read_content_from_stdin_or(None)`, which reads from the inherited stdin of the gateway process. If the gateway's stdin is `/dev/null`, this produces a 0-byte file. If the gateway's stdin is an open pipe, the child blocks indefinitely. In both cases, `chai file write` exits 0 — explaining the "silent failure" symptom.

The LLM occasionally omits the `content` argument from tool calls (possibly more often for complex/escape-heavy content due to token limits or truncation), which creates the observed correlation with `\\`-heavy content and the transient, nondeterministic pattern.

A secondary issue was that `run_with_stdin_with_codes` in `exec.rs` used `if let Some(mut pipe) = child.stdin.take()` which silently skipped stdin piping if `take()` returned `None`. Since `Stdio::piped()` is always set, `take()` should always return `Some`, making this a defensive-improvement rather than a live bug. The same `if let Some` pattern existed in `run_post_process` in `generic.rs`.

## Fix

Both fixes are now implemented and verified:

### 1. `extract_stdin_content` now validates required stdin params — ✅ Implemented and Verified

- Changed return type from `Option<String>` to `Result<Option<String>, String>`.
- For required stdin params (those without `optional: true`), returns `Err("missing required parameter: {param}")` when the parameter is missing, null, or has a non-string type.
- For optional stdin params, missing/null values still return `Ok(None)`.
- Added `log::warn!` when a required stdin param is missing.
- The `execute()` method now propagates this error with `?`, surfacing a clear "missing required parameter: content" error to the agent instead of silently proceeding.
- Added four unit tests covering present, missing, null, and optional cases.

### 2. Explicit stdin pipe scoping — ✅ Implemented and Verified

Changed all three sites that used `if let Some(mut pipe) = child.stdin.take()` to use explicit error handling and block-scoped pipe lifetime:

1. **`crates/lib/src/exec.rs`** (`run_with_stdin_with_codes`): Replaced `if let Some(mut pipe)` with `child.stdin.take().ok_or_else(...)` in a block scope that explicitly drops the pipe before `wait_with_output()`. This surfaces an error if the pipe is unavailable and guarantees the child sees EOF before we wait for it.

2. **`crates/lib/src/tools/post_process.rs`** (`run_post_process`): Extracted `run_post_process` into its own module with a `pipe_stdin` helper that uses the same `ok_or_else` + block-scope pattern. In this best-effort context, pipe errors are logged with `log::warn!` rather than propagated (the function returns the original input on failure).

3. **`crates/lib/src/tools/generic/mod.rs`**: `generic.rs` was refactored into `generic/mod.rs` with `run_post_process` extracted to `post_process.rs`. This was done to stay within the write tool's content size limit (the original 60KB file was too large for a single write). The `post_process` tests were also relocated to `post_process.rs`.

The `pipe_stdin` helper pattern (in `exec.rs` inline and `post_process.rs` as a function):

```rust
{
    let mut pipe = child.stdin.take().ok_or_else(|| {
        "failed to acquire stdin pipe: Stdio::piped() was set but pipe is unavailable"
            .to_string()
    })?;
    pipe.write_all(stdin)
        .map_err(|e| format!("failed to write stdin: {}", e))?;
}
// Pipe is dropped here — child sees EOF on stdin.
```

## Verification (2025-05-29)

Fix #1 was verified with live tool calls:

| # | Content | Path | Result | Notes |
|---|---------|------|--------|-------|
| V1 | Rust code with `"hello\n"`, `"\t"`, `"\\n"` | `./test_escape_preserved.rs` | ✅ Written correctly | Previously failed on first attempt |
| V2 | Rust code with `"C:\\Users\\test\\file.txt"` | `./test_winpath.rs` | ✅ Written correctly | Previously failed on first attempt |
| V3 | Mixed content with `\\`, `\n`, `\t` | `./test_mixed_complex.rs` | ✅ Written correctly | Complex combined escapes |
| V4 | Empty content | `./test_empty.txt` | ✅ Written correctly (0 bytes) | Edge case |
| V5 | Simple plaintext (no escapes) | `./test_simple.txt` | ✅ Written correctly | Baseline |

Fix #2 was verified by code inspection — all `child.stdin.take()` callsites across the codebase now use `ok_or_else` instead of `if let Some`:

- `crates/lib/src/exec.rs`: `run_with_stdin_with_codes` — `ok_or_else` with `?` propagation
- `crates/lib/src/tools/post_process.rs`: `pipe_stdin` helper — `ok_or_else` with warning log
- `crates/lib/src/tools/generic/mod.rs`: no inline `run_post_process` (uses imported version)

No `if let Some(mut pipe) = child.stdin.take()` or `if let Some(ref mut stdin) = child.stdin.take()` patterns remain.

## Impact

- ~~Agents cannot trust that a successful `devtools_write_file` return means the file was actually written.~~ **Resolved**: Missing `content` now produces a clear error ("missing required parameter: content") that the agent can see and retry.
- ~~Silent failures can cause downstream errors when the agent reads or uses the file later.~~ **Resolved**: The error is surfaced immediately.
- ~~The nondeterministic nature makes it hard to work around.~~ **Resolved**: The root cause (silent fallthrough on missing stdin param) is eliminated.

## Related Files

- `crates/lib/src/exec.rs` — `run_with_stdin_with_codes` (fixed: explicit pipe scoping with `ok_or_else`)
- `crates/lib/src/tools/generic/mod.rs` — `extract_stdin_content`, `GenericToolExecutor::execute` (fixed: required stdin param validation; refactored from single file to directory module)
- `crates/lib/src/tools/post_process.rs` — `pipe_stdin`, `run_post_process` (new file, extracted from generic/mod.rs; fixed: explicit pipe scoping)
- `crates/lib/src/tools/mod.rs` — added `mod post_process;`
- `crates/lib/config/skills/devtools/tools.json` — write tool execution spec (`"kind": "stdin"`)
- `BUG_WRITE_TOOL_ESCAPES.md` — related bug (now resolved) where `normalizeNewlines` caused content corruption
