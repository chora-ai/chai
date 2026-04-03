---
status: stable
---

# Skills

Skills are markdown-based instructions (one per directory) that can be loaded and used by the agent. The format and bundled skills for this project are described below.

## Format

- **Layout**: Each skill lives in its own directory. The loader discovers packages only under **`~/.chai/skills`**: immediate subdirectories containing **`SKILL.md`**. There is no config override for the skill root.
- **Content**: `SKILL.md` is Markdown with optional YAML frontmatter between `---` delimiters.
- **Optional tools**: A skill directory may also contain **`tools.json`** (see [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md)). Only skills with a valid `tools.json` expose callable tools to the agent; skills without it are still loaded for context (their SKILL.md appears in the system message when context mode is `full`, or in the compact list when `readOnDemand`).
- **Optional scripts**: A skill directory may contain a **`scripts/`** subdirectory. Tools can reference these scripts in `resolveCommand.script` (e.g. for param resolution); the executor runs them via `sh` with no allowlist entry (only files under the skill’s `scripts/` dir are executed). See [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md).

### Frontmatter

| Field | Required | Description |
|-------|----------|-------------|
| `name` | No | Skill name (defaults to directory name). |
| `description` | No | Short description for catalogs and prompts. |
| `metadata` | No | Optional structured metadata (see below). |

### Metadata (project-neutral)

This project uses a **project-neutral** metadata shape so skills can be shared across runtimes without tying them to a single product.

- **`metadata.requires.bins`** — Optional list of binary names (e.g. `["obsidian"]`). The skill is **only loaded** when every listed binary is found on the system `PATH`. If any are missing, the skill is skipped (e.g. so the Obsidian skill is only available when the Obsidian CLI is installed).

**Enabling skills:** Discovery loads all packages under **`~/.chai/skills`**; **each agent** (orchestrator and workers) opts in with its own **`skillsEnabled`** array in **`config.json`**. Missing or empty **`skillsEnabled`** for an agent ⇒ **no** skill tools and **no** skill context for **that** agent. List the skill **names** you want per role (e.g. `["notesmd"]`). If a skill uses **`metadata.requires.bins`**, it is skipped at load time when binaries are missing—ensure CLIs are on **PATH when the gateway starts**. See [README](../../README.md), [CONFIGURATION.md](CONFIGURATION.md), and [CONTEXT.md](CONTEXT.md).

Example:

```yaml
---
name: my-skill
description: Does something that needs a CLI.
metadata:
  requires:
    bins: ["some-cli", "another-tool"]
---
```

## Differences from Other Formats

- **OpenClaw / AgentSkills**: Some ecosystems use `metadata.openclaw` (or similar product-namespaced keys) with nested fields such as `metadata.openclaw.requires.bins`, plus extra keys (e.g. `emoji`, `install`). This project does **not** use or parse those namespaced keys. We use only the neutral form above: `metadata.requires.bins`. If you import skills written for OpenClaw (or another framework), update the frontmatter to the neutral shape described here; product-specific keys are ignored by this loader.

## Bundled Skills

**Bundled skills** are the skills shipped with the application (in `crates/lib/config/skills/`); `chai init` extracts them to **`~/.chai/skills`**, which is the **only** skill package root the runtime loads.