# Skills

Skills are markdown-based instructions (one per directory) that can be loaded and used by the agent. This directory holds the skill format and bundled skills for this project.

## Format

- **Layout**: Each skill lives in its own directory. The loader looks for a file named `SKILL.md` in each subdirectory of the configured skill roots (bundled, managed, workspace, extra).
- **Content**: `SKILL.md` is Markdown with optional YAML frontmatter between `---` delimiters.

### Frontmatter

| Field | Required | Description |
|-------|----------|-------------|
| `name` | No | Skill name (defaults to directory name). |
| `description` | No | Short description for catalogs and prompts. |
| `metadata` | No | Optional structured metadata (see below). |

### Metadata (project-neutral)

This project uses a **project-neutral** metadata shape so skills can be shared across runtimes without tying them to a single product.

- **`metadata.requires.bins`** — Optional list of binary names (e.g. `["obsidian-cli"]`). The skill is **only loaded** when every listed binary is found on the system `PATH`. If any are missing, the skill is skipped (e.g. so the Obsidian skill is only available when `obsidian-cli` is installed).

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

- **obsidian** — Manage Obsidian vaults and automate via `obsidian-cli`. Only loaded when `obsidian-cli` is on `PATH`.
