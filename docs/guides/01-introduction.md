# Introduction

Chai is a privacy-preserving multi-agent management system designed for constrained-model operation. It runs language models locally or in the cloud, connects them to chat channels, and gives them scoped tools through a declarative skill system — all governed by configuration, not code.

## Why Chai

Large language models are powerful, but relying on cloud APIs for every interaction creates privacy risk and cost. Chai's thesis is that critical guarantees — correctness, privacy, capability boundaries — should be properties of the system architecture, not requirements on the model. Small local models can do useful work when the system compensates for what the model lacks through compiled contracts, strict allowlists, and sandboxed execution.

## Key Concepts

### Gateway

The gateway is a single HTTP/WebSocket server that orchestrates everything. It loads your configuration, connects to model providers, accepts messages from channels, runs agent turns (model calls + tool loops), and streams responses back. You start it with `chai gateway`; the desktop application manages it for you.

### Providers

A **provider** is a model backend — where the inference happens. Chai supports two endpoint types, with four common provider configurations:

| Provider | Where it runs | Endpoint Type | Special behavior | Needs API key |
|----------|---------------|---------------|------------------|---------------|
| **Ollama** (`ollama`) | Your machine | `"ollama"` | — | No |
| **LM Studio** (`lms`) | Your machine | `"openai-compat"` | `modelDiscovery: "lmstudio"` | No |
| **NearAI** (`nearai`) | NearAI cloud | `"openai-compat"` | — | Yes |
| **NVIDIA NIM** (`nim`) | NVIDIA cloud | `"openai-compat"` | `modelDiscovery: "static"`, `staticModels` | Yes |

Any other OpenAI-compatible server (vLLM, Hugging Face TGI, OpenAI, Azure OpenAI, etc.) can be configured as an `"openai-compat"` provider with the appropriate `baseUrl` and `apiKey`.

The default provider is Ollama — local-first, no API key required.

### Agents

An **agent** is a named configuration entry that ties a provider and model to a role. There is always one **orchestrator** (owns the conversation) and optionally any number of **workers** (handle delegated subtasks). Agents are not separate services; the gateway reads the `agents` block in your config and routes each turn through the appropriate backend.

### Skills

**Skills** are declarative packages that give an agent instructions and tools. Each skill is a directory containing a `SKILL.md` (instructions the model sees), an optional `tools.json` (typed tool schemas the model can call), and optional scripts. Skills are opt-in per agent via the `skillsEnabled` config field — nothing runs unless you declare it.

### Channels

**Channels** connect the gateway to messaging platforms: Telegram, Matrix, and Signal. Users chat with agents through these channels just as they would with a person. The desktop app and WebSocket API also work as direct interfaces.

### Profiles

A **profile** is an independent configuration tree under `~/.chai/profiles/<name>/` — its own `config.json`, agent context directories, sandbox, and state. You can switch between profiles with `chai profile switch`. The active profile is a symlink at `~/.chai/active`.

### Write Sandbox

Each profile has a **write sandbox** that restricts where skill tools may write files. The sandbox enforces spatial safety — the tool allowlist controls *what* runs, and the sandbox controls *where* writes go. Agents cannot create symlinks; every write authorization is a deliberate user action.

## How the Pieces Fit Together

```text
Channels (Telegram, Matrix, Signal, WebSocket, Desktop)
  │
  ▼
Gateway ─── loads ──→ Config (config.json)
  │                      │
  │                      ├── Providers (URLs, API keys)
  │                      ├── Agents (roles, models, skills)
  │                      └── Channels (credentials)
  │
  ├── Orchestrator turn ──→ Provider API (Ollama, OpenAI, …)
  │     │                        │
  │     └── tool loop ──→ Skill tools (scripts, file ops)
  │           │
  │           └── delegate_task ──→ Worker turn ──→ Provider API
  │
  └── Response ──→ Channel / Desktop / WebSocket
```

The gateway loads configuration at startup, discovers model providers and skill packages, then listens for incoming messages. Each user message triggers an **orchestrator turn**: the model receives the system context (agent instructions, skill content, worker roster), the session history, and its tool definitions. If the model calls a tool, the gateway executes it and loops. If the model delegates to a worker, that worker gets its own turn with its own context and tools. The final response goes back through the same channel.

## Next Steps

- [Getting Started](02-getting-started.md) — Install chai and send your first message.
- [Configuration](03-configuration.md) — Customize providers, agents, channels, and more.
- [Choosing a Provider and Model](10-choosing-a-provider.md) — Decision guide for local vs. cloud providers.
- [CLI Reference](08-cli-reference.md) — Every `chai` command with flags and examples.
- [Desktop App](09-desktop.md) — GUI usage, screens, and device pairing.
- [Troubleshooting](11-troubleshooting.md) — Common errors and how to resolve them.
- [User Journeys](../journey/README.md) — Step-by-step hands-on walkthroughs for each feature (gateway, desktop, channels, skills).
- [Testing Playbooks](../testing/README.md) — Systematic model and provider comparison procedures.
