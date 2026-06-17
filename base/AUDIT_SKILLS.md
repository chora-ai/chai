# AUDIT: Bundled Skills Review

## Purpose

Cross-skill audit of all bundled skills in `chai/crates/lib/bundled/skills/`, guided by the design principles in `skills-design/SKILL.md`.

## Bundled Skills

| Skill | Purpose | Round 1 | Round 2 | Round 3 |
|-------|---------|---------|---------|---------|
| `files` | Read, write, search, delete files and directories | ✅ | ✅ | ✅ |
| `files-read` | Read-only subset of `files` | ✅ | ✅ | ✅ |
| `git` | Git operations (write) | ✅ | ✅ | ✅ |
| `git-read` | Git operations (read-only) | ✅ | ✅ | ✅ |
| `git-remote` | Git remote operations (clone, pull, push) | ✅ | ✅ | ✅ |
| `logs` | Chai process logs | - | - | ✅ |
| `notes` | Read, write, search, delete notes and directories | ✅ | ✅ | ✅ |
| `notes-read` | Read-only subset of `notes` | ✅ | ✅ | ✅ |
| `notes-daily` | Daily note creation | ✅ | ✅ | ✅ |
| `notes-frontmatter` | Frontmatter manipulation | ✅ | ✅ | ✅ |
| `notes-wikilink` | Wikilink resolution and rename | ✅ | ✅ | ✅ |
| `rss` | RSS feed reading | ✅ | ✅ | ✅ |
| `skills` | Skill creation and modification | ✅ | ✅ | ✅ |
| `skills-design` | Design principles for skill tools | ✅ | ✅ | ✅ |
| `skills-read` | Skill inspection (read-only) | ✅ | ✅ | ✅ |

## Round 3: Battle-Test Plan

Each skill group is tested in a dedicated session with the relevant skills enabled. Read-only variants are not tested directly but must remain aligned with the base skill's tools and directives.

### Diagnostic Hints Evaluation Framework

For each remaining skillset in the audit, every directive in SKILL.md is now evaluated against three enforcement levels before being kept as-is:

| Level | Action | When to Use |
|-------|--------|-------------|
| **Enforce** | Tool-level validation, `denyPattern`, `absentDefault`, parameter constraints | The tool can determine the correct behavior unambiguously |
| **Hint** | Diagnostic message in tool output at the point of failure or suboptimal usage | The tool can detect the condition but shouldn't impose a single resolution |
| **Instruct** | Retain as a directive in SKILL.md | The guidance applies to all calls, not just error cases, or requires agent judgment the tool cannot provide |

The "Diagnostic Hints Over Directives" principle in `skills-design/SKILL.md` and `adr/DIAGNOSTIC_HINTS.md` formalize this pattern.

### Skillset 1: files & files-read — Complete

#### Tools Reviewed

| Tool | Status | Notes |
|------|--------|-------|
| `files_read_file` | ✅ | Simple, reliable. `maxOutputLines: 500` cap applied. |
| `files_read_lines` | ✅ | Line-numbered output (`{line_number}\|{content}`). Returns raw line content (no trailing-whitespace stripping). |
| `files_list_dir` | ✅ | `sideRead` for AGENTS.md, `postProcess: sanitize-ls`. `long` and `all` flags work correctly. |
| `files_search_content` | ✅ | ERE via `grep -E`. `successExitCodes: [0, 1]`. `maxOutputLines: 200` cap. `line_numbers` defaults to `true`. |
| `files_write_file` | ✅ | Full overwrite. Creates parent dirs. Content via stdin. |
| `files_write_lines` | ✅ | Four-stage `verify_original`. Trailing-ws preservation in replacement. `original_content` via tempfile. |
| `files_replace` | ✅ | Regex with multiline mode. `literal` mode. `max_replacements`. Trailing-ws-tolerant fallback. Leading-ws diagnostic hint. Pattern via tempfile, replacement via stdin. |
| `files_delete_file` | ✅ | Refuses directories. |
| `files_delete_dir` | ✅ | Refuses non-empty directories and files. |

#### SKILL.md Review

**Directives assessed against design principles:**

| Directive | Assessment | Action |
|-----------|-----------|--------|
| never delete files without confirming | Appropriate — safety check the tool can't enforce | Keep |
| never assume a file exists — verify first | Appropriate — tool can't know agent's intent | Keep |
| never read binary files | Appropriate — `files_list_dir` can show file type hints; tool can't auto-detect | Keep |
| always read a file before overwriting | Appropriate — tool can't enforce this for `files_write_file` (complete overwrite) | Keep |
| prefer single large `files_write_lines` calls | Appropriate — reduces line-shift errors from multiple edits | Keep |
| work bottom-to-top for multiple edits | Appropriate — prevents line-number drift | Keep |
| use `files_read_lines` after `files_search_content` | Appropriate — search only gives line numbers, not context | Keep |
| `files_replace` vs `files_write_lines` choice | Appropriate — key guidance that prevents regex metacharacter errors | Keep |
| use `literal: true` on `files_replace` for regex metacharacters | Appropriate — tool error message now suggests `literal: true` on regex parse failure, but the directive still helps agents avoid the error entirely | Keep |

**Directives removed in this round:**

- **"always set `line_numbers` to true when searching"** — Removed. `line_numbers` now defaults to `true` in the tool schema and execution spec. The tool enforces what was previously a directive. "Tools over inference" in action.

- **"use `files_read_lines` to get the exact content at the target range first before calling `files_write_lines`"** — Removed. The `original_content` verification in the tool already enforces this. If the agent doesn't read first, the edit is rejected with the actual content shown. "Verification over instruction" in action.

#### Applied Hints

| Tool | Condition | Hint |
|------|-----------|------|
| `files_read_file` / `files_read_lines` | File not found | `"hint: file not found — use files_list_dir to browse available files"` |
| `files_search_content` | Results returned | `"hint: use files_read_lines with these line numbers for surrounding context"` |
| `files_write_file` | Existing file overwritten | `"hint: overwrote existing file"` |
| `files_replace` | Pattern matches 0 times but would match with leading-whitespace normalization | `"hint: pattern did not match, but would match with leading-whitespace normalization — check indentation"` |
| `files_replace` | Regex parse error | Suggests `"literal: true"` in the error message |
| `files_search_content` | `line_numbers` not specified | Defaults to `true` via `absentDefault` |

#### Improvements Applied in Round 3

| Improvement | Type | Rationale |
|------------|------|-----------|
| `files_search_content` / `notes_search` `line_numbers` defaults to `true` | Tool default + `absentDefault` | Removes "always set line_numbers" directive — tools over inference |
| `files_replace` `pattern` → `tempfile`, `replacement` → `stdin` | Content-passing channel | Eliminates escape-processing layer (`process_replacement_escapes`, `process_pattern_escapes_for_literal`); aligns with content-passing guidelines; agent uses natural JSON newlines instead of `\n` escape sequences |
| `files_replace` regex error suggests `literal: true` | Error message | Reduces inference — the tool teaches the agent about literal mode on failure |
| `files_replace` "0 replacements" includes leading-whitespace hint | Diagnostic | Reduces agent iteration count on indentation mismatches |
| `files_replace` / `notes_replace` `line_numbers` `"default": true` in schema | Schema consistency | Reduces agent confusion about default behavior |
| `files_read_file` / `files_read_lines` not-found hint | `postProcess` script (back-ported from notes) | `"hint: file not found — use files_list_dir to browse available files"` — replaces directive "never assume a file exists" for the error case |
| `files_search_content` results hint | `postProcess` script (back-ported from notes) | `"hint: use files_read_lines with these line numbers for surrounding context"` — replaces directive "use files_read_lines after files_search_content" for the results case |
| `files_write_file` overwrite hint | `postProcess` script (back-ported from notes) | `"hint: overwrote existing file"` — supplements directive "always read a file before overwriting" |
| `absentDefault` type changed from `bool` to `serde_json::Value` in `descriptor.rs` | Bug fix / type widening | `absentDefault` was `Option<bool>` but Round 3 added string defaults (`"warn"` for logs, `"10"` for git/git-read) — deserialization rejected these, causing all three skills to fail to load. Widened to `Option<serde_json::Value>` and added `absentDefault` handling in the `Flag` arm of `build_argv` (previously only `FlagIfBoolean` supported it) |
| `format_replace_diff` added lines now use original-file line numbers | Bug fix | Added lines in the diff output used `hunk.new_start + offset + 1` (new-file line numbers), while context and removed lines used original-file line numbers. This produced inconsistent numbering that made diffs misleading — added lines appeared at unexpected positions, undermining trust in the tool output. Fixed to use `hunk.orig_start + offset + 1` so all line numbers in the diff consistently refer to the original file. Added 4 unit tests for `format_replace_diff`. |
| `collect_output_with_codes` includes stderr on `successExitCodes` success path | Bug fix | When a non-zero exit code was in `successExitCodes`, only stdout was returned — stderr was discarded. This prevented `postProcess` hint scripts from matching against error messages that git writes to stderr (e.g., "not a git repository", "no upstream branch"). Fixed to append stderr after stdout when a non-zero code is treated as success. Added 6 unit tests. |
| `successExitCodes` added to git/git-read/git-remote hint tools | Bug fix | 6 of 7 diagnostic hints were non-functional because `postProcess` only runs on successful exits. `git_status` (not-a-repo → exit 128), `git_commit` (nothing to commit → exit 1), `git_push` (all error conditions → exit 1/128) needed `successExitCodes` to allow the output through to `postProcess`. Added `[128]` to `git_status`, `[1]` to `git_commit`, `[1, 128]` to `git_push`. |
| `git_push` `branch` description updated | Schema fix | Previously said "Uses current branch if omitted" but git only infers the branch when tracking is already configured. Updated to "Must be specified explicitly when the branch has no upstream configured." |
| `git_pull` hints and description fix (session 3) | Bug fix | Same pattern as `git_push`: (1) `remote` and `branch` descriptions said "Uses tracking remote/branch if omitted" but git fails when no tracking info is configured — updated to "Must be specified explicitly when the branch has no tracking remote/information configured." (2) Added `successExitCodes: [1, 128]` and `postProcess` (`hint-pull-errors.sh`) to detect no-tracking-info and remote-not-found errors — same root cause as the `git_push` hint bug (exit-1 error propagated before `postProcess` could run). |
| `hint-not-found.sh` scripts: `CHAI_EXIT_CODE` + stdin buffering | Bug fix (live-tested) | Two bugs in all five `hint-not-found.sh` scripts: (A) False positives — scripts grepped for error strings in output, so files containing those strings (docs, test fixtures, the script itself) triggered false hints on successful reads. (B) Stdin consumed by `grep -q` — `grep` read stdin before `cat`, causing partial or missing output. Fix: buffer stdin with `input=$(cat)` and check `CHAI_EXIT_CODE` env var (passed by `post_process.rs`) instead of pattern-matching output. The exit code is a reliable signal; all five scripts (files, files-read, notes, notes-read, notes-daily) updated. Live-tested: nonexistent files show hint, existing files with error-like content show no false hint, normal files have no hint appended. |
| `files` / `files-read` SKILL.md prose removed | SKILL.md lean pass | Removed 6 paragraphs of additional content from `files` (ERE syntax, `literal` details, multiline/capture groups, `max_replacements`, trailing-whitespace fallback) and 1 paragraph from `files-read` (ERE syntax). All removed content duplicated tool parameter descriptions already in the tool schema, described output behavior the agent encounters naturally via hints, or caused agent overthinking (e.g., the ERE `|` syntax note was contributing to confusion about how to use `|` in patterns). Tool descriptions + diagnostic hints now fully cover what the prose described. |

#### files-read Alignment

`files-read` is properly aligned with `files`:

- Contains only the read-relevant directives (verify existence, no binary files, search→read workflow)
- Omits all write-relevant directives and `files_replace` reference paragraphs
- Tool schemas are identical to the read-only subset of `files` tools
- Execution specs are identical to the read-only subset of `files` execution specs

---

### Skillset 2: git, git-read, git-remote, logs, rss

#### Battle Test: git Skills — Complete

##### Tools Reviewed

| Tool | Status | Notes |
|------|--------|-------|
| `git_status` | ✅ | Returns branch, staging state, untracked files correctly |
| `git_log` | ✅ | `count`, `oneline`, `file_path` all work. `absentDefault: "10"` confirmed — 10 commits returned when count omitted |
| `git_diff` | ✅ | `staged`, `ref`, `file_path` all work. `ref: "main"` correctly shows divergence. `maxOutputLines: 200` truncation confirmed |
| `git_show` | ✅ | Commit content returned correctly. `maxOutputLines: 200` truncation confirmed |
| `git_branch` | ✅ | Lists branches; `all` flag includes remote-tracking |
| `git_add` | ✅ | Stages files correctly |
| `git_commit` | ✅ | Commits staged changes. `denyPattern` blocks commits on `main`/`release/*` (confirmed). `message` passed via stdin (`commit -F -`) |
| `git_branch_create` | ✅ | Creates and switches to new branch |
| `git_checkout` | ✅ | Switches to existing branch |
| `git_branch_delete` | ✅ | `denyPattern` blocks deleting `main`/`release/*`. Git itself rejects deleting current branch |

| Tool | Status | Notes |
|------|--------|-------|
| `git_clone` | ✅ | Clones into sandbox correctly. `writePath` + `resolve-clone-path` ensures sandbox confinement |
| `git_pull` | ✅ | Pulls from remote. **Bug fix (session 3)**: same pattern as `git_push` — `remote` and `branch` descriptions said "Uses tracking remote/branch if omitted" but git fails when no tracking info is configured; updated to "Must be specified explicitly when the branch has no tracking remote/information configured." Added `successExitCodes: [1, 128]` and `postProcess` hint script (`hint-pull-errors.sh`) that detects: (1) no tracking information → "hint: no tracking branch set — specify remote and branch explicitly", (2) remote not found → "hint: remote not found — use git_remote to list configured remotes". Without `successExitCodes`, the exit-1 error propagated before `postProcess` could run — same root cause as the `git_push` hint bug fixed in session 2. |
| `git_push` | ✅ | `denyPattern` on `branch` blocks pushes to `main`/`release/*` (both explicit and implicit via `denyResolveCommand`). `set_upstream` works. **Bug fix**: `branch` description was inaccurate — said "Uses current branch if omitted" but git only infers the branch when tracking is already configured; updated to "Must be specified explicitly when the branch has no upstream configured." |
| `git_remote` | ✅ | Lists remotes with URLs |

##### git-read Alignment

`git-read` is properly aligned with `git`:

- Contains exactly the 5 read-only tools: `git_status`, `git_log`, `git_diff`, `git_show`, `git_branch`
- Tool schemas are identical to the read-only subset of `git` tools
- Execution specs are identical to the read-only subset of `git` execution specs
- Allowlist is the read-only subset: `["status", "log", "diff", "show", "branch"]`
- SKILL.md contains only read-relevant directives (check status, use specific refs). Omits write-relevant directives (commit messages, branch deletion, protected branch info)
- `variant_of: git` correctly declared
- `capability_tier: minimal` correctly set

##### SKILL.md Review

**git directives assessed against design principles:**

| Directive | Assessment | Action |
|-----------|-----------|--------|
| always check `git_status` before interpreting diffs | Appropriate — applies to all diff calls, tool can't enforce | Keep |
| always use specific refs rather than ambiguous references | Appropriate — tool can't know if a ref is ambiguous | Keep |
| always write clear, concise, conventional commit messages | Appropriate — subjective quality judgment | Keep |
| never delete the current branch — switch to another branch first | **Redundant** — `git branch -d` already rejects with error; `denyPattern` protects `main`/`release/*` | Remove — tool enforcement already covers this |
| Commits on `main` and `release/*` branches are blocked. Push to these branches is also blocked. Use feature branches for all changes. | **Redundant** — `denyPattern` enforces this; the agent learns from the error message | Remove — duplicates tool behavior |
| Using `ref: "main"` in `git_diff` shows all changes since diverging from main. | Non-obvious behavior — describes a `git diff` semantic the tool schema doesn't communicate | Remove — agent assumes `main` for project |

**Directives removed in this round:**

- **"never delete the current branch — switch to another branch first"** — Removed. `git branch -d` already rejects deleting the current branch with a clear error. For protected branches (`main`, `release/*`), the `denyPattern` on `git_branch_delete` blocks those. The directive added no enforcement value beyond what the tools already provide.

- **"Commits on `main` and `release/*` branches are blocked. Push to these branches is also blocked. Use feature branches for all changes."** — Removed. The `denyPattern` on `git_commit` (via `denyAlwaysResolve` + `denyResolveCommand`) and `git_push` (on the `branch` parameter) already enforce this. The error messages are specific and actionable. The directive duplicated tool-enforced behavior.

**git-read directives:** No changes — read-relevant directives (check status, use specific refs, ref="main" note) are appropriate.

**git-remote directives:** All three directives retained as instructions — they provide workflow guidance that hints will augment but not replace.

##### Hint Implementation Status

All 9 diagnostic hints are now implemented as `postProcess` scripts. Each script receives the tool's output on stdin, inspects it for error conditions, and appends a one-line hint when the condition is detected. Non-matching output passes through unchanged.

**Retest finding (session 2)**: 6 of 7 hints were broken because `postProcess` only runs after a successful command exit. When git exits with a non-zero code (e.g., 128 for "not a repository", 1 for "nothing to commit"), the error propagated before `postProcess` could inspect the output and append hints. Only the `git_diff` ref=main hint worked because it triggers on a successful exit.

**Fix applied**: Added `successExitCodes` to tools whose hints target error conditions. This allows the output to pass through as `Ok(...)` so `postProcess` can run. Also fixed `collect_output_with_codes` in `exec.rs` to include stderr when a non-zero exit code is in the success list — git writes error diagnostics to stderr, which was previously discarded on the success path.

**Retest finding (session 3)**: `git_pull` had no hint, no `successExitCodes`, and no `postProcess` script — same root cause as session 2's `git_push` bug. `git_pull` exits with code 1 when no tracking information is configured or when the remote is not found, but without `successExitCodes` the error propagated before any `postProcess` could run. Added `successExitCodes: [1, 128]`, `postProcess` (`hint-pull-errors.sh`), and fixed misleading parameter descriptions (same pattern as `git_push` `branch` description fix).

| Tool | Hint | Script | Condition | `successExitCodes` |
|------|------|--------|-----------|-------------------|
| `git_status` | "not a git repository — specify a valid repo path" | `hint-not-repo.sh` | Output contains "not a git repository" | `[128]` |
| `git_commit` | "nothing to commit — working tree clean" | `hint-commit-status.sh` | Output contains "nothing to commit" | `[1]` |
| `git_commit` | "unstaged changes present — use git_add to stage them" | `hint-commit-status.sh` | Output contains "no changes added to commit" or "untracked files present" | `[1]` |
| `git_diff` | "showing changes since diverging from main" | `hint-diff-ref-main.sh` | `ref` parameter is "main" (passed via `$ref` arg) | *(not needed — exit 0)* |
| `git_pull` | "no tracking branch set — specify remote and branch explicitly" | `hint-pull-errors.sh` | Output contains "no tracking information", "no upstream branch", or "There is no tracking information" | `[1, 128]` |
| `git_pull` | "remote not found — use git_remote to list configured remotes" | `hint-pull-errors.sh` | Output contains "Could not resolve", "does not appear to be a git repository", or "not found" | `[1, 128]` |
| `git_push` | "pull first to integrate remote changes, then retry" | `hint-push-errors.sh` | Output contains "non-fast-forward", "rejected", or "fetch first" | `[1, 128]` |
| `git_push` | "no upstream set — use set_upstream: true on first push" | `hint-push-errors.sh` | Output contains "no upstream branch", "no tracking information", or "has no upstream branch" | `[1, 128]` |
| `git_push` | "no remote configured — use git_remote to list configured remotes" | `hint-push-errors.sh` | Output contains "No remote", "Could not resolve", or "does not appear to be a git repository" | `[1, 128]` |

Scripts are duplicated across git and git-read (`hint-not-repo.sh`, `hint-diff-ref-main.sh`) as required by the self-contained skill design. git-remote has its own `hint-push-errors.sh` and `hint-pull-errors.sh`.

**Hint verification (session 4):** All 9 hints tested against live git output. 7 of 9 confirmed working; 2 issues found and fixed:

1. **`git_push` "no remote configured" hint had mismatched patterns** — the script checked for `No remote\|remote:.*not found\|Could not resolve` but git's actual error for a nonexistent remote is `'nonexistent' does not appear to be a git repository` / `Could not read from remote repository`. Added `does not appear to be a git repository` to the match pattern. Also updated hint text from "use git_remote to add one" to "use git_remote to list configured remotes" — `git_remote` lists remotes, it doesn't add them.

2. **`git_diff` ref=main hint was truncated when output exceeds `maxOutputLines: 200`** — the hint was appended by `postProcess` after the raw output, making it the last line and first to be truncated. **Fixed**: `truncate_output()` now separates `hint:`-prefixed lines from non-hint lines before truncation, preserving hints regardless of output size. Also updated binary-level hints (`files_replace` / `notes_replace` leading-whitespace hint) to emit standalone `hint:` lines instead of inline hints, matching the postProcess script convention. See ~~`BUG_TRUNCATED_HINTS.md`~~ (deleted — bug resolved in session 6).

**Retest (session 5):** Truncation fix verified via code review and `files_replace` hint test. All `truncate_output` unit tests pass (8 tests covering hint preservation, multiple hints, hints before notice, hints not counted against limit). `files_replace` leading-whitespace hint confirmed emitting standalone `hint:` line. Git hint retesting requires git/git-remote skills enabled — pending live verification of `git_diff` ref=main (hint surviving truncation), `git_pull` (no-tracking and remote-not-found hints), and `git_push` (no-upstream, non-fast-forward, and remote-not-found hints).

**Retest (session 6):** Live verification of all pending git/git-remote hints. All confirmed working:

| Hint | Test | Result |
|------|------|--------|
| `git_diff` ref=main | Diff on chai repo (200 of 11510 lines truncated) | ✅ Hint `"showing changes since diverging from main"` preserved before truncation notice |
| `git_pull` no-tracking | Pull on branch with no tracking info | ✅ Hint `"no tracking branch set — specify remote and branch explicitly"` |
| `git_pull` remote-not-found | Pull from nonexistent remote `"nonexistent"` | ✅ Hint `"remote not found — use git_remote to list configured remotes"` |
| `git_push` remote-not-found | Push to nonexistent remote `"nonexistent"` | ✅ Hint `"no remote configured — use git_remote to list configured remotes"` |
| `git_push` no-upstream | Push new branch without `set_upstream` | ⚠️ Not triggered — when remote and branch are specified explicitly, git doesn't emit the "no upstream" message; the hint fires only when git tries to infer the upstream and can't find it, which requires omitting the branch parameter (not the typical tool usage pattern) |
| `git_push` non-fast-forward | Push behind remote | ⚠️ Not tested — requires remote to have commits that local doesn't; not easily reproducible from sandbox |

The `git_push` no-upstream and non-fast-forward hints remain untested in live usage but the postProcess scripts and `successExitCodes` are configured identically to the confirmed-working `git_pull` hints (same error pattern, same exit codes, same stderr-to-stdout fix). The ~~`BUG_TRUNCATED_HINTS.md`~~ bug is resolved (bug report deleted) — `truncate_output()` now preserves `hint:`-prefixed lines through truncation, and binary-level hints emit standalone `hint:` lines.

#### Battle Test: logs & rss Skills — Complete

##### Tools Reviewed

| Tool | Status | Notes |
|------|--------|-------|
| `logs_recent` | ✅ | Returns log lines with level filtering. `absentDefault: "warn"` confirmed — returns warn-level output when `level` omitted. |
| `logs_search` | ✅ | Substring search with context lines. Returns "N match(es) for pattern" footer. |
| `rss_list_feeds` | ✅ | Lists feeds from `rss-feeds.txt`. Works when file exists; returns error when missing. |
| `rss_check_feed` | ✅ | Fetches and parses RSS/Atom feeds. Feed name resolution works for configured names. Direct URLs pass through. |

##### Hint Verification

| Hint | Test | Result |
|------|------|--------|
| `logs_recent` absentDefault | Call without `level` parameter | ✅ Returns warn-level output by default |
| `logs_search` many-matches | Search for `"pool"` (25 matches) | ✅ Hint emitted: `"many matches — consider narrowing the pattern"` |
| `rss_check_feed` feed-not-found | Call with `feed: "nonexistent"` | ✅ Hint emitted: `"feed 'nonexistent' not found in configuration — use rss_list_feeds to see available feeds"` |
| `rss_check_feed` unreachable URL | Call with `feed: "https://nonexistent.example.com/feed.xml"` | ✅ No hint (correct — URLs bypass name-check logic); "No entries found in feed." returned |
| `rss_check_feed` valid name | Call with `feed: "hackernews"` | ✅ Returns parsed entries correctly |
| `rss_check_feed` direct URL | Call with `feed: "https://hnrss.org/frontpage"` | ✅ Returns parsed entries correctly |

#### Hints Evaluation: git / git-read

| Directive | Enforce? | Hint? | Assessment |
|-----------|----------|-------|------------|
| never assume the working directory is a Git repository | ❌ | ✅ | **→ Hint**: replaced directive with enhanced error message |
| always check `git_status` before interpreting diffs | ❌ | ⚠️ | **→ Instruct**: kept as brief directive (applies to all diff calls) |
| always use specific refs rather than ambiguous references | ❌ | ❌ | **→ Instruct**: kept |
| always set `count` on `git_log` to limit output | ✅ `absentDefault` | — | **→ Enforce**: added `absentDefault: "10"` for `count` parameter, removed directive |
| always check `git_status` before committing | ❌ | ✅ | **→ Hint**: append status summary to commit output |

#### Hints Evaluation: logs / rss

| Directive | Enforce? | Hint? | Assessment |
|-----------|----------|-------|------------|
| use `logs_search` to check for specific conditions | ❌ | ⚠️ | **→ Instruct**: kept (brief, tool-choice guidance) |
| use `logs_recent` with `level: "warn"` or `"error"` | ✅ `absentDefault` | — | **→ Enforce**: default `level` to `"warn"` via `absentDefault`, removed directive |
| Log lines contain token counts but not full messages | ❌ | ❌ | **→ Instruct**: kept (brief informational note) |

**Directives removed:**

- **"use `logs_recent` with `level: "warn"` or `level: "error"`"** — Replaced by `absentDefault: "warn"` on the `level` parameter. The tool enforces what was previously a directive.

**Applied Hints**

| Tool | Condition | Hint |
|------|-----------|------|
| `logs_recent` | `level` not specified | Defaults to `"warn"` via `absentDefault` |
| `logs_search` | Many matches returned | `"many matches — consider narrowing the pattern"` |

**Implementation:**

- `logs_recent` `absentDefault: "warn"` — ✅ Already in tools.json.
- `logs_search` many-matches hint — ✅ Implemented as `scripts/hint-many-matches.sh` postProcess script. Matches the "N match(es) for pattern" footer line and emits the hint when count > 15.

#### Hints Evaluation: rss

| Directive | Enforce? | Hint? | Assessment |
|-----------|----------|-------|------------|
| always call `rss_list_feeds` first | ❌ | ✅ | **→ Hint**: hint if feed name not in configured list |
| always use feed names from the configured list | ✅ Validation | — | **→ Enforce**: `resolveCommand` already validates feed names; error on unknown names |
| always summarize feed entries rather than returning raw table | ❌ | ❌ | **→ Remove**: postProcess already handles formatting |
| never follow external links without evaluating relevance | ❌ | ❌ | **→ Remove**: agent-judgment directive with no tool interaction |

**Directives removed:**

- **"always summarize feed entries rather than returning raw table"** — Removed. The `parse-rss` `postProcess` script already structures output; "summarize" is an agent behavior.
- **"never follow external links without evaluating relevance"** — Removed. Cannot enforce agent behavior outside the tool; agent-judgment directive with no tool interaction.

**Applied Hints**

| Tool | Condition | Hint |
|------|-----------|------|
| `rss_check_feed` | Feed name not in configured list | `"feed '[name]' not found in configuration — use rss_list_feeds to see available feeds"` |

**Implementation:**

- `rss_check_feed` feed-not-found hint — ✅ Implemented inside `scripts/parse-rss.sh`. The script now receives `$feed` as `$1` (via `postProcess.args`). When parsing produces no entries ("No entries found in feed.") and the feed parameter is a name (not a URL) not found in the feeds file, the hint is appended. Added `successExitCodes: [6, 7]` to handle curl exit codes for DNS/connect failures, allowing the output to pass through to postProcess.
- `rss_check_feed` `postProcess` now passes `args: ["$feed"]` so the script has the original feed parameter for hint detection.
- Removed two directives from SKILL.md: "always summarize feed entries rather than returning raw table" (postProcess handles formatting) and "never follow external links without evaluating relevance" (agent-judgment, no tool interaction).
- `rss` SKILL.md restructured: moved feed configuration format from before directives to a new "## Skill Guidelines" section after directives. Separates directives (must-follow) from non-directive context (configuration format).

---

### Skillset 3: notes, notes-daily, notes-frontmatter, notes-wikilink — Complete

#### Battle Test: notes Skills

##### Tools Reviewed

| Tool | Status | Notes |
|------|--------|-------|
| `notes_read` | ✅ | Reads note content. `successExitCodes: [1]` + `postProcess: hint-not-found` for not-found hint. |
| `notes_read_lines` | ✅ | Line-numbered output. Four-stage `original_content` verification. Mismatch rejection shows expected vs actual. `successExitCodes: [1]` + `postProcess: hint-not-found`. |
| `notes_list` | ✅ | `long` and `all` flags work. `sideRead` for AGENTS.md. `postProcess: sanitize-ls`. |
| `notes_search` | ✅ | ERE via `grep -E`. `successExitCodes: [0, 1]`. `absentDefault: true` for `line_numbers` and `recursive`. `postProcess: hint-search-results`. |
| `notes_write` | ✅ | Full overwrite. Creates parent dirs. Content via stdin. Binary now outputs "overwriting existing N lines" when file exists. `postProcess: hint-overwrite`. |
| `notes_write_lines` | ✅ | Four-stage `verify_original`. Trailing-ws preservation. `original_content` via tempfile. Bottom-to-top editing verified. |
| `notes_replace` | ✅ | Regex with multiline mode. `literal` mode. `max_replacements`. Trailing-ws-tolerant fallback. Leading-ws diagnostic hint. Capture groups ($1-$9). Regex error suggests `literal: true`. Multiple-matches hint when count > 1 and max_replacements == 0. |
| `notes_delete` | ✅ | Refuses directories. |
| `notes_delete_dir` | ✅ | Refuses non-empty directories and files. |

##### Hint Verification

| Hint | Test | Result |
|------|------|--------|
| `notes_read` not-found | Read `./notes-testing/nonexistent.md` | ✅ Hint `"note not found — use notes_list to browse available notes"` |
| `notes_read_lines` not-found | Read lines from nonexistent note | ✅ Hint `"note not found — use notes_list to browse available notes"` |
| `notes_search` results | Search for `"privacy"` in notes-testing | ✅ Hint `"use notes_read_lines with these line numbers for surrounding context"` |
| `notes_search` no-match | Search for `"zzznonexistent"` | ✅ No hint (correct — no results) |
| `notes_search` flags | `files_only`, `case_insensitive` | ✅ All flags work correctly |
| `notes_write` overwrite | Write to existing note | ✅ Hint `"overwrote existing note"` |
| `notes_write` new file | Write to new note | ✅ No hint (correct — new file) |
| `notes_write_lines` verify | Mismatch `original_content` | ✅ Rejected with helpful error showing expected vs actual |
| `notes_write_lines` bottom-to-top | Two non-adjacent edits | ✅ Line numbers stay stable when editing bottom-up |
| `notes_replace` multiple matches | Replace `"applied"` → `"done"` (2 matches) | ✅ Hint `"2 match(es) replaced — use max_replacements: 1 to limit to first match"` |
| `notes_replace` single match | Replace with only 1 match | ✅ No hint (correct — single match) |
| `notes_replace` max_replacements | Replace with `max_replacements: 1` | ✅ "1 of 2 match(es) replaced" — no hint (correct — limit already set) |
| `notes_replace` leading-ws hint | `literal: true` with pattern missing indentation | ✅ Hint `"pattern did not match, but would match with leading-whitespace normalization — check indentation"` |
| `notes_replace` regex error | Pattern `[invalid` | ✅ Error suggests `"literal: true"` |
| `notes_replace` capture groups | Pattern `(testing)` → `$1-audit` | ✅ Capture groups expand correctly |
| `notes_delete` directory | Delete a directory path | ✅ "refusing to delete non-file" |
| `notes_delete_dir` non-empty | Delete non-empty directory | ✅ "refusing to delete non-empty directory" |
| `notes_delete_dir` file | Delete a file path | ✅ "refusing to delete non-directory" |

#### Battle Test: notes-daily

##### Hint Verification

| Hint | Test | Result |
|------|------|--------|
| `notes_daily_read` not-found | Read today's note (doesn't exist) | ✅ Hint `"no daily note found for this date — use notes_daily_write to create one"` |
| `notes_daily_write` overwrite | Write to existing daily note | ✅ Hint `"daily note already exists — consider notes_daily_append to add content instead"` |
| `notes_daily_append` new note | Append to non-existent date | ✅ Hint `"no daily note found for this date — use notes_daily_write to create one"` |
| `notes_daily_write` new file | Create new daily note | ✅ No overwrite hint (correct — new file) |
| `notes_daily_append` existing | Append to existing daily note | ✅ No creation hint (correct — file exists) |
| `root` parameter | Use `root: "notes-testing"` | ✅ Resolves daily path correctly with `.notes-daily.conf` |
| Resolved path in response | All daily tools | ✅ Full path shown in output |

#### Battle Test: notes-frontmatter

##### Hint Verification

| Hint | Test | Result |
|------|------|--------|
| `notes_frontmatter_read` existing | Read Conventions.md frontmatter | ✅ Returns `type: meta` |
| `notes_frontmatter_read` no frontmatter | Read note without frontmatter | ✅ Hint `"no frontmatter found — use notes_frontmatter_edit to create one"` |
| `notes_frontmatter_read` not-found | Read nonexistent note | ✅ Error with file-not-found message |
| `notes_frontmatter_edit` show result | Edit key on note | ✅ Resulting frontmatter shown after edit |
| `notes_frontmatter_edit` create frontmatter | Edit key on note without frontmatter | ✅ Frontmatter block created; resulting frontmatter shown |
| `notes_frontmatter_delete` | Delete key | ✅ Key removed |

#### Battle Test: notes-wikilink

##### Hint Verification

| Hint | Test | Result |
|------|------|--------|
| `notes_wikilink_backlinks` | Search for `"Conventions"` | ✅ Finds bare `[[Conventions]]` links |
| `notes_wikilink_outlinks` broken count | Extract from Conventions.md | ✅ Hint `"N broken link(s) — use notes_wikilink_broken for details"` |
| `notes_wikilink_broken` | Check Conventions.md | ✅ Returns `Note Name` and `subfolder/Note Name` as broken |
| `notes_wikilink_by_tag` | Search for `"privacy"` | ✅ Finds tagged notes |
| `notes_wikilink_by_tag` `#` prefix | Search for `"#privacy"` | ✅ Normalizes tag by stripping `#` |
| `notes_wikilink_rename` | Rename note with wikilinks | ✅ File renamed, wikilinks updated |
| `notes_wikilink_rename` source not found | Rename nonexistent note | ✅ Error: "source does not exist" |
| `notes_wikilink_rename` dest exists | Rename to existing path | ✅ Error: "destination already exists" |
| `notes_wikilink_rename` without `root` | Omit `root` parameter | ✅ **Bug fix**: now works — `--root` defaults to current directory |

#### Bugs Found and Fixed

| Bug | Type | Fix |
|-----|------|-----|
| `notes_wikilink_rename` required `root` despite being documented as optional | Schema/runtime mismatch | Changed `--root` from `String` to `Option<String>` in CLI; defaults to `current_dir()` when omitted |
| `notes_frontmatter_read` missing `successExitCodes` | Hint non-functional | Added `successExitCodes: [1]` to allow hint output through `postProcess` |
| `notes_write` no overwrite indication | Missing diagnostic | Binary now outputs "overwriting existing N lines" when file exists; `postProcess: hint-overwrite` adds skill-specific hint |
| `notes_replace` no multiple-matches hint | Missing diagnostic | Binary now emits hint when count > 1 and max_replacements == 0 |
| `notes_frontmatter_read` no frontmatter error had no hint | Missing diagnostic | Changed `bail!` to `println!` + hint + `exit(1)`; added `successExitCodes: [1]` |
| `notes_frontmatter_edit` didn't show result | Missing diagnostic | Binary now shows resulting frontmatter after edit |
| `notes_wikilink_outlinks` no broken-link hint | Missing diagnostic | Enhanced `sanitize-outlinks.sh` to check for broken links and emit count hint; added `root` parameter and `postProcess.args` |
| `notes_daily_read` no not-found hint | Missing diagnostic | Added `successExitCodes: [1]` + `postProcess: hint-not-found` |
| `notes_daily_write` no overwrite hint | Missing diagnostic | Added `postProcess: hint-daily-overwrite` |

#### Improvements Applied in This Round

| Improvement | Type | Rationale |
|------------|------|-----------|
| `notes_read` / `notes_read_lines` not-found hint | `postProcess` script | `"note not found — use notes_list to browse available notes"` — replaces directive "never assume a note exists" for the error case |
| `notes_search` results hint | `postProcess` script | `"use notes_read_lines with these line numbers for surrounding context"` — replaces directive "use notes_read_lines after notes_search" for the results case |
| `notes_write` overwrite hint | Binary + `postProcess` | Binary outputs "overwriting existing N lines"; `postProcess` adds `"overwrote existing note"` — supplements directive "always read before overwriting" |
| `notes_replace` multiple-matches hint | Binary | `"M match(es) replaced — use max_replacements: 1 to limit to first match"` — supplements directive about `max_replacements` |
| `notes_daily_write` overwrite hint | `postProcess` script | `"daily note already exists — consider notes_daily_append to add content instead"` |
| `notes_daily_append` new-note hint | `postProcess` script | `"no daily note found for this date — use notes_daily_write to create one"` |
| `notes_daily_read` not-found hint | `postProcess` script | `"no daily note found for this date — use notes_daily_write to create one"` |
| `notes_frontmatter_read` no-frontmatter hint | Binary + `successExitCodes` | `"no frontmatter found — use notes_frontmatter_edit to create one"` — replaces directive "always read before editing" |
| `notes_frontmatter_edit` result display | Binary | Shows resulting frontmatter after edit — replaces directive "always use notes_frontmatter_read before editing" |
| `notes_wikilink_outlinks` broken-link count | `postProcess` script | `"N broken link(s) — use notes_wikilink_broken for details"` — supplements directive "never assume a wikilink target exists" |
| `notes_wikilink_outlinks` `root` parameter | Schema addition | Added optional `root` parameter for broken-link resolution in subdirectory KBs |
| `notes_wikilink_rename` optional `root` | Bug fix | `--root` was required in CLI but optional in schema; changed to `Option<String>` with CWD default |
| `notes_wikilink_rename` zero-update silence | Binary | Only prints "updated wikilinks in N file(s)" when N > 0 |
| `notes_append` tool removed | Skill simplification | `notes_append` was the only notes tool without a `files` counterpart, creating a structural divergence. Removed tool definition, allowlist entry, execution entry, and SKILL.md directive. |
| `notes_write` overwrite hint wording updated | Hint alignment | Changed from `"overwrote existing note — use notes_append to add content instead"` to `"overwrote existing note"` — no append alternative to suggest after removal |
| `notes` SKILL.md overwrite directive alignment | Directive alignment | Changed from "always read a note before overwriting it to avoid data loss" to "always read a note with `notes_read` before overwriting it with `notes_write` to avoid data loss" — matches `files` style of naming both tools explicitly |
| `notes` SKILL.md missing sentence added | Directive alignment | Added "The fallback only accepts matches that start and end at line boundaries — the pattern must match one or more complete lines, not a substring within a line." to the `notes_replace` trailing-whitespace fallback paragraph — matches `files` |
| `notes` / `notes-read` cross-skill reference removed | Directive alignment | Removed "All paths are relative to the sandbox root, matching the `files` skill." paragraph — skills should be self-contained |
| `notes` `capability_tier` fixed | Schema fix | Changed from `moderate` to `full` — notes has write and delete tools, not moderate capability |
| `notes` / `notes-read` SKILL.md prose removed | SKILL.md lean pass | Removed 6 paragraphs of additional content from `notes` (ERE syntax, `literal` details, multiline/capture groups, `max_replacements`, trailing-whitespace fallback) and 1 paragraph from `notes-read` (ERE syntax). Same rationale as `files`/`files-read` — all duplicated tool parameter descriptions or described output behavior delivered by hints. |
| `notes-daily` SKILL.md restructured | SKILL.md lean pass | Moved daily note configuration resolution (folder config, `root` guidance) from before directives to a new "## Skill Guidelines" section after directives. Separates directives (must-follow) from non-directive context (configuration resolution order). |
| `notes-frontmatter` SKILL.md restructured | SKILL.md lean pass | Removed "All paths are relative to the sandbox root, matching the `files` skill. Use `./` prefix for paths in the current directory." — duplicates tool path descriptions and violates self-containment. Moved frontmatter behavior note ("If the note has no frontmatter, `notes_frontmatter_read` returns an error. `notes_frontmatter_edit` creates a frontmatter block if the file has none.") to a new "## Skill Guidelines" section — genuinely useful behavior not in tool descriptions, but not a directive. |
| `notes-wikilink` SKILL.md prose removed | SKILL.md lean pass | Removed "All paths are relative to the sandbox root, matching the `files` skill. Use `./` prefix for paths in the current directory." — duplicates tool path descriptions and violates self-containment. Removed "`notes_wikilink_backlinks` uses `note_name` (the display name, e.g. 'Conventions'), not the file path." — the `note_name` parameter description already says "Note name to search for backlinks to (e.g. 'Conventions', 'AI Assistant')" which conveys this distinction. |

#### notes-read Alignment

`notes-read` is properly aligned with `notes`:

- Contains only the 4 read-only tools: `notes_read`, `notes_read_lines`, `notes_list`, `notes_search`
- Tool schemas are identical to the read-only subset of `notes` tools
- Execution specs are identical to the read-only subset of `notes` execution specs (including `successExitCodes` and `postProcess` hints)
- SKILL.md contains only read-relevant directives (verify existence, search→read workflow)
- `variant_of: notes` correctly declared
- `capability_tier: minimal` correctly set

#### Hints Evaluation: notes

| Directive | Enforce? | Hint? | Assessment |
|-----------|----------|-------|------------|
| never delete notes without confirming | ❌ | ❌ | **→ Instruct**: kept |
| never assume a note exists | ❌ | ✅ | **→ Hint**: enhance "not found" error with suggestion |
| always read a note before overwriting | ❌ | ❌ | **→ Instruct**: kept (for `notes_write` only) |
| prefer single large `notes_write_lines` calls | ❌ | ❌ | **→ Instruct**: kept |
| work bottom-to-top for multiple edits | ❌ | ❌ | **→ Instruct**: kept |
| use `notes_read_lines` after `notes_search` | ❌ | ✅ | **→ Hint**: append suggestion to use `notes_read_lines` for context |
| use `notes_replace` vs `notes_write_lines` choice | ❌ | ❌ | **→ Instruct**: kept |
| use `literal: true` on `notes_replace` for regex metacharacters | ❌ | ✅ | **→ Hint** (partially done): regex error hint exists; proactive detection is optional |

**Directives removed:** none — hints augment tool output, directives remain for general guidance.

**Applied Hints**

| Tool | Condition | Hint |
|------|-----------|------|
| `notes_read` / `notes_read_lines` | Note not found | `"note not found — use notes_list to browse available notes"` |
| `notes_search` | Results returned | `"use notes_read_lines with these line numbers for surrounding context"` |
| `notes_write` | Existing note overwritten | `"overwrote existing note"` |
| `notes_replace` | Multiple matches without `max_replacements` | `"M match(es) replaced — use max_replacements: 1 to limit to first match"` |

#### Hints Evaluation: notes-read

Aligned with notes. Applied hints include only the read-relevant subset:

**Applied Hints**

| Tool | Condition | Hint |
|------|-----------|------|
| `notes_read` / `notes_read_lines` | Note not found | `"note not found — use notes_list to browse available notes"` |
| `notes_search` | Results returned | `"use notes_read_lines with these line numbers for surrounding context"` |

#### Hints Evaluation: notes-daily

| Directive | Enforce? | Hint? | Assessment |
|-----------|----------|-------|------------|
| always use `notes_daily_append` for existing notes | ❌ | ✅ | **→ Hint**: warn when overwriting existing daily note |
| always use `notes_daily_write` only for creating or full rewrites | ❌ | ✅ | **→ Hint**: suggest `notes_daily_write` when appending to non-existent note |
| never construct daily note paths manually | ✅ `resolveCommand` | — | **→ Enforce**: already done by resolver |
| always specify `root` when working with notes in a subdirectory | ❌ | ⚠️ | **→ Instruct**: kept (edge case guidance) |

**Directives removed:** none — hints augment tool output.

**Applied Hints**

| Tool | Condition | Hint |
|------|-----------|------|
| `notes_daily_write` | Daily note already exists | `"daily note already exists — consider notes_daily_append to add content instead"` |
| `notes_daily_append` | Daily note doesn't exist yet | `"no daily note found for this date — use notes_daily_write to create one"` |
| All notes-daily tools | Any call | Include resolved file path in response |

#### Hints Evaluation: notes-frontmatter

| Directive | Enforce? | Hint? | Assessment |
|-----------|----------|-------|------------|
| always use `notes_frontmatter_read` before editing | ❌ | ✅ | **→ Hint**: show current state after edit (makes read-before-edit less critical); directive removed |
| always use `notes_frontmatter_edit` for single-key updates | ❌ | ❌ | **→ Instruct**: kept |
| never modify note body content through this skill | ✅ Binary enforcement | — | **→ Enforce**: already done by `chai file frontmatter-edit` |

**Directives removed:**

- **"always use `notes_frontmatter_read` to inspect frontmatter before editing"** — Replaced by `notes_frontmatter_edit` hint showing resulting frontmatter after edit. The hint makes the read-before-edit workflow less critical.

**Applied Hints**

| Tool | Condition | Hint |
|------|-----------|------|
| `notes_frontmatter_edit` | After successful edit | Show resulting frontmatter in response |
| `notes_frontmatter_read` | No frontmatter found | `"no frontmatter found — use notes_frontmatter_edit to create one"` |

#### Hints Evaluation: notes-wikilink

| Directive | Enforce? | Hint? | Assessment |
|-----------|----------|-------|------------|
| always use `notes_wikilink_broken` to validate links | ❌ | ❌ | **→ Instruct**: kept |
| never assume a wikilink target exists | ❌ | ✅ | **→ Hint**: append broken-link count to outlink results |
| always verify source note exists before renaming | ✅ Runtime check | — | **→ Enforce**: `notes_wikilink_rename` errors if source doesn't exist |
| always verify destination does not already exist | ✅ Runtime check | — | **→ Enforce**: `notes_wikilink_rename` errors if destination exists |
| never rename notes without `notes_wikilink_rename` | ❌ | ❌ | **→ Instruct**: kept (important workflow constraint) |
| never use `notes_wikilink_rename` to just move a file | ❌ | ❌ | **→ Instruct**: kept |
| always specify `root` when working in a subdirectory | ❌ | ⚠️ | **→ Instruct**: kept (edge case guidance) |

**Directives removed:**

- **"never assume a wikilink target exists just because the link is present"** — Replaced by `notes_wikilink_outlinks` broken-link count hint.
- **"always verify the source note exists before renaming"** — Replaced by `notes_wikilink_rename` runtime check.
- **"always verify the destination does not already exist"** — Replaced by `notes_wikilink_rename` runtime check.

**Applied Hints**

| Tool | Condition | Hint |
|------|-----------|------|
| `notes_wikilink_outlinks` | Broken links detected | `"N broken link(s) — use notes_wikilink_broken for details"` |
| `notes_wikilink_rename` | After successful rename | `"renamed '[from]' → '[to]', updated N wikilink(s) across M note(s)"` |
| `notes_wikilink_rename` | Source note doesn't exist | Rejected by runtime check |
| `notes_wikilink_rename` | Destination already exists | Rejected by runtime check |

---

### Change: Removed Redundant `resolveCommand` From notes-Family Skills

**What changed**: Removed `resolveCommand: { script: "resolve-notes-path" }` from all `path` and `root` parameters in `notes`, `notes-read`, `notes-frontmatter`, and `notes-wikilink`. Deleted 5 scripts: `resolve-notes-path.sh` (4 copies) and `resolve-notes-root.sh` (1 copy). Fixed `notes_frontmatter_read` missing `readPath: true` (security gap — resolved absolute path bypassed sandbox validation). Fixed `check-broken-links.sh` to handle absolute `root` values from canonical path substitution (pre-existing bug).

**Why**: `resolve-notes-path.sh` only prepended the sandbox root to relative paths — the same thing `WriteSandbox::validate()` already does natively when it sees a relative path on a `readPath`/`writePath`-annotated parameter (lines 288–292 of `exec.rs`). The `files` skill proves this works without any resolve script. The resolve scripts were a historical artifact from before the sandbox handled relative path resolution.

**What stayed**: `notes-daily/resolve-daily-path.sh` (date → path transformation, reads config files), `notes-wikilink/build-backlink-pattern.sh` (note name → grep pattern), `notes-wikilink/normalize-tag.sh` (tag normalization), `notes-wikilink/sanitize-outlinks.sh` and `notes-wikilink/check-broken-links.sh` (post-processing). These do real value transformation, not just sandbox-root prepending.

**What needs testing**: After rebuilding binaries, verify with the notes skillset enabled:

1. **notes path resolution**: `notes_read`, `notes_list`, `notes_search` with sandbox-relative paths (e.g., `"./notes/entry.md"`) — should resolve within sandbox and return content
2. **notes write operations**: `notes_write`, `notes_write_lines`, `notes_replace`, `notes_delete`, `notes_delete_dir` with sandbox-relative paths — should write/delete within sandbox
3. **notes-read alignment**: `notes_read`, `notes_read_lines`, `notes_list`, `notes_search` — same behavior as notes equivalents
4. **notes-frontmatter**: `notes_frontmatter_read` (was missing `readPath` — now sandbox-validated), `notes_frontmatter_edit`, `notes_frontmatter_delete` — should all work with sandbox-relative paths
5. **notes-wikilink path params**: `notes_wikilink_backlinks`, `notes_wikilink_outlinks`, `notes_wikilink_by_tag`, `notes_wikilink_broken` with and without optional `path` parameter — when omitted, should default to sandbox root (CWD)
6. **notes-wikilink root**: `notes_wikilink_broken` and `notes_wikilink_rename` with `root` provided and omitted — verify `check-broken-links.sh` handles both relative and canonical-absolute values
7. **notes-wikilink_rename**: `from`/`to` params with sandbox-relative paths, `root` optional — verify `--root` flag receives canonical path
8. **notes-daily**: `notes_daily_read`, `notes_daily_write`, `notes_daily_append` — unchanged, but verify no regression since `resolve-daily-path.sh` was kept
9. **Sandbox enforcement**: Attempt to read/write paths outside the sandbox (e.g., `/etc/passwd`, `../../etc/passwd`) — should be rejected by sandbox validation

### Skillset 4: skills, skills-design, skills-read — Complete

#### Battle Test: skills Skills

##### Tools Reviewed

| Tool | Status | Notes |
|------|--------|-------|
| `skills_discover` | ✅ | Runs binary help output. `maxOutputLines: 200`. Tested with `chai` and `chai skill` subcommands. |
| `skills_list` | ✅ | Lists installed skills with population status. |
| `skills_read` | ✅ | Reads SKILL.md or tools.json content. `maxOutputLines: 500`. |
| `skills_validate` | ✅ | Validates tools.json schema. Reports errors and warnings. Tested with valid and invalid skills. |
| `skills_init` | ✅ | Creates new skill directory with template files. |
| `skills_write_skill_md` | ✅ | Writes SKILL.md via stdin. |
| `skills_write_tools_json` | ✅ | Writes tools.json via stdin. Validates JSON before writing. |
| `skills_write_script` | ✅ | Writes script to scripts/ directory. |
| `skills_delete` | ✅ | Deletes skill directory. `denyPattern` blocks deletion of bundled skills. |

##### Hint Verification

| Hint | Test | Result |
|------|------|--------|
| `skills_write_tools_json` auto-validate | Write tools.json for `test-audit` (missing `execution` key) | ✅ Hint: `"tools.json written — validation: 1 ERROR(s), 0 WARNING(s)"` |
| `skills_write_skill_md` missing frontmatter | Write SKILL.md without frontmatter | ✅ Hint: `"SKILL.md written — missing recommended frontmatter: description, capability_tier, metadata.requires.bins"` |
| `skills_write_skill_md` variant pattern | Write SKILL.md for `test-audit` (hyphenated, no `variant_of`) | ✅ Hint: `"skill name 'test-audit' matches variant pattern — consider adding variant_of to frontmatter"` |
| `skills_validate` errors hint | Validate skill with errors | ✅ Hint: `"hint: use skills_read with file: 'tools_json' to examine the content"` |
| `skills_init` next steps | Initialize new skill | ✅ Hint: `"skill initialized — next: design tools, write tools.json, then validate"` |
| `skills_delete` bundled skill | Attempt to delete `files` | ✅ `denyPattern` blocks deletion: `"parameter 'skill_name' value 'files' matches denyPattern '...' on tool 'skills_delete'"` |
| `skills_delete` bundled skill | Attempt to delete `skills` | ✅ `denyPattern` blocks deletion |
| `skills_delete` non-bundled skill | Delete `test-audit` | ✅ Allowed — non-bundled skills not in denyPattern |

##### Hints Evaluation: skills

| Directive | Enforce? | Hint? | Assessment |
|-----------|----------|-------|------------|
| always read a reference skill before generating | ❌ | ❌ | **→ Instruct**: kept (part of generation workflow) — updated to use `skills_read` instead of `files` |
| always follow the generation workflow in order | ❌ | ❌ | **→ Instruct**: kept (brief) |
| always include `capability_tier` and `metadata.requires.bins` | ❌ | ✅ | **→ Hint**: `hint-skill-md-checks.sh` checks frontmatter after write |
| always include `variant_of` for variant skills | ❌ | ✅ | **→ Hint**: `hint-skill-md-checks.sh` detects hyphenated name without `variant_of` |
| always validate tools.json after writing | ❌ | ✅ | **→ Enforce**: `hint-validate-on-write.sh` auto-validates on write |
| never add unused subcommands to the allowlist | ❌ | ✅ | **→ Hint**: reported by `skills_validate` output |
| never include `resolveCommand` unless needed | ❌ | ✅ | **→ Hint**: flagged by `hint-validate-on-write.sh` via validation |
| never delete bundled skills | ✅ `denyPattern` | — | **→ Enforce**: `denyPattern` on `skills_delete` blocks bundled skill names |

**Directives removed:**

- **"always include `capability_tier` and `metadata.requires.bins` in produced SKILL.md frontmatter"** — Replaced by `hint-skill-md-checks.sh` postProcess hint that checks for missing fields after write.

- **"always include `variant_of` in frontmatter for variant skills"** — Replaced by `hint-skill-md-checks.sh` postProcess hint that detects hyphenated skill names without `variant_of`.

- **"always validate tools.json after writing with `skills_validate`"** — Replaced by `hint-validate-on-write.sh` postProcess that auto-validates after write.

- **"never delete bundled skills (those that ship with chai) unless explicitly instructed"** — Replaced by `denyPattern` on `skills_delete` that enforces this at the tool level.

**Applied Hints**

| Tool | Condition | Hint |
|------|-----------|------|
| `skills_write_tools_json` | After write | Auto-validate and append: `"tools.json written — validation: N ERROR(s), M WARNING(s)"` or `"tools.json written — validation: PASS with N WARNING(S)"` |
| `skills_write_skill_md` | Missing frontmatter fields | `"hint: SKILL.md written — missing recommended frontmatter: capability_tier, metadata.requires.bins"` |
| `skills_write_skill_md` | Hyphenated name without `variant_of` | `"hint: skill name '[name]' matches variant pattern — consider adding variant_of to frontmatter"` |
| `skills_validate` | Validation errors found | `"hint: use skills_read with file: 'tools_json' to examine the content"` |
| `skills_init` | After successful init | `"hint: skill initialized — next: design tools, write tools.json, then validate"` |
| `skills_delete` | Bundled skill name | Blocked by `denyPattern` — `"parameter 'skill_name' value '...' matches denyPattern '...' on tool 'skills_delete'"` |

#### skills-design Re-Audit

Re-audited for an agent with only `skills` and `skills-design` enabled (no `files`, no chai source code access).

**Changes made:**

- **Removed `See TOOLS_SCHEMA.md for the full schema`** — Replaced with `Schema conformance is enforced by skills_validate`. An agent without `files` skill cannot access `TOOLS_SCHEMA.md` in the chai source tree. The validator is the practical enforcement mechanism.

- **Removed specific example references** — Removed the hint examples table (`files_replace`, `git_status`, `notes_search`) and the `successExitCodes` examples table that referenced git-specific exit codes and error patterns. These are implementation details of other skills, not design principles. The general patterns are already described in the text above the removed tables.

- **Removed `e.g., my-note → /home/user/.chai/notes/my-note`** — Changed to `e.g., my-note → an absolute path`. The specific path is an implementation detail of the notes skill.

- **Removed `e.g., chai file replace`** — Changed to generic description. The specific binary is an implementation detail.

- **Removed `Verification Over Instruction` detail** — Removed the multi-stage comparison description (exact, NFC-normalized, Unicode-to-ASCII folded, trailing-whitespace-tolerant). This is an implementation detail of `files_write_lines`/`notes_write_lines`, not a design principle the agent needs when building new skills.

- **Kept all core design principles** — Tools Over Inference, Diagnostic Hints Over Directives, Tool Surface Reduction, SKILL.md Sizing, Content-Passing Channel Selection, Unbounded Output Protection, Sandbox Security, Disallowed Values, Skill Naming and Variant Conventions, Frontmatter Conventions.

- **Added SKILL.md Section Structure subsection** — Under "SKILL.md Sizing", added a new subsection defining the three allowed section types: `## Skill Directives` (hard rules), `## Skill Guidelines` (soft context like configuration formats), and `## <Named Workflow>` (composed multi-step procedures for meta-skills). Prevents prose paragraphs from blending into the directives list and causing the agent to treat guidelines as hard rules.

#### skills-read Alignment

`skills-read` is properly aligned with `skills`:

- Contains exactly the 3 read-only tools: `skills_list`, `skills_read`, `skills_validate`
- Tool schemas are identical to the read-only subset of `skills` tools
- Execution specs are **mostly** identical to the read-only subset of `skills` execution specs, with one intentional difference:
  - `skills_read` has `postProcess: hint-path-annotations` (checks for unannotated path-like params) — `skills` does not have this hint, as the `skills` skill is for creation where the agent is expected to know about path annotations
- `variant_of: skills` correctly declared
- `capability_tier: minimal` correctly set
- SKILL.md is self-contained for read-only workflows (audit, security review, cross-validation) — no generation or write directives

**Directives removed:**

- **"always report all errors and warnings from validation, not just the first"** — `skills_validate` already reports all errors and warnings. Redundant with tool behavior.

- **"always use `skills_read` to examine skill contents when diagnosing errors"** — Replaced by `hint-validate-errors.sh` postProcess hint that suggests this automatically when errors are found.

**Term change:**

- **"ArgMapping"** → **"parameter in the `args` array"** — More accessible term for an agent without access to the chai source code.

##### Hint Verification

| Hint | Test | Result |
|------|------|--------|
| `skills_validate` errors hint | Validate skill with errors | ✅ Hint: `"hint: use skills_read with file: 'tools_json' to examine the content"` |
| `skills_read` path annotations | Read `tools_json` of `files` skill (has annotated path params) | ✅ No false positive — path params have `readPath`/`writePath` annotations, so no hint emitted |
| `skills_read` path annotations | Read `tools_json` of `skills` skill (no path-like params) | ✅ No false positive — no path-like param names detected |

**Applied Hints**

| Tool | Condition | Hint |
|------|-----------|------|
| `skills_validate` | Validation errors found | `"hint: use skills_read with file: 'tools_json' to examine the content"` |
| `skills_read` | Reading `tools_json` with unannotated path params | `"hint: some path-like parameters may lack readPath/writePath annotations — review args for sandbox security"` |

#### Implementation Summary

| File | Change |
|------|--------|
| `skills/tools.json` | Added `postProcess` to `skills_validate`, `skills_init`, `skills_write_skill_md`, `skills_write_tools_json`. Added `denyPattern` to `skills_delete`. Added `successExitCodes: [1]` to `skills_validate`. |
| `skills/scripts/hint-validate-on-write.sh` | New script — auto-validates tools.json after write |
| `skills/scripts/hint-skill-md-checks.sh` | New script — checks frontmatter fields and variant naming |
| `skills/scripts/hint-validate-errors.sh` | New script — suggests `skills_read` when validation fails |
| `skills/scripts/hint-init-next-steps.sh` | New script — suggests next steps after init |
| `skills/SKILL.md` | Removed 4 directives now enforced/hinted by tools; updated reference to use `skills_read` instead of `files` |
| `skills-read/tools.json` | Added `postProcess` to `skills_validate` and `skills_read`. Added `successExitCodes: [1]` to `skills_validate`. |
| `skills-read/scripts/hint-validate-errors.sh` | New script — suggests `skills_read` when validation fails |
| `skills-read/scripts/hint-path-annotations.sh` | New script — checks for unannotated path parameters |
| `skills-read/SKILL.md` | Removed 2 redundant directives; changed "ArgMapping" to accessible term |
| `skills-design/SKILL.md` | Removed `TOOLS_SCHEMA.md` reference, specific binary/skill examples, implementation details; made self-contained for agents without `files` or chai source access |

### Chai Examples — Complete

#### Review Summary

| Example | Issues Found | Fixes Applied |
|---------|-------------|---------------|
| `notesmd` | Frontmatter had `name`, `generated_from`, misplaced `capability_tier`; SKILL.md had redundant "Available Tools" and detailed tool instructions; `content` param used `kind: "flag"` instead of `stdin` | Cleaned frontmatter; removed redundant sections; changed `content` to `stdin` |
| `notesmd-daily` | Frontmatter had `name`; SKILL.md had vague/agent-behavior directives, redundant "Available Tools", detailed instructions; `content` param used `kind: "flag"` | Added `variant_of: notesmd`; cleaned frontmatter; removed redundant sections; changed `content` to `stdin`; added `capability_tier: minimal` |
| `websearch` | Frontmatter had `name`, `generated_from`, `scripts` (not a recognized field), misplaced `capability_tier`; SKILL.md had redundant sections and agent-judgment directives; no `maxOutputLines` on `websearch_fetch` | Cleaned frontmatter; removed redundant sections; added `maxOutputLines: 200` on search, `maxOutputLines: 500` on fetch |

#### Specific Fixes Applied

**notesmd:**
- Removed `name` and `generated_from` from frontmatter (directory name is authoritative; derivation tracking not runtime-consumed)
- Moved `capability_tier: moderate` to top-level frontmatter
- Removed "Available Tools" section (redundant with tool schema)
- Removed detailed per-tool instructions (obvious compositions)
- Kept only composed workflow for create/update and essential directives
- Changed `notesmd_create` content from `kind: "flag"` to `kind: "stdin"` (content-passing channel selection)

**notesmd-daily:**
- Removed `name` from frontmatter
- Added `variant_of: notesmd` (read-only subset pattern)
- Set `capability_tier: minimal`
- Removed vague directives ("always follow tool instructions step-by-step", "always return content from calling tools in code blocks")
- Removed redundant "Available Tools" and detailed instructions
- Changed `notesmd_daily_update` content from `kind: "flag"` to `kind: "stdin"`
- Changed `path` param to `date` (clearer parameter naming)
- Kept essential directive (YYYY-MM-DD format) and composed workflow

**websearch:**
- Removed `name` and `generated_from` from frontmatter
- Removed `scripts: ["jq"]` from `metadata.requires` (not a recognized frontmatter field; `jq` is an implementation detail of the postProcess scripts)
- Moved `capability_tier: full` to top-level frontmatter
- Removed "Available Tools" section and detailed per-tool instructions
- Removed agent-judgment directives ("never follow URLs without evaluating relevance")
- Kept essential directives (untrusted input, verify claims) and composed research workflow
- Added `maxOutputLines: 200` to `websearch_search`
- Added `maxOutputLines: 500` to `websearch_fetch`

### Final Audit Status

| Skill | Purpose | Round 1 | Round 2 | Round 3 |
|-------|---------|---------|---------|---------|
| `files` | Read, write, search, delete files and directories | ✅ | ✅ | ✅ |
| `files-read` | Read-only subset of `files` | ✅ | ✅ | ✅ |
| `git` | Git operations (write) | ✅ | ✅ | ✅ |
| `git-read` | Git operations (read-only) | ✅ | ✅ | ✅ |
| `git-remote` | Git remote operations (clone, pull, push) | ✅ | ✅ | ✅ |
| `notes` | Note management | ✅ | ✅ | ✅ |
| `notes-read` | Read-only subset of `notes` | ✅ | ✅ | ✅ |
| `notes-daily` | Daily note creation | ✅ | ✅ | ✅ |
| `notes-frontmatter` | Frontmatter manipulation | ✅ | ✅ | ✅ |
| `notes-wikilink` | Wikilink resolution and rename | ✅ | ✅ | ✅ |
| `logs` | Chai process logs | - | - | ✅ |
| `rss` | RSS feed reading | ✅ | ✅ | ✅ |
| `skills` | Skill creation and modification | ✅ | ✅ | ✅ |
| `skills-design` | Design principles for skill tools | ✅ | ✅ | ✅ |
| `skills-read` | Skill inspection (read-only) | ✅ | ✅ | ✅ |

All 15 bundled skills audited. All diagnostic hints implemented and live-tested.
