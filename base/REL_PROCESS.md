# REL: Release Process Design

Track requirements and open questions for the Chai release process — both the first release (v0.1.0) and subsequent releases. This is a working document for designing how releases are tagged, tracked, documented, and distributed.

## Problem Statement

Chai does not have a defined release process. There are no tagged releases, no release notes, no changelog, and no workflow for producing release artifacts. Before v0.1.0 can be tagged, the project needs to decide:

1. How release requirements are tracked and documented
2. Where release notes live and in what format
3. How changes are tracked between releases (changelog approach)
4. Whether each release has its own branch and/or tracking document
5. How release artifacts are built and distributed

## Current State

- **No tagged releases** — The repository has no git tags or release branches.
- **No CHANGELOG.md** — Changes are tracked informally through `base/` working notes and structured docs.
- **No CI/CD for releases** — No automated workflow for building release artifacts.
- **Working notes in `base/`** — `FEAT_*`, `BUG_*`, `AUDIT_*` files track active work but are not release-oriented.
- **`VISION.md`** — Describes project state and goals but is not a release document.
- **`base/` structured docs** — `adr/`, `epic/`, `spec/`, `ref/` capture decisions, features, and behavior but not release history.

## Design Questions

### 1. Should each release have a tracking document in `chai/base`?

**Option A: Per-release tracking document in `base/`**

Each release gets a `REL_V0_1_0.md`, `REL_V0_2_0.md`, etc. in the root of `base/`. These documents track the requirements checklist, scope decisions, and open questions for that specific release. After the release is tagged, the document becomes a historical reference.

- **Pro:** Keeps release planning in the same location as other working notes. Consistent with the `FEAT_*`/`BUG_*`/`AUDIT_*` pattern.
- **Pro:** Easy to find what was in scope for a given release.
- **Con:** Proliferates files in `base/` root over time.

**Option B: Release tracking in `base/release/` directory**

A dedicated `base/release/` directory holds per-release documents. Keeps the root of `base/` focused on active work.

- **Pro:** Organized; doesn't clutter `base/` root.
- **Con:** Adds a new directory to the structure; requires updating `base/README.md`.

**Option C: No persistent tracking document; use GitHub Releases**

Release requirements are tracked in issues/milestones and the release itself is documented via GitHub Releases (or equivalent).

- **Pro:** No additional files in the repository.
- **Con:** Release context lives outside the repo; not available to agents or offline workflows.

**Recommendation:** Option A (per-release document in `base/` root) for v0.1.0. This is consistent with existing conventions and the file count will be low in the near term. If the number of release documents grows, graduate to Option B (`base/release/`) at that time.

### 2. Should the working document graduate into a release document?

When a release is tagged, should the `REL_V*` working document be:

**Option A: Left as-is (historical working note)**

The document stays in `base/` as a record of what was planned and what was completed. No format change.

- **Pro:** Simple; no additional ceremony.
- **Con:** Mixes working-note style with release-record style; may not be as useful for historical reference.

**Option B: Converted to a structured release document**

Before tagging, the working document is reformatted into a standard release document (similar to how working notes graduate into structured docs per `base/AGENTS.md` conventions). The release document would follow a standard format: version, date, summary, changes, known issues, breaking changes.

- **Pro:** Clean historical record; consistent format across releases.
- **Pro:** The release document could be the source of truth for release notes / CHANGELOG entries.
- **Con:** Additional step before tagging; requires defining the standard format.

**Option C: Supplemented by a separate release record**

The working document stays as-is, and a separate `RELEASE.md` is added at the repository root (or in a release branch) at tag time. The working document records the *planning*; the release document records the *result*.

- **Pro:** Separation of concerns — planning vs. record.
- **Con:** Two documents per release; potential for drift.

**Recommendation:** Option B (convert to structured release document). The working document tracks requirements during development; before tagging, it is updated to reflect the final state (checked-off items, resolved open questions, summary). This is analogous to how `FEAT_*` notes graduate into structured docs. The format should be defined in `base/meta/` alongside the other convention files (see "Conventions" below).

### 3. Where should release notes live?

**Option A: `CHANGELOG.md` in the repository root**

A single file, appended to with each release. Standard format (e.g., [Keep a Changelog](https://keepachangelog.com/)).

- **Pro:** Industry standard; easy to find; works offline; grep-friendly.
- **Pro:** Git-tracked; visible to agents.
- **Con:** Can grow large over time; requires discipline to maintain.

**Option B: Per-release `RELEASE.md` on a release branch**

Each release branch (e.g., `release/v0.1.0`) contains a `RELEASE.md` at the repository root with the notes for that release. The `main` branch does not have a cumulative changelog.

- **Pro:** Clean separation; no large file on `main`.
- **Con:** Historical changelog requires checking multiple branches; not grep-friendly across releases.

**Option C: GitHub Releases (or equivalent) only**

Release notes live in the forge, not in the repository.

- **Pro:** No repo file to maintain.
- **Con:** Not available offline or to agents; not git-tracked.

**Option D: `CHANGELOG.md` on `main` + detailed release notes on release branches**

A `CHANGELOG.md` on `main` provides a cumulative summary (version, date, one-line summary, link to details). Full release notes live in per-release branches or GitHub Releases.

- **Pro:** Best of both worlds — quick overview in-repo, detailed notes per-release.
- **Con:** More maintenance; two sources of truth.

**Recommendation:** Option A (`CHANGELOG.md` in the repository root) for v0.1.0. It's the simplest approach that keeps release history accessible. The file can be started at v0.1.0 and maintained going forward. If it grows unwieldy, it can be split or archived later. Adopt [Keep a Changelog](https://keepachangelog.com/) format as a starting point.

### 4. Should each release have its own branch?

**Option A: Tag `main` directly**

No release branches. Tags are applied directly to `main`. Changes after a release are committed to `main` and included in the next release.

- **Pro:** Simple; no branch management overhead.
- **Con:** No way to make patch fixes to an older release without including all subsequent changes on `main`.
- **Con:** If a `RELEASE.md` is added before tagging, that commit exists on `main` but is only relevant for the release.

**Option B: Release branches (`release/v0.1.0`)**

Each release gets a branch from `main`. The branch may contain a `RELEASE.md` commit, last-minute fixes, or version bumps before the tag is applied. `main` continues to receive changes for the next release.

- **Pro:** Clean separation; patch releases possible from the branch.
- **Pro:** `RELEASE.md` or version-bump commits live on the branch, not on `main`.
- **Con:** Branch management overhead; merge discipline required.

**Option C: Tag `main` directly; no release-specific commits on `main`**

Tags are applied to `main` without any release-specific commits. Release notes live in `CHANGELOG.md` (already on `main`) and/or GitHub Releases. No `RELEASE.md` file is added.

- **Pro:** Simple; `main` stays clean.
- **Pro:** `CHANGELOG.md` is updated before tagging as a normal commit.
- **Con:** No patch release mechanism.

**Recommendation:** Option C (tag `main` directly; changelog-driven) for v0.1.0 and the near term. The project is pre-v0.1.0 and patch releases are unlikely to be needed immediately. If patch releases become necessary later, adopt Option B at that time. This avoids premature process complexity.

### 5. How should changes be tracked after v0.1.0?

After v0.1.0 ships, backwards compatibility becomes a concern (per sandbox `AGENTS.md`). Changes need to be visible and well-documented.

**Approach:** Adopt `CHANGELOG.md` with [Keep a Changelog](https://keepachangelog.com/) format:

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2025-06-XX

### Added
- Initial release of Chai multi-agent management system.
- Telegram, Matrix (optional), and Signal (BYO signal-cli) messaging channels.
- ...
```

**Breaking changes** should be called out explicitly in the changelog under a `### Changed` or `### Removed` section. After v0.1.0, breaking changes require a minor version bump (v0.2.0) per semver.

**Working notes** (`FEAT_*`, `BUG_*`, etc.) continue to track active development. When a change ships, the working note is updated and the changelog entry is added. This mirrors the graduation pattern already established in `base/AGENTS.md`.

### 6. Should experimental feature binaries be shipped as release assets?

Matrix (`--features matrix`) and potentially Signal (if it becomes `--features signal`) are optional Cargo features. The question is whether the release artifacts should include pre-built binaries with these features enabled, or whether experimental features are only available to users who build from source.

**Option A: Default binaries only; experimental features require manual builds**

Ship a single set of release binaries per platform (CLI + desktop) built without optional features. Users who want Matrix or Signal build from source via `cargo install --path crates/cli --features matrix`.

- **Pro:** Minimal release artifacts; simpler CI; no confusion about which binary to download.
- **Pro:** Clear signal that optional features are secondary — they work, but the project doesn't distribute them as first-class assets.
- **Con:** Users must have a Rust toolchain to use experimental features; raises the barrier for Matrix/Signal adopters.
- **Con:** No way to validate that experimental feature builds work correctly in CI unless you add a separate build step (which is essentially Option B without the published artifacts).

**Option B: Separate experimental binaries as additional release assets**

Ship default binaries *and* additional binaries with experimental features enabled. Naming convention distinguishes them:

```
chai-0.1.0-x86_64-linux.tar.gz              # default build
chai-0.1.0-x86_64-linux-matrix.tar.gz       # + matrix feature
chai-0.1.0-x86_64-linux-matrix-signal.tar.gz # + matrix + signal features
```

Or a single "all features" variant:

```
chai-0.1.0-x86_64-linux.tar.gz              # default build
chai-0.1.0-x86_64-linux-all-features.tar.gz # + matrix + signal
```

- **Pro:** Users can try experimental features without a Rust toolchain.
- **Pro:** CI validates that experimental feature builds compile and link correctly on every release.
- **Con:** Doubles (or more) the number of release assets; more CI matrix complexity.
- **Con:** Naming and documentation burden — users must understand which binary to choose.
- **Con:** "Experimental" labeling on a release asset sends a mixed signal: it's published and downloadable but not officially supported.

**Option C: Default binaries + CI validation only**

Ship default binaries only, but add CI steps that build with `--features matrix` (and `--features signal` if applicable) on every release to catch compilation errors. The experimental builds are not published as release assets — they exist only as CI artifacts for maintainers.

- **Pro:** Experimental features are validated in CI without bloating the release.
- **Pro:** Users who want experimental features still build from source, but they can be confident the build compiles on their platform.
- **Con:** Same barrier as Option A for users without a Rust toolchain.
- **Con:** CI-only artifacts may not catch runtime issues that only appear with the feature enabled.

**Recommendation:** Option C (default binaries + CI validation) for v0.1.0. It provides the most important benefit — knowing that experimental feature builds compile — without the complexity and mixed messaging of publishing experimental binaries. The Matrix adapter has a documented `--features matrix` build path; if Signal becomes an optional feature, it would follow the same pattern. Users comfortable with experimental features are likely also comfortable with `cargo install`.

If demand for pre-built experimental binaries emerges after v0.1.0, escalate to Option B. The CI validation step from Option C makes this transition low-risk since the build is already automated.

### 7. How should chai-examples be versioned alongside chai?

The `chai-examples` repository contains example profiles and skills that users reference alongside chai. Without version alignment, users cannot tell which examples work with which release — a profile or skill written for one version may reference config fields, tools, or agent model behavior that has changed in another.

**Option A: Aligned tags, no separate process**

Tag `chai-examples` with the same version numbers as chai (`v0.1.0`, `v0.2.0`, etc.) but without a separate release process. The chai release checklist includes a review step for the examples repository. When chai is tagged, the same tag is applied to `chai-examples` after the review.

- **Pro:** Simple — one version scheme, one tagging cadence. Users see `v0.1.0` in both repos and know they match.
- **Pro:** No separate release process to maintain for `chai-examples`. The examples are treated as a companion artifact, not an independent project.
- **Pro:** The release review catches drift (stale profiles, outdated skill schemas) before users encounter it.
- **Con:** `chai-examples` tags may include only documentation or config changes with no code — the tag semantics differ from chai's (code release vs. example alignment).
- **Con:** If `chai-examples` needs a mid-release update (e.g., a new example), the aligned tag scheme has no mechanism for it until the next chai release.

**Option B: Aligned tags with patch increments for example-only changes**

Tag `chai-examples` with the same major.minor as chai but allow independent patch versions (e.g., chai `v0.1.0` → examples `v0.1.0`; a mid-release example addition → examples `v0.1.1`). The major.minor always matches the compatible chai version.

- **Pro:** Allows `chai-examples` to evolve between chai releases without breaking version alignment.
- **Pro:** Users can still identify compatibility at a glance (`v0.1.x` works with chai `v0.1.0`).
- **Con:** Slightly more complex — two patch version sequences to track.
- **Con:** Risk of drift if patches accumulate without a corresponding chai release to anchor them.

**Option C: No version alignment; reference chai version in README**

`chai-examples` is not tagged. Instead, `chai-examples/README.md` states which chai version the examples are tested against (e.g., "These examples are compatible with chai v0.1.0"). Update the README when a new chai release ships.

- **Pro:** No tagging ceremony for `chai-examples`.
- **Con:** No git-level version signal — users must read the README. Git history alone doesn't tell you which commit corresponds to which chai version.
- **Con:** If the README falls out of date, there's no automated check.

**Recommendation:** Option A (aligned tags, no separate process). The `chai-examples` repository is small and changes infrequently — it doesn't need its own release cadence. Tying it to the chai release process means:

1. Every chai release includes a **review step** for the examples repository (check profiles against current config schema, check skills against current tools schema, update README if needed).
2. After the chai tag is applied, the same tag is applied to `chai-examples`.
3. No separate CHANGELOG for `chai-examples` — the chai CHANGELOG entry can note if examples were updated.

This keeps the process simple while giving users a clear compatibility signal. If `chai-examples` grows to the point where it needs independent releases, that's the time to reconsider.

## Requirements

### For v0.1.0

- [ ] Define release process (this document)
- [ ] Create `CHANGELOG.md` in the repository root with v0.1.0 entry
- [ ] Create CI workflow for release builds with binary assets
- [ ] Add CI step to validate experimental feature builds (`--features matrix`, `--features signal` if applicable) without publishing them
- [ ] Review `chai-examples` against v0.1.0 config schema, agent model, and skill format (see [REL_V0_1_0.md](REL_V0_1_0.md) chai-examples Alignment section)
- [ ] Tag v0.1.0 on `main` after all `REL_V0_1_0.md` requirements are met
- [ ] Tag v0.1.0 on `chai-examples` after examples review is complete
- [ ] Convert `REL_V0_1_0.md` to structured release record before tagging

### For Subsequent Releases

- [ ] Maintain `CHANGELOG.md` with every release
- [ ] Use `## [Unreleased]` section to track changes between releases
- [ ] Call out breaking changes explicitly in the changelog
- [ ] Follow semantic versioning after v0.1.0 (breaking = minor bump while pre-1.0)
- [ ] Review `chai-examples` as part of every release checklist; tag `chai-examples` with the same version after review
- [ ] Evaluate release branches if patch releases become necessary
- [ ] Consider creating a `base/meta/REL.md` convention file for release document format

## Conventions (Proposed)

If the per-release tracking document pattern is adopted, a convention file should be added to `base/meta/` defining the format. Draft structure:

### `base/meta/REL.md` — Release Document Conventions

**Naming:** `REL_V<major>_<minor>_<patch>.md` in `base/` root (e.g., `REL_V0_1_0.md`).

**Lifecycle:**

| State | Meaning |
|-------|---------|
| Planning | Requirements being gathered; scope not finalized |
| Scoped | Requirements finalized; implementation in progress |
| Ready | All requirements met; ready to tag |
| Released | Tag applied; document is historical reference |

**Required sections:**

1. **Title** — `# REL: vX.Y.Z Release`
2. **Scope** — Epics and features in scope for this release
3. **Requirements** — Checklist of deliverables (`- [ ]` / `- [x]`)
4. **Open Questions** — Unresolved decisions blocking the release
5. **Release Summary** — Added when converting to release record (version, date, summary of changes)

**Optional sections:** Known Issues, Breaking Changes, Deferred Items

## Related Documents

- [REL_V0_1_0.md](REL_V0_1_0.md) — v0.1.0 specific requirements
- [AUDIT_SKILLS.md](AUDIT_SKILLS.md) — Skills audit (v0.1.0 blocker)
- [epic/MSG_CHANNELS.md](epic/MSG_CHANNELS.md) — Messaging channels epic (v0.1.0 scope)
- [VISION.md](../VISION.md) — Project vision and current state
- [chai-examples](../../chai-examples/) — Example profiles and skills (versioned alongside chai)
- [Keep a Changelog](https://keepachangelog.com/) — Proposed changelog format
- [Semantic Versioning](https://semver.org/) — Versioning scheme
