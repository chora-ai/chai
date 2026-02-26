# Skills

Skills are markdown-based instructions (one per directory) that can be loaded and used by the agent. The format and bundled skills for this project are described below.

## Format

- **Layout**: Each skill lives in its own directory. The loader looks for a file named `SKILL.md` in each subdirectory of the configured skill roots (bundled, workspace, extra).
- **Content**: `SKILL.md` is Markdown with optional YAML frontmatter between `---` delimiters.

### Frontmatter

| Field | Required | Description |
|-------|----------|-------------|
| `name` | No | Skill name (defaults to directory name). |
| `description` | No | Short description for catalogs and prompts. |
| `metadata` | No | Optional structured metadata (see below). |

### Metadata (project-neutral)

This project uses a **project-neutral** metadata shape so skills can be shared across runtimes without tying them to a single product.

- **`metadata.requires.bins`** — Optional list of binary names (e.g. `["obsidian"]`). The skill is **only loaded** when every listed binary is found on the system `PATH`. If any are missing, the skill is skipped (e.g. so the Obsidian skill is only available when the Obsidian CLI is installed).

**Disabling a skill when both binaries are on PATH:** If you have both `obsidian` and `notesmd-cli` installed but want to load only one, set **`skills.disabled`** in your config file to an array of skill names to skip (e.g. `["obsidian"]` to use only the notesmd-cli skill). See the main [README](../../README.md) Configuration section. **If you then see "loaded 0 skill(s)"**, the remaining skill (e.g. notesmd-cli) is gated based on its binary being on **PATH when the gateway starts** — ensure that binary is on PATH in the environment where you run the gateway (e.g. run `which notesmd-cli` in the same terminal, or install the CLI so it is on the default PATH for your desktop/login).

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

- **obsidian** — Manage Obsidian vaults via the **official Obsidian CLI** (early access; binary `obsidian`). Only loaded when `obsidian` is on `PATH`. Not available to all users; see [Obsidian CLI — early access](https://help.obsidian.md/cli).
- **notesmd-cli** — Manage Obsidian vaults via [NotesMD CLI](https://github.com/yakitrak/notesmd-cli) (`notesmd-cli` binary). Works without Obsidian running. Only loaded when `notesmd-cli` is on `PATH`. Use this if you do not have access to the official early access CLI.
