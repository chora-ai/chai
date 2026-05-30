# Feature: Audit and Break Up Large Files

## Status

Open

## Summary

Several source files in the chai codebase have grown too large to manage effectively — both for human readability and for the `devtools_write_file` tool, which must write entire files at once. During the BUG_WRITE_SILENT_FAIL fix, `generic.rs` (60KB, ~1580 lines) could not be updated in a single write, requiring a directory module refactoring as a workaround. A systematic audit and refactoring would improve maintainability.

## Problem

- Large files are hard to navigate and understand.
- The `devtools_write_file` tool can only write entire files, making targeted edits to large files impossible (see `FEAT_LINE_LEVEL_WRITES.md`).
- Large files often mix concerns that could be cleanly separated.

## Known Large Files

| File | Size | Notes |
|------|------|-------|
| `crates/lib/src/tools/generic/mod.rs` | ~43KB | Partially refactored — `run_post_process` extracted to `post_process.rs`, but could benefit from further splits (e.g. `validate_write_paths`, `apply_side_read`, `build_argv`/`extract_stdin_content`) |
| `crates/lib/src/exec.rs` | ~25KB | Mostly `WriteSandbox` tests; could split `WriteSandbox` into its own module |
| `crates/lib/src/tools/generic.rs` | ~~60KB~~ | **Done** — refactored into `generic/mod.rs` + `post_process.rs` |

## Refactoring Principles

- **One concern per file**: Each major function group (validation, path resolution, side-read, argv building, etc.) is a candidate for extraction.
- **Directory modules over single files**: Convert `foo.rs` → `foo/mod.rs` + sibling files when splitting, preserving the public API.
- **Tests stay with their code**: When extracting a function, move its tests to the new module too.
- **Don't over-split**: Files under ~20KB are generally fine. Focus on files that exceed the practical write limit or mix clearly separable concerns.

## Proposed Splits

### `crates/lib/src/tools/generic/mod.rs` (~43KB)

- `validate.rs` — `validate_write_paths`, `ensure_write_path_parents`, `substitute_canonical_paths`
- `side_read.rs` — `apply_side_read` and its tests
- `argv.rs` — `build_argv`, `extract_stdin_content`, `resolve_value`, `run_script`, `transform_param_value`, helper functions
- `mod.rs` — `GenericToolExecutor` struct, `impl ToolExecutor`, thin re-exports

### `crates/lib/src/exec.rs` (~25KB)

- `sandbox.rs` — `WriteSandbox` struct, `validate`, `canonicalize_for_write`, and all sandbox tests
- `mod.rs` — `Allowlist`, `resolve_binary`, execution methods

## Related

- `FEAT_LINE_LEVEL_WRITES.md` — complementary feature for editing without full rewrites
