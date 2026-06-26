# Agents

This is the `AGENTS.md` file in the root of the `chai` directory.

## Primary Resources

This document and the `base` directory are the primary resources for agents. For knowledge base conventions (working notes lifecycle, adding and updating structured documentation), see `base/AGENTS.md`.

## Architecture Overview

| Category | Package | Purpose |
|----------|---------|---------|
| User-facing runtime | `crates/cli` | Command-line interface for the multi-agent management system. |
| User-facing runtime | `crates/desktop` | Graphical user interface for the multi-agent management system. |
| Shared runtime library | `crates/lib` | Shared business logic for the multi-agent management system. |
| Optional integration | `crates/adapters/matrix` | Optional Matrix **matrix-sdk** adapter (sync, E2EE, hooks). |
| Optional integration | `crates/adapters/signal` | Optional Signal **signal-cli** adapter (SSE, JSON-RPC send). |
| Optional research | `crates/spike` | Optional research binaries (e.g. Matrix/Signal wire probes). |

## Architecture Guidelines

### Modularity

- Maintain modularity and separation of concerns.

### Dependencies

- Add dependencies only when necessary; avoid bloat. When adding logging or other cross-cutting concerns, use what is already in the dependency tree or the smallest option that fits.

## Code Style Guidelines

### Logging and Error Messages

- Never capitalize the first letter of log messages and error messages.

## Documentation Guidelines

### Headings

- Use title case for all section headings (capitalize the first letter of each major word, and keep articles, conjunctions, and prepositions lowercase unless they start or end the title).

## Development Workflow

All changes to `main` go through feature branches and squash merges. Direct commits to `main` are not permitted (except for release commits per the release process in `base/RELEASE.md`). Agents without write access should follow the pull request process in `CONTRIBUTING.md`.

### Feature Branches

Create feature branches from `main` with name `<type>/<short-description>`. Branch name prefixes follow [Conventional Commits](https://www.conventionalcommits.org/) types. All standard types are valid: `feat/`, `fix/`, `docs/`, `style/`, `refactor/`, `perf/`, `test/`, `build/`, `ci/`, `chore/`. Choose the type that reflects the nature of the code change — for example, `test/` for adding or correcting tests, `build/` for dependency or build system updates that don't modify source code.

### Squash Merges

Merges into `main` use squash merges. This keeps `main` history linear, readable, and semantically meaningful — each commit on `main` represents one complete, reviewable change. The squash-merge message follows conventional commits and is what matters; individual commits on feature branches are working commits.

### Conventional Commits

Documentation changes can be either `chore` or `docs` depending on the audience — use the following to decide:

- `chore` for internal/project infrastructure changes (including `base/` directory updates, agent conventions, working notes management)
- `docs` for user-facing documentation changes (`docs/`, `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`)

When a branch touches multiple types, the prefix reflects the primary change — the substantive code change takes priority over incidental or supporting changes (e.g., `test` over `chore` when fixing tests also prompts a convention update, `docs` over `chore` when user documentation changes also involve internal documentation updates).

## User Documentation

### `docs/`

- This directory provides user guides, step-by-step journeys, and testing playbooks. User documentation is isolated: do not link outside `docs/`; bring relevant content into the user doc instead. Always update user documentation when merging feature branches into `main` and a change affects user-facing features.

### `CHANGELOG.md`

- This document tracks notable changes per release. Keep entries concise and user-facing; internal refactors that don't affect users need not be listed. Always update this document when merging feature branches into `main` and a change affects user-facing features.
- Subsection headings (`####`) group entries by user-facing surface area (e.g., Desktop, Skills, Skill Authoring, CLI) — use subsections when a section has entries spanning multiple surfaces, omit them when all entries share the same surface.
- `### Breaking Changes` is a chai-specific section for changes that break backwards compatibility; each entry must state what breaks and what the user must change.

### `CONTRIBUTING.md`

- This document describes how to submit issues and pull requests. Treat this as user documentation: keep it concise, up to date, and free of decision notes. Always follow this document when submitting issues or pull requests.

### `README.md`

- This document provides instructions on how to build, install, and run the runtime. Treat this as user documentation: keep it concise, up to date, free of decision notes, and focused on using `nix` and `cargo` in the root directory.
