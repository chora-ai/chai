# Skills

Skills give agents instructions and tools. Each skill is a declarative package that tells the model *what it can do* and *how to do it* — without requiring per-skill code in the gateway. This guide covers what skills are, how to create and manage them, and how the versioning system keeps them consistent.

## What a Skill Is

A skill is a directory under `~/.chai/skills/` containing at minimum a `SKILL.md` file. The skill's directory name is its stable **package id** (for example `notesmd-daily`). A skill can provide:

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
| `model_variant_of` | No | Links to a related skill at a different tier (e.g., `git-read` declares `model_variant_of: git`). The gateway warns when variant skills with overlapping tools are both enabled for the same agent. |
| `metadata.requires.bins` | No | List of binary names (e.g. `["obsidian"]`). The skill is only loaded when every listed binary is on the system `PATH`. |

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
      "name": "search_notes",
      "description": "Search notes by name.",
      "parameters": {
        "type": "object",
        "required": ["query"],
        "properties": {
          "query": { "type": "string", "description": "Search query" }
        }
      }
    }
  ],
  "allowlist": {
    "notesmd-cli": ["search", "create"]
  },
  "execution": [
    {
      "tool": "search_notes",
      "binary": "notesmd-cli",
      "subcommand": "search",
      "args": [{ "param": "query", "kind": "positional" }]
    }
  ]
}
```

The `args` array maps each JSON parameter to a command-line argument kind: `positional`, `flag`, `flagifboolean`, or `stdin`. For full field details including `resolveCommand`, `postProcess`, `sideRead`, `writePath`, and `readPath`, see the tools schema spec.

## Enabling Skills on an Agent

Skills are discovered globally under `~/.chai/skills/`, but each agent opts in independently. In your `config.json`, set the `skillsEnabled` array on an agent entry:

```json
{
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "ollama",
      "defaultModel": "llama3.2:3b",
      "skillsEnabled": ["notesmd-daily", "files"]
    }
  ]
}
```

Missing or empty `skillsEnabled` means **no** skill tools and **no** skill context for that agent. Workers can also have their own `skillsEnabled` lists, independent of the orchestrator.

## Context Modes

How skill content appears in the agent's system context depends on the `contextMode` setting for that agent:

- **`full`** — Each enabled skill's full `SKILL.md` body (frontmatter stripped) is inlined under a `### <skill name>` heading. The model sees all instructions every turn. This is the default.
- **`readOnDemand`** — Only a compact list of skill names and descriptions is included. The model gets a `read_skill` tool it can call to load a skill's full content on demand. This reduces context size when skills are large or numerous.

Set `contextMode` on the agent entry in `config.json`:

```json
{
  "id": "orchestrator",
  "role": "orchestrator",
  "skillsEnabled": ["notesmd-daily"],
  "contextMode": "readOnDemand"
}
```

## Bundled Skills

`chai init` extracts bundled skills from the application to `~/.chai/skills/` using content-addressed versioning. On re-run, new version snapshots are created but the `active` symlink for each skill is left unchanged — existing customizations are preserved. See [Configuration](03-configuration.md) for the full `chai init` behavior.

Bundled skills cover common agent operations with no external binary dependencies beyond `git` and `curl`:

| Skill | Tools | Tier | Description |
|-------|-------|------|-------------|
| `files-read` | 4 | minimal | Read-only file inspection (read, list, search, read lines) |
| `files` | 9 | full | Full file operations including write, append, delete, and line-level patching |
| `git-read` | 5 | minimal | Read-only git operations (status, log, diff, show, branch) |
| `git` | 8 | moderate | Local git operations (read + add, commit, branch create) |
| `git-remote` | 12 | full | Full git operations including clone, pull, push, and remote |
| `kb` | 6 | moderate | Knowledge base CRUD (read, write, append, delete, list, search) |
| `kb-daily` | 3 | minimal | Daily note operations with date-based path resolution |
| `kb-frontmatter` | 3 | moderate | YAML frontmatter read, edit, and delete for KB notes |
| `kb-wikilink` | 4 | moderate | Wikilink discovery: backlinks, outlinks, tag search, broken link detection |
| `kb-wikilink-write` | 1 | moderate | Rename KB notes with automatic wikilink updates |
| `rss` | 2 | moderate | RSS feed monitoring via curl |
| `skills` | 9 | full | Skill generation and management (discover, init, write, validate, delete) |
| `skills-read` | 3 | minimal | Read-only skill inspection (read, list, validate) |
| `skills-design` | 0 | minimal | Design principles for skill tools (context-only, no callable tools) |

Additional skills are available in the [chai-examples](https://github.com/chora-ai/chai-examples) repository as reference implementations.

## Skill Variants

Several bundled skills come in **variants** — related skills that provide different capability tiers for the same domain. Variants share tool names, so enabling overlapping variants for the same agent creates redundant tool surfaces.

| Domain | Variant | Tools | Tier | Best For |
|--------|---------|-------|------|----------|
| Files | `files-read` | 4 | minimal | Inspector agents that only need to read files |
| Files | `files` | 9 | full | Agents that need to write, patch, and delete files |
| Git | `git-read` | 5 | minimal | Reviewer agents that only need read access |
| Git | `git` | 8 | moderate | Local development (commit, branch) |
| Git | `git-remote` | 12 | full | Open-source workflows (clone, push) |
| Skills | `skills-read` | 3 | minimal | Inspection and validation only |
| Skills | `skills` | 9 | full | Skill authoring and management |

When you enable a variant skill, use its `model_variant_of` frontmatter to identify its parent. The gateway warns at startup when two variants share a `model_variant_of` relationship and are both enabled for the same agent — this usually indicates a configuration error.

**Rule of thumb:** Enable one variant per domain per agent. Choose the variant with the lowest tier that covers the agent's needs.

## Creating a Skill

### Using the CLI

Initialize a new skill with template files:

```bash
chai skill init --name my-skill --description "Does something useful"
```

This creates `~/.chai/skills/my-skill/` with a starter `SKILL.md` and `tools.json`. You can then customize the content using the write commands or by editing files directly.

### Inspecting Skills

```bash
# List all installed skills and their status
chai skill list

# Read a skill's SKILL.md
chai skill read --skill-name my-skill --file skill_md

# Read a skill's tools.json
chai skill read --skill-name my-skill --file tools_json
```

### Validating Skills

After creating or editing a `tools.json`, validate it against the schema:

```bash
chai skill validate --skill-name my-skill
```

This checks JSON conformance and reports errors before the gateway loads the skill.

## Updating a Skill

Updating a skill creates a **new revision** — it never edits the current snapshot in place. The versioning system computes a content hash for the new tree and stores it as an immutable snapshot under `versions/`.

### Using the CLI (Recommended)

The `chai skill write-*` commands copy the current active tree, apply your change, compute the new hash, and repoint `active`:

| Command | What It Updates |
|---------|-----------------|
| `chai skill write-skill-md --skill-name <name> --content '...'` | `SKILL.md` |
| `chai skill write-tools-json --skill-name <name> --content '...'` | `tools.json` (validated before write) |
| `chai skill write-script --skill-name <name> --script-name <base> --content '...'` | `scripts/<base>.sh` |

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
5. **Validate** — Run `chai skill validate --skill-name <name>`.

### What Not To Do

- **Do not** edit files in place under `versions/<hash>/`. Those directories are immutable; changing bytes without changing the directory name breaks the content-addressed model.
- **Do not** add a new directory under `versions/` with a made-up name. It must equal the hash of that directory's files.

## Lockfiles and Rollback

Skills are shared across profiles; each profile can pin active hashes in `~/.chai/profiles/<profile>/skills.lock`.

- **`chai skill lock`** — Record the current active hash for each discovered skill and bump the lock generation.
- **`chai skill generations`** — List stored generations.
- **`chai skill rollback <generation>`** — Restore a saved lock generation and repoint `active` symlinks for skills that still have the matching snapshot on disk.

After changing skills, whether you need `lock` depends on how strictly you use lock checking for the gateway. Lockfiles give you reproducibility: pinned versions ensure the same skill content across restarts.

## Deleting a Skill

Remove a skill package entirely (the directory and all version snapshots):

```bash
chai skill delete --skill-name my-skill
```

## Summary

| Question | Answer |
|----------|--------|
| What is a skill? | A directory under `~/.chai/skills/` with `SKILL.md` and optional `tools.json` and `scripts/`. |
| How do I enable a skill? | Add its name to the `skillsEnabled` array on an agent entry in `config.json`. |
| Can a skill provide context without tools? | Yes — a skill without `tools.json` contributes instructions only. |
| What bundled skills are available? | 13 skills covering files, git, knowledge base, RSS, and skill management. See [Bundled Skills](#bundled-skills). |
| What are skill variants? | Related skills at different tiers for the same domain (e.g., `files-read` vs `files`). See [Skill Variants](#skill-variants). |
| How do I create a skill? | `chai skill init --name <name> --description "..."`, then customize the files. |
| How do I update a skill? | Use `chai skill write-*` commands (one per file), or use the manual workflow for multi-file edits. |
| Can I name a version directory arbitrarily? | No. Under `versions/`, the name must be the 12-hex content hash. |
| How do I roll back? | `chai skill lock` to save, `chai skill rollback <generation>` to restore. |

## Try It

For hands-on skill walkthroughs, see the user journeys:

- [Skill NotesMD CLI](../journey/06-skill-notesmd.md) — Test the notesmd skill with an Obsidian vault.
- [Skill Obsidian (official CLI)](../journey/07-skill-obsidian.md) — Test the official Obsidian CLI skill.
