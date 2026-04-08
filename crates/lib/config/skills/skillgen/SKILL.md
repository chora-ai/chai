---
name: skillgen
description: Generate Chai skill packages from CLI interfaces — discover, design, write, and iterate.
metadata:
  requires:
    bins: ["chai"]
generated_from:
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: full
---

# Skill Generation

Generate new Chai skill packages by discovering CLI interfaces, designing tool
surfaces, and producing conformant SKILL.md + tools.json + optional scripts.
This skill is intended for the developer profile.

## Skill Directives

- always follow the generation workflow in order (discover, reference, design, generate)
- always read the reference skill (`notesmd-daily`) before generating a new skill
- always include `generated_from` metadata in produced SKILL.md frontmatter
- always use `normalizeNewlines: true` for any parameter that accepts multi-line content
- never add subcommands to the allowlist that the skill does not use
- never include `resolveCommand` unless a parameter genuinely needs runtime resolution

## Available Tools

- `skillgen_discover`
- `skillgen_read`
- `skillgen_init`
- `skillgen_write_skill_md`
- `skillgen_write_tools_json`
- `skillgen_write_script`

## Tool Instructions

### Discover a CLI interface

1. Call `skillgen_discover` with `binary` set to the target CLI name.
2. Record the available subcommands from the output.
3. For each subcommand relevant to the skill's purpose, call `skillgen_discover`
   with both `binary` and `subcommand` to get flags, argument types, and
   descriptions.
4. Note required vs optional arguments, flag names, and value types.

### Read a reference skill

1. Call `skillgen_read` with `skill_name` set to `notesmd-daily` and `file` set
   to `tools_json`.
2. Study the structure: how tools map to execution specs, how the allowlist is
   scoped, how arg mappings use `kind`, `flag`, `normalizeNewlines`, and
   `resolveCommand`.
3. Call `skillgen_read` with `file` set to `skill_md` to study the SKILL.md
   format: frontmatter fields, directives section, tool list, and step-by-step
   instructions.

### Generate a new skill

1. Call `skillgen_init` with `skill_name` and `description` to create the
   directory. If the skill already exists, skip this step.
2. Design the tool surface:
   - Select subcommands to expose (not every CLI subcommand needs a tool).
   - Name tools as `<skillname>_<operation>` (e.g. `notesmd_search`).
   - Design parameters using JSON Schema types matching the CLI's expectations.
   - Scope the allowlist to only the binary+subcommand pairs used.
   - Choose arg `kind` for each parameter: `positional` for bare arguments,
     `flag` for `--name value` pairs, `flagifboolean` for `--flag` toggles.
   - Add `normalizeNewlines: true` to any parameter that accepts multi-line
     content.
   - Add `resolveCommand` only when a parameter needs runtime resolution (e.g.
     computing a file path from a date).
3. Call `skillgen_write_tools_json` with the complete JSON content.
4. Write the SKILL.md:
   - Include frontmatter with `name`, `description`, `metadata.requires.bins`,
     and `generated_from` block (recording `cli`, `cli_version`, `spec_version`,
     `generator_model`, `capability_tier`).
   - If this is a constrained variant of another skill, include
     `model_variant_of`.
   - Write skill directives, available tools list, and tool instructions.
   - For `minimal` tier: every operation must be a numbered step-by-step
     sequence with no judgment required.
   - For `moderate` tier: steps may include conditional branches.
   - For `full` tier: instructions may describe goals with reasoning latitude.
5. Call `skillgen_write_skill_md` with the complete markdown content.

### Write a resolve script

1. Determine whether a parameter requires runtime resolution (e.g. a bare date
   needs to become a full file path).
2. Write the script content with a `#!/bin/sh` shebang. The script receives
   arguments from `resolveCommand.args` where `$param` tokens are replaced with
   the current parameter value. On failure or empty stdout, the original value
   is kept.
3. Call `skillgen_write_script` with `skill_name`, `script_name` (without
   `.sh`), and `content`.
4. Reference the script in the tool's `resolveCommand.script` field in
   tools.json (use the name without `.sh`).

## Schema Reference

### tools.json structure

```json
{
  "tools": [
    {
      "name": "tool_name",
      "description": "Short description for the model.",
      "parameters": {
        "type": "object",
        "required": ["param1"],
        "properties": {
          "param1": { "type": "string", "description": "Clear description" }
        }
      }
    }
  ],
  "allowlist": {
    "binary-name": ["subcommand1", "subcommand2"]
  },
  "execution": [
    {
      "tool": "tool_name",
      "binary": "binary-name",
      "subcommand": "subcommand1",
      "args": [
        { "param": "param1", "kind": "positional" }
      ]
    }
  ]
}
```

### Argument kinds

- `positional` — bare argument appended to argv
- `flag` — `--flag value` pair; uses `flag` field for the flag name (defaults to
  `param` name); omitted when param is null/missing
- `flagifboolean` — emits `flagIfTrue` when true, `flagIfFalse` when false

### resolveCommand

```json
{
  "param": "date",
  "kind": "positional",
  "resolveCommand": {
    "script": "resolve-daily-path",
    "args": ["$param"]
  }
}
```

Script must be in the skill's `scripts/` directory. `$param` is replaced with
the current parameter value. Trimmed stdout becomes the new value.

### SKILL.md frontmatter template

```yaml
---
name: <skill-name>
description: <one-line description>
metadata:
  requires:
    bins: ["<binary>"]
generated_from:
  cli: <source-binary>
  cli_version: "<version>"
  spec_version: "1.0"
  generator_model: <model>
  capability_tier: <minimal|moderate|full>
model_variant_of: <parent-skill>  # only if this is a variant
---
```

## Design Principles

- **Allowlist minimality** — only allow subcommands the skill uses.
- **Parameter clarity** — descriptions must be specific enough that the model
  never guesses. Prefer "today's date in YYYY-MM-DD format" over "date string."
- **Schema quality over tool count** — 3 well-defined tools outperform 10
  ambiguous tools. Each tool should have a singular purpose.
- **Capability tier honesty** — label the tier based on actual execution
  requirements, not aspirations.
- **Idempotent operations** — design tools so repeated calls with the same input
  produce the same result.
- **Read-back verification** — after writing, validate with the skillcheck skill
  or read back to confirm.

## Examples

### skillgen_discover

{"binary": "notesmd-cli", "subcommand": "search-content"}

### skillgen_read

{"skill_name": "notesmd-daily", "file": "tools_json"}

### skillgen_init

{"skill_name": "myfeed", "description": "Monitor RSS feeds for new content"}

### skillgen_write_skill_md

{"skill_name": "myfeed", "content": "---\nname: myfeed\ndescription: Monitor RSS feeds for new content\nmetadata:\n  requires:\n    bins: [\"curl\"]\n---\n\n# My Feed\n\nMonitor RSS feeds."}

### skillgen_write_tools_json

{"skill_name": "myfeed", "content": "{\"tools\": [], \"allowlist\": {\"curl\": [\"\"]}, \"execution\": []}"}

### skillgen_write_script

{"skill_name": "myfeed", "script_name": "resolve-feed-url", "content": "#!/bin/sh\necho \"$1\""}
