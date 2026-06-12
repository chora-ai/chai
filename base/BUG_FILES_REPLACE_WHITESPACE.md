# BUG: files_replace Whitespace/Encoding Mismatches and Corrupted Output

## Status

Open

## Discovered

2025-06-12

## Tool

`files_replace`

## Problem

`files_replace` has two distinct issues: (1) multi-line patterns sometimes fail to match despite appearing correct ("0 replacements"), and (2) successful replacements can produce corrupted output with incorrect whitespace, line ordering, or duplicate insertions.

## Issue 1: Silent Match Failures ("0 Replacements")

Multi-line patterns that appear to exactly match file content sometimes fail with "0 replacements", even when the agent copies the pattern directly from the file. The pattern and the actual file content look identical in the tool output but do not match internally.

### Observed Behavior

1. Agent reads a section of a file using `files_read_lines`.
2. Agent copies the visible content into a `files_replace` pattern.
3. The tool reports "0 replacements" — no match found.
4. Re-reading the file shows the same content, suggesting an invisible difference (trailing whitespace, line endings, or encoding).

In one session, a replacement for `execute_turn_main`'s function signature failed with "0 replacements" despite the pattern being copied directly from the file output. The same pattern structure worked for an earlier replacement of `run_turn`'s signature in the same file, suggesting the issue is intermittent and content-dependent.

### Root Cause Hypothesis

The pattern matching may be sensitive to:

1. **Trailing whitespace** — lines may have trailing spaces or tabs that are stripped in the tool output but present in the file.
2. **Line endings** — CRLF vs LF differences between the pattern string and the file content.
3. **Unicode normalization** — the tool output may display normalized characters while the file contains different byte sequences.

## Issue 2: Corrupted Replacement Output

Successful replacements can produce incorrectly formatted output. This manifests in three ways:

### 2a: Misplaced Insertion Lines

When a replacement inserts new lines into an existing code structure, the inserted lines can appear at the wrong position, breaking syntax.

**Example:** Replacing a function call's closing arguments to add a new parameter. The `run_turn_dyn` call in `run_turn` had:

```rust
        on_chunk,
    )
    .await
}
```

The replacement added `stop_flag,` as a new parameter. The expected result was:

```rust
        on_chunk,
        stop_flag,
    )
    .await
}
```

But the actual result was:

```rust
        on_chunk,
    )
        stop_flag,
    .await
}
```

The `stop_flag,` line was inserted **after** the closing `)` instead of **before** it, breaking the method call syntax. The diff in the tool output showed the line at the wrong position.

### 2b: Duplicate Match Insertion

When the same pattern appears in multiple locations in a file, `files_replace` matches and replaces **all** occurrences. This is documented behavior, but the agent may not anticipate it when the duplicated pattern is boilerplate code that appears in sibling functions.

**Example:** A Rust source file had two functions (`execute_turn_worker` and `execute_turn_main`) with nearly identical loop boilerplate:

```rust
    let mut loop_limit_reached = false;
    let mut pending_tool_calls: Vec<ToolCall> = Vec::new();

    loop {
        let use_stream = on_chunk.is_some() && loop_count == 0;
```

The agent wanted to add stop-flag logic only to `execute_turn_main`, but the pattern matched in **both** functions. The replacement inserted `let mut stopped = false;`, a stop-flag clearing block, and a stop-check block into `execute_turn_worker`, which doesn't have a `stop_flag` parameter. This caused compilation errors (undefined `stop_flag` variable).

The interleaved result looked like:

```rust
    let mut pending_tool_calls: Vec<ToolCall> = Vec::new();
    let mut stopped = false;

    // Clear any stale stop flag from a previous turn before starting.
    if let Some(ref flag) = stop_flag {   // ERROR: stop_flag doesn't exist here
        flag.store(false, Ordering::SeqCst);
    }

    loop {
        // Check stop flag before each iteration.
        if let Some(ref flag) = stop_flag {  // ERROR: stop_flag doesn't exist here
            ...
        }

        let use_stream = on_chunk.is_some() && loop_count == 0;
```

The same replacement was also applied to `execute_turn_main` (the intended target), but because both replacements happened simultaneously, the resulting file had corrupted code in two places.

### 2c: Insertion Line Interleaving

In some replacements, the new lines appear to be interleaved with the existing lines rather than replacing the matched range cleanly. This produces code where new and old lines alternate in a way that doesn't match either the original or the intended replacement.

## Impact

- **Corrupted files require full rewrites** — when `files_replace` produces malformed output, the agent must read the entire file and rewrite it with `files_write_file`, consuming significant context.
- **Duplicate match insertions are particularly dangerous** — the agent may not notice that a replacement was applied to an unintended location, especially in large files where the duplicate is far from the intended target.
- **The tool's diff output shows the corruption**, but the agent must carefully review it to detect the problem.

## Workaround

### For Issue 1 (0 Replacements)

- Use `files_replace` for simple, single-line or short patterns where whitespace is unlikely to be an issue.
- For multi-line replacements, prefer `files_write_lines` with `original_content` verified from `files_read_lines` output.
- If `files_replace` fails with "0 replacements" on content that appears correct, re-read the exact lines and try `files_write_lines` instead.
- As a last resort, rewrite the entire file with `files_write_file`.

### For Issue 2 (Corrupted Output)

- **Always review the diff output** after a `files_replace` call. Check that:
  - The replacement was applied at the correct location(s).
  - The number of replacements matches expectations (1 unless intentionally global).
  - The resulting code structure is syntactically valid (proper indentation, line ordering, bracket matching).
- **Beware of duplicate patterns** — before calling `files_replace`, search the file for the pattern to confirm it only appears once, or include enough surrounding context in the pattern to make it unique.
- **Prefer `files_write_lines` for targeted edits** where the surrounding context must be verified — it operates on explicit line ranges and cannot accidentally match the wrong location.
- **When a replacement corrupts the file**, read the affected section immediately and fix it with `files_write_lines` or rewrite the entire file.

## Proposed Fixes

### For Issue 1

Apply the same four-stage content comparison used by `files_write_lines` (exact match, NFC normalization, Unicode-to-ASCII folding, trailing-whitespace tolerance) to the `files_replace` pattern matching.

### For Issue 2a (Misplaced Insertions)

Investigate the line-placement algorithm in `files_replace`. When the replacement string contains lines that are meant to be inserted between existing lines, ensure they are placed at the correct position within the matched range, not after the closing boundary of the match.

### For Issue 2b (Duplicate Match Insertion)

Consider adding an optional `max_replacements` parameter (default: unlimited for backward compatibility) that limits the number of replacements. Setting `max_replacements: 1` would prevent the duplicate-match scenario. Alternatively, warn in the diff output when multiple replacements are made in the same call.

### For Issue 2c (Line Interleaving)

Investigate whether the replacement algorithm correctly handles the boundary between the matched pattern and the replacement string, particularly when the replacement contains both existing lines (preserved from the pattern) and new lines (inserted).
