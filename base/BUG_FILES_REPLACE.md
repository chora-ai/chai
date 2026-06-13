# BUG: files_replace Whitespace/Encoding Mismatches and Corrupted Output

## Status

Fixed

## Discovered

2025-06-12

## Tool

`files_replace`

## Problem

`files_replace` had two distinct issues: (1) multi-line patterns sometimes fail to match despite appearing correct ("0 replacements"), and (2) successful replacements can produce corrupted output with incorrect whitespace, line ordering, or duplicate insertions.

## Issue 1: Silent Match Failures ("0 Replacements") — Fixed

Multi-line patterns that appear to exactly match file content sometimes fail with "0 replacements", even when the agent copies the pattern directly from the file.

### Root Cause

The pattern matching was sensitive to trailing whitespace differences. Lines may have trailing spaces or tabs that are stripped in the tool output but present in the file. The LLM frequently drops trailing whitespace when reproducing file content from `files_read_lines` output into the pattern parameter.

### Fix Applied

When the regex produces 0 replacements, `files_replace` now automatically falls back to a trailing-whitespace-tolerant literal search:

1. Process escape sequences (`\n` → newline, `\t` → tab, `\\` → backslash) in the pattern so the literal search matches the same content the regex engine would match. Without this step, multi-line patterns using `\n` would never match in the fallback because `strip_trailing_ws_per_line` splits on actual newlines, not on the two-character sequence backslash-n.
2. Strip trailing whitespace from each line of both the pattern and the file content.
3. Search for the stripped pattern as literal text in the stripped file content.
4. If found, map the match back to the original (unstripped) content using a byte-offset mapping.
5. Apply the replacement, preserving the file's original trailing whitespace for lines that are kept from the match.

The fallback is only attempted for patterns longer than 1 character (to avoid false positives). When a match is found via the fallback, the output message includes "(trailing-whitespace-tolerant match)" so the agent can distinguish it from a normal regex match.

Additionally, the `\n` escape sequence in the `replacement` parameter is now processed as a newline character (previously it was treated as a literal backslash-n). This makes the replacement consistent with how `\n` works in the pattern. The `\t` (tab) and `\\` (literal backslash) escape sequences are also processed.

### Remaining Gaps

- **Leading whitespace (indentation)** is not normalized. If the LLM's pattern is missing indentation that the file has, the regex won't match and the literal fallback won't either. This is intentional — indentation is semantically meaningful.
- **Unicode normalization** (NFC vs NFD) and **Unicode-to-ASCII folding** are not applied in the fallback. These are handled by `files_write_lines`'s four-stage comparison but adding them to the `files_replace` fallback would increase complexity significantly for a rare case.

## Issue 2: Corrupted Replacement Output — Fixed

### 2a: Misplaced Insertion Lines — Fixed

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

**Root cause:** This was caused by the trailing-whitespace-tolerant fallback not processing `\n` escape sequences in the pattern. When the fallback received a multi-line pattern like `on_chunk,\n    )\n    .await`, it treated `\n` as literal characters (backslash-n) rather than real newlines. This caused the fallback to either fail entirely or produce garbled matches. The regex path (which correctly interprets `\n`) was never the source of this bug — the issue only manifested when the fallback path was triggered by trailing whitespace differences.

**Fix:** The fallback now processes `\n`, `\t`, and `\\` escape sequences in the pattern before performing the literal search, matching the regex engine's interpretation. With this fix, the fallback produces correct results for multi-line patterns.

### 2b: Duplicate Match Insertion — Fixed

When the same pattern appears in multiple locations in a file, `files_replace` matches and replaces **all** occurrences. This is documented behavior, but the agent may not anticipate it when the duplicated pattern is boilerplate code that appears in sibling functions.

**Example:** A Rust source file had two functions (`execute_turn_worker` and `execute_turn_main`) with nearly identical loop boilerplate:

```rust
    let mut loop_limit_reached = false;
    let mut pending_tool_calls: Vec<ToolCall> = Vec::new();

    loop {
        let use_stream = on_chunk.is_some() && loop_count == 0;
```

The agent wanted to add stop-flag logic only to `execute_turn_main`, but the pattern matched in **both** functions. The replacement inserted `let mut stopped = false;`, a stop-flag clearing block, and a stop-check block into `execute_turn_worker`, which doesn't have a `stop_flag` parameter. This caused compilation errors (undefined `stop_flag` variable).

**Fix:** Added a `max_replacements` parameter to `files_replace`. The default is 0 (unlimited, preserving backward compatibility). Setting `max_replacements: 1` replaces only the first match, preventing the duplicate-match scenario. When `max_replacements` limits the result, the output shows "N of M match(es) replaced" instead of "M replacement(s)" so the agent can see there were additional matches not replaced.

### 2c: Insertion Line Interleaving — Closed (Not Reproducible)

In some replacements, the new lines appear to be interleaved with the existing lines rather than replacing the matched range cleanly. This produces code where new and old lines alternate in a way that doesn't match either the original or the intended replacement.

**Investigation:** Extensive testing through both the regex path (with various multi-line patterns, capture groups, and insertion scenarios) and the fixed fallback path could not reproduce line interleaving. The `regex::Regex::replace_all` + `Captures::expand` pipeline produces correct output in all tested cases. The most likely explanation is that 2c was a manifestation of 2a — the unescaped-`\n` bug in the fallback caused garbled partial matches that appeared as interleaving in the diff output. With the fallback now correctly processing escape sequences, this issue no longer occurs.

## Impact

- ~~**Corrupted files require full rewrites**~~ — the regex path produces correct results; the fallback now correctly handles multi-line patterns.
- **Duplicate match insertions are now preventable** — the `max_replacements` parameter allows the agent to limit replacements to the first match.
- **The tool's diff output shows all changes** — the agent should review the diff to verify the replacement was applied correctly, especially when the output shows multiple matches.

## Workaround

### For Issue 1 (0 Replacements) — No Longer Needed

The trailing-whitespace-tolerant fallback now handles the most common cause of "0 replacements" automatically, including multi-line patterns with `\n` escapes.

### For Issue 2 (Corrupted Output) — No Longer Needed

- The regex path produces correct results for all tested scenarios.
- The fallback path now correctly handles multi-line patterns and trailing whitespace.
- Use `max_replacements: 1` to prevent unintended replacements when a pattern may match multiple locations.
- **Prefer `files_write_lines` for targeted edits** where the surrounding context must be verified — it operates on explicit line ranges and cannot accidentally match the wrong location.
