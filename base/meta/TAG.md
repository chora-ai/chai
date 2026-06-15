# Tag File Conventions

Tag files are the source of truth for release notes. They live in `base/tag/` (e.g., `tag/V0_1_0.md`). A tag file is written once at release time and is not modified after the tag is created.

## Naming

File name: `V<major>_<minor>_<patch>.md` in the `tag/` directory.

| Version | File Name |
|---------|-----------|
| v0.1.0  | `V0_1_0.md` |
| v0.2.0  | `V0_2_0.md` |
| v1.0.0  | `V1_0_0.md` |
| v0.1.1  | `V0_1_1.md` |

## Structure

### Required Sections

Sections must appear in this order:

1. **Version** — Heading: `# vX.Y.Z`
2. **Date** — Release date: `**Date:** YYYY-MM-DD`
3. **Summary** — One to three sentences describing the release.
4. **Changes** — Grouped by type using third-level headings:
   - `### Added` — New features or capabilities
   - `### Changed` — Changes to existing behavior
   - `### Fixed` — Bug fixes
   - `### Removed` — Removed features or capabilities
5. **Known Issues** — Known limitations or defects at the time of release. Use `None.` if there are none.
6. **Breaking Changes** — Changes that break backwards compatibility. Omit this section entirely if there are none.
7. **Build Instructions** — Included when a supported platform does not have a pre-built binary in the release assets. Provides the commands needed to build from source for that platform. Remove this section once pre-built binaries are included in the release assets.

### Section Order

1. Version
2. Date
3. Summary
4. Changes
5. Known Issues
6. Breaking Changes
7. Build Instructions (if applicable)

## Example

```markdown
# v0.1.0

**Date:** 2025-07-01

## Summary

First official release of the Chai multi-agent management system. Provides messaging channels, skill-based tooling, and profile-based configuration.

## Changes

### Added
- Telegram messaging channel (long-poll and webhook modes)
- Matrix adapter (experimental, `--features matrix`)
- Signal adapter (experimental, `--features signal`)
- Skill package management with content-addressed versioning
- Per-agent context directories and skill configuration

## Known Issues

None.

## Breaking Changes

None. This is the first release.

## Build Instructions

Pre-built binaries are provided for Linux (x86_64, ARM64) and macOS (ARM64). Windows users must build from source:

```bash
cargo build --release --manifest-path crates/cli/Cargo.toml
cargo build --release --manifest-path crates/desktop/Cargo.toml
```
```

## Maintenance

- Tag files are written at release time and not modified after the tag is created.
