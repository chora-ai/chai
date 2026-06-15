# Agents

This is the `AGENTS.md` file in the root of the `chai` directory.

## Primary Resources

This document and the `base` directory are the primary resources for agents.

## Issues and Pull Requests

Always follow `CONTRIBUTING.md` when submitting issues or pull requests.

## Architecture Overview

| Category | Crate | Purpose |
|----------|-------|---------|
| User-facing runtime | `crates/cli` | Command-line interface for the multi-agent management system. |
| User-facing runtime | `crates/desktop` | Graphical user interface for the multi-agent management system. |
| Shared runtime library | `crates/lib` | Shared business logic for the multi-agent management system. |
| Optional integration | `crates/adapters/matrix` | Optional Matrix **matrix-sdk** adapter (sync, E2EE, hooks). |
| Optional integration | `crates/adapters/signal` | Optional Signal **signal-cli** adapter (SSE, JSON-RPC send). |
| Optional research | `crates/spike` | Optional research binaries (e.g. Matrix/Signal wire probes). |

## Architecture Guidelines

### Modularity

- Maintain modularity and clear separation of concerns.

### Dependencies

- Add dependencies only when clearly needed; avoid bloat. When adding logging or other cross-cutting concerns, use what is already in the dependency tree or the smallest option that fits.

## Code Style Guidelines

### Logging and Error Messages

- Never capitalize the first letter of log messages and error messages.

## Documentation Guidelines

### Headings

- Use title case for all section headings (capitalize the first letter of each major word, and keep articles, conjunctions, and prepositions lowercase unless they start or end the title).

## User Documentation

### `docs/`

- This directory provides user guides, step-by-step journeys, and testing playbooks. User documentation is isolated: do not link outside `docs/`; bring relevant content into the user doc instead. Guidelines and conventions are in `docs/AGENTS.md` and each subdirectory's `AGENTS.md`.

### `CHANGELOG.md`

- This document tracks notable changes per release. Keep entries concise and user-facing; internal refactors that don't affect users need not be listed.

### `CONTRIBUTING.md`

- This document describes how to submit issues and pull requests. Treat this as user documentation: keep it concise, up to date, and free of decision notes.

### `README.md`

- This document provides instructions on how to build, install, and run the runtime. Treat this as user documentation: keep it concise, up to date, and free of decision notes.
