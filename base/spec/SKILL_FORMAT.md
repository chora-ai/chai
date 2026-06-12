---
status: stable
---

# Skills

Skills are markdown-based instructions (one per directory) that can be loaded and used by the agent. The format and bundled skills for this project are described below.

## Format

- **Layout**: Each skill lives in its own directory under **`~/.chai/skills`**. The loader discovers packages as immediate subdirectories containing **`SKILL.md`**. There is no config override for the skill root.
- **Content**: `SKILL.md` is Markdown with optional YAML frontmatter between `---` delimiters.
- **Optional tools**: A skill directory may also contain **`tools.json`** (see [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md)). Only skills with a valid `tools.json` expose callable tools to the agent; skills without it are still loaded for context (their SKILL.md appears in the system message when context mode is `full`, or in the compact list when `readOnDemand`).
- **Optional scripts**: A skill directory may contain a **`scripts/`** subdirectory. Tools can reference these scripts in `resolveCommand.script` (e.g. for param resolution); the executor runs them via `sh` with no allowlist entry (only files under the skill's `scripts/` dir are executed). See [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md).

## Skill Packages (Versioned Layout)

Each skill directory is a **skill package**: a content-addressed revision space with immutable snapshot directories and an `active` pointer. This enables reproducible resolution and rollback across profiles (see [PROFILES.md](PROFILES.md)).

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

### CLI Commands for Version and Lock Management

| Command | Behavior |
|---------|----------|
| `chai skill lock` | Pin current `active` hashes to `skills.lock` for the active profile |
| `chai skill rollback <generation>` | Restore a previous generation's lockfile and update `active` symlinks |
| `chai skill generations` | List available lockfile generations |

For lockfile schema and generation tracking, see [PROFILES.md](PROFILES.md).

## Frontmatter

Frontmatter contains only fields consumed at runtime. The directory name is the authoritative skill name; a `name` field is not needed.

| Field | Required | Description |
|-------|----------|-------------|
| `description` | No | Short description for catalogs and system context display. |
| `capability_tier` | No | Minimum model capability: `minimal` (pure schema, 7B target), `moderate` (some interpretation, 13B–30B), `full` (judgment-tier, capable cloud or 70B+). Used by gateway startup validation to warn when an enabled skill's tier exceeds the agent's likely model capability. Also informs context budget: `minimal`-tier skills should default to `readOnDemand` context mode to preserve limited context windows. |
| `variant_of` | No | Links to a related skill at a different tier (e.g., `git-read` declares `variant_of: git`). Used by startup validation to warn when variant skills with overlapping tool surfaces are both enabled for the same agent. |
| `metadata` | No | Optional structured metadata (see below). |

### Minimal Example

```yaml
---
description: Monitor RSS and Atom feeds for new content.
capability_tier: moderate
metadata:
  requires:
    bins: ["curl", "cat"]
---
```

### Variant Example

```yaml
---
description: Inspect Git repository state, history, diffs, and branches (read-only).
capability_tier: minimal
variant_of: git
metadata:
  requires:
    bins: ["git"]
---
```

## Metadata (project-neutral)

This project uses a **project-neutral** metadata shape so skills can be shared across runtimes without tying them to a single product.

- **`metadata.requires.bins`** — Optional list of binary names (e.g. `["cat", "ls", "grep", "chai"]`). The skill is **only loaded** when every listed binary is found on the system `PATH`. If any are missing, the skill is skipped (e.g. so the Obsidian skill is only available when the Obsidian CLI is installed).

**Enabling skills:** Discovery loads all packages under **`~/.chai/skills`**; **each agent** (orchestrator and workers) opts in with its own **`skillsEnabled`** array in **`config.json`**. Missing or empty **`skillsEnabled`** for an agent ⇒ **no** skill tools and **no** skill context for **that** agent. List the skill **names** you want per role (e.g. `["files", "git-read"]`). If a skill uses **`metadata.requires.bins`**, it is skipped at load time when binaries are missing—ensure CLIs are on **PATH when the gateway starts**. See [README](../../README.md), [CONFIGURATION.md](CONFIGURATION.md), and [CONTEXT.md](CONTEXT.md).

## Bundled Skills

**Bundled skills** are the skills shipped with the application (in `crates/lib/config/skills/`); `chai init` extracts them to **`~/.chai/skills`** using the versioned layout (creating `versions/<hash>/` snapshots and `active` symlinks), which is the **only** skill package root the runtime loads.

## Related Documents

| Document | Purpose |
|----------|---------|
| [PROFILES.md](PROFILES.md) | Per-profile lockfile (`skills.lock`), generation tracking, and lock verification |
| [CONFIGURATION.md](CONFIGURATION.md) | `skillLockMode` config field |
| [CONTEXT.md](CONTEXT.md) | Capability-tier and variant validation at gateway startup |
| [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md) | `tools.json` schema: tools array, allowlist, execution mapping |
| [AGENTS.md](AGENTS.md) | Per-agent skill configuration (`skillsEnabled`, `contextMode`) |
