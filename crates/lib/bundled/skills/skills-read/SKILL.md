---
description: Inspect and validate Chai skill packages (read-only).
capability_tier: minimal
variant_of: skills
metadata:
  requires:
    bins: ["chai"]
---

## Skill Directives

- always validate tools.json before reporting a skill as conformant
- always report all errors and warnings from validation, not just the first
- always use `skills_read` to examine skill contents when diagnosing errors

## Audit Workflow

1. Call `skills_validate` with `skill_name` and review the output: **ERROR** lines are structural failures, **WARNING** lines are potential issues, **PASS** indicates conformance.
2. If errors are found, call `skills_read` with `file` set to `tools_json` to examine the content.

## Security Audit

When auditing a skill for security, check every `ArgMapping` in tools.json:

1. Does the parameter receive path-like values (absolute paths, `./`-prefixed paths)? If yes, it should be annotated with `readPath: true` or `writePath: true`.
2. Is it a write target? If yes, it should use `writePath` (auto-creates parent dirs).
3. Does any parameter use `unsafePath: true`? This bypasses sandbox validation — verify it is justified.
4. If a parameter has a `resolveCommand` script that resolves paths, is `readPath`/`writePath` also set? A resolve script on a path parameter without `readPath` is an incomplete security boundary.
5. Does any tool write to a git repository? If yes, does it enforce branch protection for `main` and `release/*` at the tool level?

Note: Unannotated `positional` and `flag` parameters are subject to a runtime path-like value check by default — values starting with `/`, `~`, or containing `..` are rejected. This makes the default safe, but parameters that legitimately need path-like values must be annotated.

## Cross-Validation Workflow

1. Call `skills_list` to get the inventory.
2. For each skill of interest, call `skills_validate` to check tools.json.
3. Call `skills_read` with `file` set to `skill_md` to check SKILL.md consistency: verify frontmatter includes `description`, `capability_tier`, and `metadata.requires.bins`, and that `metadata.requires.bins` matches binaries in the allowlist.
4. Call `skills_read` with `file` set to `tools_json` and run the security audit checklist above.
