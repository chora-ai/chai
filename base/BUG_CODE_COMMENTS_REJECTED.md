# BUG: files_replace Pattern Parameter Misinterpreted by Validation and CLI Parsing

## Status

Open

## Discovered

2025-06-12

## Tool

`files_replace`

## Problem

The `pattern` parameter in `files_replace` is a free-form string that can contain any text (regex patterns, source code, comments, punctuation). However, the tool's input validation and CLI argument parsing sometimes misinterpret the pattern value as something other than a content string — treating it as a file path or as CLI flags. This causes legitimate replacement calls to be rejected or to fail with cryptic errors.

Two distinct manifestations have been observed.

## Issue 1: Patterns Starting with `///` Rejected as Absolute Paths

### Observed Behavior

When a `files_replace` pattern begins with `///` (a Rust doc comment), the tool rejects the call with:

```
parameter 'pattern' received an absolute path '/// WebSocket event name...' but is not annotated as a path parameter; add readPath, writePath, or unsafePath
```

The tool incorrectly interprets the `///` prefix as an absolute file path and applies path validation to the pattern string.

### Reproduction

```
files_replace(
  path: "chai/crates/lib/src/orchestration/delegate.rs",
  pattern: "/// WebSocket event name: tool loop iteration limit reached.\npub const EVENT_TOOL_LOOP_LIMIT...",
  replacement: "..."
)
```

This will fail. Adding a preceding line (e.g., a blank line or a non-comment line) to the pattern so it doesn't start with `///` avoids the error.

### Root Cause

The tool's input validation checks if a string parameter looks like an absolute path. A string starting with `///` has three leading slashes and is classified as an absolute path, triggering the path validation gate.

### Proposed Fix

The path validation heuristic should distinguish between file paths and content strings:

1. **Deny single `/`** — A pattern starting with a single `/` followed by a non-`/` character is likely an absolute path (e.g., `/etc/passwd`). This should remain rejected.
2. **Allow `//` and `///`** — A pattern starting with `//` or `///` is almost certainly a comment (shell comment, C++/Rust/Java line comment, or Rust doc comment), not a file path. No legitimate file path starts with `//` or `///`.

The specific rule: reject parameter values starting with `/` only when the second character is **not** also `/`. In regex terms: reject `^/[^/]` but allow `^//`.

This does not create a new security vulnerability because:

- No filesystem path on any mainstream OS starts with `//` or `///`.
- On Linux/POSIX, `//` is defined as implementation-defined but in practice resolves to `/` — it is not a distinct path that could be used for path traversal.
- On Windows, paths start with drive letters (`C:\`) or UNC prefixes (`\\`), not `//`.
- The existing single-`/` check still catches the most common absolute path pattern.

### Workaround

- Ensure the `pattern` parameter does not start with `///`. Include a preceding line (e.g., a blank line or the line before the doc comment) in the pattern.
- Use `files_write_lines` instead, which does not apply path validation to its `original_content` parameter.

## Issue 2: Patterns Containing `-` Interpreted as CLI Flags

### Observed Behavior

When a `files_replace` pattern contains a hyphen (`-`) in a position that resembles a CLI flag (e.g., a Markdown bullet list item starting with `- **[...**`), the tool rejects the call with a CLI parsing error:

```
error: unexpected argument '- ' found

Usage: chai file replace [OPTIONS] --path <PATH> --pattern <PATTERN> --replacement <REPLACEMENT>
```

The pattern string is being parsed as if its content were additional CLI arguments, and the `-` is interpreted as a flag prefix.

### Reproduction

```
files_replace(
  path: "chai/base/AGENTS.md",
  pattern: "- **[BUG_FILES_REPLACE_WHITESPACE.md](BUG_FILES_REPLACE_WHITESPACE.md)** — Multi-line patterns...",
  replacement: "..."
)
```

This will fail because the `-` at the start of the pattern is interpreted as a CLI flag. The same issue can occur with patterns that contain ` --` (double hyphen, common in prose or code comments) or any `-` prefixed token that the argument parser treats as a flag.

### Root Cause

The `files_replace` tool is likely implemented as a CLI command that passes the `pattern` parameter as a positional argument or option value. When the pattern string contains characters that the CLI argument parser recognizes as option prefixes (`-`), the parser consumes them as flags rather than as the literal pattern value. This is the classic "option injection" problem in CLI tools.

The fix is to ensure the pattern value is properly escaped or delimited when passed through the CLI layer, so its content is never interpreted as arguments.

### Proposed Fix

The `pattern` parameter should be passed through the CLI layer in a way that prevents its content from being parsed as flags:

1. **Use `--` end-of-options marker** — If the underlying CLI invocation passes parameters as positional arguments, insert `--` before the pattern value to signal end of option parsing.
2. **Pass via stdin or environment variable** — For parameters that can contain arbitrary text, avoid passing them as CLI arguments entirely. Pipe the pattern via stdin or set it as an environment variable.
3. **JSON body** — Accept the full replacement request as a JSON body (similar to how the `files_write_file` content parameter works), avoiding CLI argument parsing entirely for content-bearing parameters.

This does not create a security vulnerability because the fix is about properly delimiting parameter values, not about allowing new types of content.

### Workaround

- Avoid patterns that start with `-`, `--`, or other CLI-like flag sequences.
- Use `files_write_lines` instead, which does not pass content through CLI argument parsing.
- If the pattern must contain `-`, add preceding content so the `-` is not at the start of a token boundary (this may not always be possible).

## Impact

Both issues share the same underlying problem: the `pattern` parameter is a free-form content string that is being processed by validation/parsing layers that assume it is structured data (a file path or CLI arguments). This makes `files_replace` unreliable for patterns containing common source code constructs:

- **`///` doc comments** — ubiquitous in Rust, Java, and other languages.
- **`//` line comments** — ubiquitous in C, C++, Java, JavaScript, Rust, etc.
- **`- ` bullet list items** — common in Markdown, YAML, and other text formats.
- **`--` double hyphens** — common in prose, SQL comments, CLI help text.

When the agent encounters one of these cases, it must switch to `files_write_lines` or `files_write_file`, adding overhead and reducing the precision of edits.
