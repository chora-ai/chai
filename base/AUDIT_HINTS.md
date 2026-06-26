## Audit: Diagnostic Hints Convention Review

### Implementation Status

All fixes implemented and verified across three sessions. Session 1 (6 fixes) and session 2 (3 fixes + 2 doc updates) were verified in session 3. Session 2's blank-line fix was found ineffective due to command substitution stripping trailing newlines. Session 3 implemented the `printf '%s\n'` fix across all 28 affected hint script pass-through calls, plus documentation updates. All session 2 and session 3 fixes verified with rebuilt binary in session 4.

---

### What Was Implemented

**Session 1 — Six original fixes** (tested and verified):

1. **`verify_original` hints: removed `  ` indentation** — Four hint strings in `crates/cli/src/file.rs` changed from `\n  hint:` to `\nhint:` so they start with `hint:` at column 0, matching the ADR convention.

2. **`notes-daily` append-created hint: reworded** — `hint-daily-overwrite.sh` changed from "no daily note found for this date — use notes_daily_write to create one" to "no daily note existed for this date — appended content to a new file; use notes_daily_write to set the full content instead" to acknowledge the append already succeeded.

3. **Truncation notices: reframed from imperative to optional** — All 8 `truncationHint` values across 4 `tools.json` files (files, files-read, git, git-read, notes, notes-read) changed from "Use X to read the remaining lines" to "{omitted} more lines available. To continue reading, use X; omit end_line to read the rest." Git log notices changed from "Use skip to paginate (e.g. skip: 10...)" to "To paginate, use skip with the same count".

4. **Git hint scripts: `echo` → `printf '%s'` for pass-through** — All 11 hint scripts across git, git-read, and git-remote skills changed from `echo "$output"` to `printf '%s' "$output"` for output pass-through (pipe usages like `echo "$output" | grep -q` left unchanged).

5. **ADR: added truncation notice section** — New "Truncation Notices" section in `base/adr/DIAGNOSTIC_HINTS.md` documenting `truncationHint` template variables (`{kept}`, `{total}`, `{omitted}`, `{next_start}`), the philosophy that truncation is a preview not a mandate, and the convention that notices must frame continuation as optional.

6. **skills-design SKILL.md: added truncationHint subsection** — New "Custom Truncation Notices" subsection under "Unbounded Output Protection" documenting the `truncationHint` field, template variables, and the optional-framing convention.

**Session 2 — Three follow-up fixes** (require rebuilt binary for binary-level changes):

7. **`verify_original` hints: blank line between multiple hints** — The `anyhow::bail!` format string in `crates/cli/src/file.rs` changed from `\n{}{}` to `\n{}\n{}` so that when both `line_hint` and `byte_hint` are present, they are separated by a blank line instead of appearing as a dense block.

8. **`hint-reset.sh`: unconditional blank line before hint** — Removed the conditional `if [ -n "$output" ]; then echo ""; fi` guard so the blank line always appears before the hint, matching the convention used by all other hint scripts. Previously, when `git reset` produced no output, the hint appeared with no visual separation.

9. **`hint-skill-md-checks.sh`: blank line between multiple hints** — Added `echo ""` between the two `echo "hint: ..."` lines so that when both the missing-frontmatter and variant-naming hints fire, they are separated by a blank line.

10. **ADR: codified blank-line-before-every-hint convention** — Added "Blank line before every hint" subsection to the Hint Format Convention section in `base/adr/DIAGNOSTIC_HINTS.md`, with a table showing the pattern for each hint source (postProcess scripts, `println!`, `anyhow::bail!`) and a rule that multiple hints must not appear as a dense block.

11. **skills-design SKILL.md: added blank-line convention** — Added a new bullet to the Diagnostic Hints Over Directives section: "Hints must be **preceded by a blank line** — each hint is separated from the preceding content (or from another hint) by a blank line."

**Session 3 — Command substitution trailing-newline fix** (verified in session 4):

12. **`printf '%s'` → `printf '%s\n'` in all 28 hint script pass-through calls** — Shell command substitution `output=$(cat)` strips trailing newlines. When the script then uses `printf '%s' "$output"` to emit the content, the trailing newline is gone, so the subsequent `echo ""` only replaces the stripped newline (acting as a line terminator, not a separator). Changed all `printf '%s' "$var"` pass-through calls to `printf '%s\n' "$var"` so the trailing newline is restored and `echo ""` produces the intended blank line. Affected scripts across 10 skills: git (7), git-read (2), git-remote (2), files (3), files-read (2), notes (3), notes-daily (2), notes-read (2), skills (1 + 4 no-hint branches), skills-read (1 + 1 no-hint branch). Pipe usages (e.g., `printf '%s' "$input" | grep -q`) left unchanged.

13. **ADR: updated blank-line table with `printf '%s\n'` pattern** — The "Blank line before every hint" subsection's table now shows `printf '%s\n' "$var"` (not just `echo ""`) for `postProcess` scripts, with an explanation of why: command substitution strips trailing newlines, and without `\n` the `echo ""` only replaces the stripped newline instead of producing a visible blank line.

14. **ADR: updated `CHAI_EXIT_CODE` canonical example** — The example code block changed from `printf '%s' "$input"` to `printf '%s\n' "$input"` in both the hint and no-hint branches.

15. **skills-design SKILL.md: updated blank-line bullet with `printf '%s\n'`** — The bullet now reads: "use `printf '%s\n' "$var"` (not `printf '%s'`) to restore the trailing newline that command substitution strips, then `echo ""` before each `echo "hint: …"`."

---

### Manual Testing Instructions

After rebuilding the binary, perform the following tests to verify the session 2 and session 3 fixes. (Session 1 fixes were already verified in the previous session.)

#### Test 7: `verify_original` Hints Have Blank Line Between Them

This test verifies that when `files_write_lines` rejects an `original_content` mismatch and emits two hints, they are separated by a blank line.

1. Create a test file with known content:
   - `files_write` with `path: './test-verify-hint.txt'` and content: `line1\nline2\nline3`

2. Trigger a `verify_original` mismatch that produces two hints (both `line_hint` and `byte_hint`):
   - `files_write_lines` with `path: './test-verify-hint.txt'`, `start_line: 1`, `original_content: 'wrong'`, `content: 'replaced'`

3. **Expected**: The error message should have a blank line between the two hints:
   ```
   original_content mismatch at lines 1-1:
     expected:
       wrong
     actual:
       line1

   hint: first difference at line 1 of the content — expected: "wrong", actual: "line1"

   hint: same length (5 bytes) but differ at byte offset 0; expected byte 0x77, actual byte 0x6c
   ```
   Key assertion: there is a blank line between `hint: first difference...` and `hint: same length...`. They should NOT appear as adjacent lines.

4. Clean up: `files_delete` with `path: './test-verify-hint.txt'`

#### Test 8: `hint-reset.sh` Has Blank Line Before Hint Even With Empty Output

This test verifies that `git_reset` always has a blank line before the hint, even when the reset command produces no output.

1. Reset to the current HEAD (which produces no output):
   - `git_reset` with `repo: './chai'` (no ref parameter needed, or use `HEAD`)

2. **Expected**: The hint should be preceded by a blank line:
   ```
   hint: reset to HEAD — use git_status to inspect the current state, or git_commit to re-commit staged changes
   ```
   If there is no output from git itself, the hint still has a blank line before it (the blank line appears at the start of the output). There should be no case where the hint appears on the very first line with no visual separation.

3. Also test with a reset that produces output (e.g., `HEAD~1`):
   - `git_reset` with `ref: 'HEAD~1'` and `repo: './chai'`

4. **Expected**: The output should have a blank line between the git output and the hint, consistent with all other hint scripts.

5. Clean up: Reset back to the original ref with `git_reset` with `ref: 'fix/diagnostic-hints'` and `repo: './chai'`.

#### Test 9: `hint-skill-md-checks.sh` Has Blank Line Between Multiple Hints

This test is difficult to trigger directly (it requires writing a SKILL.md that both lacks required frontmatter and has a variant-style name). Instead, verify the source code directly:

1. Read `skills/scripts/hint-skill-md-checks.sh`:
   - `files_read` with `path: './chai/crates/lib/bundled/skills/skills/scripts/hint-skill-md-checks.sh'`

2. **Expected**: In the block that prints both hints, there should be an `echo ""` between them:
   ```sh
   if [ -n "$missing" ]; then
       echo "hint: SKILL.md written — missing recommended frontmatter: $missing"
       if [ -n "$variant_hint" ]; then
           echo ""
       fi
   fi
   if [ -n "$variant_hint" ]; then
       echo "hint: $variant_hint"
   fi
   ```

#### Test 10: ADR Documents Blank-Line Convention

1. Read `base/adr/DIAGNOSTIC_HINTS.md`:
   - `files_read` with `path: './chai/base/adr/DIAGNOSTIC_HINTS.md'`

2. **Expected**: In the "### Hint Format Convention" section, after the paragraph about `postProcess` scripts, there should be a **"Blank line before every hint"** subsection containing:
   - A rule that each hint must be preceded by a blank line
   - A table showing the pattern for each source (postProcess scripts, `println!`, `anyhow::bail!`)
   - A statement that multiple hints must not appear as a dense block

#### Test 11: Skills-Design SKILL.md Documents Blank-Line Convention

1. Read the skills-design SKILL.md:
   - `files_read` with `path: './chai/crates/lib/bundled/skills/skills-design/SKILL.md'`

2. **Expected**: In the "Diagnostic Hints Over Directives" section, there should be a bullet:
   - "Hints must be **preceded by a blank line** — each hint is separated from the preceding content (or from another hint) by a blank line. In `postProcess` scripts, use `echo ""` before each `echo "hint: …"`. When multiple hints fire, each gets its own preceding blank line."

#### Test 12: `printf '%s\n'` in Hint Scripts Produces Blank Line Before Hint

This test verifies that the `printf '%s\n'` fix actually produces a visible blank line between the tool output and the hint. The `git_reset` tool is the easiest to test.

1. Reset to the current HEAD (produces git output listing unstaged changes):
   - `git_reset` with `repo: './chai'`

2. **Expected**: There must be a blank line between the last line of git output (e.g., `M	crates/lib/...`) and the `hint:` line. The hint must NOT appear on the very next line after the git output.

3. Also test with a reset that produces no git output (just the hint):
   - First, stage all changes: `git_add` with `paths: '.'` and `repo: './chai'`
   - Then: `git_reset` with `repo: './chai'`

4. **Expected**: Even when git produces no output (or minimal output), there should be a blank line before the hint.

5. Clean up: `git_reset` with `ref: 'fix/diagnostic-hints'` and `repo: './chai'`

#### Test 13: ADR Blank-Line Table Documents `printf '%s\n'` Pattern

1. Read `base/adr/DIAGNOSTIC_HINTS.md`:
   - `files_read` with `path: './chai/base/adr/DIAGNOSTIC_HINTS.md'`

2. **Expected**: In the "Blank line before every hint" subsection, the `postProcess` scripts row of the table must say `printf '%s\n' "$var"` (not just `echo ""` then `echo "hint: …"`). There should also be a paragraph explaining that `output=$(cat)` strips trailing newlines and why `printf '%s\n'` is needed.

#### Test 14: ADR Canonical Example Uses `printf '%s\n'`

1. In the same ADR file, find the `CHAI_EXIT_CODE` section's code example.

2. **Expected**: Both `printf` calls in the example must use `printf '%s\n' "$input"` (not `printf '%s' "$input"`).

#### Test 15: Skills-Design SKILL.md Blank-Line Bullet Documents `printf '%s\n'`

1. Read the skills-design SKILL.md:
   - `files_read` with `path: './chai/crates/lib/bundled/skills/skills-design/SKILL.md'`

2. **Expected**: The blank-line bullet in "Diagnostic Hints Over Directives" → "Hint Design Rules" must mention `printf '%s\n' "$var"` and explain that `printf '%s'` is wrong because command substitution strips trailing newlines.

---

### Verification Checklist

**Session 1 (already verified):**

- [x] `verify_original` error messages have `hint:` at column 0 (no leading `  ` indentation)
- [x] `notes_daily_append` on a non-existent date produces a hint that acknowledges the file was created (not "found" implying failure)
- [x] Truncation notices for `files_read`, `git_diff`, `git_show`, `notes_read` use "{omitted} more lines available. To continue reading, use X; omit end_line to read the rest."
- [x] Truncation notices for `git_log` use "To paginate, use skip with the same count; use oneline: true for compact output."
- [x] No truncation notice says "Use X to read the remaining lines" (imperative phrasing)
- [x] Git hint scripts produce correct output on both valid and invalid repo paths (smoke test)
- [x] `printf '%s'` in git hint scripts preserves backslash sequences literally (no echo interpretation)
- [x] ADR `DIAGNOSTIC_HINTS.md` has a "Truncation Notices" section with template variables, philosophy, and convention
- [x] Skills-design `SKILL.md` has a "Custom Truncation Notices" subsection under "Unbounded Output Protection"

**Session 2 (verified in session 3, blank-line fix verified in session 4):**

- [x] `verify_original` error messages have a blank line between multiple hints (not a dense block)
- [x] `hint-reset.sh` always has a blank line before the hint, even when output is empty — **Fixed in session 3**: `printf '%s\n' "$output"` after `output=$(cat)` restores the trailing newline, so `echo ""` now produces a visible blank line.
- [x] `hint-skill-md-checks.sh` has a blank line between the two conditional hints
- [x] ADR `DIAGNOSTIC_HINTS.md` has a "Blank line before every hint" subsection in the Hint Format Convention section
- [x] Skills-design `SKILL.md` has a "preceded by a blank line" bullet in the Diagnostic Hints Over Directives section

**Session 3 — Command substitution trailing-newline fix** (verified in session 4):

- [x] All 28 `printf '%s' "$var"` pass-through calls in hint scripts changed to `printf '%s\n' "$var"` to restore the trailing newline stripped by command substitution (`var=$(cat)`)
- [x] ADR `DIAGNOSTIC_HINTS.md` blank-line table updated: `postProcess` pattern now documents `printf '%s\n' "$var"` (not just `echo ""` then `echo "hint: …"`)
- [x] ADR `CHAI_EXIT_CODE` canonical example updated: `printf '%s'` → `printf '%s\n'` in both branches
- [x] Skills-design SKILL.md blank-line bullet updated: documents `printf '%s\n' "$var"` requirement

---

### Original Audit Findings

<details>
<summary>Expand to see original findings (preserved for reference)</summary>

### 1. Convention Violations

#### 1a. Binary-level `verify_original` hints use indented `  hint:` instead of `hint:` on own line

**Location**: `chai/crates/cli/src/file.rs`, lines 1497–1530

Four hints in the `verify_original` error path use `  hint:` (two spaces of indentation before `hint:`):

```rust
"\n  hint: first difference at line {} of the content — expected: {:?}, actual: {:?}",
"\n  hint: content lines match up to line {} but lengths differ — expected {} lines, actual {} lines",
"\n  hint: same length ({} bytes) but differ at byte offset {}; expected byte 0x{:02x}, actual byte 0x{:02x}",
"\n  hint: different lengths - expected {} bytes, actual {} bytes",
```

The ADR states: *"A hint that does not start with `hint:` at the beginning of its line (e.g., embedded inline within another line) cannot be detected and will be lost to truncation."* The skills-design SKILL.md states: *"Hints must start with `hint:` on their own line."*

**Mitigating factor**: These hints are embedded in an `anyhow::bail!` error message, which exits with a non-zero code. The `files_write_lines` / `notes_write_lines` tools don't declare `successExitCodes` for this exit code, so the error propagates directly without going through `truncate_output`. The hints would always be shown in full. **However**, this is still a convention violation — the ADR says "All diagnostic hints — whether emitted by `postProcess` scripts or by the binary — must follow the same format." And if a future change admits this exit code into `successExitCodes`, these hints would silently be lost to truncation.

**Status**: ✅ Fixed — removed `  ` prefix.

---

### 2. Inaccurate/Misleading Hints

#### 2a. `notes-daily/hint-daily-overwrite.sh`: Misleading hint when append creates a new file

**Location**: `chai/crates/lib/bundled/skills/notes-daily/scripts/hint-daily-overwrite.sh`

When `notes_daily_append` creates a new file (because no daily note existed for that date), the output contains `(created new file)` and the script appends:

```
hint: no daily note found for this date — use notes_daily_write to create one
```

This is misleading — the append *already created the note* with the appended content. The hint implies the operation failed to create the note and the agent should try a different tool, but the operation succeeded.

**Status**: ✅ Fixed — reworded to acknowledge creation succeeded.

---

### 3. Truncation Notice Philosophy

The user's core point: *the purpose of truncation is to provide a preview, not to make the agent follow up with another tool call to read the full content.*

#### 3a. Custom truncationHint templates use imperative phrasing

"Use X to read the remaining lines" is imperative — it implies the agent should follow up. This defeats the purpose of truncation.

**Status**: ✅ Fixed — reframed as informational/optional with "omit end_line to read the rest" tip.

#### 3b. Generic truncation notice is fine ✓

---

### 4. Consistency Issues

#### 4a. Git hint scripts use `echo "$output"` instead of `printf '%s' "$output"` for pass-through

All other skill scripts use `printf '%s'` for pass-through. POSIX `echo` may interpret escape sequences.

**Status**: ✅ Fixed — all 11 git/git-read/git-remote hint scripts now use `printf '%s' "$output"`.

---

### 5. Documentation Gaps

#### 5a. ADR doesn't document truncationHint conventions

**Status**: ✅ Fixed — added "Truncation Notices" section to ADR.

#### 5b. skills-design SKILL.md doesn't mention `truncationHint`

**Status**: ✅ Fixed — added "Custom Truncation Notices" subsection to skills-design SKILL.md.

---

### 6. Everything That Checks Out ✓

(No changes needed — these items were already correct.)

</details>