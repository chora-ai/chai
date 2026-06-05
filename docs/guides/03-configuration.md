# Configuration

The command-line interface and desktop application use the same configuration. This guide walks you through configuring chai from the simplest working setup to more advanced multi-agent and channel configurations.

## Initialization

After installing, run `chai init` to create `~/.chai/`:

- Two default profiles: `assistant` and `developer`
- An `active` symlink → `profiles/assistant/`
- A shared `skills/` tree (bundled skills extracted from the application)
- A `sandbox/` directory per profile for write-capable tools

Each profile gets its own `config.json`, agent context directories, and local state. The active profile is `assistant` by default.

## Profiles

Each profile is an independent configuration tree under `~/.chai/profiles/<name>/`. The active profile is a symlink at `~/.chai/active`. You can:

- **List profiles** — `chai profile list`
- **Show current profile** — `chai profile current`
- **Switch profiles** — `chai profile switch <name>` (the gateway must be stopped)
- **Override per process** — Set `CHAI_PROFILE` or run `chai gateway --profile <name>`

The gateway refuses profile switches while it is running (it holds an advisory lock at `~/.chai/gateway.lock`).

## Configuration File

Each profile has a `config.json` at `~/.chai/profiles/<name>/config.json`. An empty file is valid — built-in defaults are used at runtime:

```json
{}
```

With no `agents` key, the gateway runs a single orchestrator using Ollama and `llama3.2:3b`. Everything else has sensible defaults too: the gateway binds to `127.0.0.1:15151` with no auth, no channels are configured, and no skills are enabled.

## Configuring Providers

Providers are defined as a **JSON array** in the `providers` key. Each provider has a unique `id` (referenced by agents) and an `endpoint` type that determines the wire protocol. Additional fields like `baseUrl`, `apiKey`, and behavior settings are optional.

### Endpoint Types

An **endpoint type** describes the wire protocol — what HTTP routes to call and how to serialize/deserialize messages. The `id` is just a name; the `endpoint` determines the protocol.

| Endpoint | Description | Default Base URL | Default Model |
|----------|------------|------------------|---------------|
| `"ollama"` | Native Ollama API (`/api/chat`, `/api/tags`) | `http://127.0.0.1:11434` | `llama3.2:3b` |
| `"openai-compat"` | OpenAI-compatible servers (`/v1/chat/completions`, `/v1/models`) | `http://127.0.0.1:1234/v1` | `gpt-4o-mini` |
| `"anthropic"` | Anthropic Messages API (not yet implemented) | `https://api.anthropic.com` | `claude-sonnet-4-20250514` |
| `"google"` | Google Gemini API (not yet implemented) | `https://generativelanguage.googleapis.com` | `gemini-2.5-flash` |

The `"openai-compat"` endpoint type covers any server speaking the OpenAI chat completions protocol — LM Studio, vLLM, OpenAI itself, Hugging Face TGI, NVIDIA NIM, and more. They are all the same endpoint type, differentiated by `baseUrl` and behavior fields.

### Common Provider Examples

**Ollama (default localhost — no `providers` key needed):**

```json
{}
```

The gateway defaults to a single Ollama provider at `http://127.0.0.1:11434` with model `llama3.2:3b`.

**LM Studio instead of Ollama:**

```json
{
  "providers": [
    { "id": "lms", "endpoint": "openai-compat", "modelDiscovery": "lmstudio", "autoLoad": "lmstudio" }
  ],
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "lms",
      "defaultModel": "ibm/granite-4-micro"
    }
  ]
}
```

LM Studio uses `modelDiscovery: "lmstudio"` to list models via its native `GET /api/v1/models` endpoint (instead of `GET /v1/models`), and `autoLoad: "lmstudio"` to automatically load an unloaded model and retry when LM Studio returns an "unloaded" error.

**OpenAI with an API key:**

```json
{
  "providers": [
    { "id": "openai", "endpoint": "openai-compat", "baseUrl": "https://api.openai.com/v1" }
  ],
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "openai",
      "defaultModel": "gpt-4o-mini"
    }
  ]
}
```

Set the API key via the `OPENAI_API_KEY` environment variable or add `"apiKey": "sk-..."` to the provider object. Environment variables override the file values at runtime.

**vLLM:**

```json
{
  "providers": [
    { "id": "vllm", "endpoint": "openai-compat", "baseUrl": "http://127.0.0.1:8000/v1" }
  ],
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "vllm",
      "defaultModel": "Qwen/Qwen2.5-7B-Instruct"
    }
  ]
}
```

If the vLLM server uses `--api-key`, set `VLLM_API_KEY` or add `"apiKey"` to the provider.

**Hugging Face (TGI / Inference Endpoints):**

```json
{
  "providers": [
    { "id": "hf", "endpoint": "openai-compat", "baseUrl": "https://your-deployment.endpoints.huggingface.cloud/v1" }
  ],
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "hf",
      "defaultModel": "meta-llama/Llama-3.1-8B-Instruct"
    }
  ]
}
```

Set `HF_API_KEY` or add `"apiKey"` to the provider.

**NVIDIA NIM with a static model list:**

```json
{
  "providers": [
    {
      "id": "nim",
      "endpoint": "openai-compat",
      "baseUrl": "https://integrate.api.nvidia.com/v1",
      "modelDiscovery": "static",
      "staticModels": [
        "meta/llama-3.1-8b-instruct",
        "meta/llama-3.1-70b-instruct",
        "deepseek-ai/deepseek-v3.1"
      ]
    }
  ],
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "nim",
      "defaultModel": "meta/llama-3.1-8b-instruct"
    }
  ]
}
```

NIM does not expose a `/v1/models` endpoint, so `modelDiscovery: "static"` is used with a user-curated list in `staticModels`. Set `NVIDIA_API_KEY` or add `"apiKey"`.

**OpenAI-compatible proxy (Azure, Venice, etc.):**

```json
{
  "providers": [
    { "id": "azure", "endpoint": "openai-compat", "baseUrl": "https://my-proxy.example.com/v1", "apiKey": "sk-..." }
  ]
}
```

This is useful for Azure OpenAI endpoints, Venice, or any other OpenAI-compatible proxy.

**Multiple providers:**

```json
{
  "providers": [
    { "id": "ollama", "endpoint": "ollama" },
    { "id": "lms", "endpoint": "openai-compat", "modelDiscovery": "lmstudio", "autoLoad": "lmstudio" },
    { "id": "openai", "endpoint": "openai-compat", "baseUrl": "https://api.openai.com/v1" }
  ],
  "agents": [
    {
      "id": "assistant",
      "role": "orchestrator",
      "defaultProvider": "ollama",
      "defaultModel": "llama3.2:3b",
      "enabledProviders": ["ollama", "lms", "openai"]
    }
  ]
}
```

### Behavior Fields

#### Model Discovery

The `modelDiscovery` field controls how a provider's available model list is obtained.

| Value | Description | Default For |
|-------|-------------|-------------|
| `"default"` | Use the endpoint type's standard discovery method. | All endpoint types |
| `"lmstudio"` | LM Studio native: `GET /api/v1/models`, filter `type == "llm"`. | — |
| `"static"` | Use the `staticModels` config field. No polling. | — |

When omitted, `modelDiscovery` defaults to `"default"`, which uses `GET /api/tags` for `"ollama"` and `GET /v1/models` for `"openai-compat"`.

#### Static Models

The `staticModels` field is an array of model id strings used when `modelDiscovery: "static"`. This is useful for providers that lack a model list endpoint or when you want to curate the list yourself.

#### Auto-Load

The `autoLoad` field controls whether a failed chat request triggers a model-load retry. Set to `"lmstudio"` for LM Studio's auto-load feature (on "unloaded" error, call `POST /api/v1/models/load` and retry). Default is `false`.

### Model Id Reference

Use the exact model id expected by the selected provider for `defaultModel`:

| Provider `id` (example) | Endpoint | Model id example | Where to find it |
|--------------------------|----------|-----------------|------------------|
| `ollama` | `"ollama"` | `llama3.2:3b`, `qwen3:8b` | `ollama list` |
| `lms` | `"openai-compat"` + `modelDiscovery: "lmstudio"` | `llama-3.2-3B-instruct` | LM Studio UI or `GET …/api/v1/models` |
| `vllm` | `"openai-compat"` | `Qwen/Qwen2.5-7B-Instruct` | Same id you pass to `vllm serve` |
| `hf` | `"openai-compat"` | `meta-llama/Llama-3.1-8B-Instruct` | Your endpoint's expected id |
| `nim` | `"openai-compat"` + `modelDiscovery: "static"` | `meta/llama-3.1-8b-instruct` | [NVIDIA LLM APIs](https://docs.api.nvidia.com/nim/reference/llm-apis) |
| `openai` | `"openai-compat"` | `gpt-4o-mini` | [OpenAI models](https://platform.openai.com/docs/models) |

For systematic provider and model testing, see the [Testing Playbooks](../testing/README.md).

## Configuring Channels

Channels connect the gateway to messaging platforms. Add a `channels` block to enable one.

**Telegram (long-poll):**

```json
{
  "channels": {
    "telegram": {
      "botToken": "123456:ABC-DEF..."
    }
  }
}
```

Or set `TELEGRAM_BOT_TOKEN` as an environment variable. For webhook mode (better for public gateways), also set `webhookUrl` and optionally `webhookSecret`. See [Connections](04-connections.md) for the full setup walkthrough.

**Matrix:**

```json
{
  "channels": {
    "matrix": {
      "homeserver": "https://matrix.org",
      "accessToken": "syt_...",
      "userId": "@my-bot:matrix.org"
    }
  }
}
```

Or use `user` + `password` for `m.login.password` auth. The corresponding `MATRIX_*` environment variables also work.

**Signal:**

Signal requires a running signal-cli HTTP daemon. Point to it:

```json
{
  "channels": {
    "signal": {
      "httpBase": "http://127.0.0.1:7583",
      "account": "+1234567890"
    }
  }
}
```

See [Connections](04-connections.md) for signal-cli setup instructions.

For hands-on channel setup, see the user journeys: [Telegram](../journey/05-channel-telegram.md) · [Matrix](../journey/08-channel-matrix.md) · [Signal](../journey/09-channel-signal.md).

## Configuring Agents

The `agents` array defines the orchestrator and optional workers. Omit the key entirely for a single-orchestrator default setup.

**Single orchestrator with custom provider and skills:**

```json
{
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "openai",
      "defaultModel": "gpt-4o-mini",
      "skillsEnabled": ["files", "notesmd-daily"],
      "contextMode": "full"
    }
  ]
}
```

**Orchestrator with a worker:**

```json
{
  "providers": [
    { "id": "ollama", "endpoint": "ollama" },
    { "id": "lms", "endpoint": "openai-compat", "modelDiscovery": "lmstudio", "autoLoad": "lmstudio" }
  ],
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
    }
  ]
}
```

With workers configured, the orchestrator can delegate subtasks using the built-in `delegate_task` tool. Each agent gets its own `AGENT.md` at `~/.chai/active/agents/<agentId>/AGENT.md`. See [Agents](05-agents.md) for more on orchestration and delegation.

**Key agent fields:**

- `defaultProvider` / `defaultModel` — Which backend and model the agent uses. The `defaultProvider` must match a provider `id` in the `providers` array.
- `enabledProviders` — Which providers to poll for model discovery at startup. Provider ids must match entries in the `providers` array. When omitted or empty, only the default provider is discovered.
- `skillsEnabled` — Which skill packages to load for this agent. Omitted or empty means no skills.
- `contextMode` — How skill content appears in the system context: `full` (inlined) or `readOnDemand` (compact list + `read_skill` tool).

## Securing the Gateway

By default, the gateway binds to `127.0.0.1` with no authentication — safe for local use. If you bind to a non-loopback address (to accept connections from other machines), enable token auth:

```json
{
  "gateway": {
    "bind": "0.0.0.0",
    "auth": {
      "mode": "token",
      "token": "your-secret-here"
    }
  }
}
```

Or set `CHAI_GATEWAY_TOKEN` as an environment variable. Clients must present this token when connecting via WebSocket.

## Configuration Directory

The `~/.chai/` directory structure:

| Path | Purpose |
|------|---------|
| `profiles/<name>/config.json` | Per-profile configuration |
| `profiles/<name>/agents/<agentId>/AGENT.md` | Per-agent instructions |
| `profiles/<name>/sandbox/` | Write boundary for tools (see [Write Sandbox](07-sandbox.md)) |
| `profiles/<name>/paired.json` | Desktop pairing state |
| `active` | Symlink to the active profile |
| `skills/` | Shared on-disk skills tree |
| `gateway.lock` | Advisory lock while gateway runs (profile + PID) |

---

## Configuration Reference

Complete field-level reference for `config.json`. All keys are `camelCase`.

### Gateway

| Field | Default | Override | Note |
|-------|---------|----------|------|
| `gateway.port` | `15151` | - | - |
| `gateway.bind` | `127.0.0.1` | - | - |
| `gateway.auth.mode` | `none` | - | `none` or `token` |
| `gateway.auth.token` | - | `CHAI_GATEWAY_TOKEN` | Only used if `mode` is `token` |

### Channels

| Field | Default | Override | Note |
|-------|---------|----------|------|
| `channels.telegram.botToken` | - | `TELEGRAM_BOT_TOKEN` | Required for Telegram |
| `channels.telegram.webhookUrl` | - | - | Long-poll used if not set |
| `channels.telegram.webhookSecret` | - | `TELEGRAM_WEBHOOK_SECRET` | Only used if `webhookUrl` is set |
| `channels.signal.httpBase` | - | `SIGNAL_CLI_HTTP` | Required for Signal |
| `channels.signal.account` | - | `SIGNAL_CLI_ACCOUNT` | Multi-account daemon: `+E.164` |
| `channels.matrix.homeserver` | - | `MATRIX_HOMESERVER` | Required for Matrix |
| `channels.matrix.accessToken` | - | `MATRIX_ACCESS_TOKEN` | Or `user` + `password` |
| `channels.matrix.user` | - | `MATRIX_USER` | Password login localpart or MXID |
| `channels.matrix.password` | - | `MATRIX_PASSWORD` | For `m.login.password` |
| `channels.matrix.userId` | - | `MATRIX_USER_ID` | With token auth, for echo filtering |
| `channels.matrix.deviceId` | - | `MATRIX_DEVICE_ID` | Token restore when whoami omits device id |
| `channels.matrix.roomIds` | - | `MATRIX_ROOM_ALLOWLIST` | Non-empty config list limits turns to those rooms; env (comma-separated) replaces the list when set and non-empty |

### Providers

The `providers` array contains provider definitions. Each provider has a unique `id` (referenced by agents) and an `endpoint` type.

| Field | Type | Required | Default | Note |
|-------|------|----------|---------|------|
| `id` | `string` | Yes | — | Unique provider id referenced by agents. |
| `endpoint` | `string` | Yes | — | One of: `"ollama"`, `"openai-compat"`, `"anthropic"`, `"google"`. |
| `baseUrl` | `string` | No | Per-endpoint default | Override the endpoint type's default base URL. |
| `apiKey` | `string` | No | Per-endpoint env var | API key override. Env var takes precedence when set. |
| `defaultModel` | `string` | No | Per-endpoint default | Default model id fallback for this provider when the agent's `defaultModel` is unset. |
| `modelDiscovery` | `string` | No | `"default"` | One of: `"default"`, `"lmstudio"`, `"static"`. |
| `staticModels` | `string[]` | No | `[]` | Model list when `modelDiscovery: "static"`. |
| `autoLoad` | `false` or `"lmstudio"` | No | `false` | Auto-load on "unloaded" error. |

**Endpoint type defaults:**

| Endpoint | Default `baseUrl` | Default `defaultModel` | Env var for API key |
|----------|-------------------|------------------------|---------------------|
| `"ollama"` | `http://127.0.0.1:11434` | `llama3.2:3b` | — |
| `"openai-compat"` | `http://127.0.0.1:1234/v1` | `gpt-4o-mini` | — |
| `"anthropic"` | `https://api.anthropic.com` | `claude-sonnet-4-20250514` | `ANTHROPIC_API_KEY` |
| `"google"` | `https://generativelanguage.googleapis.com` | `gemini-2.5-flash` | `GOOGLE_API_KEY` |

**Common provider configurations:**

| Provider `id` | `endpoint` | `baseUrl` | `modelDiscovery` | `autoLoad` | Notes |
|---------------|-----------|-----------|-------------------|-----------|-------|
| `ollama` | `"ollama"` | (default) | (default) | `false` | Default localhost Ollama |
| `lms` | `"openai-compat"` | (default) | `"lmstudio"` | `"lmstudio"` | LM Studio with auto-load |
| `vllm` | `"openai-compat"` | `http://127.0.0.1:8000/v1` | (default) | `false` | Include `/v1` in `baseUrl` |
| `hf` | `"openai-compat"` | Your endpoint `/v1` | (default) | `false` | Set `baseUrl` to your TGI/IE URL |
| `nim` | `"openai-compat"` | `https://integrate.api.nvidia.com/v1` | `"static"` | `false` | Set `staticModels` with your model list |
| `openai` | `"openai-compat"` | `https://api.openai.com/v1` | (default) | `false` | Set `OPENAI_API_KEY` or `apiKey` |

### Agents

The `agents` array contains exactly one `"role": "orchestrator"` and any number of `"role": "worker"` entries. Omit the `agents` key (or set `"agents": null`) for built-in defaults: a single orchestrator with id `orchestrator`.

**Orchestrator-only fields** (ignored on worker objects): `maxSessionMessages`, `maxToolLoopIterations`, `maxDelegationsPerTurn`, `maxDelegationsPerSession`, `maxDelegationsPerProvider`, `delegateBlockedProviders`, `delegationInstructionRoutes`.

| Field | Default (When Field Omitted) | Default (When `agents` Omitted) | Note |
|-------|------------------------------|----------------------------------|------|
| `id` | Required in `agents` array | `orchestrator` | Unique per entry. Worker `id` is `delegate_task` `workerId`. |
| `role` | Required in `agents` array | `orchestrator` | Must be `orchestrator` or `worker`. |
| `defaultProvider` | Orchestrator: `ollama`. Worker: same as orchestrator | `ollama` | Must match a provider `id` in the `providers` array. Fallback: `ollama`. |
| `defaultModel` | Orchestrator: provider fallback. Worker: worker string, else orchestrator string, then fallback | `llama3.2:3b` (built-in `defaultProvider` is `ollama`) | Fallbacks come from the endpoint type's `defaultModel`: `"ollama"` → `llama3.2:3b`; `"openai-compat"` → `gpt-4o-mini`; `"anthropic"` → `claude-sonnet-4-20250514`; `"google"` → `gemini-2.5-flash`. A provider's `defaultModel` field overrides the endpoint-type default. |
| `enabledProviders` | Orchestrator: only `defaultProvider` polled. Worker: see Note | same | Provider ids must match entries in the `providers` array. Orchestrator: `null` or `[]` → poll that provider only; non-empty → only those. Worker: `null` → no extra `delegate_task` restriction; `[]` → only default provider; non-empty → only listed providers. |
| `skillsEnabled` | No skill packages | same | Omitted or `[]`: nothing loaded from `~/.chai/skills`. |
| `contextMode` | `full` | same | `full` or `readOnDemand`. |
| `maxSessionMessages` | All messages (no trim) | same | Orchestrator only. When set and `> 0`, only the last N messages are sent; full history stays in the session store. |
| `maxToolLoopIterations` | `100` | `100` | Orchestrator only. Maximum LLM round-trips per turn. The loop exits naturally when the model returns no tool calls; this is a safety net against runaway loops. Applies to both orchestrator and worker (delegate) turns. |
| `maxDelegationsPerTurn` | No dedicated cap | same | Orchestrator only. Excess `delegate_task` calls error in that turn. |
| `maxDelegationsPerSession` | No limit | same | Orchestrator only. |
| `maxDelegationsPerProvider` | No per-provider cap | same | Orchestrator only. Keys are provider ids; values are max successful delegations per session. |
| `delegateAllowedModels` | Only effective default (provider, model) for that scope | same | Non-empty: only listed `{ provider, model, local?, toolCapable? }`. A non-empty worker list overrides the orchestrator list for that `workerId`. |
| `delegateBlockedProviders` | Nothing blocked | same | Orchestrator only. Non-empty: those provider ids disallowed for `delegate_task`. |
| `delegationInstructionRoutes` | None | same | Orchestrator only. `{ instructionPrefix, workerId?, provider?, model? }`; first matching prefix fills missing `delegate_task` fields. |

### Environment Variables

| Variable | Overrides | Description |
|----------|-----------|-------------|
| `CHAI_PROFILE` | Active profile | Profile name; overrides `~/.chai/active` for that process. |
| `CHAI_GATEWAY_TOKEN` | `gateway.auth.token` | Shared secret when auth mode is `token`. |
| `TELEGRAM_BOT_TOKEN` | `channels.telegram.botToken` | Telegram bot token from BotFather. |
| `TELEGRAM_WEBHOOK_SECRET` | `channels.telegram.webhookSecret` | Webhook verification secret. |
| `SIGNAL_CLI_HTTP` | `channels.signal.httpBase` | signal-cli HTTP daemon base URL. |
| `SIGNAL_CLI_ACCOUNT` | `channels.signal.account` | `+E.164` for multi-account signal-cli. |
| `MATRIX_HOMESERVER` | `channels.matrix.homeserver` | Matrix homeserver base URL. |
| `MATRIX_ACCESS_TOKEN` | `channels.matrix.accessToken` | Matrix access token. |
| `MATRIX_USER_ID` | `channels.matrix.userId` | Matrix user id for echo filtering with token auth. |
| `MATRIX_USER` | `channels.matrix.user` | Localpart or MXID for password login. |
| `MATRIX_PASSWORD` | `channels.matrix.password` | Password for `m.login.password`. |
| `MATRIX_DEVICE_ID` | `channels.matrix.deviceId` | Device id for token session restore. |
| `MATRIX_ROOM_ALLOWLIST` | `channels.matrix.roomIds` | Comma-separated room ids; replaces config allowlist when set and non-empty. |
| `ANTHROPIC_API_KEY` | Provider `apiKey` with `endpoint: "anthropic"` | API key for Anthropic (Claude). |
| `GOOGLE_API_KEY` | Provider `apiKey` with `endpoint: "google"` | API key for Google (Gemini). |
