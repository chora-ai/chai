# Skills

Skills are markdown-based instructions (one per directory) that can be loaded and used by the agent. The format and bundled skills for this project are described below.

## Format

- **Layout**: Each skill lives in its own directory. The loader discovers skills from:
  - The **primary skill root**: default is the config directory’s **skills** subdirectory (e.g. `~/.chai/skills`). This can be overridden by **`skills.directory`** in config (relative paths are resolved against the config file’s parent; absolute paths used as-is).
  - **`skills.extraDirs`**: additional directories (lowest precedence; same name overwrites).
  - In each such directory, the loader looks for subdirectories containing **`SKILL.md`**. Precedence: primary root first, then extra dirs; later entries overwrite earlier by skill name.
- **Content**: `SKILL.md` is Markdown with optional YAML frontmatter between `---` delimiters.
- **Optional tools**: A skill directory may also contain **`tools.json`** (see [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md)). Only skills with a valid `tools.json` expose callable tools to the agent; skills without it are still loaded for context (their SKILL.md appears in the system message when context mode is `full`, or in the compact list when `readOnDemand`).
- **Optional scripts**: A skill directory may contain a **`scripts/`** subdirectory. When **`skills.allowScripts`** is true in config, tools can reference these scripts in `resolveCommand.script` (e.g. for param resolution); the executor runs them via `sh` with no allowlist entry. See [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md).

### Frontmatter

| Field | Required | Description |
|-------|----------|-------------|
| `name` | No | Skill name (defaults to directory name). |
| `description` | No | Short description for catalogs and prompts. |
| `metadata` | No | Optional structured metadata (see below). |

### Metadata (project-neutral)

This project uses a **project-neutral** metadata shape so skills can be shared across runtimes without tying them to a single product.

- **`metadata.requires.bins`** — Optional list of binary names (e.g. `["obsidian"]`). The skill is **only loaded** when every listed binary is found on the system `PATH`. If any are missing, the skill is skipped (e.g. so the Obsidian skill is only available when the Obsidian CLI is installed).

**Enabling skills:** Only skills listed in **`skills.enabled`** in config are loaded; the default is none. Add the skill names you want (e.g. `["notesmd-cli-daily"]` or `["obsidian"]`). If you have both notesmd-cli and obsidian on PATH but want only one, list only that skill in **`skills.enabled`**. See the main [README](../../README.md) Configuration section. **If you see "loaded 0 skill(s)"**, add the skill name to **`skills.enabled`** and ensure its binary is on **PATH when the gateway starts** (e.g. run `which notesmd-cli` in the same terminal).

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

**Bundled skills** are the skills shipped with the application (in `crates/lib/config/skills/`); `chai init` extracts them to the user’s skill root. The **default skill root** is the directory from which skills are loaded when no override is set (e.g. `~/.chai/skills`); it can be overridden with `skills.directory`.