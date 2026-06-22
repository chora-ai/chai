---
description: Verify Rust code compiles and passes tests.
capability_tier: moderate
metadata:
  requires:
    bins: [["cargo"], ["nix"]]
---

## Skill Directives

- always run `cargo_check` after making code changes to verify they compile before proceeding
- always run `cargo_test` after implementing or modifying logic to verify tests pass
