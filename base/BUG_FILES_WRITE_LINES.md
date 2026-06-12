# BUG: files_write_lines original_content Whitespace Mismatches

## Status

Open

## Discovered

2025-06-12

## Tool

`files_write_lines`

## Problem

The `original_content` parameter in `files_write_lines` sometimes fails to match the actual file content even when the agent copies it directly from `files_read_lines` output. The error message shows content that appears identical but reports different byte lengths, suggesting invisible whitespace or encoding differences.

## Observed Behavior

1. Agent reads lines from a file using `files_read_lines`.
2. Agent copies the visible content into `files_write_lines`'s `original_content` parameter.
3. The tool rejects the edit with an error like:

```
original_content mismatch at lines 82-84:
  expected:
        chat_turn_receiver: Option<mpsc::Receiver<Result<AgentReply, String>>>,
        /// User message we sent for the in-flight turn (used when reply creates a new session).
  actual:
        chat_turn_receiver: Option<mpsc::Receiver<Result<AgentReply, String>>>,
        /// User message we sent for the in-flight turn (used when reply creates a new session).
        pending_user_message: Option<String>,

  hint: different lengths - expected 169 bytes, actual 210 bytes
```

4. The "expected" and "actual" content appear identical in the error output, but the byte lengths differ. In this example, the agent's `original_content` was 2 lines (169 bytes) but the actual file had 3 lines (210 bytes) at those line numbers.

## Two Distinct Failure Modes

### Failure Mode 1: Wrong Line Range

The most common cause is the agent specifying an incorrect line range (e.g., lines 82-84 when the content spans lines 82-85). This is an agent error, not a tool bug. The hint about different lengths helps diagnose this — the actual content at those lines includes an extra line the agent didn't account for.

### Failure Mode 2: Invisible Whitespace Differences

Less commonly, the `original_content` appears to match the file content exactly but still fails verification. The tool's four-stage comparison (exact match → NFC normalization → Unicode-to-ASCII folding → trailing-whitespace tolerance) should handle most of these cases, but some differences still escape detection or correction.

In this session, the primary failures were in Failure Mode 1 (wrong line range). The agent would specify `start_line` and `end_line` based on an earlier read, but the file had been modified between reads (by a previous `files_replace` call), shifting line numbers. The tool correctly rejected these edits.

## Impact

- Failed `files_write_lines` calls consume tool iterations.
- When the agent is working on a file that has been modified by earlier tool calls in the same session, line numbers may shift, causing `original_content` mismatches.
- The error messages are helpful but the agent must re-read the file to get correct line numbers, adding overhead.

## Workaround

- **Re-read before editing**: Always call `files_read_lines` on the target range immediately before calling `files_write_lines`, especially if other edits have been made to the same file in the same session.
- **Work from bottom to top**: When making multiple non-adjacent edits in the same file, start with the highest line numbers first so earlier edits don't shift the line numbers of later edits.
- **Prefer single large edits over multiple small ones**: Rewriting a whole affected section as one `files_write_lines` call avoids the line-shift problem.

## Proposed Fix

### For Failure Mode 1 (Wrong Line Range)

The tool already handles this correctly — the `original_content` verification catches stale line numbers and the error message shows the actual content. No fix needed; this is working as designed.

### For Failure Mode 2 (Invisible Whitespace)

If invisible whitespace mismatches persist after the four-stage comparison, consider:

1. **Show hex diffs** — When content doesn't match, include a character-level diff (showing whitespace characters like `\t`, trailing spaces, or `\r\n` vs `\n`) so the agent can diagnose the actual difference.
2. **Normalize on read** — Ensure `files_read_lines` output uses the same normalization as the `original_content` comparison, so the agent always sees content that will pass verification.
