# Bug: Write Tool Escape Sequence Corruption (`normalizeNewlines` Double-Decode)

## Status

✅ Resolved and verified

## Summary

The `normalizeNewlines: true` flag on content arg mappings caused a double-decode. `serde_json` already decodes JSON escape sequences (e.g. `\n` → the literal two-char string `\n`), then `normalize_content()` performed a second decode, converting the literal `\n` to a real newline. This made it impossible to write source code containing `\n` or `\t` string literals. Even `\\n` was incorrectly handled, producing a backslash followed by a literal newline rather than the two-character sequence `\n`. For `skillgen_write_tools_json`, the corruption produced **invalid JSON** with control characters.

## Root Cause

`"normalizeNewlines": true` on content arg mappings in `tools.json` caused a double-decode. The `serde_json` parser already decodes JSON escape sequences (e.g. `\n` → the two-char string `\n`). Then `normalize_content()` performed a second decode, converting the literal `\n` to a real newline. This made it impossible to write source code containing `\n` or `\t` string literals.

## Fix

Removed `"normalizeNewlines": true` from all 10 tool arg mappings across 6 skills. The `normalize_newlines` field was removed from the `ArgMapping` struct entirely. The flag is unnecessary — `serde_json` already handles JSON → Rust string decoding correctly. All LLM providers (Ollama, OpenAI-compat, NIM) deliver tool call arguments as already-decoded `serde_json::Value`.

### Files Changed

| File | Change |
|------|--------|
| `crates/lib/config/skills/devtools/tools.json` | Removed `normalizeNewlines` from `content` arg (fixed in prior session) |
| `crates/lib/config/skills/skillgen/tools.json` | Removed `normalizeNewlines` from 3 `content` args |
| `crates/lib/config/skills/kb/tools.json` | Removed `normalizeNewlines` from 2 `content` args |
| `crates/lib/config/skills/kb-daily/tools.json` | Removed `normalizeNewlines` from 2 `content` args |
| `crates/lib/config/skills/notesmd/tools.json` | Removed `normalizeNewlines` from 1 `content` arg |
| `crates/lib/config/skills/notesmd-daily/tools.json` | Removed `normalizeNewlines` from 1 `content` arg |
| `crates/lib/config/skills/skillgen/SKILL.md` | Replaced "always use normalizeNewlines" directive with "never use" directive; added deprecated schema reference section |
| `crates/lib/src/skills/descriptor.rs` | Removed `normalize_newlines` field from `ArgMapping` struct |
| `crates/lib/src/tools/generic.rs` | Updated module doc, deprecated `normalize_content` function doc, deprecated `transform_param_value` path doc |
| `chai/.agents/spec/TOOLS_SCHEMA.md` | Marked `normalizeNewlines` as deprecated in args table; removed from Conversion note |
| `chai/.agents/epic/BUNDLED_SKILLS.md` | Updated devtools, kb, kb-daily references; added fix to requirements checklist; added to Resolved questions |

## Verification (2026-05-28)

Re-verified all four original test scenarios after the fix was applied to both config files and source code:

1. **`devtools_write_file` ✅** — Wrote content with `"hello\n"`, `"\t"`, `"C:\\Users\\test\\file.txt"`. Read-back confirmed all escape sequences preserved byte-for-byte.

2. **`kb_write` ✅** — Wrote content with `\n`/`\t`/`\\`. Read-back confirmed all escape sequences preserved correctly.

3. **`kb_append` ✅** — Appended content with escape sequences. Read-back confirmed preservation.

4. **`kb_daily_write` ✅** — Wrote daily note with escape sequences. Read-back confirmed preservation.

5. **`skillgen_write_skill_md` / `skillgen_write_tools_json` ⚠️** — These tools fail with multiline content, but the failure is caused by a **separate bug** (multiline `--content` flag parsing, not the `normalizeNewlines` double-decode). See `BUG_SKILLCEN_MULTILINE_FLAG.md`.

**Conclusion:** The `normalizeNewlines` double-decode bug is fully resolved. All tools that use `kind: "stdin"` for content (devtools, kb, kb-daily) work correctly. The skillgen tools have a separate argparse issue.

## Original Workaround (No Longer Needed)

For newline character, replace `"\n"` with `&(10 as char).to_string()` or `msg.push(10 as char)`.
For tab character, replace `"\t"` with `&(9 as char).to_string()`.
For doc comments, rephrase to avoid mentioning `\n` or `\t` by name.
