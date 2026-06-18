---
status: stable
---

# Skill Packages

Skill packages are content-addressed revision spaces with immutable snapshot directories and an `active` pointer. This enables reproducible resolution and rollback across profiles (see [PROFILES.md](PROFILES.md)). For the architectural decision, see [adr/SKILL_PACKAGES.md](../adr/SKILL_PACKAGES.md). For the skill file format and frontmatter, see [SKILL_FORMAT.md](SKILL_FORMAT.md).

## Versioned Layout

### Directory Structure

```
skills/<name>/
  active -> versions/a1b2c3d/
  versions/
    a1b2c3d/
      SKILL.md
      tools.json
      scripts/
    f8e9d0b/
      SKILL.md
      tools.json
      scripts/
```

- **`versions/<hash>/`** — Immutable snapshot directories identified by a truncated SHA-256 content hash. Each snapshot contains the complete skill content (`SKILL.md`, `tools.json`, `scripts/`).
- **`active`** — Symlink selecting the current version. The loader resolves `active` before reading skill files; if the symlink is missing or broken, the skill is skipped.

### Content Hash Computation

The hash is a truncated SHA-256 of the canonical skill content:

- **Canonical form**: sorted relative file paths, each entry as `<relative-path>\0<file-contents>`, concatenated and hashed.
- **Line endings and trailing newlines** are hashed as-is (no normalization) — the hash reflects the exact bytes.
- **Script permissions** are not included in the hash (they are a deployment concern, not a content concern).
- The `versions/` directory and `active` symlink at the skill root are excluded from hash computation.

### Immutability and Rollback

- Version snapshots are **never modified**, only created.
- **Atomic rollback** — changing the `active` symlink is a single filesystem operation.
- Full copies per version (no delta compression). Skills are small (5–50 KB each); disk cost is negligible in practice.
- No built-in history or "why" metadata within the version store (no commit messages, changelogs, or parent-chain tracking).

### `chai init` Migration

`chai init` creates the versioned layout for each bundled skill: `versions/<hash>/` snapshot + `active` symlink, instead of writing files directly into `skills/<name>/`. On re-init, existing version snapshots are never re-written, and the `active` symlink is only set when no active version exists (fresh installation). Re-running `chai init` after a bundled skill update will create the new version snapshot on disk but will not change the `active` symlink — this preserves user customizations (manual rollbacks, edits via `skills_write_skill_md`). To adopt a new bundled version, the user can run `chai skill rollback` or manually update the symlink.

## Startup Validation

After skill loading and config resolution, the gateway runs two validation passes before accepting the configuration:

**Lockfile verification** (see [PROFILES.md](PROFILES.md)) — For each enabled skill that has an entry in the profile's `skills.lock`, the gateway checks whether the `active` symlink target matches the locked hash. Behavior on mismatch is controlled by `skills.lockMode` in `config.json` (see [CONFIGURATION.md](CONFIGURATION.md)): `"strict"` (default) refuses to start; `"warn"` logs and continues. Unlocked skills (no entry in `skills.lock`) load normally.

**Capability-tier validation** — For each agent's `enabledSkills` list:
- **Tier vs model** — Warn when an enabled skill's `capability_tier` assumes more capability than the agent's effective model is likely to provide (e.g., a `full` skill with a 7B local model). Informational warnings only; no strict mode yet.
- **Variant overlap** — Warn when two enabled skills share a `variant_of` relationship (e.g., both `git` and `git-read` enabled for the same agent), creating redundant or overlapping tool surfaces.

## CLI Commands for Version and Lock Management

| Command | Behavior |
|---------|----------|
| `chai skill lock` | Pin current `active` hashes to `skills.lock` for the active profile |
| `chai skill rollback <generation>` | Restore a previous generation's lockfile and update `active` symlinks |
| `chai skill generations` | List available lockfile generations |

For lockfile schema and generation tracking, see [PROFILES.md](PROFILES.md).

## Bundled Skills

**Bundled skills** are the skills shipped with the application (in `crates/lib/bundled/skills/`); `chai init` extracts them to **`~/.chai/skills`** using the versioned layout (creating `versions/<hash>/` snapshots and `active` symlinks), which is the **only** skill package root the runtime loads.

## Related Documents

| Document | Purpose |
|----------|---------|
| [SKILL_FORMAT.md](SKILL_FORMAT.md) | Skill directory layout, `SKILL.md` content, frontmatter fields, and `tools.json` |
| [PROFILES.md](PROFILES.md) | Per-profile lockfile (`skills.lock`), generation tracking, and lock verification |
| [CONFIGURATION.md](CONFIGURATION.md) | `skills.lockMode` config field |
| [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md) | `tools.json` schema: tools array, allowlist, execution mapping |
| [AGENTS.md](AGENTS.md) | Per-agent skill configuration (`enabledSkills`, `contextMode`) |
| [CONTEXT.md](CONTEXT.md) | Context on every turn: system message, session history, tool schemas |
