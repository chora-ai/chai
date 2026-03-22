# NVIDIA NIM (Hosted API) Reference

Reference for the **NVIDIA NIM hosted API** (free tier at build.nvidia.com): what the API offers, how the gateway uses it, and important limitations. Use this when configuring **`nim`** or when aligning with NVIDIA’s hosted NIM capabilities.

**Scope:** This document covers **only the free hosted API** at `integrate.api.nvidia.com`. It does **not** cover NIM containers or self-hosted NIM (NVIDIA AI Enterprise), which are proprietary and expensive at scale.

## Privacy and Data Handling

**NVIDIA NIM hosted API is not a privacy-preserving option.** All requests and conversation data are sent to NVIDIA’s servers. This service should be used only as a **free scratchpad to try open-source models** before investing in local or self-hosted hardware. User-facing docs and the gateway log a **warning at startup** when **`nim`** is the default provider (and when the API key is missing).

## Purpose and How to Use

- **Purpose:** Document the NIM hosted API for integration, list its capabilities and limits, and clarify that it is a non-privacy, rate-limited free tier.
- **How to use:** When configuring **`agents.defaultProvider`**: **`"nim"`** or reviewing NIM behavior, consult this doc. Ensure privacy and rate-limit caveats stay visible to users.

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
- **Model ID:** Must be one of the NIM catalog model identifiers (e.g. `qwen/qwen3-5-122b-a10b`). Use the exact id from the API docs. Full list: https://docs.api.nvidia.com/nim/reference/llm-apis  
- **Streaming:** Supported via `stream: true`; response is SSE (Server-Sent Events), same as OpenAI.  
- **Errors:** 402 Payment Required when credits/limits are exceeded; 422 for validation errors.

### Model List

- NVIDIA does not expose a single “list models” endpoint for the hosted API in the same way as Ollama or LM Studio. The set of available models is documented in the [LLM APIs reference](https://docs.api.nvidia.com/nim/reference/llm-apis). A client can either hardcode a supported model list for discovery or rely on config-only model selection (e.g. `agents.default_model` set to a known NIM model id).
- **Why the static list in this codebase:** The gateway exposes a small, curated list so the desktop (and status) can show a few options without maintaining the full NIM catalog. Criteria used: one or two chat/instruct models per major vendor (Meta, Mistral, Google, Qwen, Microsoft, NVIDIA), a mix of small and large sizes, so users can try the free API quickly. The Qwen 3.5 / Qwen3-Next models were added on user request. Any model id from the NIM docs works when set in config or per-request even if it is not in this list.

## Current Usage in the Codebase

### Client and Configuration

- **`crates/lib/src/providers/nim.rs`** — `NimClient` calls `https://integrate.api.nvidia.com/v1/chat/completions` with `Authorization: Bearer <api_key>`. Same OpenAI-compat request/response as LM Studio (internal `tool_name` ↔ `tool_call_id` mapping).
- **Config:** **`agents.defaultProvider`**: **`"nim"`**; **`agents.defaultModel`**: a NIM model id (e.g. `qwen/qwen3-5-122b-a10b`). API key from **`providers.nim.apiKey`** or **`NVIDIA_API_KEY`** env. No base URL override (fixed hosted API).

### Endpoints Used

| Endpoint | Method | Use |
|----------|--------|-----|
| **`/v1/chat/completions`** | POST | Agent turn: `model`, `messages` (OpenAI format, including tool messages keyed by `tool_call_id`), optional `tools`, `stream`. |

### Request/Response Shapes

- **Chat request:** `model`, `messages` (array of `{ role, content }` or tool messages with `tool_call_id`), `stream`, `tools` (optional). Same mapping as LM Studio: internal messages with `tool_name` for tool results must be converted to OpenAI format with `tool_call_id` before sending.
- **Streaming:** SSE; parse `data:` lines and accumulate content and `tool_calls` into a single response for the agent loop if streaming is implemented.

### Where NIM Is Referenced

- **Gateway server** — Resolves backend from **`agents.defaultProvider`** (**`"nim"`**); builds **`NimClient`** with API key from config/env; runs the agent turn via **`run_turn_dyn`** with that **`Provider`** and model id from **`agents.defaultModel`**. Logs a **warning at startup** when NIM is the default backend (privacy and rate-limit notice) and when the API key is missing.
- **Agent** — Same **`Provider`** path as other backends; gateway passes the NIM client and model id.
- **Tools** — Same skill `tools.json` and tool definitions, converted to OpenAI tool format for NIM (as with LM Studio).
- **Status** — WebSocket `status` includes **`nimModels`** (static list of known NIM model ids for UI).

## What We Do Not Support (Out of Scope)

- **NIM containers / self-hosted NIM** — Proprietary, NVIDIA AI Enterprise; not in scope for this reference or for the free “scratchpad” use case.
- **Downloadable NIM microservices** — Same as above; this doc is only for the hosted API at `integrate.api.nvidia.com`.

## Possible Future Use

- **Streaming to the channel** — Use `stream: true` and send deltas to the channel as they arrive.
- **Rate-limit handling** — Detect 429 or 402, back off, and optionally inform the user that the NIM free tier limit was hit.
- **Model discovery** — If NVIDIA adds a list endpoint or we maintain a curated list, expose available NIM models in gateway status or config UI.
