# Agent Resources

This directory includes additional resources for agents (and humans) working on the codebase. Use it to find additional context and to add and update documents for future reference.

**Read [meta/CONVENTIONS.md](meta/CONVENTIONS.md) before adding or modifying documents.**

## Directory Layout

| Location | Purpose |
|----------|---------|
| **`adr/`** | Architecture Decision Records: why we chose X, alternatives considered. |
| **`meta/`** | Conventions for each document type. Read before adding or modifying documents. |
| **`ref/`** | External systems: summaries of other systems or specs (e.g. OpenClaw, Ollama) for alignment. |
| **`spec/`** | Internal specs and design summaries: how this project works (e.g. context, LLM providers, etc). |
| **root** | Primary workspace. Deliverables and epics: what was built and what's next. |

## Current Documents

### `/adr`

- **[PROGRAMMING_LANGUAGE.md](adr/PROGRAMMING_LANGUAGE.md)** — Rationale for choosing Rust for the project.
- **[DESKTOP_FRAMEWORK.md](adr/DESKTOP_FRAMEWORK.md)** — Rationale for choosing egui/eframe for the desktop UI.
- **[SIGNAL_CLI_INTEGRATION.md](adr/SIGNAL_CLI_INTEGRATION.md)** — Rationale for BYO signal-cli for the Signal channel.

### `/meta`

- **[CONVENTIONS.md](meta/CONVENTIONS.md)** — Routing file: read before adding or modifying documents.
- **[EPIC.md](meta/EPIC.md)** — Conventions for epics: frontmatter, structure, lifecycle, naming, maintenance.
- **[SPEC.md](meta/SPEC.md)** — Conventions for specs: frontmatter, structure, naming, maintenance.
- **[ADR.md](meta/ADR.md)** — Conventions for ADRs: frontmatter, structure, naming, maintenance.
- **[REF.md](meta/REF.md)** — Conventions for reference documents: frontmatter, structure, naming, maintenance.

### `/ref`

#### Claw Ecosystem

- **[CLAW_ECOSYSTEM.md](ref/CLAW_ECOSYSTEM.md)** — Comparison of OpenClaw, IronClaw, NemoClaw, and Chai.
- **[OPENCLAW_REFERENCE.md](ref/OPENCLAW_REFERENCE.md)** — OpenClaw concepts, protocol, and design for alignment.
- **[IRONCLAW_REFERENCE.md](ref/IRONCLAW_REFERENCE.md)** — IronClaw architecture, LLM integration, and relation to Chai.
- **[NEMOCLAW_REFERENCE.md](ref/NEMOCLAW_REFERENCE.md)** — NemoClaw in OpenShell, sandbox, and Nemotron cloud inference.

#### Channels (External APIs)

- **[TELEGRAM_REFERENCE.md](ref/TELEGRAM_REFERENCE.md)** — Telegram Bot API usage in Chai: configuration, long-poll, and webhook.
- **[SIGNAL_REFERENCE.md](ref/SIGNAL_REFERENCE.md)** — Signal channel via BYO signal-cli: HTTP SSE and JSON-RPC in `crates/lib`.
- **[MATRIX_REFERENCE.md](ref/MATRIX_REFERENCE.md)** — Matrix channel via `crates/adapters/matrix` (optional `matrix` feature): federation, E2EE, room allowlist, and SAS verification.

#### Providers (External APIs)

- **[OLLAMA_REFERENCE.md](ref/OLLAMA_REFERENCE.md)** — Ollama API usage in Chai and available endpoints.
- **[LM_STUDIO_REFERENCE.md](ref/LM_STUDIO_REFERENCE.md)** — LM Studio OpenAI-compatible API usage in Chai.
- **[VLLM_REFERENCE.md](ref/VLLM_REFERENCE.md)** — vLLM OpenAI-compatible serving for self-hosted inference.
- **[HUGGINGFACE_REFERENCE.md](ref/HUGGINGFACE_REFERENCE.md)** — Hugging Face OpenAI-compatible endpoints for the `hf` provider.
- **[NVIDIA_NIM_REFERENCE.md](ref/NVIDIA_NIM_REFERENCE.md)** — NVIDIA hosted NIM API (free tier): auth, limits, and privacy caveats.
- **[OPENAI_REFERENCE.md](ref/OPENAI_REFERENCE.md)** — OpenAI HTTP API for the `openai` provider and `openai_compat` mapping.

### `/spec`

#### Configuration and Runtime

- **[CONFIGURATION.md](spec/CONFIGURATION.md)** — Draft: on-disk `config.json` top-level blocks, env overrides, and pairing with gateway status for cross-checking.
- **[GATEWAY_STATUS.md](spec/GATEWAY_STATUS.md)** — Draft: WebSocket `status` payload, block alignment with config, redaction rules, and config vs runtime comparison.

#### Context and Skills

- **[CONTEXT.md](spec/CONTEXT.md)** — How agent context is assembled and passed to the model.
- **[SKILL_FORMAT.md](spec/SKILL_FORMAT.md)** — Skill directory layout, frontmatter, metadata, and loaders.
- **[TOOLS_SCHEMA.md](spec/TOOLS_SCHEMA.md)** — `tools.json` schema for declarative skill tools.

#### Channels and Orchestration

- **[CHANNELS.md](spec/CHANNELS.md)** — Internal channel types, sessions, WebSocket delivery, and shutdown.
- **[ORCHESTRATION.md](spec/ORCHESTRATION.md)** — Orchestrator and worker roles, **`delegate_task`**, and delegation policy.

#### Providers and Models

- **[PROVIDERS.md](spec/PROVIDERS.md)** — Backend ids, configuration, discovery, and API-family comparison.
- **[MODELS.md](spec/MODELS.md)** — Model identifiers, families, repository inventory, and tool-calling fit.

### root (this directory)

#### Proof-of-Concept

- **[POC_CHANGELOG.md](POC_CHANGELOG.md)** — Chronology of proof-of-concept features.
- **[POC_DELIVERABLE.md](POC_DELIVERABLE.md)** — Proof-of-concept scope, outcomes, and follow-up themes.
- **[POC_IMPLEMENTATION.md](POC_IMPLEMENTATION.md)** — Detailed technical reference for the proof-of-concept implementation.

#### Proposals and Epics

- **[EPIC_API_ALIGNMENT.md](EPIC_API_ALIGNMENT.md)** — Proposal and tracking for multi-backend LLM alignment (Phase 1 done; Phase 2 Anthropic/Google specified in the same doc).
- **[EPIC_DESKTOP_APP.md](EPIC_DESKTOP_APP.md)** — Draft: desktop app improvements, UX, and roadmap (explorer, editing).
- **[EPIC_MSG_CHANNELS.md](EPIC_MSG_CHANNELS.md)** — Proposal and tracking for messaging channels (Telegram, Matrix, Signal).
- **[EPIC_ORCHESTRATION.md](EPIC_ORCHESTRATION.md)** — Proposal and tracking for orchestrators, workers, and delegation.
- **[EPIC_RAG_VECTOR.md](EPIC_RAG_VECTOR.md)** — Proposal and tracking for RAG with a vector store using pgvector.
- **[EPIC_RUNTIME_PROFILES.md](EPIC_RUNTIME_PROFILES.md)** — Draft: NixOS-like named runtime profiles with isolated environments.
- **[EPIC_SIMULATIONS.md](EPIC_SIMULATIONS.md)** — Draft proposal for simulation harnesses versus `crates/spike` probes.
- **[EPIC_SKILL_PACKAGES.md](EPIC_SKILL_PACKAGES.md)** — Draft: skill packages (revisions, locks, derivation metadata; flake-style resolution).
- **[EPIC_TOOL_APPROVAL.md](EPIC_TOOL_APPROVAL.md)** — Draft proposal for an approval gate before executing model tool calls.

## Adding Documents

Read the relevant convention file in [meta/](meta/) before adding or modifying documents. Each convention file defines frontmatter, structure, naming, and maintenance rules for its document type.

| Document type | Convention | Location |
|---------------|-----------|----------|
| Epic | [meta/EPIC.md](meta/EPIC.md) | Root directory (`EPIC_*.md`) |
| Spec | [meta/SPEC.md](meta/SPEC.md) | `spec/` |
| ADR | [meta/ADR.md](meta/ADR.md) | `adr/` |
| Reference | [meta/REF.md](meta/REF.md) | `ref/` |

## External Documents

The following documents are user resources; they live outside of this directory (`.agents`). While they are not intended for use by agents, they do need to be updated, especially when adding new features.

- **`/.journey/`** - User journeys for understanding the system and manually testing it.
- **`/.testing/`** - Testing playbooks for testing models using supported providers. 

