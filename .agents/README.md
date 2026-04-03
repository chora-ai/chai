# Agent Resources

This directory includes resources for agents (and humans) working on the codebase. Use it to find additional context and to add and update documents for future reference.

## Conventions

Always read the relevant convention file before adding or modifying a document in this directory. Each file under **`meta/`** defines frontmatter, structure, naming, and maintenance rules for its document type.

### Convention Files

| Type | File | When to read |
|------|------|--------------|
| ADR | [ADR.md](meta/ADR.md) | Adding or modifying an architecture decision record (`adr/*.md`) |
| Epic | [EPIC.md](meta/EPIC.md) | Adding or modifying an epic (`epic/*.md`) |
| Reference | [REF.md](meta/REF.md) | Adding or modifying a reference document (`ref/*.md`) |
| Spec | [SPEC.md](meta/SPEC.md) | Adding or modifying a spec (`spec/*.md`) |

*Note: POC documents in the `/poc` directory do not have conventions; they are historical documents.*

## Directory Layout

| Location | Purpose |
|----------|---------|
| **`adr/`** | Architecture Decision Records: why we chose X, alternatives considered. |
| **`epic/`** | Epics and proposals: significant features and architectural tracking. |
| **`meta/`** | Conventions for each document type (see **Conventions** above). |
| **`poc/`** | Proof-of-concept: changelog, deliverable summary, and implementation reference. |
| **`ref/`** | External systems: summaries of other systems or specs (e.g. OpenClaw, Ollama) for alignment. |
| **`spec/`** | Internal specs and design summaries: how this project works (e.g. context, LLM providers, etc). |
| **root** | Index and entry points (`README.md`). |

## Current Documents

### `/adr`

- **[PROGRAMMING_LANGUAGE.md](adr/PROGRAMMING_LANGUAGE.md)** — Rationale for choosing Rust for the project.
- **[DESKTOP_FRAMEWORK.md](adr/DESKTOP_FRAMEWORK.md)** — Rationale for choosing egui/eframe for the desktop UI.
- **[SIGNAL_CLI_INTEGRATION.md](adr/SIGNAL_CLI_INTEGRATION.md)** — Rationale for BYO signal-cli for the Signal channel.

### `/epic`

- **[AGENT_ISOLATION.md](epic/AGENT_ISOLATION.md)** — Proposed: per-agent workspace under `~/.chai/agents/<id>/`, per-agent skills config, role-correct prompts.
- **[API_ALIGNMENT.md](epic/API_ALIGNMENT.md)** — Proposal and tracking for multi-backend LLM alignment (Phase 1 done; Phase 2 Anthropic/Google specified in the same doc).
- **[DESKTOP_APP.md](epic/DESKTOP_APP.md)** — Draft: desktop app improvements, UX, and roadmap (explorer, editing).
- **[MSG_CHANNELS.md](epic/MSG_CHANNELS.md)** — Proposal and tracking for messaging channels (Telegram, Matrix, Signal).
- **[ORCHESTRATION.md](epic/ORCHESTRATION.md)** — Proposal and tracking for orchestrators, workers, and delegation.
- **[RAG_VECTOR.md](epic/RAG_VECTOR.md)** — Proposal and tracking for RAG with a vector store using pgvector.
- **[RUNTIME_PROFILES.md](epic/RUNTIME_PROFILES.md)** — Draft: NixOS-like named runtime profiles with isolated environments.
- **[SIMULATIONS.md](epic/SIMULATIONS.md)** — Draft proposal for simulation harnesses versus `crates/spike` probes.
- **[SKILL_PACKAGES.md](epic/SKILL_PACKAGES.md)** — Draft: skill packages (revisions, locks, derivation metadata; flake-style resolution).
- **[TOOL_APPROVAL.md](epic/TOOL_APPROVAL.md)** — Draft proposal for an approval gate before executing model tool calls.

### `/meta`

- **[ADR.md](meta/ADR.md)** — Conventions for ADRs: frontmatter, structure, naming, maintenance.
- **[EPIC.md](meta/EPIC.md)** — Conventions for epics: frontmatter, structure, lifecycle, naming, maintenance.
- **[REF.md](meta/REF.md)** — Conventions for reference documents: frontmatter, structure, naming, maintenance.
- **[SPEC.md](meta/SPEC.md)** — Conventions for specs: frontmatter, structure, naming, maintenance.

### `/poc`

- **[CHANGELOG.md](poc/CHANGELOG.md)** — Chronology of proof-of-concept features.
- **[DELIVERABLE.md](poc/DELIVERABLE.md)** — Proof-of-concept scope, outcomes, and follow-up themes.
- **[IMPLEMENTATION.md](poc/IMPLEMENTATION.md)** — Detailed technical reference for the proof-of-concept implementation.

### `/ref`

#### Claw Ecosystem

- **[CLAW_ECOSYSTEM.md](ref/CLAW_ECOSYSTEM.md)** — Comparison of OpenClaw, IronClaw, NemoClaw, and Chai.
- **[OPENCLAW.md](ref/OPENCLAW.md)** — OpenClaw concepts, protocol, and design for alignment.
- **[IRONCLAW.md](ref/IRONCLAW.md)** — IronClaw architecture, LLM integration, and relation to Chai.
- **[NEMOCLAW.md](ref/NEMOCLAW.md)** — NemoClaw in OpenShell, sandbox, and Nemotron cloud inference.

#### Channels (External APIs)

- **[TELEGRAM.md](ref/TELEGRAM.md)** — Telegram Bot API usage in Chai: configuration, long-poll, and webhook.
- **[SIGNAL.md](ref/SIGNAL.md)** — Signal channel via BYO signal-cli: HTTP SSE and JSON-RPC in `crates/lib`.
- **[MATRIX.md](ref/MATRIX.md)** — Matrix channel via `crates/adapters/matrix` (optional `matrix` feature): federation, E2EE, room allowlist, and SAS verification.

#### Providers (External APIs)

- **[OLLAMA.md](ref/OLLAMA.md)** — Ollama API usage in Chai and available endpoints.
- **[LM_STUDIO.md](ref/LM_STUDIO.md)** — LM Studio OpenAI-compatible API usage in Chai.
- **[VLLM.md](ref/VLLM.md)** — vLLM OpenAI-compatible serving for self-hosted inference.
- **[HUGGINGFACE.md](ref/HUGGINGFACE.md)** — Hugging Face OpenAI-compatible endpoints for the `hf` provider.
- **[NVIDIA_NIM.md](ref/NVIDIA_NIM.md)** — NVIDIA hosted NIM API (free tier): auth, limits, and privacy caveats.
- **[OPENAI.md](ref/OPENAI.md)** — OpenAI HTTP API for the `openai` provider and `openai_compat` mapping.

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

## External Documents

The following documents are user resources; they live outside of this directory (`.agents`). While they are not intended for use by agents, they do need to be updated, especially when adding new features.

- **`/.journey/`** - User journeys for understanding the system and manually testing it.
- **`/.testing/`** - Testing playbooks for testing models using supported providers. 

