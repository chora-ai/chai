---
status: stable
---

# Skill Format

Skills are markdown-based instructions (one per directory) that can be loaded and used by the agent. This document specifies the skill directory layout, `SKILL.md` content, frontmatter fields, and `tools.json`. For the versioned package model (content-addressed snapshots, rollback, startup validation), see [SKILL_PACKAGES.md](SKILL_PACKAGES.md).

## Layout

- Each skill lives in its own directory under **`~/.chai/skills`**. The loader discovers packages as immediate subdirectories containing **`SKILL.md`**. There is no config override for the skill root.
- **Content**: `SKILL.md` is Markdown with optional YAML frontmatter between `---` delimiters.
- **Optional tools**: A skill directory may also contain a tool descriptor consisting of **`tools.json`**, **`allowlist.json`**, and **`execution.json`** (see [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md)). Only skills with valid tool descriptor files expose callable tools to the agent; skills without them are still loaded for context (their SKILL.md appears in the system message when context mode is `full`, or in the compact list when `readOnDemand`).
- **Optional scripts**: A skill directory may contain a **`scripts/`** subdirectory. Tools can reference these scripts in `resolveCommand.script` (e.g. for param resolution); the executor runs them via `sh` with no allowlist entry (only files under the skill's `scripts/` dir are executed). See [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md).

## Frontmatter

Frontmatter contains only fields consumed at runtime. The directory name is the authoritative skill name; a `name` field is not needed.

| Field | Required | Description |
|-------|----------|-------------|
| `description` | No | Short description for catalogs and system context display. |
| `capability_tier` | No | Minimum model capability: `minimal` (pure schema, 7B target), `moderate` (some interpretation, 13B–30B), `full` (judgment-tier, capable cloud or 70B+). Used by gateway startup validation to warn when an enabled skill's tier exceeds the agent's likely model capability. Also informs context budget: `minimal`-tier skills should default to `readOnDemand` context mode to preserve limited context windows. See [SKILL_PACKAGES.md](SKILL_PACKAGES.md). |
| `variant_of` | No | Links to a related skill at a different tier (e.g., `git-read` declares `variant_of: git`). Used by startup validation to warn when variant skills with overlapping tool surfaces are both enabled for the same agent. See [SKILL_PACKAGES.md](SKILL_PACKAGES.md). |
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

### OR-Group Bins Example

```yaml
---
description: Build and check Rust projects.
capability_tier: moderate
metadata:
  requires:
    bins: [["cargo"], ["nix"]]
---
```

This skill loads if `cargo` is on PATH (e.g. standard Rust install) **or** if `nix` is on PATH (e.g. NixOS with a nix develop shell). The matched group determines which execution spec the loader selects — see `binaryWrapper` and `condition` in [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md).

## Metadata (project-neutral)

This project uses a **project-neutral** metadata shape so skills can be shared across runtimes without tying them to a single product.

- **`metadata.requires.bins`** — Optional binary requirement. Supports two forms:

  - **Flat list** (AND semantics): `["git", "curl"]` — the skill is only loaded when **every** listed binary is found on the system `PATH`. This is the backward-compatible default.
  - **OR-groups** (OR of ANDs): `[["cargo"], ["nix"]]` — the skill is loaded when **any one group** has all its binaries on PATH. Each inner list is an AND group; the outer list is an OR over those groups.

  When OR-groups are present, the loader records which group matched. This matched group index drives `condition.binGroup` selection in execution specs (see [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md)), allowing the same tool to be executed with or without a `binaryWrapper` depending on which group matched.

**Enabling skills:** Discovery loads all packages under **`~/.chai/skills`**; **each agent** (orchestrator and workers) opts in with its own **`enabledSkills`** array in **`config.json`**. Missing or empty **`enabledSkills`** for an agent ⇒ **no** skill tools and **no** skill context for **that** agent. List the skill **names** you want per role (e.g. `["files", "git-read"]`). If a skill uses **`metadata.requires.bins`**, it is skipped at load time when the binary requirement is not satisfied—ensure CLIs are on **PATH when the gateway starts**. See [README](../../README.md), [CONFIGURATION.md](CONFIGURATION.md), and [CONTEXT.md](CONTEXT.md).

## Related Documents

| Document | Purpose |
|----------|---------|
| [SKILL_PACKAGES.md](SKILL_PACKAGES.md) | Versioned layout, content hashing, rollback, startup validation, and CLI commands |
| [PROFILES.md](PROFILES.md) | Per-profile lockfile (`skills.lock`), generation tracking, and lock verification |
| [CONFIGURATION.md](CONFIGURATION.md) | `skills.lockMode` config field |
| [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md) | Tool descriptor schema: `tools.json`, `allowlist.json`, `execution.json` |
| [AGENTS.md](AGENTS.md) | Per-agent skill configuration (`enabledSkills`, `contextMode`) |
