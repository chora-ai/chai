# Release v0.3.0

**Status** — Requirements gathering

## Scope

This release implements the persistent sessions epic.

### In Scope

- **Epic: `PERSISTENT_SESSIONS.md`** — Persist chat sessions to disk so they survive gateway restarts and desktop app restarts. Expose session management (listing, loading, deleting) through the gateway protocol and desktop UI.

### Out of Scope

- All other epics: `DESKTOP_FILES`, `PARALLEL_WORKFLOWS`, `SPLIT_DEPLOYMENT`, `TOOL_APPROVAL`, `MULTI_ORCHESTRATOR`

## Requirements

### 1. Persistent Sessions Epic Implementation

- [x] Implement Phase 1: Core persistence (per `PERSISTENT_SESSIONS.md`)
- [x] Implement Phase 2: Protocol methods (`sessions.list`, `sessions.history`, `sessions.delete`, `sessions.delete_all`)
- [x] Implement Phase 3: Desktop session management
- [x] Implement Phase 4: CLI session management (`chai sessions list`, `chai sessions delete`, `chai sessions clear`)

### 2. Release Mechanics

- [ ] Validate `scripts/build-release.sh 0.3.0` and experimental feature builds
- [ ] Write tag file `base/tag/V0_3_0.md` following `base/meta/TAG.md` format
- [ ] Update knowledge base index `base/README.md` — add new tag file entry using exact Summary text
- [ ] Update `CHANGELOG.md` — replace `## [Unreleased]` with `## [0.3.0] - YYYY-MM-DD`, add new `## [Unreleased]` heading
- [ ] Bump versions in all `Cargo.toml` files to `0.3.0`
- [ ] Update lockfile (`cargo update`)
- [ ] Delete this working document (`base/RELEASE_V0_3_0.md`)
- [ ] Commit to `main` with message `v0.3.0`
- [ ] Push new release branches to replace existing ones that use the old naming convention: create `release/v0.1.x` pointing at the same commit as `release/v0.1.0`, create `release/v0.2.x` pointing at the same commit as `release/v0.2.0`, push both new branches, then delete the old `release/v0.1.0` and `release/v0.2.0` branches locally and on origin
- [ ] Create release branch `release/v0.3.x`
- [ ] Create annotated tag: `git tag -a v0.3.0 -F base/tag/V0_3_0.md --cleanup=verbatim`
- [ ] Push `main`, release branch, and tag to origin
- [ ] Build release binaries (`scripts/build-release.sh 0.3.0`)
- [ ] Publish platform release notes using tag file contents
- [ ] Review `chai-examples` — verify example profiles and skills align with the release
- [ ] Tag `chai-examples` with `v0.3.0`
