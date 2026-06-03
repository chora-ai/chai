---
name: skills
description: Manage Chai skill packages — discover, inspect, validate, create, and delete.
metadata:
  requires:
    bins: ["chai"]
generated_from:
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: full
---

# Skills

Manage Chai skill packages — discover CLI interfaces, inspect and validate existing skills, create new skill packages, and delete obsolete ones.

## Skill Directives

- always follow the generation workflow in order (discover, reference, design, generate)
- always read the reference skill (`notesmd-daily`) before generating a new skill
- always include `generated_from` metadata in produced SKILL.md frontmatter
- always validate tools.json after writing with `skills_validate`
- always verify a skill exists with `skills_list` before deleting it
- never add subcommands to the allowlist that the skill does not use
- never include `resolveCommand` unless a parameter genuinely needs runtime resolution
- never delete bundled skills (those that ship with chai) unless explicitly instructed

## Available Tools

- `skills_discover`
- `skills_list`
- `skills_read`
- `skills_validate`
- `skills_init`
- `skills_write_skill_md`
- `skills_write_tools_json`
- `skills_write_script`
- `skills_delete`

## Tool Instructions

### Discover a CLI interface

1. Call `skills_discover` with `binary` set to the target CLI name.
2. Record the available subcommands from the output.
3. For each subcommand relevant to the skill's purpose, call `skills_discover` with both `binary` and `subcommand` to get flags, argument types, and descriptions.
4. Note required vs optional arguments, flag names, and value types.

### List installed skills

1. Call `skills_list`.
2. Review the output showing each skill's SKILL.md and tools.json status.
3. Skills with `tools: 0` are either placeholders or context-only skills.

### Read a skill's files

1. Call `skills_read` with `skill_name` and `file` set to `skill_md` or `tools_json`.
2. Study the structure for reference when designing new skills.

### Validate a skill

1. Call `skills_validate` with `skill_name`.
2. Review the output:
   - **ERROR** lines indicate structural failures that must be fixed.
   - **WARNING** lines indicate potential issues that should be reviewed.
   - **PASS** indicates the tools.json is structurally conformant.
3. If errors are found, call `skills_read` with `file` set to `tools_json` to examine the actual content and identify the root cause.
4. Always validate after writing tools.json.

### Generate a new skill

1. Call `skills_init` with `skill_name` and `description` to create the directory. If the skill already exists, skip this step.
2. Design the tool surface:
   - Select subcommands to expose (not every CLI subcommand needs a tool).
   - Name tools as `<skillname>_<operation>` (e.g. `notesmd_search`).
   - Design parameters using JSON Schema types matching the CLI's expectations.
   - Scope the allowlist to only the binary+subcommand pairs used.
   - Choose arg `kind` for each parameter: `positional` for bare arguments, `flag` for `--name value` pairs, `flagifboolean` for `--flag` toggles.
   - Add `resolveCommand` only when a parameter needs runtime resolution.
3. Call `skills_write_tools_json` with the complete JSON content.
4. Call `skills_validate` to confirm the tools.json is conformant.
5. Write the SKILL.md:
   - Include frontmatter with `name`, `description`, `metadata.requires.bins`, and `generated_from` block.
   - If this is a constrained variant, include `model_variant_of`.
   - Write skill directives, available tools list, and tool instructions.
   - For `minimal` tier: every operation must be a numbered step-by-step sequence with no judgment required.
   - For `moderate` tier: steps may include conditional branches.
   - For `full` tier: instructions may describe goals with reasoning latitude.
6. Call `skills_write_skill_md` with the complete markdown content.

### Write a resolve script

1. Determine whether a parameter requires runtime resolution.
2. Write the script content with a `#!/bin/sh` shebang. The script receives arguments from `resolveCommand.args` where `$param` tokens are replaced with the current parameter value. On failure or empty stdout, the original value is kept.
3. Call `skills_write_script` with `skill_name`, `script_name` (without `.sh`), and `content`.
4. Reference the script in the tool's `resolveCommand.script` field in tools.json.

### Delete a skill

1. Call `skills_list` to confirm the skill exists.
2. Call `skills_delete` with `skill_name` set to the skill directory name.
3. The skill directory and all version snapshots are permanently removed. This action is irreversible.
4. Call `skills_list` to verify the skill is no longer listed.

## Examples

### skills_discover

{"binary": "notesmd-cli", "subcommand": "search-content"}

### skills_list

{}

### skills_read

{"skill_name": "notesmd-daily", "file": "tools_json"}

### skills_validate

{"skill_name": "notesmd-daily"}

### skills_init

{"skill_name": "myfeed", "description": "Monitor RSS feeds for new content"}

### skills_write_skill_md

{"skill_name": "myfeed", "content": "---\\nname: myfeed\\ndescription: Monitor RSS feeds\\n---\\n\\n# My Feed"}

### skills_write_tools_json

{"skill_name": "myfeed", "content": "{\\"tools\\": [], \\"allowlist\\": {\\"curl\\": [\\"\\"]}, \\"execution\\": []}"}

### skills_write_script

{"skill_name": "myfeed", "script_name": "resolve-feed-url", "content": "#!/bin/sh\\necho \\"$1\\""}

### skills_delete

{"skill_name": "test-skill"}
