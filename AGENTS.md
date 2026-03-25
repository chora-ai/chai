# Agents

Follow these guidelines when working on code in this repository.

## Architecture Overview

Maintain modularity and clear separation of concerns.

### User-Facing Runtime

- **`crates/cli`** — A command-line interface for running the gateway and a workspace.
- **`crates/desktop`** — A graphical user-interface for running the gateway and a workspace.

### Shared Runtime Library

- **`crates/lib`** — All shared business logic for the multi-agent management system.

### Optional Integrations

- **`crates/adapters/matrix`** — Optional Matrix **matrix-sdk** adapter (sync, E2EE, hooks).

### Optional Research Binaries

- **`crates/spike`** — Optional research binaries (e.g. Matrix/Signal wire probes).

The above crates are a starting point. If creating a new crate would improve modularity without adding unnecessary complexity, recommendations would be welcome.

## Architecture Guidelines

### Dependencies

- Add dependencies only when clearly needed; avoid bloat. When adding logging or other cross-cutting concerns, use what is already in the dependency tree or the smallest option that fits.

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

## Agent Resources

### `AGENTS.md` (this document)

- Information and guidelines for agents when working with code in this repository.

### `/.agents` (agent resources)

- Additional resources for agents when working with code in this repository.
- See [`/.agents/README.md`](.agents/README.md) for more information about the directory.