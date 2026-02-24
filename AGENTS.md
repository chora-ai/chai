# Agents

This file provides guidance to agents when working with code in this repository.

## Project Summary

This repository is the home of a multi-agent management system inspired by OpenClaw (main website: https://openclaw.ai/, GitHub repository: https://github.com/openclaw/openclaw). The critical difference between this project and OpenClaw is that this project has a strong preference for local-first tooling and privacy-preserving technologies, including but not limited to support for local models via Ollama (https://docs.ollama.com/) or LM Studio (https://lmstudio.ai/docs/developer). This project also uses Rust as the programming language and supports a desktop application that can be installed on both Linux and Mac operating systems.

## Architecture Overview

This repository is a monorepo but may become separate repositories if/when appropriate. A separation of concerns should be maintained within the architecture of this repository in preparation for the potential separation at a later point in time.

- **`crates/cli`** - Command-line application for multi-agent management system
- **`crates/desktop`** - Desktop application for multi-agent management system
- **`crates/lib`** - A shared library used by both applications

The above creates are a starting point and are not intended to be restrictive. If creating a new crate would improve separation of concerns without adding unnecessary complexity, recommendations would be welcome.

## Architecture Guidelines

### Minimal Dependencies

- Prefer a minimalist approach. Add dependencies only when clearly needed; avoid bloat. When adding logging or other cross-cutting concerns, use what is already in the dependency tree or the smallest option that fits (e.g. the lowercase log-message guideline below are about style only; it does not require a specific logging crate).

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

### `/.agents` (additional documents)

- additional resources for any agent when working with code in this repository
- see [`/.agents/README.md`](.agents/README.md) for more information about the directory and adding documents
