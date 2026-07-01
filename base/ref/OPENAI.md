---
status: current
---

# OpenAI Reference

Reference for the **`"openai-compat"` endpoint type** in Chai: the wire protocol, request/response shapes, model discovery options, auto-load behavior, and how different products and hosting scenarios map onto this single endpoint type. Use this when adding features, debugging provider integrations, or onboarding a new OpenAI-compatible service.

## Purpose and How to Use

- **Purpose:** Document the `openai-compat` wire protocol (routes, shapes, auth), the configurable behaviors layered on top (model discovery, auto-load), and how products like LM Studio, NearAI, and NVIDIA NIM are configured as providers that all speak this same protocol.
- **How to use:** When adding features (e.g. streaming to the channel, new behavior fields), when configuring a new OpenAI-compatible provider, or when aligning with the OpenAI chat completions API, consult this doc.

## Endpoint Type vs. Hosting Service

The **endpoint type** describes the wire protocol — what HTTP routes to call and how to serialize/deserialize messages. It is **not** a hosting service, a product, or a deployment location. A provider's **hosting service** (or lack thereof) determines where the model runs: on your personal device (local), on your infrastructure (self-hosted), or on a third-party's cloud (hosted API).

The `"openai-compat"` endpoint type covers **any server** speaking the OpenAI chat completions protocol. The same protocol is used whether the model runs in LM Studio on your laptop, on a vLLM instance in your data center, or on a cloud API like NearAI or NVIDIA NIM. The endpoint type does not change — only the `baseUrl`, `apiKey`, and behavior fields change to match the product and deployment.

| Concept | What It Is | Examples |
|---------|-----------|----------|
| **Endpoint type** (`"openai-compat"`) | Wire protocol: `POST /v1/chat/completions`, `GET /v1/models`, Bearer auth, OpenAI-shaped request/response bodies | — |
| **Hosting service** | The product or platform that serves the API | LM Studio, NearAI, NVIDIA NIM, OpenAI, vLLM, Hugging Face TGI |
| **Deployment** | Where the model physically runs | Local (your device), self-hosted (your servers), third-party cloud (provider's infrastructure) |

A single hosting service may offer multiple deployment options (e.g. NVIDIA NIM has both a free hosted API and self-hosted NIM containers). The endpoint type stays `"openai-compat"` regardless of which deployment you choose — you change `baseUrl` and `apiKey` to match.

**Why this distinction matters:** Configuring a provider with `endpointType: "openai-compat"` does **not** mean you are using OpenAI's hosting service. It means the server speaks the OpenAI wire protocol. The `baseUrl` field determines which server (and therefore which hosting service and deployment) receives your requests. Privacy, cost, rate limits, and data handling all depend on the hosting service and deployment — not on the endpoint type.

## Official Documentation

The `"openai-compat"` endpoint type follows the OpenAI API specification. When a product-specific API differs (e.g. LM Studio's native model list), that is handled through configurable behavior fields, not a separate endpoint type.

- **Chat Completions:** https://platform.openai.com/docs/api-reference/chat
- **List Models:** https://platform.openai.com/docs/api-reference/models/list
- **API overview:** https://platform.openai.com/docs/api-reference

## Current Usage in the Codebase

### Client and Configuration

- **`crates/lib/src/providers/openai_compat.rs`** — Shared **`OpenAiCompatClient`** (HTTP, serde types, streaming). Used by **all** `"openai-compat"` providers regardless of hosting service or deployment. Product-specific behaviors (LM Studio model discovery, LM Studio auto-load) are implemented as methods on this client.
- There is **no separate module per hosting service** — NearAI, NVIDIA NIM, LM Studio, and any other OpenAI-compatible server all share the same `OpenAiCompatClient`. Differentiation comes from `baseUrl`, `apiKey`, and behavior fields in the provider configuration.
- **Config** — A provider is configured with `endpointType: "openai-compat"` plus optional fields. `agents.defaultProvider` references the provider `id`. `agents.defaultModel` is the model id as expected by the server (e.g. `llama-3.2-3B-instruct` for LM Studio, `z-ai/glm-5.2` for NearAI). See `resolve_model()` in the gateway and fallback in the agent when the configured value is empty.

### Endpoints Used

| Endpoint | Method | Use |
|----------|--------|-----|
| **`/v1/chat/completions`** | POST | Agent turn: `model`, `messages` (OpenAI format, including tool messages keyed by `tool_call_id`), optional `tools`, `stream`. Used by all `"openai-compat"` providers. |
| **`/v1/models`** | GET | Model discovery when `modelDiscovery: "auto"` (the standard OpenAI list models route). Returns `data[].id`. Not available on all servers — some require `modelDiscovery: "static"` or `"lmstudio"` instead. |
| **`/api/v1/models`** | GET | LM Studio native model list when `modelDiscovery: "lmstudio"`. Filters `type == "llm"`, uses `key` as model id. Outside the `/v1` path — the client strips the `/v1` suffix from `baseUrl` to reach the LM Studio root. |
| **`/api/v1/models/load`** | POST | Automatically called when `modelDiscovery: "lmstudio"` and chat returns an "unloaded" error; loads the model by id and retries the chat request once. Request body: `{ "model": "<id>" }`. |

### Request/Response Shapes (What We Send)

- **Chat request:** `model`, `messages` (array of `{ role, content }` or tool messages with `tool_call_id`), `stream`, `tools` (optional array of function definitions in OpenAI shape). We map our internal messages (which use `tool_name` for tool results, as in the Ollama format) to OpenAI format (tool results keyed by `tool_call_id` from the previous agent `tool_calls`). See the [Tool Message Mapping](#tool-message-mapping) section below for details.
- **Streaming:** When `on_chunk` is provided, we use `chat_stream()`: POST with `stream: true`, parse SSE `data:` lines, call `on_chunk` for each content delta, and accumulate `tool_calls` from stream chunks into a single `ChatResponse`. Non-streaming is used for the main agent path (no streaming to the channel in the current implementation).
- **Auth:** Bearer token via `Authorization: Bearer <key>`. The key comes from the provider `apiKey` field or the endpoint type's canonical environment variable (generic `openai-compat` has no default env var — keys are set per-provider).

### Tool Message Mapping

Chai's internal message format uses `tool_name` on tool-result messages (matching the Ollama API convention). The `openai-compat` client converts these to OpenAI format before sending:

| Internal (Chai) | OpenAI Wire |
|------------------|-------------|
| `tool_calls` on agent messages with `name` + `arguments` | `tool_calls` with synthetic `id` (e.g. `call_0`, `call_1`), `type: "function"`, `function.name`, `function.arguments` (stringified JSON) |
| Tool results with `role: "tool"` + `tool_name` | `role: "tool"` + `tool_call_id` (matched by order to the preceding agent's `tool_calls`) |

The mapping assigns sequential `tool_call_id` values (`call_0`, `call_1`, ...) to each tool call in an agent message. The next `role: "tool"` messages are matched to these ids in order. This allows the same agent and tool infrastructure to work across both endpoint types without changes.

### Where `openai-compat` Is Referenced

- **Gateway server** — Builds the `OpenAiCompatClient` for any provider with `endpointType: "openai-compat"`. When the provider `id` is referenced in `defaultProvider`, runs the agent turn via `run_turn_dyn` with that `Provider` and the resolved model id. Model lists are stored per provider id and exposed in WebSocket `status` at `payload.providers.<id>.models`.
- **Agent** — `run_turn_dyn` uses the `Provider` trait; the gateway passes the resolved client and model id. The agent does not know or care which hosting service or deployment is behind the client.
- **Tools** — The same skill `tools.json` and `ToolDefinition` list are converted to OpenAI tool format when calling any `openai-compat` provider.
- **Startup warnings** — The gateway logs warnings when the default provider points to a known non-local URL (e.g. `*.nvidia.com`, `*.openai.com`). These are hosting-service–specific heuristics, not endpoint-type logic.

## OpenAI Chat Completions API Overview

### Chat (`POST /v1/chat/completions`)

The primary route used for agent turns. All `"openai-compat"` providers share this route.

- **What we send:** `model`, `messages`, `stream`, `tools` (optional). We do **not** send: `temperature`, `top_p`, `max_tokens`, `stop`, `presence_penalty`, `frequency_penalty`, `n`, `logprobs`, `top_logprobs`, `response_format`, `seed`, `service_tier`.
- **What we receive:** `choices[0].message` (content and/or `tool_calls`), `choices[0].finish_reason` (`"stop"`, `"length"`, `"tool_calls"`, or a custom string), and optional `usage` (prompt/completion/total tokens).

**Fields we do not yet use:**

- **`temperature`** / **`top_p`** — Sampling controls. Could allow per-request or per-model tuning (e.g. lower temperature for tool-heavy flows).
- **`max_tokens`** — Cap on completion length. Would help prevent runaway generation and manage token budgets.
- **`stop`** — Custom stop sequences. Niche but useful for structured output.
- **`response_format`** — JSON mode or JSON schema for structured output. Could enforce stricter tool/output shapes.
- **`n`** — Multiple completions per request. Not needed for the agent loop (single-continuation model).
- **`logprobs` / `top_logprobs`** — Token-level log probabilities. Research/diagnostic use.
- **`seed`** — Deterministic sampling. Useful for reproducible runs during testing.
- **`presence_penalty` / `frequency_penalty`** — Repetition controls. Useful for long-form generation.
- **`service_tier`** — OpenAI-specific routing ("auto", "default"). Not applicable to most `openai-compat` providers.

### List Models (`GET /v1/models`)

- **What we use:** `modelDiscovery: "auto"` — polls `GET /v1/models` at gateway startup and returns `data[].id`. Result is stored in gateway state and exposed in WebSocket `status`.
- **Availability:** Not all OpenAI-compatible servers expose this route. NVIDIA NIM and some self-hosted servers do not — those require `modelDiscovery: "static"` with a user-curated list.

### Streaming

- **What we implement:** `chat_stream()` with SSE parsing. Accumulates content deltas and `tool_calls` into a single `ChatResponse`. Not yet used for streaming to the channel — the main agent path is non-streaming today.
- **SSE format:** `data: <json>\n\n` lines, terminated by `data: [DONE]`. Each chunk contains `choices[0].delta.content` (text) and/or `choices[0].delta.tool_calls` (incremental function call data indexed by position).

## Configurable Behaviors

Product-specific behaviors are **not** separate endpoint types — they are configurable options layered on the `"openai-compat"` protocol. This keeps the endpoint type enum small while allowing product differences to be expressed in configuration.

### Model Discovery

The `modelDiscovery` field controls how a provider's available model list is obtained.

| Value | Description | Route Used |
|-------|-------------|-----------|
| `"auto"` | Standard OpenAI model list. | `GET /v1/models` → `data[].id` |
| `"lmstudio"` | LM Studio native model list. | `GET /api/v1/models` → filter `type == "llm"`, use `key` as model id |
| `"static"` | User-curated list from `staticModels` config field. No polling. | — |

When omitted, `modelDiscovery` defaults to `"auto"`.

### Static Models

The `staticModels` field is an array of model id strings used when `modelDiscovery: "static"`. This works for any provider that lacks a `/v1/models` endpoint or where the user prefers to curate the list themselves (e.g. behind a firewall, or when only a subset of models is needed).

```json
{
  "id": "nvidia",
  "endpointType": "openai-compat",
  "baseUrl": "https://integrate.api.nvidia.com/v1",
  "modelDiscovery": "static",
  "staticModels": ["meta/llama-3.1-8b-instruct", "meta/llama-3.1-70b-instruct"]
}
```

### LM Studio Retry on Unload

When `modelDiscovery: "lmstudio"` is configured, the gateway automatically retries chat requests that fail with an "unloaded" error. On such an error, the client calls `POST /api/v1/models/load` with the model id, then retries the chat request once. This behavior is always enabled for LM Studio providers — there is no separate configuration field.

The streaming variant retries with a single non-streaming call (to avoid invoking `on_chunk` twice if partial data was already streamed).

## Example Providers

Four providers demonstrate the key configuration patterns for `"openai-compat"`. Each shows a different combination of hosting service, deployment, and behavior fields.

### LM Studio — Local-First (Default)

The simplest `openai-compat` configuration: no `baseUrl` or `apiKey` needed. LM Studio is the reference local-first option for this endpoint type — the default base URL (`http://127.0.0.1:1234/v1`) is LM Studio's localhost address.

```json
{ "id": "lmstudio", "endpointType": "openai-compat", "modelDiscovery": "lmstudio" }
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

A bare `{ "id": "local", "endpointType": "openai-compat" }` also connects to LM Studio on localhost, using auto model discovery (`GET /v1/models`) and no automatic retry on unload.

### NearAI — Cloud

A remote OpenAI-compatible API. Needs `baseUrl` and `apiKey`. NearAI is the reference cloud example for this endpoint type.

```json
{
  "id": "nearai",
  "endpointType": "openai-compat",
  "baseUrl": "https://cloud-api.near.ai/v1",
  "apiKey": "<NEARAI_API_KEY>"
}
```

- **Endpoint Type:** `"openai-compat"` — OpenAI chat completions protocol
- **Default base URL:** None — must set `baseUrl` explicitly
- **Model discovery:** `GET /v1/models` (default) — NearAI exposes standard OpenAI model discovery
- **Auth:** API key via `apiKey` field or environment variable
- **Privacy:** Data leaves your environment — sent to NearAI servers

This pattern applies to **any remote OpenAI-compatible API**: OpenAI, Azure OpenAI, Together, Groq, etc. All are `"openai-compat"` providers with a specific `baseUrl` and `apiKey`. The endpoint type is the same as LM Studio — only the hosting service and deployment change.

### NVIDIA NIM — Cloud (Alternative)

NIM lacks a `/v1/models` endpoint, so `modelDiscovery: "static"` with a user-curated `staticModels` list replaces discovery.

```json
{
  "id": "nvidia",
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
- **Auth:** `NVIDIA_API_KEY` or provider `apiKey`
- **Privacy:** Data leaves your environment — sent to NVIDIA servers. The gateway logs a warning at startup when a NIM provider is the default provider.
- **Rate limits:** Free tier allows approximately 40 requests per minute; expect 429 responses under heavier use.
- **Gotchas:** NIM does not expose a `/v1/models` endpoint, so `modelDiscovery: "static"` is required with a user-curated list in `staticModels`.

### Applying the Patterns

Any other OpenAI-compatible server follows one of the three patterns above:

| Pattern | When to Use | Fields |
|---------|-------------|--------|
| LM Studio (local) | Local LM Studio instance | `endpointType: "openai-compat"` + `modelDiscovery: "lmstudio"` |
| Simple remote API | Server has `/v1/models` and standard discovery works | `endpointType: "openai-compat"` + `baseUrl` + `apiKey` |
| Static model list | Server lacks `/v1/models` or you want to curate the list | Add `modelDiscovery: "static"` + `staticModels` |

For example, vLLM, Hugging Face TGI, and OpenAI itself are all "simple remote API" — just `endpointType: "openai-compat"` with the appropriate `baseUrl` and `apiKey`. No special behavior fields needed.

## Configuration Quick Reference

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `id` | `String` | Yes | — | Unique provider id referenced by agents (`defaultProvider`, `enabledProviders`). |
| `endpointType` | `EndpointType` | Yes | — | Must be `"openai-compat"` for this endpoint type. |
| `baseUrl` | `String` | No | `http://127.0.0.1:1234/v1` | Base URL for the OpenAI-compatible server. The default is LM Studio's localhost address. Remote providers must set this explicitly. |
| `apiKey` | `String` | No | — | API key for Bearer auth. When unset, no auth header is sent (suitable for local servers). |
| `defaultModel` | `String` | No | `llama-3.2-3B-instruct` | Default model id for this provider. The endpoint-type default is LM Studio–compatible. |
| `modelDiscovery` | `ModelDiscovery` | No | `"auto"` | How to discover models: `"auto"` (`GET /v1/models`), `"lmstudio"` (`GET /api/v1/models`), or `"static"` (from `staticModels`). |
| `staticModels` | `String[]` | No | `[]` | Model list when `modelDiscovery: "static"`. No polling. |

## OpenAI-Compatible API vs. Ollama API

Comparison of the two endpoint types in Chai, focusing on what the gateway uses today and key protocol differences.

| Area | `openai-compat` | `ollama` |
|------|-----------------|----------|
| **Chat route** | `POST /v1/chat/completions` | `POST /api/chat` |
| **Model list route** | `GET /v1/models` (or `GET /api/v1/models` for LM Studio, or `static`) | `GET /api/tags` |
| **Streaming format** | SSE (`data: <json>\n\n`) | NDJSON (newline-delimited JSON) |
| **Tool result key** | `tool_call_id` (per-call id) | `tool_name` (function name) |
| **Default base URL** | `http://127.0.0.1:1234/v1` (LM Studio localhost) | `http://127.0.0.1:11434` (Ollama localhost) |
| **Default model** | `llama-3.2-3B-instruct` | `llama3.2:3b` |
| **Auth** | Bearer token (optional; required by remote providers) | None (local only) |
| **Finish reason** | `finish_reason` field (`"stop"`, `"length"`, `"tool_calls"`) | `done_reason` field (same values, different field name) |
| **Token usage** | `usage.prompt_tokens` / `usage.completion_tokens` / `usage.total_tokens` | `eval_count` / `prompt_eval_count` (resolved to `Usage` by the client) |
| **Configurable behaviors** | Model discovery (`default`, `lmstudio`, `static`), automatic retry on unload (with `lmstudio`) | None — single client, single protocol |
| **Hosting services** | LM Studio, NearAI, NVIDIA NIM, OpenAI, vLLM, Hugging Face TGI, any OpenAI-shaped server | Ollama |

The Chai agent and tool infrastructure are endpoint-agnostic — the same agent loop, tool definitions, and delegation logic work across both endpoint types. The gateway handles the protocol translation internally.

## Possible Future Use

- **Streaming to the channel** — Send chat deltas to Telegram (or other channels) as they arrive instead of waiting for the full reply. The `chat_stream()` implementation already supports SSE parsing; it needs to be wired to the channel output.
- **Per-request or per-model options** — Pass `temperature`, `max_tokens`, `top_p`, etc., from config when supported. Would allow per-provider or per-model tuning (e.g. lower temperature for tool-heavy flows, higher for creative tasks).
- **Structured output** — Use `response_format` (e.g. JSON schema) where we need strict tool or response shapes.
- **New behavior fields** — As other products adopt the OpenAI wire protocol with product-specific extensions (e.g. model lifecycle, custom endpoints), new behavior fields can be added without creating new endpoint types.

## Cross-References

- **[PROVIDERS.md](../spec/PROVIDERS.md)** — Provider array configuration, endpoint type definitions, categories of providers (local, self-hosted, third-party), and example configurations.
- **[MODELS.md](../spec/MODELS.md)** — Model identifiers per endpoint type, repository inventory, deployment categories, and tool-calling fit.
- **[LM_STUDIO.md](LM_STUDIO.md)** — LM Studio–specific API details (native model list, model lifecycle, auto-load).
- **[NVIDIA_NIM.md](NVIDIA_NIM.md)** — NIM hosted API details (rate limits, privacy caveats, static model list).
- **[OLLAMA.md](OLLAMA.md)** — Native Ollama endpoint type reference (the other endpoint type in Chai).
