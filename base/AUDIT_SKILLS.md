# AUDIT: Bundled Skills Review

## Status

**Active** — initial findings from `files` skill usage; full audit of all bundled skills pending.

## Purpose

This document tracks a cross-skill audit of all bundled skills in `chai/crates/lib/config/skills/`. The goal is to identify improvements that apply to individual skills or across all bundled skills, guided by the design principles in `skills-design/SKILL.md`.

## Bundled Skills

| Skill | Purpose | Audited |
|-------|---------|---------|
| `files` | Read, write, search, delete files and directories | ✅ Initial pass |
| `files-read` | Read-only subset of `files` | ❌ |
| `git` | Git operations (write) | ❌ |
| `git-read` | Git operations (read-only) | ❌ |
| `git-remote` | Git remote operations (push, etc.) | ❌ |
| `kb` | Knowledge base management | ❌ |
| `kb-daily` | Daily note creation | ❌ |
| `kb-frontmatter` | Frontmatter manipulation | ❌ |
| `kb-wikilink` | Wikilink resolution (read) | ❌ |
| `kb-wikilink-write` | Wikilink creation and modification | ❌ |
| `rss` | RSS feed reading | ❌ |
| `skills` | Skill creation and modification | ❌ |
| `skills-design` | Design principles for skill tools | ✅ Initial pass |
| `skills-read` | Skill inspection (read-only) | ❌ |

## Cross-Skill Findings

These findings may apply to multiple or all bundled skills.

### 1. SKILL.md redundancy with tool schema

**Principle**: Don't repeat what the tool schema (`tools.json`) already communicates. The schema tells the agent parameter names, types, required/optional status, and descriptions. Repeating these in SKILL.md is context waste.

**Observed in `files`**: The "Tool Instructions" section restates parameter semantics (1-indexed, inclusive, `./`-relative paths, required vs optional) that the schema already provides. The opening paragraph repeats the frontmatter `description` field.

**Action**: For each skill, compare SKILL.md content against `tools.json` parameter descriptions. Remove restatements. Keep only content the schema cannot express (workflow guidance, preferences, non-obvious constraints).

### 2. When examples are worth their context cost

**Principle**: Examples are expensive — they consume context on every turn. They are worth the cost when they demonstrate composed workflows or non-obvious parameter relationships that the schema alone cannot convey. They are not worth the cost for single-parameter calls that the schema already makes clear.

**Observed in `files`**: The examples section was the most useful part in practice — specifically the `files_write_lines` examples showing the read-then-verify workflow and the delete-lines-by-empty-content pattern. But some examples (e.g., `files_read_file`, `files_delete_file`) are trivially inferable from the schema.

**Action**: For each skill with examples, evaluate each example against this principle. Remove trivial examples. Keep composed-workflow examples and examples showing non-obvious parameter combinations.

### 3. Directive enforceability audit

**Principle**: When adding a directive to SKILL.md, check whether the tool could enforce it instead. When a tool gains a new behavior, check whether existing directives are now redundant. This is a specific application of "tools over inference" — make it a conscious step, not an afterthought.

**Action**: For each skill, review every directive in SKILL.md. Classify each as:
- **Tool-enforceable** — the tool could check this (candidate for removal from SKILL.md, add enforcement to the tool)
- **Schema-communicated** — the parameter description or type already conveys this (candidate for removal)
- **Genuinely additive** — a preference or constraint the schema/tool cannot express (keep)

### 4. SKILL.md opening paragraph

**Pattern**: Several bundled skills open with a paragraph that restates the frontmatter `description` field and/or lists the tools in the skill. Both are redundant — the description is already in context via the frontmatter, and the tool list is already in context via the API schema.

**Action**: Remove opening paragraphs that only restate frontmatter descriptions or enumerate tool names.

### 5. Frontmatter: what serves what purpose, and who maintains it

**Resolved.** The frontmatter has been simplified to include only runtime-consumed fields. The `name` field, `generated_from` block, and `recommended_models` have been removed. `capability_tier` has been promoted from inside `generated_from` to a top-level field.

**Resolution summary**:

| Decision | Rationale |
|----------|-----------|
| Removed `name` | Directory name is authoritative; no code path requires it from frontmatter |
| Removed `generated_from` block (all sub-fields) | No sub-field was consumed at runtime: `spec_version`, `generator_model`, `cli`, and `cli_version` were parsed but never read; `capability_tier` was the only consumed sub-field and has been promoted to top-level |
| Promoted `capability_tier` to top-level | Was buried inside a "derivation metadata" block despite being the only field consumed at runtime (for startup validation warnings). The spec already defined it as a top-level field; the actual data was in the wrong place |
| Removed `recommended_models` from spec | Zero skills populated it; no code parsed it; no runtime behavior depended on it. If simulation infrastructure arrives later, add it then |
| Kept `description`, `capability_tier`, `model_variant_of`, `metadata.requires.bins` | All are consumed at runtime by the loader, gateway, or validation code |

**Code changes**: Updated `SkillFrontmatter` struct in `loader.rs` (removed `name` and `generated_from` fields, added top-level `capability_tier`). Updated `parse_skill_frontmatter` to read `capability_tier` from top-level instead of from `generated_from`. Removed `GeneratedFrom` struct entirely.

**Spec changes**: Updated `spec/SKILL_FORMAT.md` frontmatter table to reflect only the four runtime fields. Added examples showing the minimal and variant frontmatter shapes. Removed the "Derivation Metadata" section.

**Skill changes**: Updated all 14 bundled skill SKILL.md frontmatters. Updated `skills` and `skills-read` SKILL.md body content to reference the new frontmatter shape. Added "Frontmatter Conventions" section to `skills-design/SKILL.md`.

**Source**: This finding was originally tracked as `FEAT_SKILL_MODE_FRONTMATTER.md`, which focused narrowly on adding `recommended_models` and verifying `capability_tier`/`model_variant_of` parsing. It has been absorbed here because the broader frontmatter question — what fields serve what purpose, who maintains them, and whether they belong in the agent's context at all — subsumed that narrower feature request.

### 6. Content-passing channel audit: `flag` vs `stdin` vs `envvar`

**Background**: The bug that broke `files_write_lines` for content containing backticks, middle dots, and ampersands was caused by passing `original_content` as a CLI flag. CLI arguments are subject to environment-specific interpretation (shell quoting, encoding) that can introduce byte-level mismatches with the original JSON value. The fix introduced `ArgKind::EnvVar`, which passes the value as an environment variable instead — bypassing the shell argument layer entirely.

**Principle**: Content-rich parameters — those carrying arbitrary text, multi-line values, or text likely to contain special characters (backticks, quotes, ampersands, unicode) — should never be passed as CLI flags. The available channels, in order of reliability for content:

1. **`stdin`** — most reliable for arbitrary content; no encoding or length limits from the OS; already used for `content` in `files_write_file`, `files_write_lines`, and `skills` write tools.
2. **`envvar`** — reliable for large or special-character content; avoids shell argument interpretation; subject to OS environment variable size limits (typically ≥128KB on Linux, may be smaller on other platforms).
3. **`flag`** — only safe for short, controlled values (paths, identifiers, booleans, numbers); vulnerable to quoting and encoding issues in the LLM JSON → gateway → CLI → OS chain.

**Affected tools currently passing content as `flag`**:

| Skill | Tool | Parameter | Risk | Recommended channel |
|-------|------|-----------|------|---------------------|
| `kb` | `kb_write` | `content` | **High** — arbitrary file content; will contain markdown, code, special chars | `stdin` |
| `kb` | `kb_append` | `content` | **High** — same as `kb_write` | `stdin` |
| `kb-daily` | `kb_daily_write` | `content` | **High** — same as `kb_write` | `stdin` |
| `kb-daily` | `kb_daily_append` | `content` | **High** — same as `kb_write` | `stdin` |
| `git` | `git_commit` | `message` | **Medium** — commit messages can contain quotes, backticks, ampersands | `envvar` or `stdin` (via `git -F -`) |
| `git-remote` | `git_commit` | `message` | **Medium** — same as `git` | `envvar` or `stdin` (via `git -F -`) |
| `kb-frontmatter` | `kb_frontmatter_edit` | `value` | **Low–Medium** — frontmatter values are typically short strings but can contain URLs with special chars | `envvar` |
| `skills` | `skills_init` | `description` | **Low** — short descriptive string, unlikely to contain problem chars | `flag` (acceptable) |

**Action**:
- Migrate `kb_write`/`kb_append` and `kb_daily_write`/`kb_daily_append` `content` from `flag` to `stdin`, matching the pattern already used by `files_write_file` and `files_write_lines`. This requires updating both `tools.json` and the CLI subcommands (`chai file write`, `chai file append`) to read content from stdin.
- Migrate `git_commit` `message` from `flag` (`-m`) to either `envvar` (`CHAI_COMMIT_MESSAGE`) or `stdin` (using `git commit -F -`). The `envvar` approach is simpler; the `stdin` approach pipes the message via `git commit -F -` which reads from stdin.
- Consider migrating `kb_frontmatter_edit` `value` to `envvar` as a lower-priority improvement.
- ~~Add guidance to `skills-design/SKILL.md` about choosing the correct content-passing channel when authoring tools.~~ **Done** — added "Content-Passing Channel Selection" section.

### 7. Unbounded tool output can exceed context length and terminate sessions

**Principle**: Tool invocations that return arbitrarily large results (e.g. searching across a large codebase) can produce output that exceeds the model's context window, causing the session to crash. This is a "tools over inference" issue — the tool should enforce bounds rather than relying on the agent to predict result sizes and pre-limit queries.

**Observed**: A `files_search_content` call with a broad pattern against a large directory tree returned enough matching lines to exceed the context window, terminating the session with no opportunity for recovery. The agent had no way to anticipate the result size before making the call.

**Affected tools**:

| Skill | Tool | Risk | Notes |
|-------|------|------|-------|
| `files` | `files_search_content` | **High** — recursive grep across a large tree with a broad pattern can return thousands of lines | No result limit exists today |
| `files` | `files_read_file` | **Medium** — reading a very large file could exceed context | No size limit exists today |
| `kb` | `kb_read` | **Low–Medium** — knowledge base entries are typically sized by authors, but could be large | Depends on entry size |
| `kb` | `kb_search` | **Medium** — same class of risk as `files_search_content` | No result limit exists today |

**Mitigation options**:

1. **Hard result cap**: Tools that return unbounded output should enforce a maximum number of result lines (e.g. 200 lines for search tools, 500 lines for file reads). When the cap is hit, the tool returns the truncated results plus a message indicating truncation — allowing the agent to narrow its query or read specific line ranges.
2. **Advisory parameters**: Add optional `max_lines` or `max_results` parameters so the agent can request a limit. However, this alone is insufficient — an agent won't think to set it until it's been burned by an oversized result. A hard cap is needed as a safety net.
3. **Pre-flight size check**: For file reads, the tool could check file size before reading and refuse or truncate files above a threshold. This is a tool-enforceable version of the "never read binary files" and "never assume a file exists" directives — make the tool protect itself.
4. **Result truncation with continuation**: Return the first N lines plus a continuation token or hint (e.g. "results truncated at line 200 of 1,847; use `files_search_content` with a narrower path or pattern, or use `files_read_lines` for specific ranges").

**Action**:
- Implement a hard result cap for `files_search_content` (highest risk tool). Start with a sensible default (e.g. 200 lines of output) and return a truncation notice when exceeded.
- Evaluate hard caps for `files_read_file` (files above a size threshold should require `files_read_lines` instead).
- ~~Add guidance to `skills-design/SKILL.md`: tools returning potentially unbounded output must enforce a result cap and communicate truncation to the agent.~~ **Done** — added "Unbounded Output Protection" section.
- Classify any existing "preference" directives about result size as tool-enforceable, and migrate them into tool behavior.

## Skill-Specific Findings

### `files`

Audited during hands-on testing of `files_write_lines` verification (`original_content` check confirmed working; the resolved bug was formerly tracked as `BUG_WRITE_LINES_BACKTICK_MISMATCH.md` in this directory, then deleted after manual verification confirmed the fix).

#### Unicode normalization in `original_content` verification

The `files_write_lines` `original_content` check can fail even when the content appears visually identical, due to Unicode normalization differences between what the agent sends and what is stored on disk. For example, the Unicode right single quotation mark (U+2019, `'`) used in markdown files (e.g. "Ollama\u2019s") may be normalized to a different byte sequence than the ASCII apostrophe (U+0027, `'`), causing a 1–2 byte length mismatch in the verification step. The `verify_original()` error message reports the byte length difference as a hint, which is how this class of issue surfaces.

**Observed instances:**

- `base/ref/OLLAMA.md` contained a Unicode right single quotation mark (U+2019) in "Ollama\u2019s" on two lines. When an agent copies visible text into `original_content`, it may send the ASCII apostrophe instead, resulting in a byte-level mismatch. The error shows "different lengths — expected N bytes, actual M bytes" with a 1–2 byte delta.

**Workaround:** When `files_write_lines` fails with a byte-length mismatch on `original_content`, re-read the exact target lines with `files_read_lines` (which returns content with line numbers) and use the returned content verbatim — do not retype or normalize it. This ensures the byte sequence matches the file exactly. Alternatively, use `files_read_file` to get the full file and extract the relevant lines, then pass them without modification.

**Broader concern:** Any content-passing channel that involves the LLM generating string content (as opposed to copying it from tool output) is susceptible to Unicode normalization differences. The `envvar` channel avoids shell-quoting issues but does not prevent the LLM from generating a differently-normalized version of the same text. This is fundamentally a "tools over inference" issue — the more the agent copies verbatim from tool output rather than generating text from scratch, the fewer normalization mismatches will occur.

#### Redundancies to remove

- **Opening paragraph**: Restates frontmatter description and lists tools — both already in context.
- **Tool Instructions subsections**: The step-by-step procedures for `files_read_file`, `files_read_lines`, `files_list_dir`, `files_search_content`, `files_write_file`, `files_delete_file`, and `files_delete_dir` largely restate what the parameter descriptions in `tools.json` already communicate. The genuinely additive content should be extracted into directives.

#### Content to keep or extract

- **Write specific lines workflow** (read → get `original_content` → write) — the most valuable instruction, not expressed anywhere in the schema. Keep as a directive or a concise workflow note.
- **Bottom-to-top rule for multiple edits** — a usage preference the schema can't express. Keep.
- **Prefer `files_read_lines` over `files_read_file`** for partial reads — a preference, keep as directive.
- **Prefer `files_write_lines` over `files_write_file`** for targeted edits — a preference, keep as directive.
- **Prefer large contiguous rewrites** over boundary edits — a preference, keep.
- **ERE regex capabilities** — the schema says "extended regex supported" but doesn't enumerate what that enables. Keep a condensed version.
- **`files_search_content` → `files_read_lines` workflow** — after searching with line numbers, read surrounding context. Keep as directive.

#### Directives to evaluate for enforceability

| Directive | Classification | Notes |
|-----------|---------------|-------|
| always use `./` prefix | Schema-communicated | Parameter descriptions say "use ./ prefix" |
| always set `line_numbers` to true when searching | Genuinely additive | Preference, keep |
| never assume a file exists — verify first | Tool-enforceable | Tool could check existence and return a clear error |
| never read binary files | Tool-enforceable | `cat` already errors on binaries; tool could detect and refuse |
| always read before overwriting with `files_write_file` | Tool-enforceable | Tool could require a state token or confirmation for existing files |
| prefer `files_read_lines` over `files_read_file` | Genuinely additive | Preference, keep |
| prefer `files_write_lines` over `files_write_file` | Genuinely additive | Preference, keep |
| always provide `original_content` | Schema-communicated | It's a required parameter; the schema enforces this |
| prefer large contiguous rewrites | Genuinely additive | Preference, keep |
| work bottom-to-top for multiple edits | Genuinely additive | Workflow guidance, keep |

### `skills-design`

#### ~~Historical measurement to replace~~

**Done.** Replaced the stale "~9.6KB to ~6.3KB" measurement with a forward-looking "Examples Sizing" subsection under SKILL.md Sizing.

#### ~~Missing principle: examples sizing~~

**Done.** Added "Examples Sizing" subsection to `skills-design/SKILL.md`.

#### ~~Missing principle: directive audit trigger~~

**Done.** Added "Directive Audit" subsection to `skills-design/SKILL.md`.

#### ~~Section placement: "Duplicated CLI Subcommands vs. Skill Tools"~~

**Done.** Reframed as "CLI Subcommands Are Shared" — a concise design principle rather than an implementation note.

#### ~~Missing principle: content-passing channel selection~~

**Done.** Added "Content-Passing Channel Selection" section to `skills-design/SKILL.md`.

#### ~~Missing principle: frontmatter conventions~~

**Done.** Added "Frontmatter Conventions" section to `skills-design/SKILL.md` after resolving finding #5.

## Audit Method

For each unaudited skill:

1. Read `SKILL.md` and `tools.json` side by side.
2. Identify redundancies between SKILL.md content and the tool schema.
3. Classify every directive (tool-enforceable, schema-communicated, genuinely additive).
4. Evaluate examples against the "worth the context cost" principle.
5. Review frontmatter fields: which serve the author, the runtime, the agent, or the build pipeline? Which justify their context cost?
6. Audit content-passing channels: for each parameter with `kind: "flag"`, evaluate whether it carries arbitrary text or special-character-prone content. Flag high-risk parameters and recommend `stdin` or `envvar` per finding #6.
7. Check for cross-skill patterns not yet captured above.
8. Record findings in this document under the appropriate section.
