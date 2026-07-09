# Release v0.5.0

## Scope

- `epic/SPLIT_DEPLOYMENT.md`

## Epic

- [x] `epic/SPLIT_DEPLOYMENT.md` — see [adr/SPLIT_DEPLOYMENT.md](adr/SPLIT_DEPLOYMENT.md) and [spec/GATEWAY.md](spec/GATEWAY.md)

## Release Process

- [x] **Audit changelog** — Verify that all entries in `CHANGELOG.md` are concise, user-facing changes since v0.4.0.
- [ ] **Validate build script and experimental feature builds** — Verify that `scripts/build-release.sh` produces build artifacts successfully and that experimental feature builds (`--features matrix`, `--features signal`) compile and link correctly.
- [ ] **Write the tag file** — Create `base/tag/V0_5_0.md` following the format defined in `base/meta/TAG.md`. Include a Build Instructions section for platforms that lack pre-built binaries. Add the new tag file entry to `base/README.md` using the exact Summary text from the tag file.
- [ ] **Update the changelog** — Replace the `## [Unreleased]` heading in `CHANGELOG.md` with `## [0.5.0] - YYYY-MM-DD`.
- [ ] **Bump versions** — Update all `Cargo.toml` files to version `0.5.0`.
- [ ] **Update the lockfile** — Run `cargo update` to sync `Cargo.lock` with the new version numbers.
- [ ] **Delete the working document** — Remove `base/RELEASE_V0_5_0.md`.
- [ ] **Commit** — Stage all changes (version bump, lockfile update, tag file, knowledge base index, changelog, doc updates, working document deletion) and commit to `main` with message `v0.5.0`.
- [ ] **Create the release branch** — `git branch release/v0.5.x` from the release commit.
- [ ] **Tag** — Create an annotated tag: `git tag -a v0.5.0 -F base/tag/V0_5_0.md --cleanup=verbatim`.
- [ ] **Build release binaries** — Run `scripts/build-release.sh` for each supported system.
- [ ] **Push** — Push `main`, the release branch, and the tag to origin.
- [ ] **Publish platform release notes** — Create a release on Codeberg/GitHub using the exact contents of `base/tag/V0_5_0.md`. Attach release binaries as assets.
- [ ] **Review `chai-examples`** — Verify example profiles and skills align with the release. Update as needed.
- [ ] **Tag `chai-examples`** — Apply the same version tag (`v0.5.0`) to `chai-examples`.
