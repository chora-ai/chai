# FEAT: Add `files_replace` Tool to the `files` Skill

## Motivation

The `files` skill can find matching lines with `files_search_content` and replace line ranges with `files_write_lines`, but it has no tool that combines the two: find every line matching a pattern and apply a replacement. This gap forces agents into a read-verify-write cycle for each individual match — a cycle that consumed 17 tool-call iterations when the same mechanical edit needed to be applied to 17 struct initializations in a single file (see now-closed `BUG_TEST_STRUCT_FRICTION.md`).

A sed-style find-and-replace tool collapses these N edits into a single call, reducing iteration cost, error surface, and tool-loop budget consumption.

## Proposed Tool

### `files_replace`

Find lines matching a pattern in a file and replace each match with a replacement string. Returns a diff of all changes made.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | File path relative to the sandbox root (use `./` prefix) |
| `pattern` | string | yes | Search pattern (extended regex supported, same as `files_search_content`) |
| `replacement` | string | yes | Replacement string. Supports `$1`–`$9` for capture group references. Use `$$` for a literal `$`. |
| `line_numbers` | boolean | no | Show line numbers in the returned diff (default: true) |

**Behavior:**

1. Read the file at `path`.
2. Apply the regex `pattern` to each line. For each match, replace the matched portion with `replacement`, substituting capture groups.
3. If no lines matched, return a message indicating zero replacements (not an error).
4. Write the modified content back to the file.
5. Return a unified diff showing every changed line with surrounding context, so the agent can verify the result.

**Example usage:**

```
files_replace(
  path: "chai/crates/lib/src/tools/generic/mod.rs",
  pattern: r"deny_always_resolve: None,\n",
  replacement: "",
)
```

This single call removes every `deny_always_resolve: None,` line in the file — the edit that previously required 17 separate `files_write_lines` invocations.

### Capture Group Support

Capture groups let the agent restructure matches rather than just delete or replace literally:

```
files_replace(
  path: "config.toml",
  pattern: r"version = \"(\d+)\.(\d+)\.(\d+)\"",
  replacement: r"version = \"$1.$2.4\"",
)
```

This bumps the patch version across all matching lines in one call.

### Implementation: `chai file replace` Subcommand

Following the existing pattern where `files_write_lines` is backed by `chai file patch` and `files_write_file` is backed by `chai file write`, the new tool should be backed by a `chai file replace` subcommand.

**`chai file replace` CLI:**

```
chai file replace --path <path> --pattern <pattern> --replacement <replacement> [--no-line-numbers]
```

- Reads the file, applies regex substitution line-by-line, writes the result back.
- Exits 0 with the diff on stdout. Exits 1 if the file doesn't exist or the pattern is invalid.
- If zero lines matched, exits 0 with a "0 replacements" message (not an error).

**`tools.json` execution entry:**

```json
{
  "tool": "files_replace",
  "binary": "chai",
  "subcommand": "file replace",
  "args": [
    { "param": "path", "kind": "flag", "flag": "path", "writePath": true },
    { "param": "pattern", "kind": "flag", "flag": "pattern" },
    { "param": "replacement", "kind": "flag", "flag": "replacement" },
    {
      "param": "line_numbers",
      "kind": "flagifboolean",
      "flagIfTrue": "--line-numbers",
      "absentDefault": true
    }
  ]
}
```

**`tools.json` tool definition:**

```json
{
  "name": "files_replace",
  "description": "Replace all occurrences of a pattern in a file. Supports capture groups ($1-$9) in the replacement string. Returns a diff of changes made. Use this instead of multiple files_write_lines calls when the same edit applies to many locations in one file.",
  "parameters": {
    "type": "object",
    "required": ["path", "pattern", "replacement"],
    "properties": {
      "path": {
        "type": "string",
        "description": "File path relative to the sandbox root (use ./ prefix)"
      },
      "pattern": {
        "type": "string",
        "description": "Search pattern (extended regex supported, same as files_search_content)"
      },
      "replacement": {
        "type": "string",
        "description": "Replacement string. Supports $1-$9 for capture group references. Use $$ for a literal $."
      },
      "line_numbers": {
        "type": "boolean",
        "description": "Show line numbers in the returned diff (default: true)"
      }
    }
  }
}
```

### SKILL.md Directives

Add the following directive to the `files` SKILL.md:

```
- use `files_replace` for bulk find-and-replace across a file; use `files_write_lines` for targeted edits where surrounding context must be verified before replacement
```

This distinguishes the two tools: `files_replace` for volume, `files_write_lines` for precision.

### Allowlist Entry

Add `replace` to the `chai` allowlist entry:

```json
"chai": ["file write", "file delete", "file patch", "file read-lines", "file replace"]
```

## Scope

- **In scope**: The `files_replace` tool, its `chai file replace` backing command, the `tools.json` and `SKILL.md` updates, and the allowlist entry.
- **Out of scope**: Multi-file replacement (the tool operates on one file per call, consistent with existing tools). Recursive directory replacement (use `files_search_content` to find files, then `files_replace` on each).

## Design Decisions

### Line-by-line, not whole-file regex

The replacement is applied line-by-line rather than against the full file content. This is deliberate:

- **Predictability**: Multi-line regex replacements are harder to reason about and easier to get wrong (greedy matches spanning too far, etc.).
- **Consistency with `files_search_content`**: That tool returns matching lines; this tool replaces matching lines. The mental model is the same.
- **Sufficient for the motivating use case**: The 17-edit scenario was 17 separate lines, each needing the same field removed. Line-by-line covers this and the vast majority of bulk edits.

If multi-line replacement is needed in the future, a `multiline` flag can be added without breaking the line-by-line default.

### No dry-run mode

The diff in the return value serves as the verification step. If the agent needs to preview changes without writing, it can use `files_search_content` with the same pattern first. Adding a dry-run flag would complicate the interface for marginal benefit.

### Write sandbox integration

The `path` parameter uses `writePath: true`, so the existing write sandbox enforcement applies automatically. The tool cannot write to paths outside the configured sandbox.

## Impact

- **Iteration cost**: N identical edits collapse from N tool calls to 1.
- **Error surface**: No content-mismatch rejections for bulk edits (the pattern either matches or it doesn't — no line-number drift or trailing-whitespace issues).
- **Tool-loop budget**: Significant savings in sessions with tight limits.
