# Configuration

The command-line interface and desktop application use the same configuration. This guide walks you through configuring chai from the simplest working setup to more advanced multi-agent and channel configurations.

## Initialization

After installing, run `chai init` to create `~/.chai/`:

```bash
chai init
```

This creates:

- Two default profiles: `assistant` and `developer`
- An `active` symlink → `profiles/assistant/`
- A shared `skills/` tree (bundled skills extracted from the application)
- A `sandbox/` directory per profile for write-capable tools

Each profile gets its own `config.json`, agent context directories, and local state. The active profile is `assistant` by default.

### Re-Running `chai init`

`chai init` is safe to run on an already-initialized configuration directory. It follows a strict non-destructive policy: existing files are never overwritten, and existing settings are preserved.

**Profile files** — Each file is only written when it does not already exist:

| File | Behavior |
|------|----------|
| `profiles/<name>/config.json` | Created with `{}` if absent; existing configuration is preserved |
| `profiles/<name>/agents/orchestrator/AGENT.md` | Seeded from bundled template if absent; existing instructions are preserved |
| `profiles/<name>/sandbox/AGENTS.md` | Seeded from bundled template if absent; existing content is preserved |
| `profiles/<name>/sandbox/README.md` | Seeded from bundled template if absent; existing content is preserved |

**Bundled skills** — Each bundled skill is extracted into `~/.chai/skills/<name>/` using content-addressed versioning:

| Component | Behavior |
|-----------|----------|
| `versions/<hash>/` snapshot | Created if absent. Immutable — never re-written once created. |
| `active` symlink | Set only when no active version exists (fresh installation). If the skill already has an `active` symlink pointing to a valid version, it is left unchanged — this preserves user customizations such as manual rollbacks or edits via `skills_write_skill_md`. The new bundled version snapshot is still written to disk, so the user can switch to it manually if desired. |

**Profile `active` symlink** — `~/.chai/active` is set to `profiles/assistant/` only when no valid `active` symlink already exists (fresh installation or broken symlink). If the symlink already points to a valid profile directory, it is left unchanged — this preserves the user's active profile choice across re-initialization.

### When to Re-Run

Re-running `chai init` is useful when:

- A new version of chai ships updated bundled skills — the new version snapshots will be created on disk (you can adopt them with `chai skill rollback` or by manually updating the `active` symlink)
- A profile directory or `sandbox/` was accidentally deleted — the missing directories and template files will be re-created
- You want to ensure the default profile scaffold is complete

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
| `"openai-compat"` | OpenAI-compatible servers (`/v1/chat/completions`, `/v1/models`) | `http://127.0.0.1:1234/v1` | `llama-3.2-3B-instruct` |

The `"openai-compat"` endpoint type covers any server speaking the OpenAI chat completions protocol — LM Studio, NearAI, NVIDIA NIM, and more. They are all the same endpoint type, differentiated by `baseUrl` and behavior fields.

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
      "defaultModel": "openai/gpt-oss-20b"
    }
  ]
}
```

LM Studio uses `modelDiscovery: "lmstudio"` to list models via its native `GET /api/v1/models` endpoint (instead of `GET /v1/models`), and `autoLoad: "lmstudio"` to automatically load an unloaded model and retry when LM Studio returns an "unloaded" error.

**NearAI (remote OpenAI-compatible API):**

```json
{
  "providers": [
    { "id": "nearai", "endpoint": "openai-compat", "baseUrl": "https://cloud-api.near.ai/v1" }
  ],
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "nearai",
      "defaultModel": "zai-org/GLM-5.1-FP8"
    }
  ]
}
```

Set the API key via the `apiKey` field in the provider object. This same pattern applies to any remote OpenAI-compatible API — OpenAI itself, Azure OpenAI, Together, Groq, etc. — just change the `baseUrl` and model id.

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

NIM does not expose a `/v1/models` endpoint, so `modelDiscovery: "static"` is used with a user-curated list in `staticModels`. Set `"apiKey"` to a literal key string or an environment variable reference like `"<NVIDIA_API_KEY>"` (the named variable is read from the shell environment or a `.env` file in the profile directory).

**Multiple providers:**

```json
{
  "providers": [
    { "id": "ollama", "endpoint": "ollama" },
    { "id": "lms", "endpoint": "openai-compat", "modelDiscovery": "lmstudio", "autoLoad": "lmstudio" },
    { "id": "nearai", "endpoint": "openai-compat", "baseUrl": "https://cloud-api.near.ai/v1" }
  ],
  "agents": [
    {
      "id": "assistant",
      "role": "orchestrator",
      "defaultProvider": "ollama",
      "defaultModel": "llama3.2:3b",
      "enabledProviders": ["ollama", "lms", "nearai"]
    }
  ]
}
```

### Other OpenAI-Compatible Servers

Any server that exposes OpenAI-shaped routes (`/v1/chat/completions`, optionally `/v1/models`) can be configured as an `"openai-compat"` provider with the appropriate `baseUrl` and `apiKey`. This includes vLLM, Hugging Face TGI, OpenAI, Azure OpenAI, and any other OpenAI-compatible proxy — no special behavior fields are needed, just `endpoint: "openai-compat"` and the correct base URL.

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
| `nearai` | `"openai-compat"` | `zai-org/GLM-5.1-FP8` | [NearAI model catalog](https://near.ai) |
| `nim` | `"openai-compat"` + `modelDiscovery: "static"` | `meta/llama-3.1-8b-instruct` | [NVIDIA LLM APIs](https://docs.api.nvidia.com/nim/reference/llm-apis) |

For other OpenAI-compatible servers, use the model id that the server expects (e.g. the same id you pass to `vllm serve`, your endpoint's Hugging Face model id, etc.).

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

For hands-on channel setup, see the user journeys: [Telegram](../journey/04-channel-telegram.md) · [Matrix](../journey/08-channel-matrix.md) · [Signal](../journey/09-channel-signal.md).
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
      "defaultModel": "llama-3.2-3B-instruct",
      "skillsEnabled": ["files", "git-read"],
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
      "defaultModel": "openai/gpt-oss-20b",
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
| `endpoint` | `string` | Yes | — | One of: `"ollama"`, `"openai-compat"`. |
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
| `"openai-compat"` | `http://127.0.0.1:1234/v1` | `llama-3.2-3B-instruct` | — |

**Common provider configurations:**

| Provider `id` | `endpoint` | `baseUrl` | `modelDiscovery` | `autoLoad` | Notes |
|---------------|-----------|-----------|-------------------|-----------|-------|
| `ollama` | `"ollama"` | (default) | (default) | `false` | Default localhost Ollama |
| `lms` | `"openai-compat"` | (default) | `"lmstudio"` | `"lmstudio"` | LM Studio with auto-load |
| `nearai` | `"openai-compat"` | `https://cloud-api.near.ai/v1` | (default) | `false` | Set `apiKey` |
| `nim` | `"openai-compat"` | `https://integrate.api.nvidia.com/v1` | `"static"` | `false` | Set `staticModels` with your model list |

### Agents

The `agents` array contains exactly one `"role": "orchestrator"` and any number of `"role": "worker"` entries. Omit the `agents` key (or set `"agents": null`) for built-in defaults: a single orchestrator with id `orchestrator`.

**Orchestrator-only fields** (ignored on worker objects): `maxSessionMessages`, `maxToolLoopIterations`, `maxDelegationsPerTurn`, `maxDelegationsPerSession`, `maxDelegationsPerProvider`, `delegateBlockedProviders`.

| Field | Default (When Field Omitted) | Default (When `agents` Omitted) | Note |
|-------|------------------------------|----------------------------------|------|
| `id` | Required in `agents` array | `orchestrator` | Unique per entry. Worker `id` is `delegate_task` `workerId`. |
| `role` | Required in `agents` array | `orchestrator` | Must be `orchestrator` or `worker`. |
| `defaultProvider` | Orchestrator: `ollama`. Worker: same as orchestrator | `ollama` | Must match a provider `id` in the `providers` array. Fallback: `ollama`. |
| `defaultModel` | Orchestrator: provider fallback. Worker: worker string, else orchestrator string, then fallback | `llama3.2:3b` (built-in `defaultProvider` is `ollama`) | Fallbacks come from the endpoint type's `defaultModel`: `"ollama"` → `llama3.2:3b`; `"openai-compat"` → `llama-3.2-3B-instruct`. A provider's `defaultModel` field overrides the endpoint-type default. |
| `enabledProviders` | Orchestrator: only `defaultProvider` polled. Worker: see Note | same | Provider ids must match entries in the `providers` array. Orchestrator: `null` or `[]` → poll that provider only; non-empty → only those. Worker: `null` → no extra `delegate_task` restriction; `[]` → only default provider; non-empty → only listed providers. |
| `skillsEnabled` | No skill packages | same | Omitted or `[]`: nothing loaded from `~/.chai/skills`. |
| `contextMode` | `full` | same | `full` or `readOnDemand`. |
| `maxSessionMessages` | All messages (no trim) | same | Orchestrator only. When set and `> 0`, only the last N messages are sent; full history stays in the session store. |
| `maxToolLoopIterations` | `100` | `100` | Orchestrator only. Maximum LLM round-trips per turn. The loop exits naturally when the model returns no tool calls; this is a safety net against runaway loops. Applies to both orchestrator and worker (delegate) turns. When the limit is reached during an orchestrator turn, the turn is interrupted: the last tool call is not executed, a `session.tool_loop_limit` event is emitted (with the pending tool calls), and the desktop displays a banner explaining what happened. The user must send another message to continue. |
| `maxDelegationsPerTurn` | No dedicated cap | same | Orchestrator only. Excess `delegate_task` calls error in that turn. |
| `maxDelegationsPerSession` | No limit | same | Orchestrator only. |
| `maxDelegationsPerProvider` | No per-provider cap | same | Orchestrator only. Keys are provider ids; values are max successful delegations per session. |
| `delegateAllowedModels` | Only effective default (provider, model) for that scope | same | Non-empty: only listed `{ provider, model, local?, toolCapable? }`. A non-empty worker list overrides the orchestrator list for that `workerId`. |
| `delegateBlockedProviders` | Nothing blocked | same | Orchestrator only. Non-empty: those provider ids disallowed for `delegate_task`. |
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
