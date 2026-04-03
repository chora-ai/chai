# Agents

Information and guidelines for working on code in this repository.

## Primary Resources

This document and the `.agents` directory are the primary resources.

**Always read [.agents/README.md](.agents/README.md)** — the entry point for the `.agents` directory.

## Architecture Overview

| Category | Crate | Purpose |
|----------|-------|---------|
| User-facing runtime | `crates/cli` | Command-line interface for the gateway and a workspace. |
| User-facing runtime | `crates/desktop` | Graphical user interface for the gateway and a workspace. |
| Shared runtime library | `crates/lib` | Shared business logic for the multi-agent management system. |
| Optional integration | `crates/adapters/matrix` | Optional Matrix **matrix-sdk** adapter (sync, E2EE, hooks). |
| Optional research | `crates/spike` | Optional research binaries (e.g. Matrix/Signal wire probes). |

## Architecture Guidelines

### Modularity

- Maintain modularity and clear separation of concerns.

### Dependencies

- Add dependencies only when clearly needed; avoid bloat. When adding logging or other cross-cutting concerns, use what is already in the dependency tree or the smallest option that fits.

### New Crates

- The crates in **Architecture Overview** are a starting point. If creating a new crate would improve modularity without adding unnecessary complexity, recommendations would be welcome.

## Code Style Guidelines

### Logging and Error Messages

- Never capitalize the first letter of log messages and error messages.

## Documentation Guidelines

### Headings

- Use title case for all section headings (capitalize the first letter of each major word, and keep articles, conjunctions, and prepositions lowercase unless they start or end the title).

## User Documentation

### `README.md`

- This document provides instructions on how to build, install, and configure the runtime. Treat this as user documentation: keep it concise, up to date, and free of decision notes.

### `VISION.md`

- This document provides the state of the project and outlines short and long-term goals.