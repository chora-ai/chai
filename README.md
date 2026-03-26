# chai

A multi-agent management system.

## Overview

- **`crates/cli`** — A command-line interface for running the gateway and a workspace
- **`crates/desktop`** — A graphical user-interface for running the gateway and a workspace
- **`crates/lib`** — All shared business logic for the multi-agent managements system

## Commands

```bash
# Build everything
cargo build

# Build specific crates
cargo build -p cli
cargo build -p desktop
cargo build -p lib

# Run the command-line interface
cargo run -p cli -- --help
cargo run -p cli -- version
cargo run -p cli -- init
cargo run -p cli -- gateway

# Run the desktop application
cargo run -p desktop

# Test everything
cargo test
```

Use `--features matrix` to build or run with the `matrix` adaptor.

## Command-Line Interface

Install the CLI locally:

```bash
cargo install --path crates/cli
```

Use `--features matrix` to install the `matrix` adaptor.

Run the installed CLI:

```bash
chai --help
chai version
chai init
chai gateway
```

## Desktop Application

Install the app locally:

```bash
cargo install --path crates/desktop
```

Use `--features matrix` to install the `matrix` adaptor.

Run the installed app:

```bash
chai-desktop
```

## Configuration

The command-line interface and desktop application use the same configuration.

### Initialization

After installing, run **`chai init`** to create the configuration directory (`~/.chai/`).

### Configuration File (`config.json`)

The main configuration is loaded from a JSON file. The default path is `~/.chai/config.json`. The 
default path can be overridden with `CHAI_CONFIG_PATH`. An empty configuration file is created at initialization.

**Minimal example** — a valid configuration file (built-in defaults are used at runtime).

```json
{}
```

**Runtime example** — the effective values for **`{}`** (shown here for reference, not required). With no **`agents`** key, **`defaultProvider`** and **`defaultModel`** are unset on disk; **`ollama`** and **`llama3.2:3b`** are the defaults the gateway uses at runtime for routing and model selection (other providers use their own fallbacks when **`defaultModel`** is unset; see **Providers and Models** below).

```json
{
  "gateway": {
    "port": 15151,
    "bind": "127.0.0.1",
    "auth": {
      "mode": "none"
    }
  },
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "ollama",
      "defaultModel": "llama3.2:3b"
    }
  ],
  "skills": {
    "contextMode": "full"
  }
}
```

**Full example** — a valid configuration with all top-level fields (plus a worker agent).

```json
{
  "gateway": {
    "port": 15151,
    "bind": "127.0.0.1",
    "auth": {
      "mode": "none",
      "token": null
    }
  },
  "channels": {
    "telegram": {
      "botToken": null,
      "webhookUrl": null,
      "webhookSecret": null
    },
    "matrix": {
      "homeserver": null,
      "accessToken": null,
      "user": null,
      "password": null,
      "userId": null,
      "storePath": null,
      "deviceId": null
    },
    "signal": {
      "httpBase": null,
      "account": null
    }
  },
  "providers": {
    "ollama": {
      "baseUrl": null
    },
    "lms": {
      "baseUrl": null
    },
    "vllm": {
      "apiKey": null,
      "baseUrl": null
    },
    "hf": {
      "apiKey": null,
      "baseUrl": null
    },
    "nim": {
      "apiKey": null
    },
    "openai": {
      "apiKey": null,
      "baseUrl": null
    }
  },
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "ollama",
      "defaultModel": "llama3.2:3b",
      "enabledProviders": [],
      "workspace": null,
      "maxSessionMessages": null,
      "maxDelegationsPerTurn": null,
      "maxDelegationsPerSession": null,
      "maxDelegationsPerProvider": null,
      "delegateAllowedModels": [
        {
          "provider": "ollama",
          "model": "llama3.2:3b",
          "local": false,
          "toolCapable": null
        }
      ],
      "delegateBlockedProviders": [],
      "delegationInstructionRoutes": [
        {
          "instructionPrefix": "[worker]",
          "workerId": "worker",
          "provider": null,
          "model": null
        }
      ]
    },
    {
      "id": "worker",
      "role": "worker",
      "defaultProvider": null,
      "defaultModel": null,
      "enabledProviders": [],
      "delegateAllowedModels": [
        {
          "provider": "ollama",
          "model": "llama3.2:3b",
          "local": false,
          "toolCapable": null
        }
      ]
    }
  ],
  "skills": {
    "contextMode": "full",
    "directory": null,
    "extraDirs": [],
    "enabled": []
  }
}
```

### Gateway

**`gateway`** — HTTP/WebSocket listen address and auth.

| Field | In `config.json` |
|-------|------------------|
| `port` | TCP port (default **15151**). |
| `bind` | Listen address (default **127.0.0.1**). |
| `auth.mode` | **`none`** or **`token`**. |
| `auth.token` | Shared secret when **`auth.mode`** is **`token`**. Can be set via **`CHAI_GATEWAY_TOKEN`** instead (see **Environment variables**). |

### Channels

**`channels`** — Optional channel integrations.

- **`channels.telegram`**
  - Optional **`botToken`** (defaults to **`TELEGRAM_BOT_TOKEN`** when not set)
  - Optional **`webhookUrl`**
  - Optional **`webhookSecret`**
  - **`TELEGRAM_BOT_TOKEN`** overrides **`botToken`** (see **Environment variables**)
- **`channels.signal`**
  - **`httpBase`** — Required base URL of a **user-run** [signal-cli](https://github.com/AsamK/signal-cli) HTTP daemon (e.g. **`http://127.0.0.1:7583`**)
  - Optional **`account`** (Signal account **`+E.164`** for multi-account daemons)
  - **`SIGNAL_CLI_HTTP`** overrides **`httpBase`** and **`SIGNAL_CLI_ACCOUNT`** overrides **`account`** (see **Environment variables**)
  - Chai does **not** ship signal-cli; see **`.agents/adr/SIGNAL_CLI_INTEGRATION.md`**
- **`channels.matrix`**
  - **`homeserver`** — Required HTTPS base URL (e.g. **`https://matrix.example.org`**)
  - Either **`accessToken`** (optional **`userId`**) or **`user`** + **`password`**
  - Optional **`storePath`** (defaults to **`~/.chai/matrix`** when not set)
  - Optional **`deviceId`** (used with **`accessToken`** when server does not return device id)
  - **`MATRIX_HOMESERVER`**, **`MATRIX_ACCESS_TOKEN`**, **`MATRIX_USER_ID`**, **`MATRIX_USER`**, **`MATRIX_PASSWORD`**, **`CHAI_MATRIX_STORE`**, and **`MATRIX_DEVICE_ID`** override config fields (see **Environment variables**)

### Providers

**`providers`** — Per-backend connection overrides.

- **`providers.ollama`**
  - Optional **`baseUrl`** (defaults to **`http://127.0.0.1:11434`** when not set)
- **`providers.lms`**
  - Optional **`baseUrl`** (defaults to **`http://127.0.0.1:1234/v1`** when not set)
- **`providers.hf`**
  - Optional **`baseUrl`** (defaults to **`http://127.0.0.1:8080/v1`** when not set; set a real Inference Endpoint or TGI URL including **`/v1`**)
  - Optional **`apiKey`**
  - **`HF_API_KEY`** overrides **`apiKey`** (see **Environment variables**)
- **`providers.vllm`**
  - Optional **`baseUrl`** (defaults to **`http://127.0.0.1:8000/v1`** when not set)
  - Optional **`apiKey`**
  - **`VLLM_API_KEY`** overrides **`apiKey`** (see **Environment variables**)
- **`providers.nim`**
  - Always **`https://integrate.api.nvidia.com/v1`**
  - Optional **`apiKey`**
  - Optional **`extraModels`** — array of NIM model ids merged into the gateway **`nimModels`** list (in addition to the built-in static catalog) for desktop and **`status`**
  - **`NVIDIA_API_KEY`** overrides **`apiKey`** (see **Environment variables**)
- **`providers.openai`**
  - Optional **`baseUrl`** (defaults to **`https://api.openai.com/v1`** when not set)
  - Optional **`apiKey`**
  - **`OPENAI_API_KEY`** overrides **`apiKey`** (see **Environment variables**)

### Agents

**`agents`** — JSON array: one **`"role": "orchestrator"`** and any number of **`"role": "worker"`**. Omit **`agents`** entirely to use built-in defaults (single orchestrator **`id`** **`orchestrator`**).

- **Orchestrator entry**
  - Required **`id`**
  - Optional **`defaultProvider`** (defaults to **`ollama`**)
  - Optional **`defaultModel`** (defaults to a provider-specific fallback, e.g. **`llama3.2:3b`** for **`ollama`**, **`llama-3.2-3B-instruct`** for **`lms`**, **`meta/llama-3.2-3b-instruct`** for **`nim`**)
  - Optional **`enabledProviders`**
  - Optional **`workspace`**
  - Optional **`maxSessionMessages`**
  - Optional **`maxDelegationsPerTurn`**
  - Optional **`maxDelegationsPerSession`**
  - Optional **`maxDelegationsPerProvider`**
    - Object whose keys are canonical provider ids (**`ollama`**, **`lms`**, **`vllm`**, **`hf`**, **`nim`**, **`openai`**) and whose values are integer caps (successful delegations per session to that provider).
  - Optional **`delegateAllowedModels`**
    - Array of objects; each object has:
      - **`provider`** (canonical provider id)
      - **`model`** (model id for that provider)
      - **`local`** (optional boolean hint)
      - **`toolCapable`** (optional boolean hint)
    - Omit or use an empty array to allow only the orchestrator’s effective default **`provider`** / **`model`** for **`delegate_task`** calls that do not specify **`workerId`**.
  - Optional **`delegateBlockedProviders`**
    - Array of canonical provider id strings (**`ollama`**, **`lms`**, **`hf`**, **`vllm`**, **`nim`**, **`openai`**).
  - Optional **`delegationInstructionRoutes`**
    - Array of objects; each object has:
      - **`instructionPrefix`**
      - **`workerId`** (optional)
      - **`provider`** (optional)
      - **`model`** (optional)
- **Worker entry**
  - Required **`id`** (referenced by **`delegate_task`** **`workerId`**)
  - Optional **`defaultProvider`** (defaults to orchestrator provider)
  - Optional **`defaultModel`** (defaults to orchestrator model)
  - Optional **`enabledProviders`**
  - Optional **`delegateAllowedModels`**
    - Same shape as on the orchestrator (array of objects with **`provider`**, **`model`**, optional **`local`**, optional **`toolCapable`**).
    - Omit or use an empty array to allow only that worker’s effective default **`provider`** / **`model`** for **`delegate_task`** when **`workerId`** matches.

### Skills

**`skills`** — Skill loading and layout.

| Field | In `config.json` |
|-------|------------------|
| `contextMode` | **`full`** or **`readOnDemand`** (default **`full`**). |
| `directory` | Optional root directory for on-disk skills (default: `~/.chai/skills/`). |
| `extraDirs` | Additional skill directory paths (array of strings). |
| `enabled` | Skill names to load (array of strings; default **none**). |

### Configuration Directory (`~/.chai/`)

The configuration directory contains the following:

- **`config.json`** — Main configuration file; see **Gateway** through **Skills** above for top-level keys.
- **`skills`** — On-disk skills tree. After **`chai init`**, bundled skills are extracted here.
- **`workspace`** — Default on-disk agent workspace (`AGENTS.md`, etc.); override with **`workspace`** on the orchestrator entry in **`config.json`**. See **Agents** → [Orchestrator workspace](#orchestrator-workspace).

### Environment variables

| Variable | Overrides | Description |
|----------|-----------|-------------|
| `CHAI_CONFIG_PATH` | Config file path | Full path to the configuration file. The default path is `~/.chai/config.json`. |
| `CHAI_GATEWAY_TOKEN` | `gateway.auth.token` | Shared secret for WebSocket connect when auth mode is `token`. |
| `TELEGRAM_BOT_TOKEN` | `channels.telegram.botToken` | Telegram bot token from BotFather. |
| `SIGNAL_CLI_HTTP` | `channels.signal.httpBase` | signal-cli HTTP daemon base URL (`http://127.0.0.1:7583`). |
| `SIGNAL_CLI_ACCOUNT` | `channels.signal.account` | Optional `+E.164` for multi-account signal-cli JSON-RPC. |
| `MATRIX_HOMESERVER` | `channels.matrix.homeserver` | Matrix homeserver base URL (`https://…`). |
| `MATRIX_ACCESS_TOKEN` | `channels.matrix.accessToken` | Matrix client access token. |
| `MATRIX_USER_ID` | `channels.matrix.userId` | Matrix user id (`@user:server`) when using an access token without password login. |
| `MATRIX_USER` | `channels.matrix.user` | Localpart or full MXID for password login. |
| `MATRIX_PASSWORD` | `channels.matrix.password` | Password for **`m.login.password`**. |
| `MATRIX_DEVICE_ID` | `channels.matrix.deviceId` | Device id for access-token session restore when whoami omits it. |
| `CHAI_MATRIX_STORE` | `channels.matrix.storePath` | Directory for Matrix SDK SQLite + crypto store (default `~/.chai/matrix`). |
| `VLLM_API_KEY` | `providers.vllm.apiKey` | Bearer token for vLLM when the server was started with `--api-key`. |
| `HF_API_KEY` | `providers.hf.apiKey` | Bearer token for Hugging Face OpenAI-compatible endpoints when required. |
| `NVIDIA_API_KEY` | `providers.nim.apiKey` | API key for NVIDIA NIM hosted API at `https://integrate.api.nvidia.com`. When set, this is used for the NIM provider. |
| `OPENAI_API_KEY` | `providers.openai.apiKey` | API key for the OpenAI API (or compatible **`providers.openai.baseUrl`**). |

## Connections

### WebSocket

Clients connect at `ws://<bind>:<port>/ws` (from **`gateway.bind`** and **`gateway.port`**), call `connect`, then `agent` (run model) and `send` (deliver message to a channel). Used by the desktop application and for scripting.

When **`gateway.bind`** is not loopback, use **`gateway.auth`** with **`mode`** **`token`** and a secret (or **`CHAI_GATEWAY_TOKEN`**).

### Telegram

**Long-poll** — The gateway calls Telegram’s **`getUpdates`**; good for local use. Set **`channels.telegram.botToken`** (or **`TELEGRAM_BOT_TOKEN`**).

**Webhook** — Telegram POSTs updates to your URL; better for a public gateway. Set **`channels.telegram.webhookUrl`** and optionally **`channels.telegram.webhookSecret`**.

### Signal

The gateway connects to a **BYO** signal-cli **`daemon --http`** instance: **`GET /api/v1/events`** (SSE) for inbound messages and **`POST /api/v1/rpc`** with method **`send`** for replies. Install and run signal-cli yourself (see upstream docs); start the daemon before the gateway, e.g. **`signal-cli -a +1234567890 daemon --http 127.0.0.1:7583`**, then set **`channels.signal.httpBase`** or **`SIGNAL_CLI_HTTP`**. Policy: **`.agents/adr/SIGNAL_CLI_INTEGRATION.md`**. **`/new`** in a 1:1 or group context starts a fresh session for that **`conversation_id`**, same as other channels.

### Matrix

The gateway uses **[matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk)** with a **SQLite** store under **`~/.chai/matrix`** by default (override **`CHAI_MATRIX_STORE`** or **`channels.matrix.storePath`**). It syncs with the **Client-Server API**, decrypts **encrypted** rooms when the account has keys, and sends replies with **`m.room.message`** (**plain text**; encrypted in **encrypted** rooms). Configure **`channels.matrix`** (see **Configuration → Channels**) or the **`MATRIX_*`** environment variables. The bot user must already be a member of rooms you expect to use; invite the bot from Element (or another client) first. **`/new`** in a room starts a fresh session for that room, same as Telegram.

## Agents

In Chai, **agents** are the **policy** for the assistant the gateway runs: they name the **orchestrator** that owns the conversation, optionally define **workers** for delegated subtasks, and set **defaults** for which **provider** and **model** to use, where **workspace** files such as **`AGENTS.md`** live, and how **model discovery** is scoped. An agent is not a separate service or binary—it is **configuration** that the gateway reads to route each turn and merge context. **Skills** supply instructions and optional tools; top-level **`providers`** supply URLs and API keys; the **`agents`** block ties those inputs to one orchestrator and any workers you define.

### Agent Orchestration

Each entry in **`agents`** has a unique **`id`**, a **`role`** (`orchestrator` or `worker`), and the optional fields listed under **Configuration → Agents** above. The gateway uses this to route turns to the right backend, pass model ids to each provider, decide which APIs to poll for model discovery, and load **`AGENTS.md`** from the orchestrator’s workspace. With multiple workers are configured, the orchestrator can delegate subtasks using the built-in **`delegate_task`** tool.

The orchestrator entry may set **`workspace`** (directory for agent context such as **`AGENTS.md`**). Default is `~/.chai/workspace/` when not set. **`AGENTS.md`** is created by **`chai init`** when missing; the gateway loads it as agent-level context and prepends it to the skills context each turn. For how to edit **`AGENTS.md`** and what else lives in the workspace directory, see **Workspace** below.

**Multi-agent example** — only the **`agents`** array; orchestration agent and worker agents:

```json
"agents": [
  {
    "id": "assistant",
    "role": "orchestrator",
    "defaultProvider": "ollama",
    "defaultModel": "llama3.2:3b",
    "enabledProviders": ["ollama", "lms"]
  },
  {
    "id": "engineer",
    "role": "worker",
    "defaultProvider": "lms",
    "defaultModel": "ibm/granite-4-micro",
    "enabledProviders": ["lms"]
  },
  {
    "id": "researcher",
    "role": "worker",
    "defaultProvider": "lms",
    "defaultModel": "ibm/granite-4-micro",
    "enabledProviders": ["lms"]
  }
]
```

### Providers and Models

The gateway integrates **six** model **backends** (named by **`agents.defaultProvider`**): **Ollama** (native Ollama API), **LM Studio** (`lms`, OpenAI-compatible local server), **vLLM** (OpenAI-compatible **`vllm serve`** for self-hosted inference), **Hugging Face** (`hf`, OpenAI-compatible Inference Endpoints, TGI, or similar), **NVIDIA NIM** (`nim`, hosted NVIDIA catalog API), **OpenAI** (`openai`, and OpenAI HTTP API or compatible base URL). They differ in **where** the model runs (your machine, your infrastructure, or a cloud API), **which** wire protocol and discovery endpoints Chai uses, and **whether** an API key or fixed base URL applies.

For **provider** taxonomy, configuration, and API comparisons, see [.agents/spec/PROVIDERS.md](.agents/spec/PROVIDERS.md). For **model** ids, repository inventory, and tool-fit notes, see [.agents/spec/MODELS.md](.agents/spec/MODELS.md). For the **API alignment** roadmap, see [.agents/EPIC_API_ALIGNMENT.md](.agents/EPIC_API_ALIGNMENT.md). To run **repeatable model tests** by deployment category, see [.testing](.testing/README.md). Endpoint-level detail and how Chai calls each API are in the per-backend references:

| Backend | Document |
|---------|----------|
| Ollama (`ollama`) | [.agents/ref/OLLAMA_REFERENCE.md](.agents/ref/OLLAMA_REFERENCE.md) |
| LM Studio (`lms`) | [.agents/ref/LM_STUDIO_REFERENCE.md](.agents/ref/LM_STUDIO_REFERENCE.md) |
| vLLM (`vllm`) | [.agents/ref/VLLM_REFERENCE.md](.agents/ref/VLLM_REFERENCE.md) |
| Hugging Face (`hf`) | [.agents/ref/HUGGINGFACE_REFERENCE.md](.agents/ref/HUGGINGFACE_REFERENCE.md) |
| NVIDIA NIM (`nim`) | [.agents/ref/NVIDIA_NIM_REFERENCE.md](.agents/ref/NVIDIA_NIM_REFERENCE.md) |
| OpenAI (`openai`) | [.agents/ref/OPENAI_REFERENCE.md](.agents/ref/OPENAI_REFERENCE.md) |

Set **`defaultProvider`** on the orchestrator entry to **`ollama`**, **`lms`**, **`vllm`**, **`hf`**, **`nim`**, or **`openai`** when no per-request override is used. Optional **`enabledProviders`** on the orchestrator entry lists which providers to poll for model discovery at startup (e.g. `["ollama", "lms", "vllm", "hf", "nim", "openai"]`). When absent or empty, only the default provider (`ollama`) is discovered.


Use the exact model id expected by the selected provider for **`defaultModel`**:

- For `ollama`, use the name from `ollama list` (e.g. `llama3.2:3b`, `qwen3:8b`).
- For `lms`, use the name from LM Studio or `GET /v1/models` (e.g. `llama-3.2-3B-instruct`, `openai/gpt-oss-20b`).
- For `vllm`, use the same id you pass to `vllm serve` (e.g. `Qwen/Qwen2.5-7B-Instruct`).
- For `hf`, use the model id your endpoint expects (e.g. `meta-llama/Llama-3.1-8B-Instruct`).
- For `nim`, use a NIM catalog id (e.g. `meta/llama-3.2-3b-instruct`); see [LLM APIs reference](https://docs.api.nvidia.com/nim/reference/llm-apis).
- For `openai`, use an OpenAI model id (e.g. `gpt-4o-mini`); see [OpenAI models](https://platform.openai.com/docs/models).

## Skills

Skills are markdown-based instructions (one per directory with a `SKILL.md` file) that are loaded into the agent’s context. A skill can optionally include a **`tools.json`** in the same directory to declare callable tools (name, parameters, and how they map to a CLI). **Only skills that have a `tools.json` expose tools to the agent;** skills without `tools.json` still provide their SKILL.md text as context but have no callable tools.

### Context Mode

`skills.contextMode`: how skill documentation is provided to the model.

- **`full`** (default) — All loaded skills’ full SKILL.md content is injected into the system message each turn. Best for few skills and smaller local models (e.g. 7B–9B).
- **`readOnDemand`** — The system message contains only a compact list (name, description). The model uses the **`read_skill`** tool to load a skill’s full SKILL.md when it clearly applies. Keeps the prompt small and scales to many skills; requires the model to call the tool before using a skill.

### Declaring Skills

Only skills listed in **`skills.enabled`** are loaded; the default is none. Add the skill names you want (e.g. `["notesmd-daily"]` or `["notesmd", "notesmd-daily"]`). A skill is only loaded when enabled and its `metadata.requires.bins` are on the gateway's PATH.

### Bundled skills

- **notesmd** — Create, read, search, update, and delete notes. Uses [NotesMD CLI](https://github.com/yakitrak/notesmd-cli) (binary `notesmd-cli`). Add `"notesmd"` to `skills.enabled` and ensure `notesmd-cli` is on PATH.
- **notesmd-daily** — Create, read, and update daily notes. Uses [NotesMD CLI](https://github.com/yakitrak/notesmd-cli) (binary `notesmd-cli`). Add `"notesmd-daily"` to `skills.enabled` and ensure `notesmd-cli` is on PATH.
- **obsidian** — Create, read, search, update, and delete notes. Uses [Obsidian CLI](https://help.obsidian.md/cli) (binary `obsidian`). Add `"obsidian"` to `skills.enabled` and ensure `obsidian` is on PATH.
- **obsidian-daily** — Create, read, and update daily notes. Uses [Obsidian CLI](https://help.obsidian.md/cli) (binary `obsidian`). Add `"obsidian-daily"` to `skills.enabled` and ensure `obsidian` is on PATH.

### Custom skills

Add skills to the config directory’s **`skills`** subdirectory (`~/.chai/skills`), or set **`skills.directory`** in config to another path (e.g. a repo’s `skills/` folder), or add paths in **`skills.extraDirs`**. One subdirectory per skill with a **`SKILL.md`** file; add **`tools.json`** in that directory to define the skill’s tools (without it, the skill has no callable tools). Use `name` and `description` in the frontmatter; use `metadata.requires.bins` so the skill loads only when those binaries are on PATH.

## Workspace

The workspace directory includes **frontloaded context** for the agent (e.g. `AGENTS.md`).

- **`AGENTS.md`** — Created when you run `chai init` (and only recreated if the file is missing). Edit the file to customize your agent. The gateway loads it as **agent-level context** and prepends it to the skills context on every turn. Recommendations vary based on the size and capabilities of the model.
