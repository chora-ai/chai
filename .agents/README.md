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

*Note: POC documents in the `poc/` directory do not have conventions; they are historical documents.*

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

- **[PROGRAMMING_LANGUAGE.md](adr/PROGRAMMING_LANGUAGE.md)** — Why the runtime and tools are implemented in Rust.
- **[DESKTOP_FRAMEWORK.md](adr/DESKTOP_FRAMEWORK.md)** — Why the desktop UI uses egui and eframe.
- **[SIGNAL_CLI_INTEGRATION.md](adr/SIGNAL_CLI_INTEGRATION.md)** — Why Signal uses a BYO signal-cli HTTP daemon.

### `/epic`

- **[AGENT_ISOLATION.md](epic/AGENT_ISOLATION.md)** — (complete) Per-agent dirs and skill lists; shared packages under `~/.chai/skills`.
- **[API_ALIGNMENT.md](epic/API_ALIGNMENT.md)** — (in-progress) Shipped local and OpenAI-compat stacks; Anthropic and Google next.
- **[BUNDLED_SKILLS.md](epic/BUNDLED_SKILLS.md)** — (in-progress) Bundled skill inventory, skill generation and validation workflow.
- **[DESKTOP_APP.md](epic/DESKTOP_APP.md)** — (in-progress) egui console for gateway, status, chat; editing and UX still in flight.
- **[MSG_CHANNELS.md](epic/MSG_CHANNELS.md)** — (in-progress) Telegram, Matrix, Signal wired; logging and hardening remain.
- **[ORCHESTRATION.md](epic/ORCHESTRATION.md)** — (complete) `delegate_task`, policy, merged `status` catalog, desktop events.
- **[RAG_VECTOR.md](epic/RAG_VECTOR.md)** — (draft) pgvector retrieval and embeddings; unbuilt; ties to future projects.
- **[RUNTIME_PROFILES.md](epic/RUNTIME_PROFILES.md)** — (complete) Profile trees, active symlink, gateway lock, one shared skills root.
- **[SIMULATIONS.md](epic/SIMULATIONS.md)** — (draft) Fixture harness someday; `spike` crates stay small live probes.
- **[SKILL_PACKAGES.md](epic/SKILL_PACKAGES.md)** — (complete) Pins, locks, rollback metaphor; no resolver in tree yet.
- **[TOOL_APPROVAL.md](epic/TOOL_APPROVAL.md)** — (draft) Optional human gate before tools; auto-run is today’s default.
- **[WRITE_SANDBOX.md](epic/WRITE_SANDBOX.md)** — (in-progress) Runtime `WriteSandbox` and `writePath` enforcement.

### `/meta`

- **[ADR.md](meta/ADR.md)** — ADR frontmatter, structure, naming, and when to update.
- **[EPIC.md](meta/EPIC.md)** — Epic lifecycle, frontmatter, structure, naming, maintenance.
- **[REF.md](meta/REF.md)** — Reference doc frontmatter, structure, naming, maintenance.
- **[SPEC.md](meta/SPEC.md)** — Spec frontmatter, structure, naming, and maintenance rules.

### `/poc`

- **[CHANGELOG.md](poc/CHANGELOG.md)** — Dated list of proof-of-concept features and changes.
- **[DELIVERABLE.md](poc/DELIVERABLE.md)** — POC scope, outcomes, and suggested follow-up themes.
- **[IMPLEMENTATION.md](poc/IMPLEMENTATION.md)** — Deep technical notes for the historical POC codebase.

### `/ref`

#### Claw Ecosystem

- **[CLAW_ECOSYSTEM.md](ref/CLAW_ECOSYSTEM.md)** — How OpenClaw, IronClaw, NemoClaw, and Chai relate.
- **[OPENCLAW.md](ref/OPENCLAW.md)** — OpenClaw concepts and protocol for Chai alignment.
- **[IRONCLAW.md](ref/IRONCLAW.md)** — IronClaw stack and how it compares to Chai.
- **[NEMOCLAW.md](ref/NEMOCLAW.md)** — NemoClaw, OpenShell, and Nemotron-hosted inference.

#### Channels (External APIs)

- **[TELEGRAM.md](ref/TELEGRAM.md)** — Bot API: config, long-poll, webhook, and gateway wiring.
- **[SIGNAL.md](ref/SIGNAL.md)** — BYO signal-cli: SSE inbound, JSON-RPC send, `crates/lib` channel.
- **[MATRIX.md](ref/MATRIX.md)** — Optional adapter crate: E2EE, allowlist, SAS routes on the gateway.

#### Providers (External APIs)

- **[OLLAMA.md](ref/OLLAMA.md)** — Native Ollama chat, tags, and how Chai calls them.
- **[LM_STUDIO.md](ref/LM_STUDIO.md)** — OpenAI-compat chat and native `/api/v1/models` listing.
- **[VLLM.md](ref/VLLM.md)** — OpenAI-compat `/v1` serving for self-hosted vLLM.
- **[HUGGINGFACE.md](ref/HUGGINGFACE.md)** — `hf` provider: TGI, Inference Endpoints, OpenAI-shaped URLs.
- **[NVIDIA_NIM.md](ref/NVIDIA_NIM.md)** — Hosted NIM: keys, quotas, privacy, static model list.
- **[OPENAI.md](ref/OPENAI.md)** — Official API, proxies, and mapping into `openai_compat`.

### `/spec`

#### Configuration and Runtime

- **[CONFIGURATION.md](spec/CONFIGURATION.md)** — On-disk `config.json` blocks, env overrides, pairing with `status`.
- **[GATEWAY_STATUS.md](spec/GATEWAY_STATUS.md)** — WebSocket `status` shape, key order, redaction, runtime snapshot.

#### Context and Skills

- **[CONTEXT.md](spec/CONTEXT.md)** — System string, workers roster, skills modes, tools, date and hint lines.
- **[SKILL_FORMAT.md](spec/SKILL_FORMAT.md)** — Package layout, frontmatter, metadata, optional `tools.json`.
- **[TOOLS_SCHEMA.md](spec/TOOLS_SCHEMA.md)** — Declarative tools, allowlist, argv mapping, scripts, resolvers.

#### Channels and Orchestration

- **[CHANNELS.md](spec/CHANNELS.md)** — Inbound queue, registry, session bindings, WebSocket send and agent.
- **[ORCHESTRATION.md](spec/ORCHESTRATION.md)** — `delegate_task`, policy, worker rows under `status.agents`.

#### Providers and Models

- **[PROVIDERS.md](spec/PROVIDERS.md)** — Backend ids, URLs and keys, discovery, native vs OpenAI-compat paths.
- **[MODELS.md](spec/MODELS.md)** — Model strings, families, repo inventory, tool-calling expectations.

## External Documents

The following documents are user resources; they live outside of this directory. While they are not intended for use by agents, they do need to be updated, especially when adding new features.

- **`/.journey/`** — Step-by-step journeys to learn and manually exercise the stack.
- **`/.testing/`** — Playbooks to compare models and providers with a live gateway.

