---
description: Manage skill packages — discover, inspect, validate, create, and delete.
capability_tier: full
metadata:
  requires:
    bins: ["chai"]
---

## Skill Directives

- Never add subcommands to the allowlist that the skill does not use
- Never include `resolveCommand` unless a parameter genuinely needs runtime resolution

## Skill Guidelines

- Read a reference skill (e.g., `skills_read` with `file: 'tools_json'`) before generating a new skill.

## Three-File Tool Descriptor

A skill's tool surface is defined by three JSON files with distinct responsibilities:

| File | Responsibility | Root type |
|------|---------------|-----------|
| `tools.json` | Tool definitions for the LLM (name, description, parameter schemas) | Array |
| `allowlist.json` | Binary→subcommand security grants | Object |
| `execution.json` | Per-tool execution mapping (binary, subcommand, arg mapping) | Array |

Write each file independently with `skills_write_tools_json`, `skills_write_allowlist_json`, and `skills_write_execution_json`. Validate with `skills_validate` to check schema conformance and cross-file consistency.

## Generation Workflow

1. Call `skills_discover` with `binary` to discover the CLI interface. Call again with `subcommand` for detailed flags and argument types.
2. Design the tool surface: select subcommands to expose, name tools as `<skillname>_<operation>`, choose parameter kinds (`positional`, `flag`, `flagifboolean`), scope the allowlist to only used subcommands, and add `resolveCommand` only when needed.
3. Annotate path parameters per Sandbox Security in `skills-design/SKILL.md`.
4. Call `skills_init` with `skill_name` and `description`.
5. Call `skills_write_tools_json` with the tool definitions array.
6. Call `skills_write_allowlist_json` with the allowlist object.
7. Call `skills_write_execution_json` with the execution specs array.
8. Call `skills_validate` to confirm conformance.
9. Write the SKILL.md: frontmatter with `description`, `capability_tier`, and `metadata.requires.bins`; skill directives (only genuinely additive content — see `skills-design/SKILL.md`); composed workflows for multi-step operations. Do not enumerate tool names or restate parameter descriptions.
10. Call `skills_write_skill_md` with the complete markdown content.

## Resolve Scripts

Scripts are referenced by `resolveCommand` in execution.json. Write with `skills_write_script`. The script must have a `#!/bin/sh` shebang and receives arguments from `resolveCommand.args` where `$param` tokens are replaced with the current parameter value. On failure or empty stdout, the original value is kept.

Resolve scripts handle path resolution (relative-to-absolute, defaults). Both a resolve script and `readPath`/`writePath` are needed for sandbox-validated path access.
