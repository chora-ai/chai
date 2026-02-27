# Agents

This file provides guidance to agents when working with code in this repository.

## Architecture Overview

This repository is a monorepo but may become separate repositories when appropriate. A separation of concerns should be maintained within the architecture of this repository in preparation for any potential separations at a later point in time.

- **`crates/cli`** — A command-line interface for creating, managing, and orchestrating agents
- **`crates/desktop`** — A graphical user interface for creating, managing, and orchestrating agents
- **`crates/lib`** — A shared library for creating, managing, and orchestrating agents

The above crates are a starting point and are not intended to be restrictive. If creating a new crate would improve separation of concerns without adding unnecessary complexity, recommendations would be welcome.

## Architecture Guidelines

### Minimal Dependencies

- Prefer a minimalist approach. Add dependencies only when clearly needed; avoid bloat. When adding logging or other cross-cutting concerns, use what is already in the dependency tree or the smallest option that fits.

## Code Style Guidelines

### Logging and Error Messages

- Never capitalize the first letter of log messages and error messages
- Example: `log::info!("starting server on port {}", port);`
- Example: `anyhow::bail!("failed to connect to server");`

## Documentation Guidelines

### Headings

- Use title case for all section headings (Capitalize the first letter of each major word, and keep articles, conjunctions, and prepositions lowercase unless they start or end the title)

## User Documentation

### `README.md`

- This document should be treated as user-facing documentation about the project and it should not include notes about decisions made by any agent such as why content was added or moved.

## Agent Resources

### `AGENTS.md` (this document)

- the primary resource for any agent when working with code in this repository

### `/.agents` (additional resources)

- additional resources for any agent when working with code in this repository
- see [`/.agents/README.md`](.agents/README.md) for more information about the directory and documents