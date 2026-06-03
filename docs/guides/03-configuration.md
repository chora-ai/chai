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

## Configuring a Provider

When you want to use a provider other than Ollama (or a different Ollama address), add a `providers` block. Each provider entry is optional — include only the ones you need.

**Using LM Studio instead of Ollama:**

```json
{
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

**Using OpenAI with an API key:**

```json
{
  "providers": {
    "openai": {
      "apiKey": "sk-..."
    }
  },
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

You can also set API keys via environment variables (`OPENAI_API_KEY`, `NVIDIA_API_KEY`, `HF_API_KEY`, `VLLM_API_KEY`) instead of putting them in `config.json`. Environment variables override the file values at runtime.

**Overriding a provider's base URL:**

```json
{
  "providers": {
    "openai": {
      "baseUrl": "https://my-proxy.example.com/v1",
      "apiKey": "sk-..."
    }
  }
}
```

This is useful for Azure OpenAI endpoints or other OpenAI-compatible proxies.

Use the exact model id expected by the selected provider for `defaultModel`:

| Provider | Model id example | Where to find it |
|----------|-----------------|------------------|
| `ollama` | `llama3.2:3b`, `qwen3:8b` | `ollama list` |
| `lms` | `llama-3.2-3B-instruct` | LM Studio UI or `GET …/api/v1/models` |
| `vllm` | `Qwen/Qwen2.5-7B-Instruct` | Same id you pass to `vllm serve` |
| `hf` | `meta-llama/Llama-3.1-8B-Instruct` | Your endpoint's expected id |
| `nim` | `meta/llama-3.2-3b-instruct` | [NVIDIA LLM APIs](https://docs.api.nvidia.com/nim/reference/llm-apis) |
| `openai` | `gpt-4o-mini` | [OpenAI models](https://platform.openai.com/docs/models) |

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

- `defaultProvider` / `defaultModel` — Which backend and model the agent uses.
- `enabledProviders` — Which providers to poll for model discovery at startup. When omitted or empty, only the default provider is discovered.
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

| Field | Default | Override | Note |
|-------|---------|----------|------|
| `providers.ollama.baseUrl` | `http://127.0.0.1:11434` | - | Ollama client default when unset |
| `providers.lms.baseUrl` | `http://127.0.0.1:1234/v1` | - | OpenAI-compatible LM Studio API |
| `providers.vllm.baseUrl` | `http://127.0.0.1:8000/v1` | - | Include `/v1` |
| `providers.vllm.apiKey` | - | `VLLM_API_KEY` | When server uses `--api-key` |
| `providers.hf.baseUrl` | `http://127.0.0.1:8080/v1` | - | Set a real Inference Endpoint or TGI URL with `/v1` |
| `providers.hf.apiKey` | - | `HF_API_KEY` | - |
| `providers.nim.apiKey` | - | `NVIDIA_API_KEY` | Base URL is fixed (`https://integrate.api.nvidia.com/v1`) |
| `providers.nim.extraModels` | - | - | NIM model id array; merged into gateway `nimModels` / desktop `status` |
| `providers.openai.baseUrl` | `https://api.openai.com/v1` | - | Override for Azure or other compatible endpoints |
| `providers.openai.apiKey` | - | `OPENAI_API_KEY` | - |

### Agents

The `agents` array contains exactly one `"role": "orchestrator"` and any number of `"role": "worker"` entries. Omit the `agents` key (or set `"agents": null`) for built-in defaults: a single orchestrator with id `orchestrator`.

**Orchestrator-only fields** (ignored on worker objects): `maxSessionMessages`, `maxToolLoopIterations`, `maxDelegationsPerTurn`, `maxDelegationsPerSession`, `maxDelegationsPerProvider`, `delegateBlockedProviders`, `delegationInstructionRoutes`.

| Field | Default (When Field Omitted) | Default (When `agents` Omitted) | Note |
|-------|------------------------------|----------------------------------|------|
| `id` | Required in `agents` array | `orchestrator` | Unique per entry. Worker `id` is `delegate_task` `workerId`. |
| `role` | Required in `agents` array | `orchestrator` | Must be `orchestrator` or `worker`. |
| `defaultProvider` | Orchestrator: `ollama`. Worker: same as orchestrator | `ollama` | Unknown id → `ollama`. |
| `defaultModel` | Orchestrator: provider fallback. Worker: worker string, else orchestrator string, then fallback | `llama3.2:3b` (built-in `defaultProvider` is `ollama`) | Fallbacks: `ollama` → `llama3.2:3b`; `lms` → `llama-3.2-3B-instruct`; `vllm` → `Qwen/Qwen2.5-7B-Instruct`; `nim` → `meta/llama-3.2-3b-instruct`; `openai` → `gpt-4o-mini`; `hf` → `meta-llama/Llama-3.1-8B-Instruct`. |
| `enabledProviders` | Orchestrator: only `defaultProvider` polled. Worker: see Note | same | Orchestrator: `null` or `[]` → poll that provider only; non-empty → only those. Worker: `null` → no extra `delegate_task` restriction; `[]` → only default provider; non-empty → only listed providers. |
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
| `VLLM_API_KEY` | `providers.vllm.apiKey` | Bearer token for vLLM. |
| `HF_API_KEY` | `providers.hf.apiKey` | Bearer token for Hugging Face endpoints. |
| `NVIDIA_API_KEY` | `providers.nim.apiKey` | API key for NVIDIA NIM. |
| `OPENAI_API_KEY` | `providers.openai.apiKey` | API key for OpenAI or compatible endpoint. |
