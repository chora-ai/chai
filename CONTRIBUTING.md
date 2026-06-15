# Contributing

Thank you for your interest in contributing to Chai. This document covers how to submit issues and pull requests.

## Issues

### Bug Reports

When filing a bug report, please include:

- **Summary** — A short description of the problem.
- **Steps to Reproduce** — The minimal steps needed to trigger the bug.
- **Expected Behavior** — What you expected to happen.
- **Actual Behavior** — What happened instead.
- **Environment** — Relevant details such as OS, chai version, and build features.

### Feature Requests

When requesting a feature, please include:

- **Summary** — A short description of the proposed feature.
- **Motivation** — The problem or use case this addresses.
- **Proposed Solution** — How you imagine the feature working (optional but helpful).

## Pull Requests

### Conventional Commits

All commits merged into `main` follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <description>

[optional body]
```

**Types:** `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`

**Scope:** The crate or area affected (e.g., `cli`, `lib`, `desktop`, `matrix`, `signal`, `spike`, `base`, `docs`).

### Squash Merging

Pull requests are squash-merged into `main`. The resulting commit message is formed from the pull request:

- **Title** — Must follow conventional commits. This becomes the first line of the commit message.
- **Description** — Appended below the title as the commit body.

Because the squash commit is what lands on `main`, the pull request title is what matters most. Individual commits within a pull request should follow conventional commits where possible, but the title is the authoritative message.

### Checklist

Before submitting a pull request:

- [ ] The pull request title follows conventional commits.
- [ ] The pull request description provides context for the change.
- [ ] `cargo test` passes.
- [ ] `cargo build` succeeds (with any relevant feature flags).
