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
- A `skills.lock` file per newly seeded profile (pins bundled skill versions so `skills.lockMode: strict` takes effect immediately)

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

**Sandbox recovery** — If a profile directory already exists but its `sandbox/` subdirectory has been deleted, `chai init` re-creates the sandbox directory and seeds template files. Other files within the profile are not modified.
**Bundled skills** — Each bundled skill is extracted into `~/.chai/skills/<name>/` using content-addressed versioning:

| Component | Behavior |
|-----------|----------|
| `versions/<hash>/` snapshot | Created if absent. Immutable — never re-written once created. |
| `active` symlink | Set only when no active version exists (fresh installation). If the skill already has an `active` symlink pointing to a valid version, it is left unchanged — this preserves user customizations such as manual rollbacks or edits via `skills_write_skill_md`. The new bundled version snapshot is still written to disk, so the user can switch to it manually if desired. |

**Skills lock** — For each newly seeded profile, `chai init` writes a `skills.lock` file that pins all bundled skills at their current content hashes:

| Component | Behavior |
|-----------|----------|
| `profiles/<name>/skills.lock` | Generated when the profile is newly seeded. If the profile directory already exists, the existing lock file is left unchanged. |

**Profile `active` symlink** — `~/.chai/active` is set to `profiles/assistant/` only when no valid `active` symlink already exists (fresh installation or broken symlink). If the symlink already points to a valid profile directory, it is left unchanged — this preserves the user's active profile choice across re-initialization.

### When to Re-Run

Re-running `chai init` is useful when:

- A new version of chai ships updated bundled skills — the new version snapshots will be created on disk (you can adopt them with `chai skill rollback` or by manually updating the `active` symlink)
- A `sandbox/` directory was accidentally deleted — the missing directory and template files will be re-created for existing profiles without affecting other profile files
- A profile directory was deleted — the entire profile will be re-seeded from scratch
- You want to ensure the default profile scaffold is complete

## Profiles

Each profile is an independent configuration tree under `~/.chai/profiles/<name>/`. The active profile is a symlink at `~/.chai/active`. You can:

- **List profiles** — `chai profile list`
- **Show current profile** — `chai profile current`
- **Switch profiles** — `chai profile switch <name>` (the gateway must be stopped)
- **Override per process** — Set `CHAI_PROFILE` or run `chai gateway --profile <name>`

The gateway refuses profile switches while it is running (it holds an advisory lock at `~/.chai/gateway.lock`).

### Creating a New Profile

Profiles created by `chai init` (`assistant`, `developer`) come with a `skills.lock` file that pins bundled skill versions. When you create a profile manually, you must also generate a lock file — in `strict` mode (the default), the gateway refuses to start without one.

1. Create the profile directory and a minimal config:
   ```bash
   mkdir -p ~/.chai/profiles/my-profile
   echo '{}' > ~/.chai/profiles/my-profile/config.json
   ```
2. Create the sandbox directory:
   ```bash
   mkdir -p ~/.chai/profiles/my-profile/sandbox
   ```
3. Switch to the new profile and generate the skills lock:
   ```bash
   chai profile switch my-profile
   chai skill lock
   ```

The `chai skill lock` command records the current active hash for each discovered skill into `~/.chai/profiles/my-profile/skills.lock`. After this, `skills.lockMode: strict` will enforce that those skill versions are used on gateway startup. See [Skills → Lockfiles and Rollback](06-skills.md#lockfiles-and-rollback) for the full lock behavior.

## Configuration File

Each profile has a `config.json` at `~/.chai/profiles/<name>/config.json`. An empty file is valid — built-in defaults are used at runtime:

```json
{}
```

With no `agents` key, the gateway runs a single orchestrator using Ollama and `llama3.2:3b`. Everything else has sensible defaults too: the gateway binds to `127.0.0.1:15151` with no auth, no channels are configured, and no skills are enabled.

## Configuring Providers

Providers are defined as a **JSON array** in the `providers` key. Each provider has a unique `id` (referenced by agents) and an `endpointType` type that determines the wire protocol. Additional fields like `baseUrl`, `apiKey`, and behavior settings are optional.

### Endpoint Types

An **endpoint type** describes the wire protocol — what HTTP routes to call and how to serialize/deserialize messages. The `id` is just a name; the `endpointType` determines the protocol.

| Endpoint Type | Description | Default Base URL | Default Model |
|---------------|-------------|------------------|---------------|
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
    { "id": "lms", "endpointType": "openai-compat", "modelDiscovery": "lmstudio" }
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

LM Studio uses `modelDiscovery: "lmstudio"` to list models via its native `GET /api/v1/models` endpoint (instead of `GET /v1/models`). When this is set, the gateway also automatically retries chat requests that fail with an "unloaded" error by loading the model and retrying once.

**NearAI (remote OpenAI-compatible API):**

```json
{
  "providers": [
    { "id": "nearai", "endpointType": "openai-compat", "baseUrl": "https://cloud-api.near.ai/v1" }
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
      "endpointType": "openai-compat",
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
    { "id": "ollama", "endpointType": "ollama" },
    { "id": "lms", "endpointType": "openai-compat", "modelDiscovery": "lmstudio" },
    { "id": "nearai", "endpointType": "openai-compat", "baseUrl": "https://cloud-api.near.ai/v1" }
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

Any server that exposes OpenAI-shaped routes (`/v1/chat/completions`, optionally `/v1/models`) can be configured as an `"openai-compat"` provider with the appropriate `baseUrl` and `apiKey`. This includes vLLM, Hugging Face TGI, OpenAI, Azure OpenAI, and any other OpenAI-compatible proxy — no special behavior fields are needed, just `endpointType: "openai-compat"` and the correct base URL.

### Behavior Fields

#### Model Discovery

The `modelDiscovery` field controls how a provider's available model list is obtained.

| Value | Description | Default For |
|-------|-------------|-------------|
| `"auto"` | Use the endpoint type's standard discovery method. | All endpoint types |
| `"lmstudio"` | LM Studio native: `GET /api/v1/models`, filter `type == "llm"`. | — |
| `"static"` | Use the `staticModels` config field. No polling. | — |

When omitted, `modelDiscovery` defaults to `"auto"`, which uses `GET /api/tags` for `"ollama"` and `GET /v1/models` for `"openai-compat"`.

#### Static Models

The `staticModels` field is an array of model id strings used when `modelDiscovery: "static"`. This is useful for providers that lack a model list endpoint or when you want to curate the list yourself.

### Model Id Reference

Use the exact model id expected by the selected provider for `defaultModel`:

| Provider `id` (example) | Endpoint Type | Model id example | Where to find it |
|-------------------------|---------------|------------------|------------------|
| `ollama` | `"ollama"` | `llama3.2:3b`, `qwen3:8b` | `ollama list` |
| `lms` | `"openai-compat"` + `modelDiscovery: "lmstudio"` | `llama-3.2-3B-instruct` | LM Studio UI or `GET …/api/v1/models` |
| `nearai` | `"openai-compat"` | `zai-org/GLM-5.1-FP8` | [NearAI model catalog](https://near.ai) |
| `nim` | `"openai-compat"` + `modelDiscovery: "static"` | `meta/llama-3.1-8b-instruct` | [NVIDIA LLM APIs](https://docs.api.nvidia.com/nim/reference/llm-apis) |

For other OpenAI-compatible servers, use the model id that the server expects (e.g. the same id you pass to `vllm serve`, your endpoint's Hugging Face model id, etc.).

For systematic provider and model testing, see the [Testing Playbooks](../testing/README.md).

## Configuring Channels

Channels connect the gateway to messaging platforms. Telegram is included by default. Matrix and Signal are optional channels that require Cargo feature flags at build time:

| Channel | Feature flag | Status |
|---------|-------------|--------|
| Telegram | (always on) | Supported |
| Matrix | `--features matrix` | Experimental (opt-in) |
| Signal | `--features signal` | Experimental (opt-in) |

Add a `channels` block to enable one or more.

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

**Matrix** (experimental; requires `--features matrix` at build time):

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

**Signal** (experimental; requires `--features signal` at build time):

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

For secrets management and rotation, see [Connections → Secrets and Rotation](04-connections.md#secrets-and-rotation).

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
      "enabledSkills": ["files", "git-read"],
      "contextMode": "full"
    }
  ]
}
```

**Orchestrator with a worker:**

```json
{
  "providers": [
    { "id": "ollama", "endpointType": "ollama" },
    { "id": "lms", "endpointType": "openai-compat", "modelDiscovery": "lmstudio" }
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
- `enabledSkills` — Which skill packages to load for this agent. Omitted or empty means no skills.
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
| `profiles/<name>/skills.lock` | Pinned skill hashes for lock verification (see [Skills → Lockfiles](06-skills.md#lockfiles-and-rollback)) |
| `profiles/<name>/paired.json` | Desktop pairing state |
| `active` | Symlink to the active profile |
| `skills/` | Shared on-disk skills tree |
| `gateway.lock` | Advisory lock while gateway runs (profile + PID) |
| `desktop.json` | Desktop appearance and log settings (see [Desktop Settings](09-desktop.md)) |

---

## Configuration Reference

Complete field-level reference for `config.json`. All keys are `camelCase`.

For desktop-specific settings (theme, font size, log buffer size), see [Desktop Settings](09-desktop.md). These live in `~/.chai/desktop.json`, not `config.json`.

### Gateway

| Field | Default | Override | Note |
|-------|---------|----------|------|
| `gateway.port` | `15151` | - | - |
| `gateway.bind` | `127.0.0.1` | - | - |
| `gateway.auth.mode` | `none` | - | `none` or `token` |
| `gateway.auth.token` | - | `CHAI_GATEWAY_TOKEN` | Only used if `mode` is `token` |
| `sandbox.mode` | `strict` | - | `strict`, `current`, or `unsafe`. Controls sandbox enforcement at startup. `"strict"` (default): gateway refuses to start if the sandbox directory is missing. `"current"`: uses the current working directory as the sole writable root when the sandbox directory is missing (path validation remains active). `"unsafe"`: gateway starts without a sandbox; CWD confinement and path validation are disabled. When the sandbox directory exists, both `"strict"` and `"current"` behave identically. See [Write Sandbox](07-sandbox.md). |
| `skills.lockMode` | `strict` | - | `strict` or `warn`. Controls lockfile verification at startup. `strict` (default): the lockfile acts as a complete manifest — gateway refuses to start when the lockfile is missing, any enabled skill has no lock entry (unpinned), or any pinned skill's active version does not match its locked hash. `warn`: logs warnings on mismatches, allows unpinned skills, and skips verification when no lockfile is present. See [Skills → Skill Lock Mode](06-skills.md#skill-lock-mode). |

### Channels

| Field | Default | Override | Note |
|-------|---------|----------|------|
| `channels.telegram.botToken` | - | `TELEGRAM_BOT_TOKEN` | Required for Telegram |
| `channels.telegram.webhookUrl` | - | - | Long-poll used if not set |
| `channels.telegram.webhookSecret` | - | `TELEGRAM_WEBHOOK_SECRET` | Only used if `webhookUrl` is set |
| `channels.signal.httpBase` | - | `SIGNAL_CLI_HTTP` | Required for Signal (experimental; `--features signal`) |
| `channels.signal.account` | - | `SIGNAL_CLI_ACCOUNT` | Multi-account daemon: `+E.164` |
| `channels.matrix.homeserver` | - | `MATRIX_HOMESERVER` | Required for Matrix (experimental; `--features matrix`) |
| `channels.matrix.accessToken` | - | `MATRIX_ACCESS_TOKEN` | Or `user` + `password` |
| `channels.matrix.user` | - | `MATRIX_USER` | Password login localpart or MXID |
| `channels.matrix.password` | - | `MATRIX_PASSWORD` | For `m.login.password` |
| `channels.matrix.userId` | - | `MATRIX_USER_ID` | With token auth, for echo filtering |
| `channels.matrix.deviceId` | - | `MATRIX_DEVICE_ID` | Token restore when whoami omits device id |
| `channels.matrix.roomIds` | - | `MATRIX_ROOM_ALLOWLIST` | Non-empty config list limits turns to those rooms; env (comma-separated) replaces the list when set and non-empty |

### Providers

The `providers` array contains provider definitions. Each provider has a unique `id` (referenced by agents) and an `endpointType`.

| Field | Type | Required | Default | Note |
|-------|------|----------|---------|------|
| `id` | `string` | Yes | — | Unique provider id referenced by agents. |
| `endpointType` | `string` | Yes | — | One of: `"ollama"`, `"openai-compat"`. |
| `baseUrl` | `string` | No | Per-endpoint type default | Override the endpoint type's default base URL. |
| `apiKey` | `string` | No | — | API key. Supports the `<VAR_NAME>` syntax to read from an environment variable (resolved at runtime from the shell environment or a `.env` file in the profile directory). When absent, no key is sent. |
| `defaultModel` | `string` | No | Per-endpoint type default | Default model id fallback for this provider when the agent's `defaultModel` is unset. |
| `modelDiscovery` | `string` | No | `"auto"` | One of: `"auto"`, `"lmstudio"`, `"static"`. When `"lmstudio"`, the gateway automatically retries chat requests on "unloaded" errors. |
| `staticModels` | `string[]` | No | `[]` | Model list when `modelDiscovery: "static"`. |

**Endpoint type defaults:**

| Endpoint Type | Default `baseUrl` | Default `defaultModel` |
|---------------|-------------------|------------------------|
| `"ollama"` | `http://127.0.0.1:11434` | `llama3.2:3b` |
| `"openai-compat"` | `http://127.0.0.1:1234/v1` | `llama-3.2-3B-instruct` |

**Common provider configurations:**

| Provider `id` | `endpointType` | `baseUrl` | `modelDiscovery` | Notes |
|---------------|-----------|-----------|-------------------|-------|
| `ollama` | `"ollama"` | (default) | (default) | Default localhost Ollama |
| `lms` | `"openai-compat"` | (default) | `"lmstudio"` | LM Studio with automatic retry on unload |
| `nearai` | `"openai-compat"` | `https://cloud-api.near.ai/v1` | (default) | Set `apiKey` |
| `nim` | `"openai-compat"` | `https://integrate.api.nvidia.com/v1` | `"static"` | Set `staticModels` with your model list |
### Agents

The `agents` array contains exactly one `"role": "orchestrator"` and any number of `"role": "worker"` entries. Omit the `agents` key (or set `"agents": null`) for built-in defaults: a single orchestrator with id `orchestrator`.

**Orchestrator-only fields** (rejected on worker entries at parse time): `maxToolLoopsPerTurn`, `maxDelegationsPerTurn`, `maxDelegationsPerSession`, `maxDelegationsPerWorker`.

| Field | Default (When Field Omitted) | Default (When `agents` Omitted) | Note |
|-------|------------------------------|----------------------------------|------|
| `id` | Required in `agents` array | `orchestrator` | Unique per entry. Worker `id` is `delegate_task` `workerId`. |
| `role` | Required in `agents` array | `orchestrator` | Must be `orchestrator` or `worker`. |
| `defaultProvider` | Orchestrator: `ollama`. Worker: same as orchestrator | `ollama` | Must match a provider `id` in the `providers` array. Fallback: `ollama`. |
| `defaultModel` | Orchestrator: provider fallback. Worker: worker string, else orchestrator string, then fallback | `llama3.2:3b` (built-in `defaultProvider` is `ollama`) | Fallbacks come from the endpoint type's `defaultModel`: `"ollama"` → `llama3.2:3b`; `"openai-compat"` → `llama-3.2-3B-instruct`. A provider's `defaultModel` field overrides the endpoint-type default. |
| `enabledProviders` | Only `defaultProvider` polled | same | Orchestrator-only. Provider ids must match entries in the `providers` array. `null` or `[]` → poll `defaultProvider` only; non-empty → only those. Workers do not have `enabledProviders`; a worker's provider must already be enabled at the orchestrator level. |
| `enabledSkills` | No skill packages | same | Omitted or `[]`: nothing loaded from `~/.chai/skills`. |
| `contextMode` | `full` | same | `full` or `readOnDemand`. |
| `maxToolLoopsPerTurn` | No limit | same | Orchestrator-only configuration field that acts as a global cap for both orchestrator and worker tool loops. Maximum tool loops per turn. The loop exits naturally when the model returns no tool calls; this is a safety net against runaway loops. Workers cannot override this with their own value. When the limit is reached during an orchestrator turn, the turn is interrupted: the last tool call is not executed, a `session.tool_loop_limit` event is emitted (with the pending tool calls), and the desktop displays a banner explaining what happened. The user must send another message to continue. |
| `maxDelegationsPerTurn` | No dedicated cap | same | Orchestrator only. Excess `delegate_task` calls error in that turn. |
| `maxDelegationsPerSession` | No limit | same | Orchestrator only. |
| `maxDelegationsPerWorker` | No per-worker cap | same | Orchestrator only. Keys are worker ids; values are max successful delegations per session. |
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
