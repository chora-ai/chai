
## Configuration

The command-line interface and desktop application use the same configuration.

### Initialization

After installing, run **`chai init`** to create **`~/.chai/`**: default profiles **`assistant`** and **`developer`**, a symlink **`active`** → **`profiles/assistant/`**, and a shared **`skills/`** tree.

### Configuration File (`config.json`)

Each **profile** has its own **`config.json`** at **`~/.chai/profiles/<name>/config.json`**. The active profile is **`~/.chai/active`** (symlink). Override for one process with **`CHAI_PROFILE`** or **`chai gateway --profile <name>`**. Use **`chai profile list`**, **`chai profile current`**, and **`chai profile switch <name>`** (gateway must be stopped) to inspect or change the persistent active profile. An empty **`config.json`** is created per profile at initialization.

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
      "defaultModel": "llama3.2:3b",
      "contextMode": "full"
    }
  ]
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
      "deviceId": null,
      "roomIds": null
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
      "apiKey": null,
      "extraModels": null
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
  ]
}
```

### Gateway

**`gateway`** — HTTP/WebSocket listen address and auth.

| Field | Default | Override | Note |
|-------|---------|----------|------|
| `gateway.port` | `15151` | - | - |
| `gateway.bind` | `127.0.0.1` | - | - |
| `gateway.auth.mode` | `none` | - | `none` or `token` |
| `gateway.auth.token` | - | `CHAI_GATEWAY_TOKEN` | Only used if `mode` is `token` |

### Channels

**`channels`** — Channel integrations (Telegram, Signal, Matrix).

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
| `channels.matrix.userId` | - | `MATRIX_USER_ID` | With token auth, for echo filtering. |
| `channels.matrix.deviceId` | - | `MATRIX_DEVICE_ID` | Token restore when whoami omits device id. |
| `channels.matrix.roomIds` | - | `MATRIX_ROOM_ALLOWLIST` | Non-empty config list limits turns to those rooms; env (comma-separated) replaces the list when set and non-empty. |

### Providers

**`providers`** — Per-backend URLs and API keys.

| Field | Default | Override | Note |
|-------|---------|----------|------|
| `providers.ollama.baseUrl` | `http://127.0.0.1:11434` | - | Ollama client default when unset. |
| `providers.lms.baseUrl` | `http://127.0.0.1:1234/v1` | - | OpenAI-compatible LM Studio API. |
| `providers.vllm.baseUrl` | `http://127.0.0.1:8000/v1` | - | Include **`/v1`**. |
| `providers.vllm.apiKey` | - | `VLLM_API_KEY` | When server uses **`--api-key`**. |
| `providers.hf.baseUrl` | `http://127.0.0.1:8080/v1` | - | Set a real Inference Endpoint or TGI URL with **`/v1`**. |
| `providers.hf.apiKey` | - | `HF_API_KEY` | - |
| `providers.nim.apiKey` | - | `NVIDIA_API_KEY` | Base URL is fixed (**`https://integrate.api.nvidia.com/v1`**). |
| `providers.nim.extraModels` | - | - | NIM model id array; merged into gateway **`nimModels`** / desktop **`status`**. |
| `providers.openai.baseUrl` | `https://api.openai.com/v1` | - | Override for Azure or other compatible endpoints. |
| `providers.openai.apiKey` | - | `OPENAI_API_KEY` | - |

### Agents

**`agents`** — JSON array: exactly one **`"role": "orchestrator"`** and any number of **`"role": "worker"`**. Omit the **`agents`** key (or use **`"agents": null`**) to use built-in defaults: a single orchestrator with **`id`** **`orchestrator`** and effective **`role`** **`orchestrator`**. Fields below are **camelCase** keys on each array object.

**Orchestrator-only** (ignored if present on a worker object): **`maxSessionMessages`**, **`maxDelegationsPerTurn`**, **`maxDelegationsPerSession`**, **`maxDelegationsPerProvider`**, **`delegateBlockedProviders`**, **`delegationInstructionRoutes`**.

The table uses two default columns: **Default (property omitted)** is the effective behavior when that JSON property is missing on an **`agents`** array entry (orchestrator vs worker called out where they differ). **When `agents` omitted** is the built-in config when the top-level **`agents`** key is absent or **`null`** (no worker entries). **same** means that case matches the orchestrator line in **Default (property omitted)** for that row (there is only an implicit orchestrator).

| Field | Default (when field omitted) | Default (when `agents` omitted) | Note |
|-------|---------------------------|------------------------|------|
| `id` | Required in **`agents`** array | **`orchestrator`** | Unique per entry. Worker **`id`** is **`delegate_task`** **`workerId`**. |
| `role` | Required in **`agents`** array | **`orchestrator`** | In the array, must be **`orchestrator`** or **`worker`** (no serde default). With no **`agents`** key, the implicit single agent is the orchestrator. |
| `defaultProvider` | Orchestrator: **`ollama`**. Worker: same effective provider as orchestrator | **`ollama`** | Unknown id → **`ollama`**. Drives orchestrator turns and discovery when **`enabledProviders`** is unset or empty. |
| `defaultModel` | Orchestrator: provider fallback (see below). Worker: worker string, else orchestrator string, then fallback for worker’s provider | **`llama3.2:3b`** (built-in **`defaultProvider`** is **`ollama`**) | Fallback when still unset: **`ollama`** → **`llama3.2:3b`**; **`lms`** → **`llama-3.2-3B-instruct`**; **`vllm`** → **`Qwen/Qwen2.5-7B-Instruct`**; **`nim`** → **`meta/llama-3.2-3b-instruct`**; **`openai`** → **`gpt-4o-mini`**; **`hf`** → **`meta-llama/Llama-3.1-8B-Instruct`**. |
| `enabledProviders` | Orchestrator: only effective **`defaultProvider`** polled. Worker: see Note | **same** | Orchestrator: **`null`** or **`[]`** → poll that provider only; non-empty → only those. Worker: **`null`** → no extra **`delegate_task`** restriction (still subject to orchestrator discovery). **`[]`** → **`delegate_task`** only to that worker’s effective default provider. Non-empty → only listed providers for that worker. |
| `skillsEnabled` | No skill packages | **same** | Omitted or **`[]`**: nothing loaded from **`~/.chai/skills`**. |
| `contextMode` | **`full`** | **same** | **`full`** or **`readOnDemand`**. |
| `maxSessionMessages` | All messages (no trim) | **same** | Orchestrator only. When set and **`> 0`**, only the last N session messages are sent to the provider; full history stays in the session store. |
| `maxDelegationsPerTurn` | No dedicated cap | **same** | Orchestrator only. Tool loop iteration limit still applies. When set, excess **`delegate_task`** calls error in that turn. |
| `maxDelegationsPerSession` | No limit | **same** | Orchestrator only. |
| `maxDelegationsPerProvider` | No per-provider cap | **same** | Orchestrator only. Non-empty: keys are canonical provider ids; values are max successful delegations per session to that provider. |
| `delegateAllowedModels` | Only effective default **`(provider, model)`** for that scope | **same** | Missing, **`null`**, or **`[]`**: **`delegate_task`** must match orchestrator **`resolve_effective_provider_and_model`** or worker **`effective_worker_defaults`**. Non-empty: only listed **`{ provider, model, local?, toolCapable? }`**; a non-empty worker list overrides the orchestrator list for that **`workerId`**. |
| `delegateBlockedProviders` | Nothing blocked | **same** | Orchestrator only. Non-empty: those canonical provider ids disallowed for **`delegate_task`**. |
| `delegationInstructionRoutes` | None | **same** | Orchestrator only. **`{ instructionPrefix, workerId?, provider?, model? }`**; first matching prefix fills missing **`delegate_task`** fields. |

### Configuration Directory (`~/.chai/`)

- **`profiles/<name>/`** — Per-profile **`config.json`**, **`agents/<agentId>/`** (**`AGENT.md`** per agent), **`paired.json`**, device identity, Matrix store (defaults), and other profile-local state.
- **`active`** — Symlink to **`profiles/<name>/`** (persistent active profile).
- **`skills/`** — Shared on-disk skills tree. After **`chai init`**, bundled skills are extracted here.
- **`gateway.lock`** — While a gateway runs, this file is held with an **advisory exclusive lock** and contains profile name + PID (for debugging). **`chai profile switch`** and the desktop profile control refuse while another process holds that lock.

### Environment variables

| Variable | Overrides | Description |
|----------|-----------|-------------|
| `CHAI_PROFILE` | Active profile | Profile name; overrides **`~/.chai/active`** for config resolution for that process. |
| `CHAI_GATEWAY_TOKEN` | `gateway.auth.token` | Shared secret for WebSocket connect when auth mode is `token`. |
| `TELEGRAM_BOT_TOKEN` | `channels.telegram.botToken` | Telegram bot token from BotFather. |
| `TELEGRAM_WEBHOOK_SECRET` | `channels.telegram.webhookSecret` | Optional webhook verification secret (header **`X-Telegram-Bot-Api-Secret-Token`**). |
| `SIGNAL_CLI_HTTP` | `channels.signal.httpBase` | signal-cli HTTP daemon base URL (`http://127.0.0.1:7583`). |
| `SIGNAL_CLI_ACCOUNT` | `channels.signal.account` | Optional `+E.164` for multi-account signal-cli JSON-RPC. |
| `MATRIX_HOMESERVER` | `channels.matrix.homeserver` | Matrix homeserver base URL (`https://…`). |
| `MATRIX_ACCESS_TOKEN` | `channels.matrix.accessToken` | Matrix client access token. |
| `MATRIX_USER_ID` | `channels.matrix.userId` | Matrix user id (`@user:server`) when using an access token without password login. |
| `MATRIX_USER` | `channels.matrix.user` | Localpart or full MXID for password login. |
| `MATRIX_PASSWORD` | `channels.matrix.password` | Password for **`m.login.password`**. |
| `MATRIX_DEVICE_ID` | `channels.matrix.deviceId` | Device id for access-token session restore when whoami omits it. |
| `MATRIX_ROOM_ALLOWLIST` | `channels.matrix.roomIds` | Comma-separated room ids; when set and non-empty, replaces the config allowlist. |
| `VLLM_API_KEY` | `providers.vllm.apiKey` | Bearer token for vLLM when the server was started with `--api-key`. |
| `HF_API_KEY` | `providers.hf.apiKey` | Bearer token for Hugging Face OpenAI-compatible endpoints when required. |
| `NVIDIA_API_KEY` | `providers.nim.apiKey` | API key for NVIDIA NIM hosted API at `https://integrate.api.nvidia.com`. When set, this is used for the NIM provider. |
| `OPENAI_API_KEY` | `providers.openai.apiKey` | API key for the OpenAI API (or compatible **`providers.openai.baseUrl`**). |
