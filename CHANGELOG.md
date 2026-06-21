# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

#### Skills

- `git` skill tools for branch integration: `git_merge` (squash merge by default), `git_rebase`, `git_rebase_continue`, `git_rebase_abort`, `git_cherry_pick`, `git_cherry_pick_continue`, `git_cherry_pick_abort`, and `git_reset` (mixed reset, default `HEAD~1`)
- `absentDefault` support for positional args (e.g., `git_reset` default ref `HEAD~1`), with `postProcess` arg substitution
- `split` positional args for whitespace-separated multi-value parameters (e.g., `git_add` files, `git_cherry_pick` commits)
- `tools.json` schema additions: `kind: "literal"` and `kind: "tempfile"` arg kinds, `value` field for literal args, `split` field for positional args

## [0.1.0] - 2026-06-20

### Added

#### Runtime and Configuration

- Gateway with WebSocket API for agent turns, status, and streaming events
- Named runtime profiles with per-profile `config.json`, sandbox, agents, skills lockfile, device identity, and `.env` secrets
- Profile switching via CLI (`chai profile switch`) and desktop (disabled while gateway running)
- Gateway lock — only one gateway process per installation
- Token-based gateway authentication with loopback-only enforcement for unauthenticated binds
- Ed25519 device pairing protocol for WebSocket clients
- Configuration-driven architecture: `gateway`, `channels`, `providers`, `sandbox`, `agents`, `skills` blocks in `config.json`

#### Agents and Orchestration

- Orchestrator–worker delegation model with bracket-prefix targeting (`[workerId]`)
- Per-agent context directories (`AGENT.md`), skill configuration, system context, and tool lists
- Two skill context modes: `full` (inlined SKILL.md bodies) and `readOnDemand` (compact list with `read_skill` tool)
- Delegation caps: `maxDelegationsPerTurn`, `maxDelegationsPerSession`, `maxDelegationsPerWorker`
- Tool loop limit (`maxToolLoopsPerTurn`) with interrupted-state events
- User-initiated turn stop via WebSocket

#### Messaging Channels

- Telegram channel (long-poll and webhook modes; always on)
- Matrix adapter (experimental, `--features matrix`) — E2EE, room allowlist, SAS device verification via gateway HTTP routes
- Signal adapter (experimental, `--features signal`) — BYO signal-cli daemon, SSE inbound, JSON-RPC send

#### LLM Providers

- Ollama endpoint type (native `/api/chat` and `/api/tags`; local default)
- OpenAI-compatible endpoint type (`/v1/chat/completions` and `/v1/models`) — covers LM Studio, NearAI, NVIDIA NIM, and any compatible server
- Model discovery modes: `auto` (default), `lmstudio` (native LM Studio API with auto-retry on model unload), `static` (user-curated list)
- API key resolution from shell env, profile `.env`, or `config.json` (in precedence order)
- Startup warnings for privacy-sensitive provider configurations (e.g., NVIDIA NIM as default)

#### Skills

- 15 bundled skills: `files`, `files-read`, `git`, `git-read`, `git-remote`, `logs`, `notes`, `notes-daily`, `notes-frontmatter`, `notes-read`, `notes-wikilink`, `rss`, `skills`, `skills-design`, `skills-read`
- Declarative `tools.json` schema: typed tool definitions, binary allowlist, execution mapping (positional, flag, stdin, workingdir args)
- Skill scripts (`resolveCommand.script`, `postProcess.script`) for param resolution and output processing
- Sandbox annotations on tool parameters: `writePath`, `readPath`, `unsafePath`
- Deny patterns (`denyPattern`, `denyResolveCommand`, `denyAlwaysResolve`) for semantic constraints (e.g., branch protection)
- Content-addressed skill packages with immutable snapshots and atomic rollback via symlink
- Per-profile `skills.lock` with content hash pinning, monotonic generation counter, and strict/warn modes
- Capability tiers (`minimal`, `moderate`, `full`) for model–skill matching
- Skill variants (e.g., `files-read` as read-only variant of `files`) with `variant_of` frontmatter

#### Security and Sandboxing

- Per-profile write sandbox with symlink-as-authorization model
- Three-layer defense: runtime path-like value check, CWD confinement, sandbox path validation
- Binary allowlist with no shell execution (direct `execvp`)
- Skill lockfile integrity verification at gateway startup
- Agent isolation — workers receive only their own context, tools, and skills

#### CLI

- `chai init` — creates `assistant` and `developer` profiles with default config and bundled skills
- `chai gateway` — starts the gateway with optional `--profile` and `--port`
- `chai chat` — interactive CLI chat with `/new`, `/help`, `/exit`
- `chai profile` — list, current, switch
- `chai skill` — list, read, validate, init, delete, write, lock, generations, rollback, discover
- `chai file` — read-lines, write, append, patch, replace, delete, frontmatter operations, rename
- `chai logs` — recent, search

#### Desktop Application

- Native egui/eframe GUI (Chat, Skills, Agent, Tools, Config, Gateway, Logging, Settings screens)
- Gateway lifecycle: spawn mode (start/stop subprocess) or attach mode (connect to existing)
- Profile switching, session panel, model/provider dropdowns
- Streaming chat with real-time tool call/result display and worker delegation rendering
- Turn stop button and tool loop limit banner
- Desktop settings (`desktop.json`): theme, font size, log buffer size

#### Build and Distribution

- Nix flake with CLI and desktop build targets for Linux (x86_64, ARM64) and macOS (ARM64)
- Pre-built release binaries for Linux (x86_64)
- Release build script (`scripts/build-release.sh`) producing tarballs and SHA-256 checksums
- `chai init` idempotent re-runs and sandbox directory recovery

#### Documentation

- User guides (11 topics): introduction, getting started, configuration, connections, agents, skills, sandbox, CLI reference, desktop, choosing a provider, troubleshooting
- Hands-on journeys (14 walkthroughs) covering setup, channels, skills, providers, multi-agent, auth, and profiles
- Testing playbooks with provider/model coverage and standardized message sequences

### Changed

- License changed from LGPL-3.0 to GPL-3.0
