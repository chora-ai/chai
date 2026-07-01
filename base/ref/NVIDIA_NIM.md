---
status: current
---

# NVIDIA NIM Reference

Reference for the **NVIDIA NIM hosted API** (free tier at build.nvidia.com): what the API offers, how the gateway uses it, and important limitations. Use this when configuring a NIM provider or when aligning with NVIDIA's hosted NIM capabilities.

**Scope:** This document covers **only the free hosted API** at `integrate.api.nvidia.com`. It does **not** cover NIM containers or self-hosted NIM (NVIDIA AI Enterprise), which are proprietary and expensive at scale.

## Privacy and Data Handling

**NVIDIA NIM hosted API is not a privacy-preserving option.** All requests and conversation data are sent to NVIDIA's servers. This service should be used only as a **free scratchpad to try open-source models** before investing in local or self-hosted hardware. User-facing docs and the gateway log a **warning at startup** when a NIM provider is the default provider (and when the API key is missing).

## Purpose and How to Use

- **Purpose:** Document the NIM hosted API for integration, list its capabilities and limits, and clarify that it is a non-privacy, rate-limited free tier.
- **How to use:** When configuring a provider with `endpointType: "openai-compat"` for NIM usage, consult this doc.

## Official NVIDIA NIM API

- **LLM APIs overview:** https://docs.api.nvidia.com/nim/reference/llm-apis  
- **Chat completion (OpenAI-compatible):** POST `/v1/chat/completions` — see https://platform.openai.com/docs/api-reference/chat/create for the request/response shape.  
- **Base URL:** `https://integrate.api.nvidia.com`  
- **Free tier / API keys:** https://build.nvidia.com — sign up and create an API key to use the hosted NIM endpoints.  
- **Auth:** Bearer token in `Authorization: Bearer $NVIDIA_API_KEY`. The API key is required.

## Rate Limits and Quotas

- The **free hosted API** allows approximately **40 requests per minute**. Implementations should expect 429 or similar rate-limit responses under heavier use; consider backoff or user-facing messaging when this backend is selected.

## API Overview

### Chat (`POST /v1/chat/completions`)

- **OpenAI-compatible:** Same path and request/response shape as OpenAI chat completions (`model`, `messages`, `stream`, `temperature`, `top_p`, `max_tokens`, `stop`, etc.). Tool/function calling follows the OpenAI format (e.g. `tool_call_id` for tool results).
- **Model ID:** Must be one of the NIM catalog model identifiers (e.g. `meta/llama-3.2-3b-instruct`). Use the exact id from the API docs. Full list: https://docs.api.nvidia.com/nim/reference/llm-apis  
- **Streaming:** Supported via `stream: true`; response is SSE (Server-Sent Events), same as OpenAI.  
- **Errors:** 402 Payment Required when credits/limits are exceeded; 422 for validation errors.

### Model List

- NVIDIA does not expose a single "list models" endpoint for the hosted API in the same way as Ollama or LM Studio. The set of available models is documented in the [LLM APIs reference](https://docs.api.nvidia.com/nim/reference/llm-apis). A client can either hardcode a supported model list for discovery or rely on config-only model selection (e.g. `agents.defaultModel` set to a known NIM model id).
- **In Chai, NIM uses `modelDiscovery: "static"` with `staticModels`:** Users curate their own model list in the `staticModels` config field rather than relying on a discovery endpoint. Any model id from the NIM docs works when added to the `staticModels` array.

## Current Usage in the Codebase

### Client and Configuration

- NIM is configured as an `"openai-compat"` provider with `modelDiscovery: "static"` and a user-curated `staticModels` list. Example:

  ```json
  {
    "id": "nvidia",
    "endpointType": "openai-compat",
    "baseUrl": "https://integrate.api.nvidia.com/v1",
    "modelDiscovery": "static",
    "staticModels": ["meta/llama-3.1-8b-instruct", "meta/llama-3.1-70b-instruct"],
    "apiKey": "<NVIDIA_API_KEY>"
  }
  ```

- **Client** — Uses `OpenAiCompatClient` (shared with all `"openai-compat"` providers). Same `tool_name` ↔ `tool_call_id` mapping.
- **Config:** Provider `id` (e.g. `"nvidia"`), `endpointType: "openai-compat"`, `baseUrl: "https://integrate.api.nvidia.com/v1"`, `modelDiscovery: "static"`, `staticModels` array. API key from provider `apiKey` or `NVIDIA_API_KEY` env. `agents.defaultProvider` references the provider `id`; `agents.defaultModel` is a NIM model id (e.g. `meta/llama-3.1-8b-instruct`).

### Endpoints Used

| Endpoint | Method | Use |
|----------|--------|-----|
| **`/v1/chat/completions`** | POST | Agent turn: `model`, `messages` (OpenAI format, including tool messages keyed by `tool_call_id`), optional `tools`, `stream`. |

### Request/Response Shapes

- **Chat request:** `model`, `messages` (array of `{ role, content }` or tool messages with `tool_call_id`), `stream`, `tools` (optional). Same mapping as other OpenAI-compat providers: internal messages with `tool_name` for tool results must be converted to OpenAI format with `tool_call_id` before sending.
- **Streaming:** SSE; parse `data:` lines and accumulate content and `tool_calls` into a single response for the agent loop if streaming is implemented.

### Where NIM Is Referenced

- **Gateway server** — Resolves backend from provider `id`; builds `OpenAiCompatClient` with API key from config/env; runs the agent turn via `run_turn_dyn` with that `Provider` and model id from `agents.defaultModel`. Logs a **warning at startup** when a NIM provider is the default backend (privacy and rate-limit notice) and when the API key is missing.
- **Agent** — Same `Provider` path as other backends; gateway passes the client and model id.
- **Tools** — Same skill `tools.json` and tool definitions, converted to OpenAI tool format for NIM (as with all OpenAI-compat providers).
- **Status** — WebSocket `status` includes models from the provider's `staticModels` config field.

## What We Do Not Support (Out of Scope)

- **NIM containers / self-hosted NIM** — Proprietary, NVIDIA AI Enterprise; not in scope for this reference or for the free "scratchpad" use case.
- **Downloadable NIM microservices** — Same as above; this doc is only for the hosted API at `integrate.api.nvidia.com`.

## Possible Future Use

- **Streaming to the channel** — Use `stream: true` and send deltas to the channel as they arrive.
- **Rate-limit handling** — Detect 429 or 402, back off, and optionally inform the user that the NIM free tier limit was hit.
- **Model discovery** — If NVIDIA adds a list endpoint, Chai's `"auto"` model discovery could be used instead of `"static"`.
