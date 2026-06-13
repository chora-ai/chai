# BUG: files_write_lines original_content Whitespace Mismatches

## Status

Fixed

## Discovered

2025-06-12

## Tool

`files_write_lines`

## Problem

The `original_content` parameter in `files_write_lines` sometimes fails to match the actual file content even when the agent copies it directly from `files_read_lines` output. The error message shows content that appears identical but reports different byte lengths, suggesting invisible whitespace or encoding differences.

## Two Distinct Failure Modes

### Failure Mode 1: Wrong Line Range — Agent Error, Working As Designed

The most common cause is the agent specifying an incorrect line range (e.g., lines 82-84 when the content spans lines 82-85). The hint about different lengths helps diagnose this — the actual content at those lines includes an extra line the agent didn't account for.

This is an agent error, not a tool bug. The tool correctly rejects these edits and provides a helpful error message showing the actual content.

### Failure Mode 2: Invisible Whitespace Differences — Fixed

Less commonly, the `original_content` appears to match the file content exactly but still fails verification. This is caused by trailing whitespace that is invisible in the tool output but present in the file.

## Fix Applied

The four-stage comparison in `verify_original` (exact match, NFC normalization, Unicode-to-ASCII folding, trailing-whitespace tolerance) already handles this. When the only difference is trailing whitespace per line, the match is accepted and the file's original trailing whitespace is preserved in the replacement content.

The trailing-whitespace-tolerant match was already implemented before this bug was filed. The bug was kept open because the LLM was still experiencing friction — the LLM would drop trailing whitespace, the match would succeed via stage 4, but then the replacement would also drop the trailing whitespace. This has been addressed by the trailing whitespace preservation logic: when stage 4 matches, the tool captures the original trailing whitespace and re-applies it to the replacement lines.

The remaining friction is with `files_replace`, which did not have a similar trailing-whitespace-tolerant matching mode. This has been addressed in the fix for [BUG_FILES_REPLACE.md](BUG_FILES_REPLACE.md).

## Impact

- Failed `files_write_lines` calls consume tool iterations but are now rare.
- The trailing whitespace preservation ensures that even when the LLM drops trailing whitespace, the file's original whitespace is preserved in the output.
- Line number shifts from earlier edits remain an issue but are correctly handled by the `original_content` verification (the agent must re-read before editing).

## Workaround

- **Re-read before editing**: Always call `files_read_lines` on the target range immediately before calling `files_write_lines`, especially if other edits have been made to the same file in the same session.
- **Work from bottom to top**: When making multiple non-adjacent edits in the same file, start with the highest line numbers first so earlier edits don't shift the line numbers of later edits.
- **Prefer single large edits over multiple small ones**: Rewriting a whole affected section as one `files_write_lines` call avoids the line-shift problem.
