# Release v0.2.0

**Status** — Requirements gathering

## Scope

This release addresses changelog accuracy (including an undocumented breaking CLI change), audits all bundled skills against the tool and parameter naming ADR, and reviews structured and user documentation for currency.

### In Scope

- Changelog accuracy (two missing entries, one of which is a breaking change)
- Skill audit against `adr/TOOL_PARAMETER_NAMING.md`
- Structured documentation review
- User documentation review

### Out of Scope

- All epics (see [RELEASE_V0_3_0.md](RELEASE_V0_3_0.md) for persistent sessions)

## Requirements

### 1. Changelog Accuracy and Breaking Change

The `## [Unreleased]` changelog has two gaps that must be resolved before tagging:

#### 1a. Missing CLI breaking change from `395696d` (fix: align CLI flags with ADR parameter naming conventions)

The fix commit renamed CLI subcommands and flags to align with `adr/TOOL_PARAMETER_NAMING.md`. These are **breaking changes for CLI users** that are not reflected in the changelog:

- **Subcommand rename**: `chai file rename-note` → `chai file rename`
- **Flag renames**:
  - `git diff-lines`: `--path` → `--repo` (repo root), `--file-path` → `--path` (file within repo)
  - `git show-lines`: `--path` → `--repo` (repo root)
  - `file rename`: `--root` → `--scope`
  - `file replace`: `--line-numbers` → `--line-number`

The changelog's `### Changed` section mentions *tool parameter* renames but not these *CLI subcommand and flag* renames. Since v0.2.0 is a minor bump (pre-1.0), breaking changes are allowed, but they **must be documented** — both in the `### Changed` section and in a `### Breaking Changes` section (or equivalent).

- [ ] Add CLI subcommand rename (`rename-note` → `rename`) to changelog `### Changed`
- [ ] Add CLI flag renames to changelog `### Changed`
- [ ] Add a `### Breaking Changes` section or clearly mark these as breaking in the changelog
- [ ] Verify the tag file for v0.2.0 includes a `## Breaking Changes` section listing these CLI renames

#### 1b. Missing fix from `795c71a` (fix: prevent worker tool call index collisions across delegations)

This commit fixed a bug where the orchestrator's `tool_index_offset` only accounted for orchestrator-level tool calls, not worker tool calls from previous delegations. This caused overlapping indices that triggered the desktop's duplicate detection, silently dropping worker tool call events. The changelog does not mention this fix.

- [ ] Add a changelog entry under `### Fixed` for the worker tool call index collision fix (in a new `#### Orchestration` subsection)

### 2. Skill Audit Against `adr/TOOL_PARAMETER_NAMING.md`

The ADR `TOOL_PARAMETER_NAMING.md` was added in `eddf391` and updated in `395696d`. It establishes naming conventions for all bundled skills. Since the implementation spanned two commits (initial rename + CLI flag alignment fix), there is a risk that some skills or CLI subcommands still deviate from the conventions.

Audit all bundled skills against the ADR:

- [ ] Verify all tool names follow `{skill}_{verb}` (noun suffix only for disambiguation)
- [ ] Verify all sub-skill tool names follow `{skill}_{subskill}_{verb}`
- [ ] Verify parameter naming: `path` (target), `repo` (git root), `scope` (directory narrowing), `{domain}_name` (qualified identifiers), plural for multi-value
- [ ] Verify numeric parameters use `integer` type
- [ ] Verify external binary flags align to the binary's flag names (e.g., `--ignore-case` → `ignore_case`)
- [ ] Verify chai CLI flags align to ADR conventions (e.g., `--repo` not `--path` for repo root, `--scope` not `--root` for search directory)
- [ ] Verify the `tools.json` `flagIfTrue` values match the actual CLI flag names after the renames
- [ ] Report any remaining deviations and fix them

### 3. Structured Documentation Review

Ensure all specs, ADRs, and other structured docs in `base/` are current with changes merged since v0.1.0.

#### Specs to review

- [ ] `spec/TOOLS_SCHEMA.md` — Confirm naming conventions section matches the ADR (was updated in both `eddf391` and `395696d`; verify consistency)
- [ ] `spec/SANDBOX.md` — Confirm tool name references are current (was updated in `eddf391` to use `files_write` instead of `files_write_file`)
- [ ] `spec/ORCHESTRATION.md` — Confirm tool event index semantics section is current (was updated in `795c71a` to document the offset fix)
- [ ] `spec/DESKTOP.md` — Confirm tool event deduplication and worker reply rendering are current (was updated in `795c71a`)
- [ ] `spec/SKILL_FORMAT.md` — Confirm schema additions are documented (`binaryWrapper`, `condition`, OR-group bins, `split`, `absentDefault`, `truncationHint`, `subcommandOverride`, `kind: literal/tempfile`)
- [ ] `spec/CONTEXT.md` — Confirm no changes needed
- [ ] `spec/CHANNELS.md` — Confirm no changes needed
- [ ] `spec/PROFILES.md` — Confirm no changes needed
- [ ] `spec/AGENTS.md` — Confirm no changes needed
- [ ] `spec/CONFIGURATION.md` — Confirm no changes needed

#### ADRs to review

- [ ] `adr/TOOL_PARAMETER_NAMING.md` — Confirm it is current after the `395696d` update (directional flag alignment rule, chai binary flag alignment)
- [ ] `adr/WRITE_SANDBOX.md` — Confirm tool name references are current (was updated in `eddf391`)

### 4. User Documentation Review

Ensure `docs/` and `README.md` are current with changes merged since v0.1.0.

- [ ] `docs/guides/06-skills.md` — Confirm tool names and parameter names are current after renames
- [ ] `docs/guides/07-sandbox.md` — Confirm `.git/` exclusion is documented (was updated in `b5cfb98`); confirm tool name references are current
- [ ] `docs/guides/08-cli-reference.md` — Confirm all CLI subcommand names and flag names are current after renames (`rename-note` → `rename`, `--line-numbers` → `--line-number`, `--root` → `--scope`, git `--path` → `--repo`, git `--file-path` → `--path`). Was updated in `395696d` but should be verified
- [ ] `docs/guides/09-desktop.md` — Confirm no changes needed
- [ ] `docs/guides/11-troubleshooting.md` — Confirm no changes needed
- [ ] `README.md` — Confirm no changes needed (or update if CLI subcommands are mentioned)
- [ ] `CHANGELOG.md` — Final accuracy review after requirements 1a and 1b are addressed

### 5. Release Mechanics

- [ ] Update structured documentation per requirement 3
- [ ] Update user documentation per requirement 4
- [ ] Validate experimental feature builds
- [ ] Write tag file `base/tag/V0_2_0.md` following `base/meta/TAG.md` format
- [ ] Update `CHANGELOG.md` — replace `## [Unreleased]` with `## [0.2.0] - YYYY-MM-DD`, add new `## [Unreleased]` heading
- [ ] Bump versions in all `Cargo.toml` files to `0.2.0`
- [ ] Update lockfile (`cargo update`)
- [ ] Delete this working document (`base/RELEASE_V0_2_0.md`)
- [ ] Commit to `main` with message `v0.2.0`
- [ ] Create release branch `release/v0.2.0`
- [ ] Create annotated tag: `git tag -a v0.2.0 -F base/tag/V0_2_0.md --cleanup=verbatim`
- [ ] Push `main`, release branch, and tag to origin
- [ ] Build release binaries (`scripts/build-release.sh 0.2.0`)
- [ ] Publish platform release notes using tag file contents
- [ ] Review `chai-examples` — verify example profiles and skills align with the release
- [ ] Tag `chai-examples` with `v0.2.0`
