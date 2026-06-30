# Release v0.4.0

## Scope

Minor version bump for the Multiple Orchestrator Configuration epic.

## Epic: Multiple Orchestrator Configuration

- [x] **Phase 1 — Config layer**: Relax validation (at least one orchestrator, not exactly one), refactor `AgentsConfig` to `Vec<OrchestratorConfig>` with accessor methods, add `OrchestratorConfig` type including `enabledWorkers` field and validation.
- [x] **Phase 2 — Gateway runtime**: Per-orchestrator `OrchestratorRuntime`, per-orchestrator session stores, `agent` RPC `orchestratorId` parameter, `sessions` RPC `orchestratorId` parameter, `agentDetail` per-orchestrator resolution, `enabledWorkers` system prompt filtering and delegation enforcement, `enabledProviders` enforcement for shared workers.
- [x] **Phase 3 — Desktop UI + CLI**: Orchestrator selector in sessions sidebar, `--agent` CLI flag, orchestrator labeling on Agent/Tools screens, provider/model cascade.
- [x] **Phase 4 — Spec and ADR updates**: Document new behavior in all affected specs (`spec/AGENTS.md`, `spec/ORCHESTRATION.md`, `spec/CONFIGURATION.md`, `spec/GATEWAY_STATUS.md`, `spec/CONTEXT.md`, `spec/SESSIONS.md`) and update `adr/ORCHESTRATION.md` for per-orchestrator runtime isolation and `enabledProviders` enforcement.
- [x] **Graduate the epic**: Graduate the `epic/MULTI_ORCHESTRATOR.md` working note into structured documentation and delete the epic file before squash-merge.

## Release Process

- [x] **Audit changelog** — Verify that all entries in `CHANGELOG.md` are concise, user-facing changes since v0.3.0.
- [ ] **Validate build script and experimental feature builds** — Verify that `scripts/build-release.sh` produces build artifacts successfully and that experimental feature builds (`--features matrix`, `--features signal`) compile and link correctly.
- [ ] **Write the tag file** — Create `base/tag/V0_4_0.md` following the format defined in `base/meta/TAG.md`. Include a Build Instructions section for platforms that lack pre-built binaries. Add the new tag file entry to `base/README.md` using the exact Summary text from the tag file.
- [ ] **Update the changelog** — Replace the `## [Unreleased]` heading in `CHANGELOG.md` with `## [0.4.0] - YYYY-MM-DD`.
- [ ] **Bump versions** — Update all `Cargo.toml` files to version `0.4.0`.
- [ ] **Update the lockfile** — Run `cargo update` to sync `Cargo.lock` with the new version numbers.
- [ ] **Delete the working document** — Remove `base/RELEASE_V0_4_0.md`.
- [ ] **Commit** — Stage all changes (version bump, lockfile update, tag file, knowledge base index, changelog, doc updates, working document deletion) and commit to `main` with message `v0.4.0`.
- [ ] **Create the release branch** — `git branch release/v0.4.x` from the release commit.
- [ ] **Tag** — Create an annotated tag: `git tag -a v0.4.0 -F base/tag/V0_4_0.md --cleanup=verbatim`.
- [ ] **Push** — Push `main`, the release branch, and the tag to origin.
- [ ] **Build release binaries** — Run `scripts/build-release.sh` for each supported system.
- [ ] **Publish platform release notes** — Create a release on Codeberg/GitHub using the exact contents of `base/tag/V0_4_0.md`. Attach release binaries as assets.
- [ ] **Review `chai-examples`** — Verify example profiles and skills align with the release. Update as needed.
- [ ] **Tag `chai-examples`** — Apply the same version tag (`v0.4.0`) to `chai-examples`.
