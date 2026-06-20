# Vision

This document provides the vision for the project.

## Current State

A working multi-agent management system with messaging channels, skill-based tooling, and profile-based configuration. Chai runs a local gateway with orchestrator–worker delegation, supports Telegram as a default channel with experimental Matrix and Signal adapters, ships 15 bundled skills, and provides both a CLI and a native desktop application.

## Long-Term Goals

A privacy-preserving multi-agent management system designed for constrained-model operation.

### Identity

Chai's thesis: critical guarantees — correctness, privacy, capability boundaries — should be properties of the system architecture, not requirements on the model. This identity is expressed through:

- **Compiled contracts** — Skills are authored at design time into typed `tools.json` schemas that encode the mapping between intent and execution. The executing model generates well-formed function calls against the schema; it does not need to interpret instructions or determine correct actions at runtime. This enables small local models to perform work that other frameworks reserve for more capable cloud models.
- **Declarative, default-closed architecture** — The system *is* its configuration. No skills, no tools, no access until the config declares them. This NixOS-aligned principle — nothing exists unless explicitly declared — is the architectural prerequisite for profiles, rollback, reproducibility, and the allowlist model.
- **Privacy by construction** — Sensitive data stays on the local machine, handled by local models. Skill authoring operates only on non-sensitive structural artifacts, may be performed by cloud models. Runtime profiles enforce trust boundaries architecturally — a developer profile *cannot* access an assistant profile's content.
- **Three-tier execution** — Scripts handle deterministic work. Schemas handle structured operations. The model handles what genuinely requires reasoning. Every task pushed down the tiers reduces the capability requirement and increases reliability.

### Security Approach

Security in Chai follows the same architectural philosophy: constraints are structural, not advisory. The system is designed so that the agent cannot bypass its boundaries even if the model produces malicious requests:

- **Default-closed execution** — The allowlist blocks all commands by default; only explicitly declared (binary, subcommand) pairs can run. No shell execution eliminates injection. Deny patterns enforce semantic constraints that schemas cannot express.
- **Sandboxed filesystem access** — Write and read paths are validated against per-profile writable roots before execution. The agent cannot write to or read from arbitrary locations on the host. Symlink-as-authorization makes the filesystem the policy — explicit, auditable, and revocable.
- **Per-profile isolation** — Each profile is a trust domain with its own device identity, pairing state, secrets, and sandbox. Profiles share a skill package store but differ in enablement and lockfile pins.
- **Agent isolation** — Workers receive only their own context and tools. No orchestrator identity, no delegation capability. Role confusion cannot escalate privileges.
- **Integrity verification** — Skill lockfiles pin content hashes. In strict mode, the gateway refuses to start if any skill has been tampered with.

These mechanisms compose into defense-in-depth: the skill schema constrains what the model knows, the allowlist constrains what operations can run, and the sandbox constrains where writes land. No single layer is sufficient; together they compensate for model limitations. See [SECURITY.md](SECURITY.md) for the full threat model and known vulnerabilities.

### Guiding Principles

- **Minimalism** — Single Rust binary. Static skill files. No container runtime, no WASM sandbox, no heavy infrastructure.
- **Sovereignty** — The owner controls capabilities and what they can do. No self-expansion, no agent-initiated skill installation.
- **Local-first** — Default provider is Ollama (local). Default bind is localhost. Compiled contracts make local-first viable — small models do useful work because schemas compensate for what the model lacks.
- **Privacy-preserving** — The authoring/execution separation creates a privacy boundary by design. Runtime profiles make it structural.
- **Open-source** — The software and any modifications to the software remain open-source. Skills are portable, inspectable text files.
