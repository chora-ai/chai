# RELEASE: v0.1.0 Release Requirements

Track requirements and open questions for the first official release of Chai (v0.1.0). This is a working document; items will be checked off as they are completed. For the release process itself (how releases are tagged, where notes live, changelog conventions), see [RELEASE.md](RELEASE.md).

## v0.1.0 Scope

v0.1.0 is the first tagged release. It establishes the baseline: a working multi-agent management system with messaging channels, skill-based tooling, and profile-based configuration. The bar is "usable and well-documented for early adopters," not "feature-complete."

Per project conventions (see sandbox `AGENTS.md`), backwards compatibility is **not** a concern before v0.1.0. This release is a clean slate — no migration shims, no deprecated fields, no compat layers.

### Channels in v0.1.0

| Channel | Feature Gate | Status in v0.1.0 | Notes |
|---------|-------------|-------------------|-------|
| **Telegram** | Always on | Supported | Default channel; long-poll and webhook modes. |
| **Matrix** | `--features matrix` (opt-in) | Experimental | Separate adapter crate (`crates/adapters/matrix`); E2EE, room allowlist, SAS verification; hardening in progress. |
| **Signal** | `--features signal` (opt-in) | Experimental | Separate adapter crate (`crates/adapters/signal`); BYO signal-cli; basic text only; hardening in progress. |

### Epics Explicitly Out of Scope

| Epic | Reason |
|------|--------|
| [DESKTOP_FILES.md](epic/DESKTOP_FILES.md) | Draft; not scheduled |
| [PARALLEL_WORKFLOWS.md](epic/PARALLEL_WORKFLOWS.md) | Draft; not scheduled |
| [PERSISTENT_SESSIONS.md](epic/PERSISTENT_SESSIONS.md) | Draft; not scheduled |
| [TOOL_APPROVAL.md](epic/TOOL_APPROVAL.md) | Draft; not scheduled |

## Requirements

### Skills Audit and Refinement

- [ ] Complete cross-skill audit ([AUDIT_SKILLS.md](AUDIT_SKILLS.md)) — all bundled skills reviewed
- [ ] Confirm [BUG_FILES_REPLACE.md](BUG_FILES_REPLACE.md) and [BUG_FILES_WRITE_LINES.md](BUG_FILES_WRITE_LINES.md) are resolved

### Messaging Channels

- [ ] Test Signal integration via `crates/spike` (signal-probe) and gateway end-to-end
- [ ] Test Matrix integration via `crates/spike` (matrix-probe) and gateway end-to-end

### Code Quality and Structure

- [ ] Audit naming conventions — consistent naming across crates, modules, types, and config fields
- [ ] Audit file structure — no oversized files, logical module organization

### Documentation

- [ ] Review and update `chai/docs/` for accuracy against shipped features
- [ ] Review and update `tag/V0_1_0.md` for accuracy against shipped features 

### Release Build and Distribution

- [ ] Create CI workflow for Nix flake builds on `x86_64-linux`, `aarch64-linux`, and `aarch64-darwin`
- [ ] Verify `nix build .#cli` produces a working binary from the release tag
- [ ] Verify `nix build .#desktop` produces a working binary from the release tag
- [ ] Verify `cargo install --path crates/cli` works cleanly from the release tag (for Windows / non-Nix users)
- [ ] Verify `cargo install --path crates/desktop` works cleanly from the release tag (for Windows / non-Nix users)
- [ ] Test `--features matrix` builds produce working binaries
- [ ] Test `--features signal` builds produce working binaries
- [ ] Test `--features matrix,signal` builds produce working binaries
- [ ] Include Build Instructions for Windows in `tag/V0_1_0.md`

### License and Legal

- [ ] Replace `LICENSE` with GPL-3.0 license: https://www.gnu.org/licenses/gpl-3.0.html
- [ ] Replace all references and links to GPL-3.0 (including `README.md`, all `Cargo.toml` files, and `base/VISION.md`)

### Release Commit and Tag

- [ ] Update all `Cargo.toml` files to version `0.1.0`
- [ ] Remove this working document if it still exists
- [ ] Update `CHANGELOG.md` unreleased heading to version number heading following conventions
- [ ] Create commit using version number as the commit message `v0.1.0` and push to `release/v0.1.0`
- [ ] Create annotated tag (`git tag -a`) with exact contents from `tag/V0_1_0.md` and push to origin
- [ ] Create and switch to release branch `release/v0.1.0` and push to origin

### Post-Commit and Tag

- [ ] Update Codeberg/GitHub release notes using exact content from `tag/V0_1_0.md`

## Additional Requirements

### chai-examples

The `chai-examples` repository contains example profiles and skills that users reference alongside chai. Before v0.1.0, these examples must be reviewed and updated to align with the release. The examples repository should be tagged with the same version number as chai so users can identify which examples work with which release (see [RELEASE.md](RELEASE.md) design question 7).

- [ ] Add `skills.lock` for example profiles using v0.1.0 skill format and tools schema (no example skills locked)
- [ ] Review example profiles (`assistant`, `developer`, `skillsmith`) against v0.1.0 config schema and agent model
- [ ] Review example skills (`notesmd`, `notesmd-daily`, `websearch`) against v0.1.0 skill format and tools schema
