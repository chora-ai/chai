# Release v0.3.0

**Status** — Requirements gathering

## Scope

This release implements the persistent sessions epic and reviews all documentation for currency.

### In Scope

- **Epic: [PERSISTENT_SESSIONS.md](epic/PERSISTENT_SESSIONS.md)** — Persist chat sessions to disk so they survive gateway restarts and desktop app restarts. Expose session management (listing, loading, deleting) through the gateway protocol and desktop UI.

### Out of Scope

- All other epics: `DESKTOP_FILES`, `PARALLEL_WORKFLOWS`, `SPLIT_DEPLOYMENT`, `TOOL_APPROVAL`, `MULTI_ORCHESTRATOR`

## Requirements

### 1. Persistent Sessions Epic Implementation

- [ ] Implement Phase 1: Core persistence (per `epic/PERSISTENT_SESSIONS.md`)
- [ ] Implement Phase 2: Protocol methods (`sessions.list`, `sessions.history`, `sessions.delete`, `sessions.delete_all`)
- [ ] Implement Phase 3: Desktop session management
- [ ] Implement Phase 4: CLI session management (`chai sessions list`, `chai sessions delete`, `chai sessions clear`)
- [ ] Implement Phase 5: Hardening

### 2. Structured Documentation Review

Ensure all specs, ADRs, and other structured docs in `base/` are current with the persistent sessions implementation.

- [ ] `spec/CONTEXT.md` — Update for session persistence (new `created_at`, `updated_at` fields, `Serialize`/`Deserialize` on `Session`)
- [ ] `spec/CHANNELS.md` — Update for session binding persistence (`bindings.json`)
- [ ] `spec/PROFILES.md` — Update for `sessions/` directory under agent context directories
- [ ] `spec/AGENTS.md` — Update if session storage affects agent context directories
- [ ] `spec/CONFIGURATION.md` — Update if session persistence adds configuration options
- [ ] `spec/DESKTOP.md` — Update for session sidebar enhancements, session loading, delete/clear actions
- [ ] `spec/ORCHESTRATION.md` — Confirm no changes needed (sessions are orchestrator-scoped)
- [ ] `spec/TOOLS_SCHEMA.md` — Confirm no changes needed
- [ ] `spec/SANDBOX.md` — Confirm no changes needed
- [ ] `spec/SKILL_FORMAT.md` — Confirm no changes needed
- [ ] `adr/RUNTIME_PROFILES.md` — Confirm no changes needed (already notes session state is torn down on restart; this epic addresses that gap)
- [ ] Review all other ADRs and specs for any needed updates

### 3. User Documentation Review

Ensure `docs/` and `README.md` are current with the persistent sessions implementation.

- [ ] `docs/guides/09-desktop.md` — Update for session sidebar enhancements, session loading, delete/clear
- [ ] `docs/guides/08-cli-reference.md` — Add `chai sessions list`, `chai sessions delete`, `chai sessions clear` subcommands
- [ ] `docs/guides/02-getting-started.md` — Confirm no changes needed (or update if sessions are mentioned)
- [ ] `docs/guides/06-skills.md` — Confirm no changes needed
- [ ] `README.md` — Confirm no changes needed (or update if session subcommands are mentioned)
- [ ] `CHANGELOG.md` — Add entries for all persistent sessions features

### 4. Release Mechanics

- [ ] Update structured documentation per requirement 2
- [ ] Update user documentation per requirement 3
- [ ] Validate experimental feature builds
- [ ] Write tag file `base/tag/V0_3_0.md` following `base/meta/TAG.md` format
- [ ] Update `CHANGELOG.md` — replace `## [Unreleased]` with `## [0.3.0] - YYYY-MM-DD`, add new `## [Unreleased]` heading
- [ ] Bump versions in all `Cargo.toml` files to `0.3.0`
- [ ] Update lockfile (`cargo update`)
- [ ] Delete this working document (`base/RELEASE_V0_3_0.md`)
- [ ] Commit to `main` with message `v0.3.0`
- [ ] Create release branch `release/v0.3.0`
- [ ] Create annotated tag: `git tag -a v0.3.0 -F base/tag/V0_3_0.md --cleanup=verbatim`
- [ ] Push `main`, release branch, and tag to origin
- [ ] Build release binaries (`scripts/build-release.sh 0.3.0`)
- [ ] Publish platform release notes using tag file contents
- [ ] Review `chai-examples` — verify example profiles and skills align with the release
- [ ] Tag `chai-examples` with `v0.3.0`
