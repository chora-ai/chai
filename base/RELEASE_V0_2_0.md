# Release v0.2.0

**Status** — In-progress

## Scope

This release addresses changelog accuracy (including an undocumented breaking CLI change), audits all bundled skills against the tool and parameter naming ADR, and reviews structured and user documentation for currency.

### In Scope

- Changelog accuracy (missing entries, breaking changes)
- Skill audit against `adr/TOOL_PARAMETER_NAMING.md`
- Structured documentation review
- User documentation review

### Out of Scope

- All epics (see [RELEASE_V0_3_0.md](RELEASE_V0_3_0.md) for persistent sessions)

## Requirements

### 1. Changelog Accuracy and Breaking Change — Completed

All changelog gaps resolved. CLI subcommand and flag renames added to `### Changed` and `### Breaking Changes`; worker tool call index collision fix added to `### Fixed` → `#### Desktop`. Changelog structural conventions documented in `AGENTS.md`.

### 2. Skill Audit Against `adr/TOOL_PARAMETER_NAMING.md` — Completed

All 86 tools across 16 skill directories audited; tool naming, parameter naming, external binary flag alignment, chai CLI flag alignment, and `flagIfTrue` values all conform to the ADR. 34 hint scripts across 15 skills audited; all follow `hint: <message>` format, fire on appropriate conditions, and the DIAGNOSTIC_HINTS ADR is current. No deviations found, no fixes required.

### 3. Structured Documentation Review — Completed

All 12 structured documents (10 specs, 2 ADRs) reviewed against 15 commits that touched `base/` since v0.1.0; all changes properly graduated at the time of each commit, no additional updates needed.

### 4. User Documentation Review — Completed

6 issues found and fixed in `docs/guides/06-skills.md`: `git` tool count 10 → 20, `git-read` tool count 5 → 7, `notes` tool count 10 → 9, descriptions updated for git/git-read to reflect merge/rebase/cherry-pick/reset/line-range tools, duplicate word "for notes notes" → "for notes". All other user-facing docs verified current.

### 5. Release Mechanics

- [x] Update structured documentation per requirement 3 (no changes needed — all docs current)
- [x] Update user documentation per requirement 4 (6 issues fixed in `docs/guides/06-skills.md`)
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
