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

**Background**: The frontmatter in `SKILL.md` currently carries a mix of fields serving different audiences and lifecycles. An audit of all 14 bundled skills reveals the following usage:

| Field | Present in | Purpose | Set by |
|-------|-----------|---------|--------|
| `name` | All 14 | Display/documentation; directory name is authoritative | Skill author |
| `description` | All 14 | Catalog and prompt description | Skill author |
| `metadata.requires.bins` | All 14 | Binaries the skill's allowlist needs | Skill author |
| `capability_tier` | All 14 | Minimum model capability for context budget and startup warnings | Skill author |
| `model_variant_of` | 4 (`files-read`, `git-read`, `git-remote`, `skills-read`) | Links variant skills to their full-tier counterpart | Skill author |
| `generated_from.spec_version` | All 14 | Format version used to produce this skill | Generator |
| `generated_from.generator_model` | All 14 | Model that produced this skill | Generator |
| `generated_from.cli` / `cli_version` | 3 (`git`, `git-read`, `git-remote`) | CLI source and version for derivation tracking | Generator |
| `recommended_models` | 0 of 14 | Models empirically tested against this skill's schema | Simulation |

**Questions to resolve**:

1. **Which fields are the skill author's concern vs. derived metadata?** The `generated_from` block is clearly derived — it records what produced the skill, not what the skill *is*. But `capability_tier` is a judgment call that sits at the boundary: the author sets it based on experience, but runtime behavior (context mode inference, startup warnings) depends on it. Should runtime behavior infer tier from the skill's tool surface instead, making the frontmatter field informational?

2. **Is `recommended_models` worth keeping?** It's in the spec but not populated in any bundled skill. It's informational only — no runtime behavior gates on it. If it's never populated and never consumed, it's dead weight in the spec and in context. Conversely, if simulation infrastructure arrives later, it could become valuable. What's the threshold for keeping an unpopulated field?

3. **Is `generated_from` serving its purpose?** Every bundled skill has this block, but the information is only useful during skill generation/derivation — not at runtime, not for the agent, and not for skill authors modifying existing skills. It consumes frontmatter lines on every context load. Could this be moved to a sidecar file or a manifest that's not loaded into the agent's context?

4. **How should frontmatter be maintained across the skill lifecycle?** When a skill author edits `SKILL.md` content, should they also update `generated_from`? When `chai init` extracts a bundled skill, does it overwrite the author's frontmatter or preserve it? There's no documented convention for this.

5. **Context cost of frontmatter**: Frontmatter is loaded into the LLM's context on every turn (it's part of `SKILL.md`). Fields that serve the runtime loader (like `metadata.requires.bins`, `capability_tier`) or the agent (like `description`) justify their cost. Fields that serve only the build/distribution pipeline (like `generated_from`) may not. Should frontmatter be split — a runtime-facing subset in `SKILL.md`, a build-facing subset in a separate file?

**Action**: Resolve these questions as part of the audit. When conventions become concrete, document them in `skills-design/SKILL.md` and update `spec/SKILL_FORMAT.md` accordingly. Remove `recommended_models` from the spec if it remains unpopulated after the audit, or populate it if simulation results are available.

**Source**: This finding was originally tracked as `FEAT_SKILL_MODE_FRONTMATTER.md`, which focused narrowly on adding `recommended_models` and verifying `capability_tier`/`model_variant_of` parsing. It has been absorbed here because the broader frontmatter question — what fields serve what purpose, who maintains them, and whether they belong in the agent's context at all — subsumes that narrower feature request.

## Skill-Specific Findings

### `files`

Audited during hands-on testing of `files_write_lines` verification (`original_content` check confirmed working; the resolved bug that implemented it was formerly tracked in this directory).

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

#### Historical measurement to replace

The SKILL.md sizing section includes a specific measurement: "The `files` skill SKILL.md was reduced from ~9.6KB to ~6.3KB with no loss of effectiveness by applying these cuts." This is a historical anecdote that will become stale. Replace with a forward-looking principle.

#### Missing principle: examples sizing

The sizing section doesn't address examples. Add guidance on when examples justify their context cost (see finding #2 above).

#### Missing principle: directive audit trigger

Add guidance: when adding a new directive, check whether the tool could enforce it instead; when a tool gains new behavior, check whether existing directives are now redundant. This makes "tools over inference" actionable as a maintenance discipline, not just an authoring principle.

#### Section placement: "Duplicated CLI Subcommands vs. Skill Tools"

This section is an implementation detail for skill authors, not a design principle that shapes skill behavior at runtime. Consider whether it belongs in `skills-design/SKILL.md` or in a separate authoring guide. No immediate action — flag for consideration.

#### Missing principle: frontmatter conventions

When frontmatter conventions are resolved (see cross-skill finding #5), concrete guidance should be added to `skills-design/SKILL.md` — specifically: which frontmatter fields are the skill author's concern vs. derived metadata, and how frontmatter should be maintained across the skill lifecycle (authoring, generation, validation).

## Audit Method

For each unaudited skill:

1. Read `SKILL.md` and `tools.json` side by side.
2. Identify redundancies between SKILL.md content and the tool schema.
3. Classify every directive (tool-enforceable, schema-communicated, genuinely additive).
4. Evaluate examples against the "worth the context cost" principle.
5. Review frontmatter fields: which serve the author, the runtime, the agent, or the build pipeline? Which justify their context cost?
6. Check for cross-skill patterns not yet captured above.
7. Record findings in this document under the appropriate section.
