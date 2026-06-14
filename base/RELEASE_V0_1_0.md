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

### Release Build and Distribution

- [ ] Create CI workflow for release builds with binary assets (Linux, macOS, Windows)
- [ ] Decide on release asset naming and structure (see [RELEASE.md](RELEASE.md) design questions)
- [ ] Verify `cargo install --path crates/cli` and `cargo install --path crates/desktop` work cleanly from the release tag
- [ ] Test `--features matrix` builds produce working binaries
- [ ] Test `--features signal` builds produce working binaries
- [ ] Test `--features matrix,signal` builds produce working binaries

### Documentation

- [ ] Review and update `chai/docs/` guides for accuracy against shipped features

### chai-examples Alignment

The `chai-examples` repository contains example profiles and skills that users reference alongside chai. Before v0.1.0, these examples must be reviewed and updated to align with the release. The examples repository should be tagged with the same version number as chai so users can identify which examples work with which release (see [RELEASE.md](RELEASE.md) design question 7).

- [ ] Add `skills.lock` for example profiles using v0.1.0 skill format and tools schema (no example skills locked)
- [ ] Review example profiles (`assistant`, `developer`, `skillsmith`) against v0.1.0 config schema and agent model
- [ ] Review example skills (`notesmd`, `notesmd-daily`, `websearch`) against v0.1.0 skill format and tools schema

### License and Legal

- [ ] Decide on license for v0.1.0 — current license is LGPL-3.0; the question of switching to GPL has been raised (see open questions below)

## Open Questions

### License

The question of switching from LGPL-3.0 to GPL has been raised. This is a significant decision with implications for how others can use Chai as a library. The current LGPL-3.0 license allows Chai to be used as a library in non-GPL applications; GPL would not.

**Decision needed:** Confirm LGPL-3.0 or switch to GPL before v0.1.0. If switching, update `LICENSE`, all `Cargo.toml` files, and `base/VISION.md`.
