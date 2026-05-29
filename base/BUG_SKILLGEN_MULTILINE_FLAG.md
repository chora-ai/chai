# Bug: Skillgen Write Tools Fail With Multiline Content

## Status

Fix applied and verified ✅

## Summary

The skillgen write tools (`skillgen_write_skill_md`, `skillgen_write_tools_json`, `skillgen_write_script`) pass their `content` parameter as a CLI flag (`kind: "flag"`, `--content <value>`). When the content contains real newlines — which is always the case for SKILL.md and tools.json — `clap` treats the text after the first newline as unexpected positional arguments and rejects the invocation.

## Impact

- Skill generation via the skillgen skill is broken for any content that spans multiple lines (i.e. all non-trivial use).
- `skillgen_init` and `skillgen_discover`/`skillgen_read` are unaffected (no multiline content args).

## Root Cause

In `crates/lib/config/skills/skillgen/tools.json`, the `content` arg mappings use `"kind": "flag"`:

```json
{ "param": "content", "kind": "flag", "flag": "content" }
```

This produces argv: `--content <multiline-string>`. Newlines in the string value are passed as literal newlines to `clap`, which interprets them as argument boundaries. The result:

```
error: unexpected argument '---
name: escape-test
...' found
```

The `chai file write` and `chai file append` subcommands already handle this correctly — they accept content via **stdin** when `--content` is omitted, and the devtools/kb skills use `kind: "stdin"` for their `content` parameters.

## Fix Applied

### 1. `crates/cli/src/main.rs`

- Changed `SkillCmd::WriteSkillMd`, `WriteToolsJson`, and `WriteScript` variants to use `content: Option<String>` instead of `content: String`, with `#[arg(long, allow_hyphen_values = true)]`.
- Extracted a shared `read_content_from_stdin_or(content: Option<String>)` helper (also refactored `FileCmd::Write` and `FileCmd::Append` to use it, eliminating duplicated stdin-reading code).
- Updated handler bodies to call `read_content_from_stdin_or(content)?` before processing, so content is read from stdin when `--content` is omitted.
- Updated doc comments to note stdin fallback behavior.

### 2. `crates/lib/config/skills/skillgen/tools.json`

Changed all three `content` arg mappings from `"kind": "flag"` to `"kind": "stdin"`:

```json
// Before:
{ "param": "content", "kind": "flag", "flag": "content" }

// After:
{ "param": "content", "kind": "stdin" }
```

This routes the content parameter through the subprocess's stdin pipe instead of passing it as a `--content` CLI argument, bypassing the `clap` parsing issue entirely.

### Additional Refactoring

The `read_content_from_stdin_or` helper was extracted from the `FileCmd::Write` and `FileCmd::Append` handlers and is now reused by all five subcommands that accept optional `--content` with stdin fallback. This eliminates duplicated stdin-reading code and ensures consistent behavior across all content-accepting commands.

## Evidence

Tested on 2026-05-28. All three skillgen write tools fail with multiline content before the fix:

- `skillgen_write_skill_md` → `error: unexpected argument '---\nname: ...' found`
- `skillgen_write_tools_json` → `skill 'escape-test' not found` (earlier failure; same root cause)
- `skillgen_write_script` → same pattern (untested but same mechanism)

Meanwhile, `devtools_write_file` (stdin), `kb_write` (stdin), `kb_append` (stdin), and `kb_daily_write` (stdin) all handle multiline content correctly.

## Verification

Verified on 2026-05-28 after rebuild. Tested all three skillgen write tools with multiline content containing `\n` and `\t` escape sequences:

1. `skillgen_init multiline-test` — ✅ created test skill
2. `skillgen_write_skill_md` with SKILL.md content including `\n` and `\t` literals — ✅ wrote 522 bytes
3. `skillgen_write_tools_json` with JSON containing `\n` in descriptions — ✅ wrote 588 bytes, valid JSON
4. `skillgen_write_script` with script containing `\n` in comments — ✅ wrote 205 bytes
5. Read back SKILL.md and tools.json — ✅ multiline content preserved, `\n` and `\t` remain as literal two-character sequences (not decoded to real newlines/tabs)

## Same Bug in Other Skills

Audit of all bundled skills reveals the same `kind: "flag"` pattern on content-capable parameters:

### `notesmd` — `notesmd_create` content parameter (NOT FIXED)

**File**: `crates/lib/config/skills/notesmd/tools.json`

```json
{ "param": "content", "kind": "flag", "flag": "content" }
```

`notesmd_create` passes note content via `--content <value>` on the `notesmd-cli create` subcommand. This is the **exact same bug** — note content is always multiline markdown with YAML frontmatter, and will fail the same way the skillgen tools did. Fixing this requires adding stdin fallback to `notesmd-cli create` (external binary, not part of chai) and changing the arg mapping to `"kind": "stdin"`. **Not fixed** — notesmd and obsidian skills are likely to be removed from bundled skills in favor of kb.

### `git` / `git-remote` — `git_commit` message parameter (LOW RISK)

**File**: `crates/lib/config/skills/git/tools.json`, `crates/lib/config/skills/git-remote/tools.json`

```json
{ "param": "message", "kind": "flag", "flag": "m" }
```

This produces `git commit -m <message>`. Git's own CLI handles multiline `-m` values natively (not clap), so this is not broken. Multiline commit messages could be more robust with `--file -` and stdin, but not urgent.

### Other `kind: "flag"` parameters — No risk

All remaining flag parameters carry short single-line values (paths, keys, names, counts, vault names, dates) where multiline content is not expected. No action needed.

### Audited skills (no multiline flag issues)

- **kb**: `path` flags — single-line paths
- **kb-daily**: `path`, `date` flags — single-line values
- **kb-frontmatter**: `path`, `key`, `value` flags — single-line entries
- **kb-wikilink-write**: `from`, `to`, `root` flags — file paths
- **obsidian**: empty stub (no tools)
- **obsidian-daily**: empty stub (no tools)
- **skillval**: `file` flag — file path
- **devtools**: `path` flags — file paths

## Related

- `BUG_WRITE_TOOL_ESCAPES.md` — The `normalizeNewlines` double-decode bug that was previously assumed to be the cause of skillgen write failures. That bug is resolved; this is a separate issue.
