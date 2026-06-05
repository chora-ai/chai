---
status: stable
---

# Providers

Internal spec for **LLM backends** in Chai: provider array configuration, endpoint types, configurable behaviors, discovery, and compatibility targets. For **model** identifiers, repository inventory, and tool-fit notes, see [MODELS.md](MODELS.md).

## Relationship to Other Documents

- **[API_ALIGNMENT.md](../epic/API_ALIGNMENT.md)** — Proposal and tracking for API alignment, message/tool mapping, and [Phase 2 (Anthropic/Google)](../epic/API_ALIGNMENT.md#phase-2-anthropic-and-google). This spec lists **which** backends exist and **how** to configure them; the epic defines **what "done" means** across backends.
- **[MODELS.md](MODELS.md)** — Model ids, repository inventory, deployment categories, and Chai tool compatibility.

## Provider Configuration

Providers are configured as a **JSON array** of provider objects, each with an `id`, `endpoint` type, and optional configuration fields. This follows the same array pattern used for agents.

### Example Configuration

```json
{
  "providers": [
    { "id": "ollama", "endpoint": "ollama" },
    { "id": "lms", "endpoint": "openai-compat", "modelDiscovery": "lmstudio", "autoLoad": "lmstudio" },
    { "id": "vllm", "endpoint": "openai-compat", "baseUrl": "http://127.0.0.1:8000/v1" },
    { "id": "nim", "endpoint": "openai-compat", "baseUrl": "https://integrate.api.nvidia.com/v1", "apiKey": null, "modelDiscovery": "static", "staticModels": ["meta/llama-3.1-8b-instruct", "meta/llama-3.1-70b-instruct"] },
    { "id": "openai", "endpoint": "openai-compat", "baseUrl": "https://api.openai.com/v1", "apiKey": null },
    { "id": "hf", "endpoint": "openai-compat", "baseUrl": "http://127.0.0.1:8080/v1", "apiKey": null },
    { "id": "anthropic", "endpoint": "anthropic", "apiKey": null },
    { "id": "gemini", "endpoint": "google", "apiKey": null },
    { "id": "my-custom", "endpoint": "openai-compat", "baseUrl": "http://my-server:8080/v1" }
  ],
  "agents": [
    { "id": "main", "role": "orchestrator", "defaultProvider": "ollama", "defaultModel": "llama3.2:3b" },
    { "id": "fast", "role": "worker", "defaultProvider": "lms", "defaultModel": "llama-3.2-3B-instruct" }
  ]
}
```

### ProviderDefinition Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `id` | `String` | Yes | — | Unique provider id referenced by agents (`defaultProvider`, `enabledProviders`). |
| `endpoint` | `EndpointType` | Yes | — | Wire protocol / API family: `"ollama"`, `"openai-compat"`, `"anthropic"`, or `"google"`. |
| `baseUrl` | `String` | No | Per-endpoint default | Base URL override. When unset, the endpoint type default is used. |
| `apiKey` | `String` | No | Per-endpoint env var | API key override. When unset, the endpoint type's canonical environment variable is checked. |
| `defaultModel` | `String` | No | Per-endpoint default | Default model id fallback for this provider. |
| `modelDiscovery` | `ModelDiscovery` | No | `"default"` | How to discover available models: `"default"`, `"lmstudio"`, or `"static"`. |
| `staticModels` | `String[]` | No | `[]` | Model list when `modelDiscovery: "static"`. No polling. |
| `autoLoad` | `AutoLoad` | No | `false` | Auto-load behavior on "unloaded" error: `false` or `"lmstudio"`. |

### Key Concepts

**Provider `id` vs `endpoint` type.** The `id` is just a name — it is what agents reference in `defaultProvider` and `enabledProviders`. The `endpoint` determines the wire protocol. This decouples provider identity from the API family. For example, `"vllm"` is no longer both "I am vLLM" and "I speak OpenAI-compat" — instead, `"vllm"` is the user-chosen `id`, and `"openai-compat"` is the endpoint type that specifies the protocol.

**Multiple providers of the same endpoint type.** Two Ollama instances or two OpenAI-compat endpoints can coexist in the array, each with its own `id`. No repurposing of provider names as generic carriers.

**Dynamic provider list.** Only providers in the array are instantiated at startup.

## Endpoint Types

An **endpoint type** describes the wire protocol — what HTTP routes to call and how to serialize/deserialize messages. This is distinct from a provider's identity (its `id`).

| Endpoint | Wire Protocol | Default Base URL | Default Model | Default Discovery | Env Var |
|----------|--------------|------------------|---------------|-------------------|---------|
| `"ollama"` | Native Ollama: `POST /api/chat`, `GET /api/tags` | `http://127.0.0.1:11434` | `llama3.2:3b` | `GET /api/tags` | — |
| `"openai-compat"` | OpenAI: `POST /v1/chat/completions`, `GET /v1/models` | `http://127.0.0.1:1234/v1` | `gpt-4o-mini` | `GET /v1/models` | — |
| `"anthropic"` | Anthropic: `POST /v1/messages` | `https://api.anthropic.com` | `claude-sonnet-4-20250514` | Static catalog or documented | `ANTHROPIC_API_KEY` |
| `"google"` | Gemini: `generateContent` / `contents` + `tools` | `https://generativelanguage.googleapis.com` | `gemini-2.5-flash` | Static catalog or listed | `GOOGLE_API_KEY` |

Endpoint types are a **closed enum**, validated at config load time. Unknown endpoint values produce a clear error. The set is small and grows slowly — new endpoint types require a code change (new `Provider` impl) but new providers of an existing endpoint type are config-only.

### Default Base URL for `openai-compat`

The default base URL for `openai-compat` is `http://127.0.0.1:1234/v1` (LM Studio's localhost address). Since LM Studio is the local alternative to Ollama, most `openai-compat` providers on localhost will be LM Studio. Remote providers (OpenAI, Groq, Together, etc.) already set `baseUrl` explicitly. A bare `{ "id": "local", "endpoint": "openai-compat" }` connects to LM Studio on localhost.

### OpenAI-Compatible Is Not a Product

The `"openai-compat"` endpoint type covers any server speaking the OpenAI chat completions protocol. Products like LM Studio, vLLM, NVIDIA NIM, Hugging Face TGI, and OpenAI itself all use this protocol — they are configured as providers with `endpoint: "openai-compat"`, differentiated by `baseUrl`, `apiKey`, and behavior fields.

## Configurable Behaviors

Product-specific behaviors (LM Studio auto-load, LM Studio model listing, NVIDIA NIM static catalog) are not separate endpoint types — they are configurable options on the `openai-compat` endpoint type.

### Model Discovery

The `modelDiscovery` field controls how a provider's available model list is obtained.

| Value | Description | Applicable Endpoints |
|-------|-------------|---------------------|
| `"default"` | Use the endpoint type's standard discovery method (`GET /api/tags` for `ollama`, `GET /v1/models` for `openai-compat`). | All |
| `"lmstudio"` | LM Studio native model list: `GET /api/v1/models`, filter `type == "llm"`, use `key` as model id. | `openai-compat` |
| `"static"` | Use the model list from the `staticModels` config field. No polling. | All |

When omitted, `modelDiscovery` defaults to `"default"`.

**`"default"` is the right name** because the actual discovery method varies by endpoint type — `GET /api/tags` for `ollama`, `GET /v1/models` for `openai-compat`. The name means "use the default for this endpoint."

**There is no `"none"` option.** If you don't want model discovery for a provider, omit that provider from an agent's `enabledProviders` (which already gates discovery). If you want a curated model list without polling, use `staticModels` with `modelDiscovery: "static"`.

### Static Models

The `staticModels` field is an array of model id strings used when `modelDiscovery: "static"`. This replaces the old NIM hardcoded catalog and `extraModels` field — users curate their own list.

```json
{
  "id": "nim",
  "endpoint": "openai-compat",
  "baseUrl": "https://integrate.api.nvidia.com/v1",
  "apiKey": null,
  "modelDiscovery": "static",
  "staticModels": [
    "meta/llama-3.1-8b-instruct",
    "meta/llama-3.1-70b-instruct",
    "deepseek-ai/deepseek-v3.1"
  ]
}
```

This is useful for any provider that lacks a `/v1/models` endpoint or where the user prefers to curate the list themselves (e.g. behind a firewall, or when only a subset of models is needed).

### Auto-Load

The `autoLoad` field controls whether a failed chat request triggers a model-load retry.

| Value | Description |
|-------|-------------|
| `false` | No auto-load; errors are returned as-is. (Default.) |
| `"lmstudio"` | On "unloaded" error, call `POST /api/v1/models/load` with the model id, then retry the chat request once. |

When omitted, `autoLoad` defaults to `false`.

```json
{
  "id": "lms",
  "endpoint": "openai-compat",
  "modelDiscovery": "lmstudio",
  "autoLoad": "lmstudio"
}
```

The streaming variant retries with a single non-streaming call (to avoid invoking `on_chunk` twice if partial data was already streamed).

## Categories of Providers

Providers are grouped into three categories by **where** the model runs (or who hosts the API). The distinction matters for privacy, cost, and operations.

### 1. Local (Personal Device)

**Description:** Models run directly on personal hardware (laptop, desktop).

**Also called:** Self-hosted (but "local" here implies "self-hosted on your own machine").

**Examples:** Running Llama 3, Qwen, or DeepSeek using Ollama, LM Studio, llama.cpp.

| Aspect | Details |
|--------|---------|
| **Key traits** | **Full data control and privacy**; constrained by device hardware (VRAM, CPU, cooling). |
| **Best for** | Development, experimentation, offline use. |

### 2. Self-Hosted (On-Premise or Private Cloud)

**Description:** Models run on your own infrastructure—physical servers, cloud VMs, or VPCs.

**Also called:** On-premise, private deployment; when on personal hardware, also called "local".

**Examples:** Running Llama 3, Qwen, or DeepSeek using Ollama, vLLM, LocalAI, or Hugging Face Inference Endpoints / TGI.

| Aspect | Details |
|--------|---------|
| **Key traits** | **Full data control and privacy**; upfront cost (hardware) or ongoing (cloud instance); supports fine-tuning and customization; requires ML/DevOps expertise. |
| **Best for** | High-volume usage, regulated industries, data-sensitive applications. |

### 3. Third-Party (Cloud / API-Based)

**Description:** Models hosted and managed by external providers (e.g. OpenAI, Anthropic, Google).

**Also called:** LLM-as-a-Service (LLMaaS), cloud APIs, hosted APIs.

**Examples:** Using GPT, Opus, or Gemini via OpenAI, Claude, or Google APIs.

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

## Status of Supported Providers

| Category | Current Implementation | Planned / Future |
|----------|------------------------|------------------|
| **Local** (personal device) | **Ollama** (`"ollama"` endpoint), **LM Studio** (`"openai-compat"` + `modelDiscovery: "lmstudio"`) | — |
| **Self-hosted** (on-prem / private cloud) | **vLLM** (`"openai-compat"` + custom `baseUrl`), **Hugging Face** (`"openai-compat"` + `baseUrl`), **LocalAI** (via `"ollama"` or `"openai-compat"` endpoints) | **llama.cpp** only via existing paths when OpenAI-compat (or Ollama-compatible) HTTP is enabled; no dedicated endpoint type |
| **Hosted APIs** (privacy varies) | **NVIDIA NIM** (`"openai-compat"` + `modelDiscovery: "static"` + `staticModels`), **OpenAI** (`"openai-compat"` + `baseUrl`) | — |
| **Third-party** | — | Claude (Anthropic `"anthropic"` endpoint), Gemini (Google `"google"` endpoint) |

**Configuration:** Providers are defined in the `providers` array. Each provider has a unique `id` and an `endpoint` type. Agents reference providers by `id` in `defaultProvider` and `enabledProviders`. See [Provider Configuration](#provider-configuration) for the full field reference.

**Discovery:** Model discovery is controlled per-provider by the `modelDiscovery` field. When an agent's `enabledProviders` is absent or empty, only the default provider is polled for models at startup. When set, only listed providers are polled. WebSocket **`status`** returns a `providers` object keyed by provider `id`, each with **`endpoint`**, **`discovery`**, and **`models`** (list of model names).

**Ollama-compatible backends:** If a server exposes the native Ollama API (`/api/chat`, `/api/tags`), use **`endpoint: "ollama"`** and set **`baseUrl`** if not on the default port. LocalAI in Ollama mode is an example.

**OpenAI-compatible backends:** If a server exposes OpenAI-shaped routes (`/v1/chat/completions`, and optionally `/v1/models`), use **`endpoint: "openai-compat"`** with appropriate `baseUrl`, `modelDiscovery`, `autoLoad`, and `staticModels` settings. Different products are just different configurations of the same endpoint type.

### Provider Configuration Examples

**Ollama (default localhost):**

```json
{ "id": "ollama", "endpoint": "ollama" }
```

**LM Studio with auto-load and LM Studio model discovery:**

```json
{ "id": "lms", "endpoint": "openai-compat", "modelDiscovery": "lmstudio", "autoLoad": "lmstudio" }
```

**vLLM with custom base URL:**

```json
{ "id": "vllm", "endpoint": "openai-compat", "baseUrl": "http://127.0.0.1:8000/v1" }
```

**NVIDIA NIM with static model list:**

```json
{ "id": "nim", "endpoint": "openai-compat", "baseUrl": "https://integrate.api.nvidia.com/v1", "modelDiscovery": "static", "staticModels": ["meta/llama-3.1-8b-instruct", "meta/llama-3.1-70b-instruct"] }
```

**OpenAI:**

```json
{ "id": "openai", "endpoint": "openai-compat", "baseUrl": "https://api.openai.com/v1" }
```

**Hugging Face (local TGI):**

```json
{ "id": "hf", "endpoint": "openai-compat", "baseUrl": "http://127.0.0.1:8080/v1" }
```

**Custom OpenAI-compat server:**

```json
{ "id": "my-server", "endpoint": "openai-compat", "baseUrl": "http://my-server:8080/v1" }
```

### Compatibility: LocalAI, llama.cpp, and Venice

None of these uses a dedicated provider `id` or endpoint type in Chai; they are **compatibility** stories (see [API_ALIGNMENT.md](../epic/API_ALIGNMENT.md) — **Compatibility Targets**).

| Product | How to Use in Chai |
|---------|-------------------|
| **LocalAI** (Ollama-compatible API) | `endpoint: "ollama"` + `baseUrl` pointing at LocalAI's Ollama-mode server. |
| **LocalAI** (OpenAI-compatible API) | `endpoint: "openai-compat"` + `baseUrl` pointing at LocalAI's `/v1` base. |
| **llama.cpp** (OpenAI-compatible server, e.g. `llama-server` with `/v1/...`) | `endpoint: "openai-compat"` + `baseUrl` pointing at llama.cpp's `/v1` origin. |
| **llama.cpp** (custom or legacy HTTP not matching Ollama or OpenAI chat) | Not supported until a dedicated adapter is added; treat as future epic work. |
| **Venice** (hosted OpenAI-compatible API) | `endpoint: "openai-compat"` + `baseUrl: "https://api.venice.ai/api/v1"` + Venice API key via `apiKey` field. See [OPENAI.md](../ref/OPENAI.md). |

## API Comparison

Canonical comparison of what the gateway uses vs what each API offers. For endpoint details and shapes, see the per-backend references under [base/ref/](../ref/).

**Ollama: current usage vs full API vs hosted**

| Area | Current | Ollama Full API | Hosted (OpenAI/Anthropic) |
|------|---------|-----------------|---------------------------|
| **Base** | `OllamaClient`, default local URL | Same | Remote URL + API key |
| **Chat** | `/api/chat` with model, messages, stream, tools | + options, keep_alive, format, think, logprobs | Different URL/params, similar roles/messages/tools |
| **Streaming** | Implemented but not used to channel; NDJSON | Same | SSE or similar, different format |
| **Models** | `GET /api/tags` at startup; one default model from config | + pull, delete, copy, show | Model ID only, no local lifecycle |
| **Generate** | Not used | `/api/generate` (prompt-only) | Often "completions" vs "chat" |
| **Embed** | Not used | `/api/embed` | Separate embedding APIs |
| **State** | Client sends full history + system each time | N/A (stateless) | Same (stateless) |

**OpenAI-compat family:** Shared patterns in **`openai_compat`** module — `POST /v1/chat/completions`, `GET /v1/models` where supported. See [VLLM.md](../ref/VLLM.md), [HUGGINGFACE.md](../ref/HUGGINGFACE.md), [NVIDIA_NIM.md](../ref/NVIDIA_NIM.md), [OPENAI.md](../ref/OPENAI.md). Provider-specific behaviors (LM Studio auto-load, LM Studio model discovery, NIM static catalog) are expressed through configurable behavior fields, not separate code modules.

## Providers at a Glance

| Provider | Example `id` | Endpoint | Hosting | Status |
|----------|-------------|----------|---------|--------|
| **Ollama** | `ollama` | `"ollama"` | Your machine | Supported |
| **LM Studio** | `lms` | `"openai-compat"` + `modelDiscovery: "lmstudio"`, `autoLoad: "lmstudio"` | Your machine | Supported |
| **vLLM** | `vllm` | `"openai-compat"` + custom `baseUrl` | Your infra | Supported |
| **LocalAI** | any | `"ollama"` or `"openai-compat"` + `baseUrl` | Your infra | Compatibility (see [Compatibility](#compatibility-localai-llamacpp-and-venice)) |
| **llama.cpp** | any | `"openai-compat"` + `baseUrl` | Your infra | Compatibility when OpenAI-compat |
| **Venice** | any | `"openai-compat"` + `baseUrl` | Venice | Compatibility (see [Compatibility](#compatibility-localai-llamacpp-and-venice)) |
| **Hugging Face** | `hf` | `"openai-compat"` + `baseUrl` | Your endpoint | Supported |
| **NVIDIA NIM** | `nim` | `"openai-compat"` + `modelDiscovery: "static"`, `staticModels` | NVIDIA | Supported |
| **OpenAI** | `openai` | `"openai-compat"` + `baseUrl: "https://api.openai.com/v1"` | OpenAI | Supported |
| **Claude** | `anthropic` | `"anthropic"` | Anthropic | Planned |
| **Gemini** | `gemini` | `"google"` | Google | Planned |
