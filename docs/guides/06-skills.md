# Skills

Skills give agents instructions and tools. Each skill is a declarative package that tells the model *what it can do* and *how to do it* — without requiring per-skill code in the gateway. This guide covers what skills are, how to create and manage them, and how the versioning system keeps them consistent.

## What a Skill Is

A skill is a directory under `~/.chai/skills/` containing at minimum a `SKILL.md` file. The skill's directory name is its stable **package id** (for example `files`). A skill can provide:

- **Instructions** — `SKILL.md` contains Markdown (with optional YAML frontmatter) that the model reads. This is the skill's "brain": what it knows, how it should behave, and how to use its tools.
- **Tool definitions** — An optional `tools.json` declares typed tool schemas that the model can call. Without this file, the skill contributes context only (no callable tools).
- **Scripts** — An optional `scripts/` subdirectory holds helper scripts that tool execution can invoke (for parameter resolution, post-processing, etc.).

A skill without `tools.json` is still useful — its `SKILL.md` content appears in the agent's system context, giving the model domain knowledge. A skill with `tools.json` adds callable tools on top of that knowledge.

## Directory Layout

The simplest skill (instructions only):

```text
~/.chai/skills/my-skill/
  SKILL.md
```

A skill with tools and scripts:

```text
~/.chai/skills/my-skill/
  SKILL.md
  tools.json
  scripts/
    resolve-path.sh
```

A versioned skill (the standard layout after `chai init` or `chai skill init`):

```text
~/.chai/skills/my-skill/
  active -> versions/a1b2c3d4e5f6/    # symlink: which revision is "live"
  versions/
    a1b2c3d4e5f6/                      # immutable snapshot (content hash)
      SKILL.md
      tools.json
      scripts/
        resolve-path.sh
    b2c3d4e5f6a7/                      # older snapshot
      SKILL.md
      tools.json
```

The gateway loads files from the directory that `active` resolves to. Older snapshots under `versions/` stay on disk for rollback unless you delete them.

## SKILL.md

`SKILL.md` is a Markdown file with optional YAML frontmatter:

```yaml
---
name: my-skill
description: Short description for catalogs and prompts.
metadata:
  requires:
    bins: ["some-cli"]
---

# My Skill

Instructions for the model go here. This content is included in the
agent's system context when the skill is enabled.
```

**Frontmatter fields:**

| Field | Required | Description |
|-------|----------|-------------|
| `name` | No | Skill name (defaults to directory name). |
| `description` | No | Short description for catalogs and prompts. |
| `capability_tier` | No | Minimum model capability: `minimal` (pure schema, 7B target), `moderate` (some interpretation, 13B–30B), or `full` (judgment-tier, 70B+ or cloud). The gateway warns at startup when an enabled skill's tier exceeds the agent's likely model capability. |
| `variant_of` | No | Links to a related skill at a different tier (e.g., `git-read` declares `variant_of: git`). The gateway warns when variant skills with overlapping tools are both enabled for the same agent. |
| `metadata.requires.bins` | No | List of binary names (e.g. `["git"]`). The skill is only loaded when every listed binary is on the system `PATH`. |

When the gateway builds the agent's system context, it strips the frontmatter and inlines the body (see [Context Modes](#context-modes) below).

## tools.json

`tools.json` declares the callable tools a skill provides. It has three top-level sections:

- **`tools`** — Array of tool definitions for the LLM (name, description, JSON Schema parameters).
- **`allowlist`** — Binary name → array of allowed subcommands. Only these (binary, subcommand) pairs may be executed.
- **`execution`** — Array mapping each tool to a binary, subcommand, argument template, and optional processing hooks.

A minimal example with one tool:

```json
{
  "tools": [
    {
      "name": "git_status",
      "description": "Git status",
    }
  ],
  "allowlist": {
    "git": ["status"]
  },
  "execution": [
    {
      "tool": "git_status",
      "binary": "git",
      "subcommand": "status"
    }
  ]
}
```

The `args` array maps each JSON parameter to a command-line argument kind: `positional`, `flag`, `flagifboolean`, or `stdin`. For full field details including `resolveCommand`, `postProcess`, `sideRead`, `writePath`, and `readPath`, see the tools schema spec.

## Enabling Skills on an Agent

Skills are discovered globally under `~/.chai/skills/`, but each agent opts in independently. In your `config.json`, set the `enabledSkills` array on an agent entry:

```json
{
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "ollama",
      "defaultModel": "llama3.2:3b",
      "enabledSkills": ["files-read"]
    }
  ]
}
```

Missing or empty `enabledSkills` means **no** skill tools and **no** skill context for that agent. Workers can also have their own `enabledSkills` lists, independent of the orchestrator.

## Context Modes

How skill content appears in the agent's system context depends on the `contextMode` setting for that agent:

- **`full`** — Each enabled skill's full `SKILL.md` body (frontmatter stripped) is inlined under a `### <skill name>` heading. The model sees all instructions every turn. This is the default.
- **`readOnDemand`** — Only a compact list of skill names and descriptions is included. The model gets a `read_skill` tool it can call to load a skill's full content on demand. This reduces context size when skills are large or numerous.

Set `contextMode` on the agent entry in `config.json`:

```json
{
  "id": "orchestrator",
  "role": "orchestrator",
  "enabledSkills": ["files"],
  "contextMode": "readOnDemand"
}
```

## Bundled Skills

`chai init` extracts bundled skills from the application to `~/.chai/skills/` using content-addressed versioning. On re-run, new version snapshots are created but the `active` symlink for each skill is left unchanged — existing customizations are preserved. See [Configuration](03-configuration.md) for the full `chai init` behavior.

Bundled skills cover common agent operations with no external binary dependencies beyond `git` and `curl`:

| Skill | Tools | Tier | Description |
|-------|-------|------|-------------|
| `files-read` | 4 | minimal | Read-only file inspection (read, list, search, read lines) |
| `files` | 9 | full | Full file operations including write, append, delete, line-level patching, and bulk find-and-replace |
| `git-read` | 5 | minimal | Read-only git operations (status, log, diff, show, branch) |
| `git` | 10 | moderate | Local git operations (read + add, commit, branch create, checkout, branch delete) |
| `git-remote` | 4 | minimal | Git remote operations (clone, pull, push, remote) |
| `notes-read` | 4 | minimal | Read-only inspection (read, list, search, read lines) |
| `notes` | 10 | moderate | Notes CRUD (read, write, append, delete, list, search, read lines, write lines, replace, delete dir) |
| `notes-daily` | 3 | minimal | Daily note operations with date-based path resolution |
| `notes-frontmatter` | 3 | moderate | YAML frontmatter read, edit, and delete for notes notes |
| `notes-wikilink` | 5 | moderate | Wikilink discovery and rename: backlinks, outlinks, tag search, broken link detection, note rename |
| `logs` | 2 | minimal | Gateway log access (recent lines, pattern search) |
| `rss` | 2 | moderate | RSS feed monitoring via curl |
| `skills` | 9 | full | Skill generation and management (discover, init, write, validate, delete) |
| `skills-read` | 3 | minimal | Read-only skill inspection (read, list, validate) |
| `skills-design` | 0 | minimal | Design principles for skill tools (context-only, no callable tools) |

Additional skills are available in the [chai-examples](https://github.com/chora-ai/chai-examples) repository as reference implementations.

## Skill Variants

Several bundled skills come in **variants** — related skills that provide different capability tiers or extensions for the same domain. Variants with overlapping tool surfaces (declared via `variant_of`) should not be co-enabled for the same agent.

| Domain | Variant | Tools | Tier | Best For |
|--------|---------|-------|------|----------|
| Files | `files-read` | 4 | minimal | Inspector agents that only need to read files |
| Files | `files` | 9 | full | Agents that need to write, patch, replace, and delete files |
| Git | `git-read` | 5 | minimal | Reviewer agents that only need read access |
| Git | `git` | 10 | moderate | Local development (commit, branch, checkout) |
| Git | `git-remote` | 4 | minimal | Remote operations (clone, push, pull) — use alongside `git` or independently |
| notes | `notes-read` | 4 | minimal | Inspector agents that only need to read notes |
| notes | `notes-daily` | 3 | minimal | Daily note creation and appending |
| notes | `notes-wikilink` | 5 | moderate | Wikilink discovery and note renaming |
| notes | `notes-frontmatter` | 3 | moderate | Frontmatter read, edit, and delete |
| notes | `notes` | 10 | moderate | Full note CRUD including bulk find-and-replace |
| Skills | `skills-read` | 3 | minimal | Inspection and validation only |
| Skills | `skills` | 9 | full | Skill authoring and management |

Read-only variants (`-read` suffix) declare `variant_of` to indicate their tool surface is a subset of the base skill. The gateway warns at startup when two skills with a `variant_of` relationship are both enabled for the same agent — this usually indicates a configuration error. Extension variants (e.g., `git-remote`, `notes-wikilink`, `notes-frontmatter`) have complementary tools and do not declare `variant_of`, so they can be safely co-enabled with the base skill. Note that `notes-daily` does declare `variant_of: notes` — co-enabling `notes-daily` with `notes` will trigger a variant overlap warning.

**Rule of thumb:** Enable one `-read` variant per domain per agent. Extension variants without `variant_of` can be added freely alongside the base skill. Skills with `variant_of` (including `notes-daily`) should not be co-enabled with their parent skill unless you accept the warning.

## Creating a Skill

### Using the CLI

Initialize a new skill with template files:

```bash
chai skill init my-skill --description "Does something useful"
```

This creates `~/.chai/skills/my-skill/` with a starter `SKILL.md` and `tools.json`. You can then customize the content using the write commands or by editing files directly.

### Inspecting Skills

```bash
# List all installed skills and their status
chai skill list

# Read a skill's SKILL.md
chai skill read my-skill --file skill_md

# Read a skill's tools.json
chai skill read my-skill --file tools_json
```

### Validating Skills

After creating or editing a `tools.json`, validate it against the schema:

```bash
chai skill validate my-skill
```

This checks JSON conformance and reports errors before the gateway loads the skill.

## Updating a Skill

Updating a skill creates a **new revision** — it never edits the current snapshot in place. The versioning system computes a content hash for the new tree and stores it as an immutable snapshot under `versions/`.

### Using the CLI (Recommended)

The `chai skill write-*` commands copy the current active tree, apply your change, compute the new hash, and repoint `active`:

| Command | What It Updates |
|---------|-----------------|
| `chai skill write-skill-md <name> --content '...'` | `SKILL.md` |
| `chai skill write-tools-json <name> --content '...'` | `tools.json` (validated before write) |
| `chai skill write-script <name> <base> --content '...'` | `scripts/<base>.sh` |

Each command creates a **new** revision. For multi-file changes, run one command per file — each builds a new snapshot from whatever `active` was at the start of that command. For example, updating `SKILL.md` and then `tools.json` creates two new revisions. That is normal.

### What the Version Directory Name Means

Under `versions/`, each directory **must** be named with exactly **12 lowercase hexadecimal characters**: the truncated SHA-256 of the skill's canonical content. The name is both the address and the integrity check. If you create a directory with a made-up name, the hash will not match the content and downstream checks can fail.

**How the hash is computed:**

1. Collect every **regular file** under the directory, with paths relative to that directory, sorted lexicographically.
2. Exclude the top-level `versions/` directory and `active` symlink (so old snapshots don't affect the hash).
3. Exclude symlinks everywhere (only regular files count).
4. For each path in order, update SHA-256 with the path bytes, then a `NUL` byte, then the raw file bytes.
5. Take the first 12 hex characters of the digest.

The exact bytes (including newlines), path spellings, and which files exist all affect the hash. File permissions are **not** part of the hash.

### Manual Workflow

Use this when you want to edit several files in an editor and produce **one** new revision:

1. **Copy the active tree** — Resolve `active` (or copy from `versions/<current>/`) into a temporary working directory. Copy only the skill payload (`SKILL.md`, `tools.json`, `scripts/`); do not include `versions/` or `active`.
2. **Edit** your files in the working directory.
3. **Compute the 12-character content hash** using the algorithm above. There is no `chai skill hash` command today; use a small script or reproduce the algorithm from `versioning.rs`.
4. **Install the snapshot** — `mkdir -p ~/.chai/skills/<name>/versions/<hash>`, copy your working tree into it, and repoint `active` to `versions/<hash>` (relative symlink).
5. **Validate** — Run `chai skill validate <name>`.

### What Not To Do

- **Do not** edit files in place under `versions/<hash>/`. Those directories are immutable; changing bytes without changing the directory name breaks the content-addressed model.
- **Do not** add a new directory under `versions/` with a made-up name. It must equal the hash of that directory's files.

## Lockfiles and Rollback

Skills are shared across profiles; each profile can pin active hashes in `~/.chai/profiles/<profile>/skills.lock`.

- **`chai skill lock`** — Record the current active hash for each discovered skill and bump the lock generation.
- **`chai skill generations`** — List stored generations.
- **`chai skill rollback <generation>`** — Restore a saved lock generation and repoint `active` symlinks for skills that still have the matching snapshot on disk.

After changing skills, whether you need `lock` depends on how strictly you use lock checking for the gateway. Lockfiles give you reproducibility: pinned versions ensure the same skill content across restarts.

### Skill Lock Mode

The `skills.lockMode` field in `config.json` controls how the gateway handles the lockfile at startup:

| Mode | Behavior |
|------|----------|
| `"strict"` (default) | The lockfile acts as a **complete manifest**. The gateway **refuses to start** when the lockfile is missing, any enabled skill has no lock entry (unpinned), or any pinned skill's active version does not match its locked hash. |
| `"warn"` | The gateway logs a warning for each mismatched skill but continues loading. Unpinned skills (no lock entry) load normally. When no lockfile is present, verification is skipped entirely. |

In `strict` mode, every enabled skill must be pinned in the lockfile — no orphans allowed. If the lockfile is missing or incomplete, the gateway refuses to start. Run `chai skill lock` to create or update the lockfile, or set `lockMode` to `"warn"` to allow unpinned skills.

### When `chai init` Generates a Lock

`chai init` automatically generates `skills.lock` for each profile it creates. If the `assistant` or `developer` profile directory does not yet exist, `chai init` seeds the profile and writes a lock file that pins all bundled skills at their current versions. This ensures the gateway can start under the default `strict` mode.

Re-running `chai init` on an already-initialized directory does **not** overwrite existing lock files. Profiles that already exist retain whatever lock state they have.

### When to Run `chai skill lock` Manually

You need to call `chai skill lock` yourself when:

- **You create a new profile manually** — Profiles added by hand (outside of `chai init`) do not have a lock file. Run `chai skill lock` after creating the profile and switching to it.
- **You enable a new skill** — After adding a skill to `enabledSkills` in `config.json`, the new skill has no lock entry. Run `chai skill lock` to pin it, or the gateway will refuse to start in `strict` mode.
- **You update a skill** — After writing a new skill version (`chai skill write-*`), the active hash changes. Run `chai skill lock` to update the lock, or the gateway will refuse to start in `strict` mode.
- **You rollback a skill** — After `chai skill rollback`, the lock file is already updated for you. No additional step is needed.

```bash
# After creating a new profile or updating skills
chai skill lock
```

## Deleting a Skill

Remove a skill package entirely (the directory and all version snapshots):

```bash
chai skill delete my-skill
```

## Summary

| Question | Answer |
|----------|--------|
| What is a skill? | A directory under `~/.chai/skills/` with `SKILL.md` and optional `tools.json` and `scripts/`. |
| How do I enable a skill? | Add its name to the `enabledSkills` array on an agent entry in `config.json`. |
| Can a skill provide context without tools? | Yes — a skill without `tools.json` contributes instructions only. |
| What bundled skills are available? | 15 skills covering files, git, logs, notes, RSS, and skill management. See [Bundled Skills](#bundled-skills). |
| What are skill variants? | Related skills at different tiers for the same domain (e.g., `files-read` vs `files`). See [Skill Variants](#skill-variants). |
| How do I create a skill? | `chai skill init <name> --description "..."`, then customize the files. |
| How do I update a skill? | Use `chai skill write-*` commands (one per file), or use the manual workflow for multi-file edits. |
| Can I name a version directory arbitrarily? | No. Under `versions/`, the name must be the 12-hex content hash. |
| How do I roll back? | `chai skill lock` to save, `chai skill rollback <generation>` to restore. |
| What is `skills.lockMode`? | Controls lock verification at startup: `strict` (default, gateway refuses to start on mismatch) or `warn` (log warning, continue). No effect until a `skills.lock` file exists. |
| When do I need to run `chai skill lock`? | After creating a profile manually or updating skills. `chai init` generates the lock for profiles it creates. |

## Try It

For hands-on skill walkthroughs, see the user journeys:

- [Skill: Files](../journey/05-skill-files.md) — Test the files skill: read, write, patch, search, and delete.
- [Skill: Notes](../journey/06-skill-notes.md) — Test the notes skill and its extensions.
- [Skill: Skills](../journey/07-skill-skills.md) — Test the skills skill: inspect, validate, and create skill packages.
