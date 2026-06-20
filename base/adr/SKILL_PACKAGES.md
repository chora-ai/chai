---
status: accepted
---

# Skill Packages

Content-addressed skill package versioning with per-profile lockfiles, generation-level rollback, and startup validation — modeled on the Nix flake metaphor without a Nix dependency.

## Context

Before skill packages, the gateway loaded whatever was on disk at `~/.chai/skills/<name>/` with no integrity verification, no version tracking, and no way to reproduce a previous state. Skills were plain directories; there was no first-class revision concept. Users who iterated with a developer profile and promoted to an assistant profile had no mechanism to pin or verify which skill content ran in each profile. The gateway's `load_skills` always read files directly from the skill directory — if the content changed between restarts, the next restart picked up the new content silently, with no audit trail and no way to roll back.

This meant:

- No **reproducible restarts** — the same profile could load different skill content after a filesystem change.
- No **rollback** — reverting a skill to a previous state required manual backups or hoping the content was still available.
- No **integrity verification** — nothing checked that skill content matched a known-good state.
- No **promotion workflow** — no controlled path from "developer iterated on this skill" to "assistant profile locks to this version."

Per-agent `enabledSkills` (see [AGENT_ISOLATION.md](AGENT_ISOLATION.md)) already determined *which* skills each agent loaded, and runtime profiles (see [RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)) already isolated *which* config was active — but neither solved the version-integrity problem within the shared `~/.chai/skills/` store.

## Decision

Model each `skills/<name>/` tree as a **skill package** with content-addressed revisions and per-profile lockfiles:

- **Content-addressed version storage.** Each skill package stores immutable snapshot directories under `versions/<hash>/`, identified by a truncated SHA-256 content hash. An `active` symlink selects the current version. The loader resolves `active` before reading skill files; if the symlink is missing or broken, the skill is skipped. The directory name *is* the integrity check — no separate verification step.
- **Per-profile lockfile.** `profiles/<name>/skills.lock` (JSON) maps skill directory name → content hash, with a monotonic generation counter. Different profiles legitimately pin different revisions (developer iterates, assistant pins stable). Lockfile keys on directory name (authoritative).
- **Startup verification against lock.** On gateway startup, the gateway verifies enabled skills against the lockfile. Behavior is controlled by `skills.lockMode` in `config.json`: `"strict"` (default) treats the lockfile as a complete manifest — refuses to start when the lockfile is missing, any enabled skill has no lock entry (unpinned), or any pinned skill's active version does not match its locked hash; `"warn"` logs warnings on mismatches, allows unpinned skills, and skips verification when no lockfile is present.
- **Generation-level rollback.** Each lockfile update increments the generation counter and preserves the previous lockfile as `skills.lock.<N>`. Rollback restores a previous generation's lockfile and updates `active` symlinks to match. This is generation-level (all skills at once), not per-package — matching the NixOS "switch to a previous configuration" contract.
- **Frontmatter for runtime validation.** `SKILL.md` frontmatter records `capability_tier` (minimal/moderate/full) and `variant_of` (links variant skills). The gateway validates tier/model fit and variant overlap at startup, warning when composition problems are detected.

## Alternatives Considered

| Alternative | Why not |
|-------------|---------|
| **Plain directories, no versioning** (prior state) | No reproducible restarts, no rollback, no integrity verification. The gateway loads whatever is on disk with no audit trail. |
| **Git-based versioning** (use git commits/tags in each skill directory) | Adds a git dependency. Chai's minimal-dependency principle favors native solutions. Git also brings unnecessary complexity (staging, commits, branch semantics) for what is fundamentally a content-addressed snapshot problem. |
| **Global lockfile** (one lock for all profiles) | Different profiles legitimately need different pins. The developer profile iterates freely; the assistant profile pins stable versions. Per-profile locks allow independent promotion workflows. |
| **Per-package rollback** (revert a single skill independently) | Makes the lockfile semantics harder to reason about — partial rollbacks can create inconsistent skill sets. Generation-level rollback (all skills at once) is simpler, auditable, and matches the NixOS configuration-switching model. |
| **Hot reload** of resolved package revisions without gateway restart | Significantly more complex — the gateway would need to rebuild skill tools, context strings, and executor state dynamically. The restart requirement matches the profile switching contract and keeps the system auditable. |

## Consequences

- **Reproducible restarts are first-class.** A profile with a lockfile loads the exact same skill content on every gateway start. The lockfile records what ran; frontmatter records capability requirements for startup validation.
- **Self-verifying versions.** The content hash is both the address and the integrity check. If the content matches the hash, it is the correct version — no separate verification step needed.
- **Atomic rollback.** Changing the `active` symlink is a single filesystem operation. Generation-level rollback restores the entire skill set at once, not individual packages.
- **Shared store, different pins.** All profiles reference the same `versions/<hash>/` directories — no file copying between profiles. The lockfile is the only thing that differs, making promotion a matter of copying hash entries.
- **No git required.** Content-addressed versioning is implemented natively. Aligns with Chai's minimal-dependency principle.
- **Garbage collection is deferred.** Old version snapshots accumulate. No pruning policy or `chai skill gc` command exists yet. Candidates: keep N most recent, keep all locked versions across profiles, prune unlocked versions older than N days.

## References

- [spec/SKILL_FORMAT.md](../spec/SKILL_FORMAT.md) — Skill directory layout, `SKILL.md` content, and frontmatter fields.
- [spec/SKILL_PACKAGES.md](../spec/SKILL_PACKAGES.md) — Skill package versioned layout, startup validation, and CLI commands.
- [spec/PROFILES.md](../spec/PROFILES.md) — Per-profile lockfile schema, strictness modes, generation tracking, rollback, and promotion.
- [spec/CONFIGURATION.md](../spec/CONFIGURATION.md) — `skills.lockMode` config field.
- [adr/RUNTIME_PROFILES.md](RUNTIME_PROFILES.md) — Named runtime profiles with restart-required switching.
- [adr/AGENT_ISOLATION.md](AGENT_ISOLATION.md) — Per-agent context and skill configuration.
