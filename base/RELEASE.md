# Release

This document defines the official release process for Chai: how releases are planned, tagged, documented, and distributed.

## Versioning

Chai follows [Semantic Versioning](https://semver.org/). Before v1.0.0, breaking changes increment the minor version (e.g., v0.1.0 → v0.2.0). After v1.0.0, breaking changes increment the major version. Patch versions are reserved for bug fixes that do not change public interfaces.

## Release Branches

Each release has a branch named `release/vX.Y.Z`, created from `main`. The release branch exists for the lifetime of that version — it is the target for patch releases.

The release commit (version bump, tag file, doc updates, working document deletion) is made on `main`. The release branch is then created from that commit. This ensures `main` reflects the current release and the tag file is accessible on `main`.

For patch releases, fixes are committed on the release branch. See [Patch Releases](#patch-releases).

## Changelog

`CHANGELOG.md` in the repository root is the cumulative record of all notable changes. It follows [Keep a Changelog](https://keepachangelog.com/) format and uses a `## [Unreleased]` section to track changes between releases.

When a release is prepared, the `## [Unreleased]` heading is replaced with the version number and date. After tagging, a new `## [Unreleased]` heading is added at the top for subsequent changes.

## Tag Files

Tag files live in `base/tag/` as per-version files named `VX_Y_Z.md` (e.g., `tag/V0_1_0.md`). A tag file contains only the changes for that specific release — it is a snapshot, not a cumulative record. Always read `base/meta/TAG.md` before creating or modifying a tag file.

### Annotated Tags

All git tags must be annotated. The annotation message contains the exact contents of the corresponding tag file:

```bash
git tag -a vX.Y.Z -F base/tag/VX_Y_Z.md --cleanup=verbatim
```

### Platform Release Notes

When creating a release on Codeberg, GitHub, or another Git hosting platform, use the exact contents of the tag file as the release description. The tag file and the platform release notes must be identical.

### Build Instructions in Tag Files

If a supported platform does not have a pre-built binary in the release assets, the tag file must include a **Build Instructions** section with the commands to build from source for that platform. Remove this section once pre-built binaries are included in the release assets.

## Working Documents

Each release has a working document in the root of `base/` named `RELEASE_VX_Y_Z.md` (e.g., `RELEASE_V0_1_0.md`). This document tracks requirements, scope decisions, and open questions while the release is in progress. It is not a permanent record.

### Lifecycle

1. **Creation** — When a release is scoped, create the working document with requirements and open questions.
2. **Updates** — Check off requirements as they are completed; record decisions; add new items as needed.
3. **Deletion** — Delete the working document before committing release changes and tagging the commit.

## Release Process

### Initial Release

1. **Scope the release** — Create `base/RELEASE_VX_Y_Z.md` with requirements and open questions.
2. **Complete all requirements** — Check off each item as it is completed.
3. **Update structured documentation** — Ensure all specs, refs, ADRs, and other structured docs in `base/` are current with the release.
4. **Update user documentation** — Ensure `docs/` and the repository `README.md` are current.
5. **Review `chai-examples`** — Verify example profiles and skills align with the release. Update as needed.
6. **Write the tag file** — Create `base/tag/VX_Y_Z.md` following the format defined in `base/meta/TAG.md`. If any supported platform lacks a pre-built binary, include a Build Instructions section.
7. **Update the changelog** — Replace the `## [Unreleased]` heading in `CHANGELOG.md` with `## [X.Y.Z] - YYYY-MM-DD`.
8. **Bump versions** — Update all `Cargo.toml` files to the release version.
9. **Update the lockfile** — Run `cargo update` to sync `Cargo.lock` with the new version numbers.
10. **Delete the working document** — Remove `base/RELEASE_VX_Y_Z.md`.
11. **Commit** — Stage all changes (version bump, lockfile update, tag file, changelog, doc updates, working document deletion) and commit to `main` with message `vX.Y.Z`.
12. **Create the release branch** — `git branch release/vX.Y.Z` from the release commit.
13. **Tag** — Create an annotated tag: `git tag -a vX.Y.Z -F base/tag/VX_Y_Z.md --cleanup=verbatim`.
14. **Push** — Push `main`, the release branch, and the tag to origin.
15. **Build release binaries** — Run `scripts/build-release.sh` for each supported system (see [Build and Distribution](#build-and-distribution)).
16. **Publish platform release notes** — Create a release on Codeberg/GitHub using the exact contents of `base/tag/VX_Y_Z.md`. Attach release binaries as assets.
17. **Validate experimental feature builds** — Verify that experimental feature builds compile and link correctly (see [Experimental Features](#experimental-features)).
18. **Tag `chai-examples`** — Apply the same version tag to `chai-examples`.

### Patch Releases

1. **Make fixes** — Apply bug fixes to the `release/vX.Y.Z` branch.
2. **Write the tag file** — Create `base/tag/VX_Y_Z_P.md` for the patch version.
3. **Update the changelog** — Add a `## [X.Y.P] - YYYY-MM-DD` entry in `CHANGELOG.md` with the patch changes.
4. **Bump the patch version** — Update `Cargo.toml` files to `X.Y.P`.
5. **Update the lockfile** — Run `cargo update` to sync `Cargo.lock` with the new version numbers.
6. **Commit and tag** — Commit on the release branch with message `vX.Y.P`, then create an annotated tag using the tag file with `--cleanup=verbatim` to preserve markdown formatting.
7. **Push** — Push the release branch and the new tag to origin.
8. **Build release binaries** — Run `scripts/build-release.sh` for each supported system.
9. **Publish platform release notes** — Create a release on Codeberg/GitHub using the tag file contents. Attach release binaries as assets.
10. **Validate experimental feature builds** — Verify that experimental feature builds compile and link correctly.
11. **Merge fixes to `main`** — Cherry-pick or merge the fixes and changelog entry from the release branch into `main`.

## Build and Distribution

### Build Script

`scripts/build-release.sh` automates building and packaging release binaries from a tagged commit. It builds both CLI and desktop targets for a given system and produces tarballs plus a checksums file.

```bash
# Build for current host
./scripts/build-release.sh 0.1.0

# Build for a specific system
./scripts/build-release.sh 0.1.0 x86_64-linux
./scripts/build-release.sh 0.1.0 aarch64-linux
./scripts/build-release.sh 0.1.0 aarch64-darwin
```

Pre-built release assets are only produced for Linux (x86_64) until a CD pipeline is in place.

**Output** (in `dist/`):

| File | Description |
|------|-------------|
| `chai-vX.Y.Z-{system}.tar.gz` | CLI binary tarball |
| `chai-desktop-vX.Y.Z-{system}.tar.gz` | Desktop binary tarball |
| `checksums-vX.Y.Z-{system}.txt` | SHA-256 checksums for both tarballs |

### Nix Flake

The `flake.nix` in the repository root defines all release build targets. Binaries are built with `nix build` from the release tag.

**Supported platforms:**

| Platform | Nix System | CLI | Desktop | Local Build | Release Assets |
|----------|-----------|-----|---------|-------------|----------------|
| Linux (x86_64) | `x86_64-linux` | ✅ | ✅ | Native | ✅ Pre-built |
| Linux (ARM64) | `aarch64-linux` | ✅ | ✅ | Requires binfmt | Build from source |
| macOS (ARM64) | `aarch64-darwin` | ✅ | ✅ | Requires macOS hardware | Build from source |

Pre-built release assets are only provided for Linux (x86_64) until a CD pipeline is in place. Tag files must include a Build Instructions section for all platforms that lack pre-built binaries.

**Setting up aarch64-linux builds on NixOS:**

Building for `aarch64-linux` from an `x86_64-linux` host requires user-mode emulation. On NixOS, add to `configuration.nix`:

```nix
boot.binfmt.emulatedSystems = [ "aarch64-linux" ];
```

Then reboot or restart the service:

```bash
systemctl restart systemd-binfmt
```

After this, `nix build` and the build script will handle `aarch64-linux` transparently.

**Flake outputs:**

| Output | Command | Description |
|--------|---------|-------------|
| `default` | `nix build` | All default workspace members (cli + desktop) |
| `cli` | `nix build .#cli` | `chai` CLI binary only |
| `desktop` | `nix build .#desktop` | `chai-desktop` GUI binary only |

**Building from a release tag:**

```bash
git checkout vX.Y.Z
nix build .#cli
nix build .#desktop
```

The resulting binaries are in `result/bin/`.

### Platforms Without Pre-Built Binaries

Pre-built release assets are only provided for Linux (x86_64). All other supported platforms must build from source.

**Linux (ARM64) and macOS (ARM64)** — build from source using Nix:

```bash
git checkout vX.Y.Z
nix build .#cli
nix build .#desktop
```

Linux (ARM64) on NixOS requires binfmt emulation (see above). macOS (ARM64) requires macOS hardware.

**Windows** — Windows binaries are not built with Nix. Build from source using `cargo` on a Windows host:

```bash
cargo build --release --manifest-path crates/cli/Cargo.toml
cargo build --release --manifest-path crates/desktop/Cargo.toml
```

### Experimental Features

Matrix (`--features matrix`) and Signal (`--features signal`) are optional Cargo features. They are not included in published release binaries. CI validates that experimental feature builds compile and link correctly on every release, but the resulting binaries are not published.

Experimental feature builds use `cargo` directly (not Nix), because their dependency trees (e.g., `matrix-sdk` with E2EE) require system libraries that are not currently wired into the flake:

```bash
# Matrix adapter
cargo build --release --manifest-path crates/cli/Cargo.toml --features matrix

# Signal adapter
cargo build --release --manifest-path crates/cli/Cargo.toml --features signal

# Both adapters
cargo build --release --manifest-path crates/cli/Cargo.toml --features matrix,signal
```

Users who want experimental features build from source:

```bash
cargo install --path crates/cli --features matrix
```

## chai-examples Versioning

`chai-examples` is versioned alongside chai using the same tag numbers. There is no separate release process for `chai-examples`:

- Every chai release includes a review of `chai-examples` (profiles against current config schema, skills against current tools schema).
- After the chai tag is applied, the same version tag is applied to `chai-examples`.
- No separate changelog for `chai-examples` — the chai tag file notes if examples were updated.
