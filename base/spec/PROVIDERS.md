---
status: stable
---

# Providers

Internal spec for **LLM backends** in Chai: provider array configuration, endpoint types, configurable behaviors, discovery, and compatibility targets. For **model** identifiers, repository inventory, and tool-fit notes, see [MODELS.md](MODELS.md).

## Relationship to Other Documents

- **[MODELS.md](MODELS.md)** — Model ids, repository inventory, deployment categories, and Chai tool compatibility.

## Provider Configuration

Providers are configured as a **JSON array** of provider objects, each with an `id`, `endpointType`, and optional configuration fields. This follows the same array pattern used for agents.

### Example Configuration

```json
{
  "providers": [
    { "id": "ollama", "endpointType": "ollama" },
    { "id": "nearai", "endpointType": "openai-compat", "baseUrl": "https://cloud-api.near.ai/v1", "apiKey": "<NEAR_API_KEY>" },
    { "id": "nim", "endpointType": "openai-compat", "baseUrl": "https://integrate.api.nvidia.com/v1", "apiKey": "<NVIDIA_API_KEY>", "modelDiscovery": "static", "staticModels": ["meta/llama-3.1-8b-instruct", "meta/llama-3.1-70b-instruct"] },
    { "id": "lms", "endpointType": "openai-compat", "modelDiscovery": "lmstudio" }
  ],
  "agents": [
    { "id": "orchestrator", "role": "orchestrator", "defaultProvider": "nearai", "defaultModel": "zai-org/GLM-5.1-FP8" },
    { "id": "worker-1", "role": "worker", "defaultProvider": "ollama", "defaultModel": "llama3.2:3b" }
  ]
}
```

### ProviderDefinition Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `id` | `String` | Yes | — | Unique provider id referenced by agents (`defaultProvider`, `enabledProviders`). |
| `endpointType` | `EndpointType` | Yes | — | Wire protocol / API family: `"ollama"` or `"openai-compat"`. |
| `baseUrl` | `String` | No | Per-endpoint type default | Base URL override. When unset, the endpoint type default is used. |
| `apiKey` | `String` | No | — | API key. A literal key string, an environment variable reference (`"<VAR_NAME>"`), or omitted. See [API Key Resolution](#api-key-resolution). |
| `defaultModel` | `String` | No | Per-endpoint type default | Default model id fallback for this provider. |
| `modelDiscovery` | `ModelDiscovery` | No | `"auto"` | How to discover available models: `"auto"`, `"lmstudio"`, or `"static"`. |
| `staticModels` | `String[]` | No | `[]` | Model list when `modelDiscovery: "static"`. No polling. |

### Key Concepts

**Provider `id` vs `endpointType`.** The `id` is just a name — it is what agents reference in `defaultProvider` and `enabledProviders`. The `endpointType` determines the wire protocol. This decouples provider identity from the API family. Two providers with different `id`s can use the same `endpointType`, differentiated by `baseUrl` and behavior fields.

**Multiple providers of the same endpoint type.** Two Ollama instances or two OpenAI-compat endpoint types can coexist in the array, each with its own `id`. No repurposing of provider names as generic carriers.

**Dynamic provider list.** Only providers in the array are instantiated at startup.

## Endpoint Types

An **endpoint type** describes the wire protocol — what HTTP routes to call and how to serialize/deserialize messages. This is distinct from a provider's identity (its `id`).

| Endpoint Type | Wire Protocol | Default Base URL | Default Model | Default Discovery |
|---------------|---------------|------------------|---------------|-------------------|
| `"ollama"` | Native Ollama: `POST /api/chat`, `GET /api/tags` | `http://127.0.0.1:11434` | `llama3.2:3b` | `GET /api/tags` |
| `"openai-compat"` | OpenAI: `POST /v1/chat/completions`, `GET /v1/models` | `http://127.0.0.1:1234/v1` | `llama-3.2-3B-instruct` | `GET /v1/models` |

Endpoint types are a **closed enum**, validated at config load time. Unknown endpoint type values produce a clear error. The set is small and grows slowly — new endpoint types require a code change (new `Provider` impl) but new providers of an existing endpoint type are config-only.

### Default Base URL for `openai-compat`

The default base URL for `openai-compat` is `http://127.0.0.1:1234/v1` (LM Studio's localhost address). Since LM Studio is the local alternative to Ollama, most `openai-compat` providers on localhost will be LM Studio. Remote providers (NVIDIA NIM, NearAI, etc.) set `baseUrl` explicitly. A bare `{ "id": "local", "endpointType": "openai-compat" }` connects to LM Studio on localhost.

### OpenAI-Compatible Is Not a Product

The `"openai-compat"` endpoint type covers any server speaking the OpenAI chat completions protocol. Products like LM Studio, NVIDIA NIM, and NearAI all use this protocol — they are configured as providers with `endpointType: "openai-compat"`, differentiated by `baseUrl`, `apiKey`, and behavior fields. Any other OpenAI-compatible server (vLLM, Hugging Face TGI, OpenAI itself, etc.) can also be configured as an `openai-compat` provider by setting `baseUrl` and `apiKey` appropriately.

## Configurable Behaviors

Product-specific behaviors (LM Studio model listing with automatic retry on unload, NVIDIA NIM static catalog) are not separate endpoint types — they are configurable options on the `openai-compat` endpoint type.

### Model Discovery

The `modelDiscovery` field controls how a provider's available model list is obtained.

| Value | Description | Applicable Endpoint Types |
|-------|-------------|---------------------------|
| `"auto"` | Use the endpoint type's standard discovery method (`GET /api/tags` for `ollama`, `GET /v1/models` for `openai-compat`). | All |
| `"lmstudio"` | LM Studio native model list: `GET /api/v1/models`, filter `type == "llm"`, use `key` as model id. | `openai-compat` |
| `"static"` | Use the model list from the `staticModels` config field. No polling. | All |

When omitted, `modelDiscovery` defaults to `"auto"`.

### Static Models

The `staticModels` field is an array of model id strings used when `modelDiscovery: "static"`.

```json
{
  "id": "nim",
  "endpointType": "openai-compat",
  "baseUrl": "https://integrate.api.nvidia.com/v1",
  "apiKey": "<NVIDIA_API_KEY>",
  "modelDiscovery": "static",
  "staticModels": [
    "meta/llama-3.1-8b-instruct",
    "meta/llama-3.1-70b-instruct",
    "deepseek-ai/deepseek-v3.1"
  ]
}
```

This is useful for any provider that lacks a `/v1/models` endpoint or where the user prefers to curate the list themselves (e.g. behind a firewall, or when only a subset of models is needed).

### LM Studio Retry on Unload

When `modelDiscovery: "lmstudio"` is configured, the gateway automatically retries chat requests that fail with an "unloaded" error. On such an error, the client calls `POST /api/v1/models/load` with the model id, then retries the chat request once. This behavior is always enabled for LM Studio providers — there is no separate configuration field.

```json
{
  "id": "lms",
  "endpointType": "openai-compat",
  "modelDiscovery": "lmstudio"
}
```

The streaming variant retries with a single non-streaming call (to avoid invoking `on_chunk` twice if partial data was already streamed).
### API Key Resolution

The `apiKey` field supports three forms:

| Form | Example | Behavior |
|------|---------|----------|
| Omitted / `null` | `"apiKey": null` | No API key is sent. Use for local providers (Ollama, LM Studio) or when the key is not required. |
| Literal string | `"apiKey": "sk-abc123"` | The value is sent as-is in the `Authorization: Bearer` header. |
| Environment variable reference | `"apiKey": "<NEAR_API_KEY>"` | The named environment variable is read at runtime. If set and non-empty, its value is used as the API key; if unset or empty, no key is sent. |

The `<VAR_NAME>` syntax keeps secrets out of `config.json`. Environment variables can come from the shell environment, or from a `.env` file in the profile directory.

#### `.env` File

If a `.env` file exists in the profile directory (e.g. `~/.chai/profiles/assistant/.env`), it is loaded at startup — before logger initialization — so that all supported environment variables take effect. Variables from `.env` are set in the process environment **only if they are not already set** — shell environment variables always take precedence.

```
# ~/.chai/profiles/assistant/.env
NEAR_API_KEY=sk-near-abc123
NVIDIA_API_KEY=nvapi-xyz789
```

With this `.env` file, the following config resolves both keys without hardcoding them:

```json
{
  "providers": [
    { "id": "nearai", "endpointType": "openai-compat", "baseUrl": "https://cloud-api.near.ai/v1", "apiKey": "<NEAR_API_KEY>" },
    { "id": "nim", "endpointType": "openai-compat", "baseUrl": "https://integrate.api.nvidia.com/v1", "apiKey": "<NVIDIA_API_KEY>", "modelDiscovery": "static", "staticModels": ["meta/llama-3.1-8b-instruct"] }
  ]
}
```

## Categories of Providers

Providers are grouped into three categories by **where** the model runs (or who hosts the API). The distinction matters for privacy, cost, and operations.

### 1. Local (Personal Device)

**Description:** Models run directly on personal hardware (laptop, desktop).

**Also called:** Self-hosted (but "local" here implies "self-hosted on your own machine").

**Examples:** Running Llama 3, Qwen, or DeepSeek using Ollama, LM Studio.

| Aspect | Details |
|--------|---------|
| **Key traits** | **Full data control and privacy**; constrained by device hardware (VRAM, CPU, cooling). |
| **Best for** | Development, experimentation, offline use. |

### 2. Self-Hosted (On-Premise or Private Cloud)

**Description:** Models run on your own infrastructure—physical servers, cloud VMs, or VPCs.

**Also called:** On-premise, private deployment; when on personal hardware, also called "local".

**Examples:** Running Llama 3, Qwen, or DeepSeek using Ollama, vLLM, or Hugging Face TGI.

| Aspect | Details |
|--------|---------|
| **Key traits** | **Full data control and privacy**; upfront cost (hardware) or ongoing (cloud instance); supports fine-tuning and customization; requires ML/DevOps expertise. |
| **Best for** | High-volume usage, regulated industries, data-sensitive applications. |

### 3. Third-Party (Cloud / API-Based)

**Description:** Models hosted and managed by external providers (e.g. NearAI, NVIDIA NIM, OpenAI).

**Also called:** LLM-as-a-Service (LLMaaS), cloud APIs, hosted APIs.

**Examples:** Using models via NearAI, NVIDIA NIM, or OpenAI APIs.

| Aspect | Details |
|--------|---------|
| **Key traits** | **Data leaves your environment**; Pay-per-use or subscription pricing; no infrastructure to manage; fast setup and access to cutting-edge models. |
| **Best for** | Rapid prototyping, low-to-moderate usage, teams without dedicated AI/ML ops. |

### Summary of Approaches

| Approach     | Where it runs      | Data privacy        | Setup effort | Cost model                |
|-------------|--------------------|---------------------|--------------|---------------------------|
| **Local** (subset) | Your personal device | High (data stays) | Medium       | One-time hardware cost    |
| **Self-hosted** | Your infrastructure | High (data stays) | High         | Hardware + maintenance    |
| **Third-party** | Provider's cloud   | Low (data leaves)   | Minimal      | Pay-per-token             |

### Hybrid Approaches Are Common

Many organizations combine both:

- **Self-hosted** providers for privacy, cost control, and customization when data must stay in your environment.
- **Third-party** providers for low-sensitivity, bursty workloads (e.g. experimentation, occasional heavy lifting).

Note: A **multi-agent management system** extends this idea: one agent or model acts as the **orchestrator**, delegating subtasks to other agents or models based on the task and each one's abilities. The orchestrator chooses which agent and which model are best suited to complete a given step—e.g. route sensitive data only to local or self-hosted models, and send less sensitive or capability-heavy work to a third-party API when appropriate. That way, combining local, self-hosted, and third-party providers can be used with multi-agent workflows so that the right model and the right agent handle each part of the job.

## Example Providers

Four providers demonstrate the key configuration patterns. Each shows a different combination of endpoint type and behavior fields.

### Ollama (`"ollama"` endpoint type)

The simplest configuration: native Ollama endpoint type, no `baseUrl` or `apiKey` needed.

```json
{ "id": "ollama", "endpointType": "ollama" }
```

- **EndpointType:** `"ollama"` — native Ollama REST API (`/api/chat`, `/api/tags`)
- **Default base URL:** `http://127.0.0.1:11434`
- **Model discovery:** `GET /api/tags` (default)
- **Auth:** None (local only)
- **Privacy:** Full — data stays on your machine

### NearAI (`"openai-compat"` endpoint type)

A remote OpenAI-compatible API. Needs `baseUrl` and `apiKey`.

```json
{ "id": "nearai", "endpointType": "openai-compat", "baseUrl": "https://cloud-api.near.ai/v1", "apiKey": "<NEAR_API_KEY>" }
```

- **Endpoint Type:** `"openai-compat"` — OpenAI chat completions protocol
- **Default base URL:** None — must set `baseUrl` explicitly
- **Model discovery:** `GET /v1/models` (default)
- **Auth:** API key via `apiKey` field (literal string or `<ENV_VAR>` reference)
- **Privacy:** Data leaves your environment — sent to NearAI servers

This pattern applies to any remote OpenAI-compatible API: OpenAI itself, Azure OpenAI, Together, Groq, etc. All are `"openai-compat"` providers with a specific `baseUrl` and `apiKey`.

### NVIDIA NIM (`"openai-compat"` + `staticModels`)

NIM lacks a `/v1/models` endpoint, so `modelDiscovery: "static"` with a user-curated `staticModels` list replaces discovery.

```json
{
  "id": "nim",
  "endpointType": "openai-compat",
  "baseUrl": "https://integrate.api.nvidia.com/v1",
  "apiKey": "<NVIDIA_API_KEY>",
  "modelDiscovery": "static",
  "staticModels": ["meta/llama-3.1-8b-instruct", "meta/llama-3.1-70b-instruct"]
}
```

- **Endpoint Type:** `"openai-compat"` — OpenAI chat completions protocol
- **Default base URL:** None — must set `baseUrl` explicitly
- **Model discovery:** `"static"` — no polling; models from `staticModels` config field only
- **Auth:** API key via `apiKey` field (literal string or `<NVIDIA_API_KEY>` reference)
- **Privacy:** Data leaves your environment — sent to NVIDIA servers. The gateway logs a warning at startup when a NIM provider is the default provider.
- **Rate limits:** Free tier allows approximately 40 requests per minute; expect 429 responses under heavier use.
- **Gotchas:** NIM does not expose a `/v1/models` endpoint, so `modelDiscovery: "static"` is required with a user-curated list in `staticModels`.

### LM Studio (`"openai-compat"` + `modelDiscovery: "lmstudio"`)

LM Studio uses `modelDiscovery: "lmstudio"` for its native model list. When this is set, the gateway also automatically retries chat requests that fail with an "unloaded" error by loading the model and retrying once.

```json
{ "id": "lms", "endpointType": "openai-compat", "modelDiscovery": "lmstudio" }
```

- **Endpoint Type:** `"openai-compat"` — OpenAI chat completions protocol
- **Default base URL:** `http://127.0.0.1:1234/v1` (`openai-compat` default — LM Studio's localhost address)
- **Model discovery:** `"lmstudio"` — uses LM Studio's native `GET /api/v1/models` endpoint (filters `type == "llm"`, uses `key` as model id)
- **Retry on unload:** Automatic — on "unloaded" error, calls `POST /api/v1/models/load` and retries once (always enabled with `modelDiscovery: "lmstudio"`)
- **Auth:** None (local only)
- **Privacy:** Full — data stays on your machine
- **Gotchas:**
  - LM Studio must be installed and running
  - Developer settings must be on with runtime set to CPU
  - Models must be manually loaded (e.g. `lms load <model path>`) or rely on automatic retry on unload
  - All models support the tools API but some models are not trained on tool use

### Applying the Patterns

Any other OpenAI-compatible server follows one of the three `openai-compat` patterns above:

| Pattern | When to Use | Fields |
|---------|-------------|--------|
| Simple remote API | Server has `/v1/models` and standard discovery works | `endpointType: "openai-compat"` + `baseUrl` + `apiKey` |
| Static model list | Server lacks `/v1/models` or you want to curate the list | Add `modelDiscovery: "static"` + `staticModels` |
| LM Studio | Local LM Studio instance | `modelDiscovery: "lmstudio"` |

For example, vLLM, Hugging Face TGI, and OpenAI itself are all "simple remote API" — just `endpointType: "openai-compat"` with the appropriate `baseUrl` and `apiKey`. No special behavior fields needed.

## API Comparison

Canonical comparison of what the gateway uses for the two endpoint types. For endpoint type details and shapes, see the per-backend references under [base/ref/](../ref/).

**Ollama: current usage vs full API vs hosted**

| Area | Current | Ollama Full API | Hosted (Cloud APIs) |
|------|---------|-----------------|---------------------------|
| **Base** | `OllamaClient`, default local URL | Same | Remote URL + API key |
| **Chat** | `/api/chat` with model, messages, stream, tools | + options, keep_alive, format, think, logprobs | Different URL/params, similar roles/messages/tools |
| **Streaming** | Implemented but not used to channel; NDJSON | Same | SSE or similar, different format |
| **Models** | `GET /api/tags` at startup; one default model from config | + pull, delete, copy, show | Model ID only, no local lifecycle |
| **Generate** | Not used | `/api/generate` (prompt-only) | Often "completions" vs "chat" |
| **Embed** | Not used | `/api/embed` | Separate embedding APIs |
| **State** | Client sends full history + system each time | N/A (stateless) | Same (stateless) |

**OpenAI-compat family:** Shared patterns in **`openai_compat`** module — `POST /v1/chat/completions`, `GET /v1/models` where supported. Provider-specific behaviors (LM Studio model discovery with automatic retry on unload, NIM static catalog) are expressed through configurable behavior fields, not separate code modules. See [OPENAI.md](../ref/OPENAI.md), [NVIDIA_NIM.md](../ref/NVIDIA_NIM.md), [LM_STUDIO.md](../ref/LM_STUDIO.md), [OLLAMA.md](../ref/OLLAMA.md).
