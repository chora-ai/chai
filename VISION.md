# Vision

This document provides an overview of the vision for this project.

## Current State

An exploratory project (proof-of-concept + partially completed epics).

## Short-Term Goals

A minimalist multi-agent management system that includes the following:

- Running a gateway with the CLI or Desktop application
- Support for large language models running locally (via Ollama)
- Support for at least one communication channel (i.e. Telegram)
- Support for at least one skill (i.e. managing an Obsidian vault)
- A modular architecture that makes it easy to extend the above

Status: These goals were met with the proof-of-concept implementation.

## Long-Term Goals

A privacy-preserving multi-agent management system designed for constrained-model operation.

### Identity

Chai's thesis: critical guarantees — correctness, privacy, capability boundaries — should be properties of the system architecture, not requirements on the model. This identity is expressed through:

- **Compiled contracts** — Skills are authored at design time into typed `tools.json` schemas that encode the mapping between intent and execution. The executing model generates well-formed function calls against the schema; it does not need to interpret instructions or determine correct actions at runtime. This enables small local models to perform work that other frameworks reserve for more capable cloud models.
- **Declarative, default-closed architecture** — The system *is* its configuration. No skills, no tools, no access until the config declares them. This NixOS-aligned principle — nothing exists unless explicitly declared — is the architectural prerequisite for profiles, rollback, reproducibility, and the allowlist model.
- **Privacy by construction** — Sensitive data stays on the local machine, handled by local models. Skill authoring operates only on non-sensitive structural artifacts, may be performed by cloud models. Runtime profiles enforce trust boundaries architecturally — a developer profile *cannot* access assistant workspace content regardless of model behavior.
- **Three-tier execution** — Scripts handle deterministic work. Schemas handle structured operations. The model handles what genuinely requires reasoning. Every task pushed down the tiers reduces the capability requirement and increases reliability.

### Guiding Principles

- **Minimalism** — Single Rust binary. Static skill files. No container runtime, no WASM sandbox, no heavy infrastructure.
- **Sovereignty** — The owner controls exactly which capabilities exist and what they can do. No self-expansion, no agent-initiated skill installation.
- **Local-first** — Default provider is Ollama (local). Default bind is localhost. Compiled contracts make local-first viable — small models do useful work because schemas compensate for what the model lacks.
- **Privacy-preserving** — The authoring/execution separation creates a privacy boundary by design. Runtime profiles make it structural.
- **Open-source** — The software and any modifications to the software remain open-source (LGPL licensed). Skills are portable, inspectable text files.
