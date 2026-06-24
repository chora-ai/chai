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
| Specification | [SPEC.md](meta/SPEC.md) | Adding or modifying a specification (`spec/*.md`) |
| Tag | [TAG.md](meta/TAG.md) | Adding or modifying a release tag file (`tag/*.md`) |

## Directory Layout

| Location | Purpose |
|----------|---------|
| **`adr/`** | Architecture Decision Records: why we chose X, alternatives considered. |
| **`epic/`** | Epics and proposals: significant features and architectural tracking. |
| **`meta/`** | Conventions for each document type (see **Conventions** above). |
| **`ref/`** | External systems: summaries of other systems or specs (e.g. OpenClaw, Ollama) for alignment. |
| **`spec/`** | Internal specs and design summaries: how this project works (e.g. context, LLM providers, etc). |
| **`tag/`** | Release tag files: per-version release notes. Source of truth for git tags and release notes. |
| **root** | Index and entry points (`README.md`), release process (`RELEASE.md`), security considerations (`SECURITY.md`), project vision (`VISION.md`), and working notes (`AUDIT_*`/`BUG_*`/`FEAT_*`/`RELEASE_*`). |

## Current Documents

### `/adr`

- **[PROGRAMMING_LANGUAGE.md](adr/PROGRAMMING_LANGUAGE.md)** — Why the runtime is implemented in Rust.
- **[DESKTOP_FRAMEWORK.md](adr/DESKTOP_FRAMEWORK.md)** — Why the desktop application uses egui and eframe.
- **[ORCHESTRATION.md](adr/ORCHESTRATION.md)** — Why Chai uses an orchestrator–worker delegation model.
- **[AGENT_ISOLATION.md](adr/AGENT_ISOLATION.md)** — Why each agent has its own context and skill configuration.
- **[RUNTIME_PROFILES.md](adr/RUNTIME_PROFILES.md)** — Why Chai uses named runtime profiles with restart-required switching.
- **[WRITE_SANDBOX.md](adr/WRITE_SANDBOX.md)** — Why tools are validated against a per-profile sandbox with symlink-as-authorization.
- **[SKILL_PACKAGES.md](adr/SKILL_PACKAGES.md)** — Why skill packages use content-addressed versioning with per-profile lockfiles.
- **[MATRIX_ADAPTER.md](adr/MATRIX_ADAPTER.md)** — Why Matrix lives in a separate adapter package with an optional Cargo feature (experimental).
- **[SIGNAL_ADAPTER.md](adr/SIGNAL_ADAPTER.md)** — Why Signal lives in a separate adapter package with an optional Cargo feature (experimental).
- **[DIAGNOSTIC_HINTS.md](adr/DIAGNOSTIC_HINTS.md)** — Why chai skill tools use diagnostic hints in tool output instead of directives in SKILL.md.
- **[TOOL_PARAMETER_NAMING.md](adr/TOOL_PARAMETER_NAMING.md)** — Why bundled skills follow consistent tool and parameter naming conventions (`{skill}_{verb}`, `path`/`repo`/`scope` semantics, qualified identifiers, flag alignment).

### `/epic`

- **[DESKTOP_FILES.md](epic/DESKTOP_FILES.md)** — (draft) File explorer and editor for Chai agent context, skill files, and sandbox.
- **[MULTI_ORCHESTRATOR.md](epic/MULTI_ORCHESTRATOR.md)** — (draft) Enable support for multiple orchestrator agents in a single profile.
- **[PARALLEL_WORKFLOWS.md](epic/PARALLEL_WORKFLOWS.md)** — (draft) Enable the orchestrator agent to run multiple delegation calls in parallel.
- **[PERSISTENT_SESSIONS.md](epic/PERSISTENT_SESSIONS.md)** — (draft) Persist chat sessions to disk so they survive gateway and desktop restarts.
- **[SPLIT_DEPLOYMENT.md](epic/SPLIT_DEPLOYMENT.md)** — (draft) Enable a hosted-gateway deployment model with secure data transfer.
- **[TOOL_APPROVAL.md](epic/TOOL_APPROVAL.md)** — (draft) Optional human gate before tools; auto-run is today's default.

### `/meta`

- **[ADR.md](meta/ADR.md)** — ADR frontmatter, structure, naming, and when to update.
- **[EPIC.md](meta/EPIC.md)** — Epic lifecycle, frontmatter, structure, naming, maintenance.
- **[REF.md](meta/REF.md)** — Reference doc frontmatter, structure, naming, maintenance.
- **[SPEC.md](meta/SPEC.md)** — Spec frontmatter, structure, naming, and maintenance rules.
- **[TAG.md](meta/TAG.md)** — Tag file structure, naming, format, and maintenance rules.

### `/ref`

#### Channels

- **[TELEGRAM.md](ref/TELEGRAM.md)** — Bot API: config, long-poll, webhook, and gateway wiring.
- **[SIGNAL.md](ref/SIGNAL.md)** — BYO signal-cli: SSE inbound, JSON-RPC send, experimental adapter package.
- **[MATRIX.md](ref/MATRIX.md)** — Optional adapter package (experimental): E2EE, allowlist, SAS routes on the gateway.

#### Providers

- **[OLLAMA.md](ref/OLLAMA.md)** — Ollama endpoint type: native chat, tags, and how Chai calls them.
- **[OPENAI.md](ref/OPENAI.md)** — OpenAI-compatible endpoint type: wire protocol, model discovery, auto-load, and provider patterns.
- **[LM_STUDIO.md](ref/LM_STUDIO.md)** — OpenAI-compat chat and native `/api/v1/models` listing.
- **[NVIDIA_NIM.md](ref/NVIDIA_NIM.md)** — OpenAI-compat: keys, quotas, privacy, static model list.

### `/spec`

#### Configuration and Runtime

- **[CONFIGURATION.md](spec/CONFIGURATION.md)** — On-disk `config.json` blocks, `skills.lockMode`, env overrides, pairing with `status`.
- **[GATEWAY_STATUS.md](spec/GATEWAY_STATUS.md)** — WebSocket `status` shape, key order, redaction, runtime snapshot, paring with `config.json`.
- **[PROFILES.md](spec/PROFILES.md)** — Profile directory, active profile, gateway lock, skill lockfile and generation tracking, switching.

#### Agents, Context, and Skills

- **[AGENTS.md](spec/AGENTS.md)** — Per-agent context directories, skill configuration, system context, and tool lists.
- **[CONTEXT.md](spec/CONTEXT.md)** — Per-agent context on every turn: system message (not persisted), session history (including tool calls/results), and tool schemas (separate from messages).
- **[SKILL_FORMAT.md](spec/SKILL_FORMAT.md)** — Skill directory layout, `SKILL.md` content, frontmatter fields, and `tools.json`.
- **[SKILL_PACKAGES.md](spec/SKILL_PACKAGES.md)** — Skill package versioned layout, content hashing, rollback, startup validation.
- **[TOOLS_SCHEMA.md](spec/TOOLS_SCHEMA.md)** — Declarative tools, allowlist, argv mapping, scripts, resolvers.
- **[SANDBOX.md](spec/SANDBOX.md)** — Write-path enforcement, writable roots, symlink-as-authorization.

#### Channels and Orchestration

- **[CHANNELS.md](spec/CHANNELS.md)** — Inbound queue, registry, session bindings, WebSocket send and agent.
- **[ORCHESTRATION.md](spec/ORCHESTRATION.md)** — `delegate_task`, policy, worker rows under `status.agents`.

#### Providers and Models

- **[PROVIDERS.md](spec/PROVIDERS.md)** — Backend ids, URLs and keys, discovery, native vs OpenAI-compat paths.
- **[MODELS.md](spec/MODELS.md)** — Model strings, families, repo inventory, tool-calling expectations.

#### Desktop Application

- **[DESKTOP.md](spec/DESKTOP.md)** — Current state of the desktop application: screens, data sources, interactions, known gaps.

### `/tag`

- **[V0_1_0.md](tag/V0_1_0.md)** — v0.1.0 release notes.
