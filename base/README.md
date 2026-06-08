# Knowledge Base

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

## Directory Layout

| Location | Purpose |
|----------|---------|
| **`adr/`** | Architecture Decision Records: why we chose X, alternatives considered. |
| **`epic/`** | Epics and proposals: significant features and architectural tracking. |
| **`meta/`** | Conventions for each document type (see **Conventions** above). |
| **`ref/`** | External systems: summaries of other systems or specs (e.g. OpenClaw, Ollama) for alignment. |
| **`spec/`** | Internal specs and design summaries: how this project works (e.g. context, LLM providers, etc). |
| **root** | Index and entry points (`README.md`). |

## Current Documents

### `/adr`

- **[PROGRAMMING_LANGUAGE.md](adr/PROGRAMMING_LANGUAGE.md)** — Why the runtime and tools are implemented in Rust.
- **[DESKTOP_FRAMEWORK.md](adr/DESKTOP_FRAMEWORK.md)** — Why the desktop UI uses egui and eframe.
- **[SIGNAL_CLI_INTEGRATION.md](adr/SIGNAL_CLI_INTEGRATION.md)** — Why Signal uses a BYO signal-cli HTTP daemon.
- **[ORCHESTRATION.md](adr/ORCHESTRATION.md)** — Why Chai uses an orchestrator–worker delegation model with a single `agents` array in config.
- **[AGENT_ISOLATION.md](adr/AGENT_ISOLATION.md)** — Why each agent has its own context directory and skill configuration.
- **[RUNTIME_PROFILES.md](adr/RUNTIME_PROFILES.md)** — Why Chai uses named runtime profiles with restart-required switching.
- **[WRITE_SANDBOX.md](adr/WRITE_SANDBOX.md)** — Why write-path tools are validated against a per-profile sandbox with symlink-as-authorization.
- **[SKILL_PACKAGES.md](adr/SKILL_PACKAGES.md)** — Why skill packages use content-addressed versioning with per-profile lockfiles and generation-level rollback.

### `/epic`

- **[DESKTOP_FILES.md](epic/DESKTOP_FILES.md)** — (in-progress) File explorer and constrained file editing for Chai config, agent context, and skill files.
- **[MSG_CHANNELS.md](epic/MSG_CHANNELS.md)** — (in-progress) Telegram, Matrix, Signal wired; logging and hardening remain.
- **[RAG_VECTOR.md](epic/RAG_VECTOR.md)** — (draft) pgvector retrieval and embeddings; unbuilt; ties to future projects.
- **[SIMULATIONS.md](epic/SIMULATIONS.md)** — (draft) Fixture harness someday; `spike` crates stay small live probes.
- **[TOOL_APPROVAL.md](epic/TOOL_APPROVAL.md)** — (draft) Optional human gate before tools; auto-run is today's default.

### `/meta`

- **[ADR.md](meta/ADR.md)** — ADR frontmatter, structure, naming, and when to update.
- **[EPIC.md](meta/EPIC.md)** — Epic lifecycle, frontmatter, structure, naming, maintenance.
- **[REF.md](meta/REF.md)** — Reference doc frontmatter, structure, naming, maintenance.
- **[SPEC.md](meta/SPEC.md)** — Spec frontmatter, structure, naming, and maintenance rules.

### `/ref`

#### Channels (External APIs)

- **[TELEGRAM.md](ref/TELEGRAM.md)** — Bot API: config, long-poll, webhook, and gateway wiring.
- **[SIGNAL.md](ref/SIGNAL.md)** — BYO signal-cli: SSE inbound, JSON-RPC send, `crates/lib` channel.
- **[MATRIX.md](ref/MATRIX.md)** — Optional adapter crate: E2EE, allowlist, SAS routes on the gateway.

#### Providers (External APIs)

- **[OLLAMA.md](ref/OLLAMA.md)** — Ollama endpoint type: native chat, tags, and how Chai calls them.
- **[OPENAI.md](ref/OPENAI.md)** — OpenAI-compatible endpoint type: wire protocol, model discovery, auto-load, and provider patterns (LM Studio, NearAI, NVIDIA NIM).
- **[LM_STUDIO.md](ref/LM_STUDIO.md)** — OpenAI-compat chat and native `/api/v1/models` listing.
- **[NVIDIA_NIM.md](ref/NVIDIA_NIM.md)** — OpenAI-compat: keys, quotas, privacy, static model list.

### `/spec`

#### Configuration and Runtime

- **[CONFIGURATION.md](spec/CONFIGURATION.md)** — On-disk `config.json` blocks, `skillLockMode`, env overrides, pairing with `status`.
- **[PROFILES.md](spec/PROFILES.md)** — Profile directory structure, active profile resolution, gateway lock, skill lockfile and generation tracking, switching.
- **[GATEWAY_STATUS.md](spec/GATEWAY_STATUS.md)** — WebSocket `status` shape, key order, redaction, runtime snapshot.

#### Agents, Context, and Skills

- **[AGENTS.md](spec/AGENTS.md)** — Per-agent context dirs, skill configuration, system context, and tool lists.
- **[CONTEXT.md](spec/CONTEXT.md)** — System string, workers roster, skills modes, tools, startup validation (lockfile verification, capability-tier and variant overlap warnings).
- **[SKILL_FORMAT.md](spec/SKILL_FORMAT.md)** — Skill package versioned layout (`versions/<hash>/`, `active` symlink), frontmatter (`description`, `capability_tier`, `model_variant_of`, `metadata.requires.bins`), CLI lock/rollback commands.
- **[TOOLS_SCHEMA.md](spec/TOOLS_SCHEMA.md)** — Declarative tools, allowlist, argv mapping, scripts, resolvers.
- **[SANDBOX.md](spec/SANDBOX.md)** — Write-path enforcement, writable roots, symlink-as-authorization.

#### Desktop

- **[DESKTOP.md](spec/DESKTOP.md)** — Current state of the desktop application: screens, data sources, interactions, known gaps.

#### Channels and Orchestration

- **[CHANNELS.md](spec/CHANNELS.md)** — Inbound queue, registry, session bindings, WebSocket send and agent.
- **[ORCHESTRATION.md](spec/ORCHESTRATION.md)** — `delegate_task`, policy, worker rows under `status.agents`.

#### Providers and Models

- **[PROVIDERS.md](spec/PROVIDERS.md)** — Backend ids, URLs and keys, discovery, native vs OpenAI-compat paths.
- **[MODELS.md](spec/MODELS.md)** — Model strings, families, repo inventory, tool-calling expectations.

### Root (Working Notes)

- **[AUDIT_SKILLS.md](AUDIT_SKILLS.md)** — Cross-skill audit of all bundled skills.
- **[FEAT_DESKTOP_UX.md](FEAT_DESKTOP_UX.md)** — Desktop UX polish and quality-of-life improvements.
- **[FEAT_SKILL_CARGO.md](FEAT_SKILL_CARGO.md)** — Cargo skill for agent-driven code verification.
- **[FEAT_SKILL_LOGS.md](FEAT_SKILL_LOGS.md)** — Logs skill for agent access to diagnostic output.
- **[FEAT_USER_GUIDES.md](FEAT_USER_GUIDES.md)** — User guides improvement.
- **[FEAT_USER_JOURNEY.md](FEAT_USER_JOURNEY.md)** — User journeys improvement.
- **[FEAT_USER_TESTING.md](FEAT_USER_TESTING.md)** — User testing playbooks improvement.
