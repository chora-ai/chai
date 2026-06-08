---
description: Inspect and validate Chai skill packages (read-only).
capability_tier: minimal
model_variant_of: skills
metadata:
  requires:
    bins: ["chai"]
---
# Skills (Read-Only)

Inspect and validate Chai skill packages. This skill provides read-only access to installed skills for conformance checking, structural validation, and inventory reporting.

## Skill Directives

- always validate tools.json before reporting a skill as conformant
- always report all errors and warnings from validation, not just the first
- never modify skill files — this skill is read-only
- always use `skills_read` to examine skill contents when diagnosing errors

## Available Tools

- `skills_list`
- `skills_read`
- `skills_validate`

## Tool Instructions

### List installed skills

1. Call `skills_list`.
2. Review the output showing each skill's SKILL.md and tools.json status.
3. Skills with `tools: 0` are either placeholders or context-only skills.

### Read a skill's files

1. Call `skills_read` with `skill_name` and `file` set to `skill_md` or `tools_json`.
2. Study the structure for reference.

### Validate a skill

1. Call `skills_validate` with `skill_name`.
2. Review the output:
   - **ERROR** lines indicate structural failures that must be fixed.
   - **WARNING** lines indicate potential issues that should be reviewed.
   - **PASS** indicates the tools.json is structurally conformant.
3. If errors are found, call `skills_read` with `file` set to `tools_json` to examine the actual content and identify the root cause.
4. Report findings with specific errors, the affected tool names, and what needs to change.

### Audit a skill's SKILL.md

1. Call `skills_read` with `skill_name` and `file` set to `skill_md`.
2. Verify:
   - Frontmatter includes `description`, `capability_tier`, and `metadata.requires.bins`.
   - `capability_tier` is one of `minimal`, `moderate`, or `full`.
   - If the skill has tools, the body lists them in an "Available Tools" section.
   - Tool names in SKILL.md match the names in tools.json.
   - Instructions reference only tools that exist in tools.json.

### Cross-validate a skill

1. Call `skills_list` to get the inventory.
2. For each skill of interest, call `skills_validate` to check tools.json.
3. Call `skills_read` with `file` set to `skill_md` to check SKILL.md.
4. Verify consistency: tools listed in SKILL.md should match tools.json, and `metadata.requires.bins` should match binaries in the allowlist.

## Examples

### skills_list

{}

### skills_read

{"skill_name": "notesmd-daily", "file": "tools_json"}

### skills_validate

{"skill_name": "notesmd-daily"}
