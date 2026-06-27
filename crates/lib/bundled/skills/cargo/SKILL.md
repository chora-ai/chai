---
description: Verify Rust code compiles and passes tests.
capability_tier: moderate
metadata:
  requires:
    bins: [["cargo"], ["nix"]]
---

## Skill Directives

- Always run `cargo_check` after making code changes to verify they compile
- Always run `cargo_test` after making code changes to verify tests are passing

## Skill Guidelines

- Use `path` and `package` to target the repository path and a specific package
