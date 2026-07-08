# Audit: Skill Architecture

**Status** — Active

## Scope

This audit examines the architecture of chai's skill system — the declarative tool definitions (`tools.json`), skill instructions (`SKILL.md`), shell scripts (`scripts/`), and the runtime executor pipeline — to identify structural sources of complexity that make skills harder to build, maintain, and secure. All short-term improvements identified by this audit have been completed or tracked as separate features. This document now tracks the remaining long-term improvements.

## Findings

### 1. Read-Only Variant Duplication Remains

The skill system has 4 read-only variants (files-read, git-read, notes-read, skills-read) that are strict subsets of their parent skills. Each variant must contain a complete copy of the tools, allowlists, and execution specs it exposes, creating significant duplication.

The `notes` and `files` skills are intentionally separate — they serve different contexts and should remain independent. The duplication concern is limited to read-only variants, which are strict subsets with no unique logic of their own.

**Why this matters:** Every bug fix, hint improvement, or security patch must be applied to every variant copy. The hintConditions migration eliminated the worst hint-script duplication across variants, but the tools.json and execution spec duplication remains. L1 would allow variants to inherit from their parent skill instead of duplicating.

### 2. The `tools.json` Schema Has Accumulated Ad-Hoc Features

The `ArgMapping` structure in `descriptor.rs` has 15 optional fields. The `ExecutionSpec` has 12 optional fields. These fields were added incrementally to solve specific problems, and they interact in non-obvious ways:

| Feature | Added For | Interacts With |
|---|---|---|
| `absentDefault` | Defaulting count=10, recursive=true | `postProcess` args (must be augmented before substitution), `hintConditions` `whenArg` and `{param}` templates |
| `split` | Space-separated multi-value (git_add paths) | `positional` kind, `resolveCommand` |
| `kind: "tempfile"` | Multi-line pattern + stdin coexistence | Temp file lifecycle, `--flag` injection |
| `kind: "literal"` | --continue/--abort for rebase/cherry-pick | Subcommand resolution, argv ordering |
| `subcommandOverride` | force: true → branch -D instead of branch -d | `FlagIfBoolean`, `allowlist` |
| `disambiguateAfterSkippedPositionals` | git log/diff optional positional args | `--` separator, positional ordering |
| `denyPattern` + `denyResolveCommand` + `denyAlwaysResolve` | Branch protection | `resolveCommand`, `workingDir` |
| `condition.binGroup` + `binaryWrapper` | NixOS cargo support | `allowlist`, loader-time filtering |
| `successExitCodes` | grep exit 1, cargo exit 101, git exit 128 | `postProcess` (only runs on success-path codes), `hintConditions` `exitCode` (same gate), stderr merging |
| `hintConditions` | Inline declarative hints | `successExitCodes` (required for exitCode: "nonzero"), `postProcess` (runs after), `absentDefault` (augments args for whenArg/templates), `truncate_output` (preserves hint: lines) |
| `sideRead` | Auto-loading AGENTS.md | Session-scoped dedup, `maxOutputLines` ordering |
| `truncationHint` | Custom pagination notices | Template variables, hint-line preservation |

Each feature solves a real problem. The issue is that the interaction surface is growing faster than the individual features: `successExitCodes` + `postProcess` + `denyPattern` + `resolveCommand` on the same tool creates a 4-way interaction that the skill author must reason about correctly. This is a long-term concern addressed by L3 (deeper schema validation).

### 3. Shell Scripts Remain a Maintenance Burden

The most security-relevant are `resolve-daily-path.sh` (which hardcodes the sandbox root and includes a path-traversal guard but lacks the full symlink-resolution validation that `chai resolve` provides) and `resolve-current-branch.sh` (which runs `git branch --show-current` in an arbitrary working directory). Neither contains `is_inside_sandbox()` — that function now exists only in the `chai resolve` binary implementation.

Shell scripts still have no type system, no linting in the project, no test coverage, and no schema validation — the concerns from the original finding still apply to the remaining scripts, particularly `parse-rss.sh` (148 lines of fragile sed/awk XML parsing) and `resolve-daily-path.sh` (which implements its own traversal guard instead of using `chai resolve`). These remaining scripts are addressed by L2.

### 4. Complex `postProcess` Scripts Remain

The remaining 8 `postProcess` hint scripts are genuinely complex and cannot be expressed as inline conditions:

| Skill | Script | Why It Cannot Use `hintConditions` |
|---|---|---|
| cargo | `hint-check.sh` | Filters progress lines, collapses blank lines, classifies errors/warnings — output transformation |
| cargo | `hint-test.sh` | Heavy output transformation: filters progress/passing lines, 4-branch classification |
| skills | `hint-skill-md-checks.sh` | Reads a file from disk, parses YAML frontmatter, checks naming — mini-linter |
| skills | `hint-validate-on-write.sh` | Runs `chai skill validate` externally, parses output — external command |
| skills-read | `hint-path-annotations.sh` | Parses tools.json from stdin, counts annotated vs total params — structured data parsing |
| logs | `hint-many-matches.sh` | Extracts and compares a number (`> 15`) — arithmetic comparison |
| notes-wikilink | `sanitize-outlinks.sh` | Full output transformation: sanitizes output, checks filesystem for each link target |
| notes-wikilink | `build-backlink-pattern.sh`, `check-broken-links.sh`, `normalize-tag.sh` | Resolve/process scripts (not hint-only), filesystem traversal, structured data |

These scripts are candidates for L2 (move shell logic to binary) or for future `hintConditions` extensions (e.g., `matchRegex` for regex matching, or numeric comparison operators).

### 5. The Binary vs. Skill Boundary Is Not Always in the Right Place

After implementing the resolve-script migration, the principle is established: deterministic work and complex data transformations belong in the binary; skill-level scripts handle only context-dependent path resolution and output inspection for hints. The remaining misaligned scripts (`parse-rss.sh`, `resolve-daily-path.sh`) are addressed by L2.

### 6. The Instruction Surface (SKILL.md) Is Well-Managed but Fragile

The directives audit reduced 39 directives to 14 hard directives (64% reduction). 11 directives were deleted (already enforced), 14 were moved to guidelines (workflow guidance), and 1 enforcement was added (`hintConditions` on delete tools). A second pass should verify the remaining directives and identify new candidates for enforcement or elimination.

## Long-Term Improvements

These improvements require architectural changes.

### L1: Skill Inheritance for Read-Only Variants

**Problem:** Read-only variants (files-read, git-read, notes-read, skills-read) are strict subsets of their parent skills. Each variant must contain a complete copy of the tools, allowlists, and execution specs it exposes, creating duplication across all variants.

**Proposal:** Introduce a skill composition system where a read-only variant can declare that it extends its parent skill:

```yaml
---
description: Read files, list directories, and search file contents (read-only).
capability_tier: minimal
extends: files
include: [files_read, files_read_lines, files_list, files_search]
---
```

The loader would:
1. Load the parent skill's `tools.json`, `allowlist.json`, and `execution.json`
2. Filter tools and execution specs to only those in `include`; filter allowlist to only the (binary, subcommand) pairs used by the included execution specs
3. Merge the variant's `SKILL.md` directives (replacing the parent's)
4. Apply the variant's `capability_tier`
5. Share the parent skill's scripts directory

This eliminates the need for separate `tools.json`, `allowlist.json`, and `execution.json` files for read-only variants. Each read-only variant would become a SKILL.md-only file.

**Scope boundary:** This applies only to read-only variants — skills that expose a strict subset of another skill's tools with no unique logic of their own. The `notes` and `files` skills are intentionally separate and will remain independent; they serve different contexts and should not depend on each other.

**Complexity reduction:**
- 4 read-only variant skills reduced to SKILL.md-only files
- ~1,500 lines of duplicated tools.json and scripts eliminated
- Single source of truth for the parent skill's tool definitions

**Risk:** High. This changes the skill loading model, the versioning system (which version hash applies when a variant inherits tools from a parent?), and the lockfile semantics. The inheritance system only needs to compose `tools.json` entries while sharing `allowlist.json` and `execution.json` — a simpler model than composing a single monolithic `tools.json`.

### L2: Move Shell-Heavy Logic into the Binary

**Problem:** Some shell scripts implement logic that is better suited to the Rust binary — particularly RSS/Atom parsing (`parse-rss.sh`, 148 lines of fragile sed/awk), output filtering (`hint-check.sh`, `hint-test.sh` which grep out progress lines), and wikilink resolution (`check-broken-links.sh`, `sanitize-outlinks.sh` which walk the filesystem).

**Proposal:** Extend the `chai` binary with subcommands for operations that are currently implemented in shell:

1. **`chai rss parse`** — Parse RSS/Atom XML from stdin, output structured entries. Handles XML properly, produces consistent output, and handles malformed feeds gracefully.
2. **`chai filter`** — Filter tool output by removing progress lines, collapsing blank lines, extracting diagnostics. Replaces the progress-filtering logic in `hint-check.sh` and `hint-test.sh`.
3. **`chai wikilink check-broken`** / **`chai wikilink sanitize-outlinks`** — Wikilink operations that require filesystem walks.

The principle: scripts should only handle simple text inspection (pattern matching for hints) and path resolution. Any operation that requires structured data parsing, filesystem traversal, or complex transformation should be a binary subcommand.

**Complexity reduction:**
- ~350 lines of fragile shell code replaced by robust Rust implementations
- RSS parsing gains proper XML handling and error recovery
- Output filtering becomes reusable across skills
- Skills that need these operations just add the subcommand to their allowlist

**Risk:** Medium. Each new binary subcommand increases the binary size and must be maintained. However, the existing shell scripts are already security-relevant and untested — moving them into Rust improves both correctness and testability. The principle is already established: `chai file` contains 2981 lines of binary-side resilience logic.

### L3: Schema Validation Beyond Syntax

**Problem:** `skills_validate` checks syntax and structural conformance (required keys, tool-execution cross-references, allowlist membership). It does not check for security issues (missing path annotations, unguarded resolve scripts) or semantic issues (unused parameters, conflicting directives, missing hints for common error conditions).

**Proposal:** Extend `skills_validate` with optional deeper checks:

1. **Security audit mode** (`--audit`): Runs the security checklist from skills-design SKILL.md programmatically — checks every parameter for path annotations, flags `unsafePath` usage, detects `resolveCommand` scripts that produce paths without `readPath`/`writePath`, and identifies tools with `workingDir` that lack upward-traversal validation.
2. **Completeness check** (`--lint`): Detects tools that lack `maxOutputLines` on potentially unbounded output, missing `truncationHint` when a companion pagination tool exists, `postProcess` scripts that exist but have no `successExitCodes` for the error conditions they check, and parameters that appear in tool definitions but not in execution args (or vice versa).
3. **Dry-run validation** (`--dry-run`): Given sample tool calls, shows the constructed argv, sandbox validation results, and deny pattern checks.

**Complexity reduction:**
- Security audits become automated instead of manual
- Common skill authoring mistakes are caught at validation time
- The skills-read SKILL.md's "Security Audit" and "Cross-Validation" workflows become partially automated

**Risk:** Low. These are additive checks that don't change the runtime. The `--audit` mode may produce false positives (flagging `unsafePath` that is legitimately justified), but these are warnings, not errors. After the S6 descriptor split, the standalone `allowlist.json` becomes a natural target for security-focused validation — the audit mode can validate the allowlist in isolation without parsing the full `tools.json`, and the completeness check can validate cross-file consistency across the three files.

## Additional Notes

- [ ] "The `chai skill write-*` commands copy the current active tree, apply your change, compute the new hash, and repoint `active`" - how to make this easier? "There is no `chai skill hash` command today; use a small script or reproduce the algorithm from `versioning.rs`."
- [ ] Is there a convention for the ordering of parameters within a tool call? Should there be? For example, should the parameters reflect the order of the arguments and flags so they are more intuitive? Should parameters that are specific to the tool and environment such as "path" and "repo" always appear first or last? Should there be more consistency in the order across all tool calls?
