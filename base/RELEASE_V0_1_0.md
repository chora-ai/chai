# RELEASE: v0.1.0 Release Requirements

Track requirements and open questions for the first official release of Chai (v0.1.0). This is a working document; items will be checked off as they are completed. For the release process itself, see [RELEASE.md](RELEASE.md).

## v0.1.0 Scope

v0.1.0 is the first tagged release. It establishes the baseline: a working multi-agent management system with messaging channels, skill-based tooling, and profile-based configuration. The bar is "usable and well-documented for early adopters," not "feature-complete."

### Channels in v0.1.0

| Channel | Feature Gate | Status in v0.1.0 | Notes |
|---------|-------------|-------------------|-------|
| **Telegram** | Always on | Supported | Default channel; long-poll and webhook modes. |
| **Matrix** | `--features matrix` (opt-in) | Experimental | Separate adapter package (`crates/adapters/matrix`); E2EE, room allowlist, SAS verification; hardening in progress. |
| **Signal** | `--features signal` (opt-in) | Experimental | Separate adapter package (`crates/adapters/signal`); BYO signal-cli; basic text only; hardening in progress. |

### Epics Explicitly Out of Scope

| Epic | Reason |
|------|--------|
| [DESKTOP_FILES.md](epic/DESKTOP_FILES.md) | Draft; not scheduled |
| [PARALLEL_WORKFLOWS.md](epic/PARALLEL_WORKFLOWS.md) | Draft; not scheduled |
| [PERSISTENT_SESSIONS.md](epic/PERSISTENT_SESSIONS.md) | Draft; not scheduled |
| [TOOL_APPROVAL.md](epic/TOOL_APPROVAL.md) | Draft; not scheduled |

## Requirements

### Documentation

- [ ] Create `tag/V0_1_0.md` with overview of shipped features (no breaking changes) 

### License and Legal

- [ ] Replace `LICENSE` with GPL-3.0 license: https://www.gnu.org/licenses/gpl-3.0.html
- [ ] Replace all references and links to GPL-3.0 (including `README.md`, all `Cargo.toml` files, and `base/VISION.md`)

### Release Commit and Tag

- [ ] Update all `Cargo.toml` files to version `0.1.0`
- [ ] Update `CHANGELOG.md` unreleased heading to version number heading following conventions
- [ ] Remove this working document before proceeding with the release commit and release tag
- [ ] Create commit using version number as the commit message `v0.1.0` and push to `release/v0.1.0`
- [ ] Create annotated tag (`git tag -a`) with exact contents from `tag/V0_1_0.md` and push to origin
- [ ] Create and switch to release branch `release/v0.1.0` and push to origin

### Post-Commit and Tag

- [ ] Update Codeberg/GitHub release notes using exact content from `tag/V0_1_0.md`

## Additional Requirements

### chai-examples

The `chai-examples` repository contains example profiles and skills that users reference alongside chai. Before v0.1.0, these examples must be reviewed and updated to align with the release. The examples repository should be tagged with the same version number as chai so users can identify which examples work with which release (see [RELEASE.md](RELEASE.md) design question 7).
