---
status: current
---

# vLLM Reference

Reference for the **vLLM** inference and serving stack: OpenAI-compatible HTTP APIs, vLLM-specific options, and how it is used in this codebase. Use this when configuring or extending the vLLM backend.

**Scope:** This document focuses on the **OpenAI-compatible HTTP server** (`vllm serve`) and the integration path shared with other `"openai-compat"` providers. It does not cover every vLLM feature (tensor parallelism, disaggregated serving, etc.); see the official docs for those topics.

## Purpose and How to Use

- **Purpose:** Summarize vLLM's serving APIs relevant to chat agents, note behaviors that differ from plain OpenAI clients, and document how the gateway uses vLLM via the `"openai-compat"` endpoint type.
- **How to use:** Configure a provider with `endpoint: "openai-compat"` and the vLLM base URL. Consult this doc alongside [LM_STUDIO.md](LM_STUDIO.md) (same request/response shapes for chat).

## Official vLLM Documentation

- **Docs home:** https://docs.vllm.ai/
- **OpenAI-compatible server (endpoints, chat template, extra parameters):** https://docs.vllm.ai/en/latest/serving/openai_compatible_server.html
- **Quickstart:** https://docs.vllm.ai/en/latest/getting_started/quickstart/
- **Installation:** https://docs.vllm.ai/en/latest/getting_started/installation/
- **`vllm serve` / configuration:** https://docs.vllm.ai/en/latest/configuration/serve_args/
- **Docker deployment:** https://docs.vllm.ai/en/latest/deployment/docker/
- **OpenAI Chat Completions (request/response shape):** https://platform.openai.com/docs/api-reference/chat — vLLM's chat server aims to match this API.

## Typical Deployment and Base URL

- **Process:** Install vLLM, then run `vllm serve <model_id>` (see [serve arguments](https://docs.vllm.ai/en/latest/configuration/serve_args/)). Docker images are available for server deployments.
- **Base URL (OpenAI client):** The docs commonly use **`http://localhost:8000/v1`**. In config, set provider `baseUrl` (default when absent for `"openai-compat"` is **`http://127.0.0.1:1234/v1`**, so you must set `baseUrl` explicitly for vLLM). The path **`/v1`** is where OpenAI-compatible routes live.
- **Auth:** Optional. If you start the server with **`--api-key <token>`**, set provider `apiKey` or `VLLM_API_KEY` so the gateway sends `Authorization: Bearer <token>`.
- **Privacy:** Self-hosted vLLM keeps inference on **your** hardware (unlike hosted cloud APIs). Network placement and who can reach the server still matter for operational security.

## Current Usage in the Codebase

### Client and Configuration

- vLLM is configured as an `"openai-compat"` provider with a custom `baseUrl`. Example:

  ```json
  { "id": "vllm", "endpoint": "openai-compat", "baseUrl": "http://127.0.0.1:8000/v1" }
  ```

- **Client** — Uses `OpenAiCompatClient` (shared with all `"openai-compat"` providers). Same `tool_name` ↔ `tool_call_id` mapping.
- **Config:** Provider `id` (e.g. `"vllm"`), `endpoint: "openai-compat"`, `baseUrl: "http://127.0.0.1:8000/v1"`, optional `apiKey` / `VLLM_API_KEY`. `agents.defaultProvider` references the provider `id`; `agents.defaultModel` is the model id as served (e.g. **`Qwen/Qwen2.5-7B-Instruct`**).
- **Fallback model** when **`defaultModel`** is empty: **`Qwen/Qwen2.5-7B-Instruct`** (set `defaultModel` explicitly to match your `vllm serve` model).

### Endpoints Used

| Endpoint | Method | Use |
|----------|--------|-----|
| **`/v1/models`** | GET | Model discovery when `modelDiscovery: "default"` (the default); exposed in WebSocket **`status`**. |
| **`/v1/chat/completions`** | POST | Agent turn with messages and optional tools (same mapping as other OpenAI-compat providers: internal **`tool_name`** ↔ **`tool_call_id`**). |

### Request/Response Shapes

- **Chat request:** **`model`**, **`messages`**, **`stream`**, optional **`tools`** — OpenAI-compatible; see LM Studio reference for internal message mapping.
- **Streaming:** SSE; parsed like other OpenAI-compat providers into a single **`ChatResponse`** for the agent loop.

### Where vLLM Is Referenced

- **Gateway server** — Builds `OpenAiCompatClient` for the provider with `baseUrl` and `apiKey` from config; when the provider `id` is referenced in `defaultProvider`, runs `run_turn_dyn` with that `Provider` and `agents.defaultModel`.
- **WebSocket `status`** — Includes models from `GET /v1/models` discovery.
- **Agent** — Implements `Provider` for `OpenAiCompatClient`; same tool loop as other OpenAI-compat providers.
- **Desktop** — Chat backend dropdown and model list use discovered models when this provider is selected.

## vLLM OpenAI-Compatible API Overview

### Chat (`POST /v1/chat/completions`)

- **Compatible with** OpenAI Chat Completions: **`model`**, **`messages`**, **`stream`**, **`temperature`**, **`tools`**, etc. Tool/function calling follows the OpenAI format; streaming uses SSE when **`stream: true`**.
- **`parallel_tool_calls`:** vLLM documents that setting this to **`false`** forces at most one tool call per request; **`true`** (default) allows more than one, but actual behavior depends on the model.
- **`user`:** Documented as **ignored** by vLLM for chat completions.

### Other OpenAI-Style Endpoints (Not Used by the Current Agent)

The server also exposes other routes (e.g. **Completions**, **Embeddings**, **audio**). The agent stack here is **chat-centric**.

## vLLM-Specific Behaviors (Chat Integration)

- **Chat template:** Chat requests need a model whose tokenizer defines a **chat template**, or you must supply **`--chat-template`**. See the OpenAI-compatible server page.
- **Default sampling from Hugging Face:** By default, the server may apply **`generation_config.json`** from the model repo. To avoid that, start the server with **`--generation-config vllm`** (as documented).
- **Extra parameters:** Parameters not in OpenAI's API (e.g. **`top_k`**) can be passed via **`extra_body`** in OpenAI clients; the Rust client does not send these unless we add them later.

## Comparison With Other Backends in This Repo

| Aspect | Ollama | LM Studio | vLLM |
|--------|--------|-----------|------|
| **API style** | Native Ollama REST | OpenAI-compatible `/v1/...` | OpenAI-compatible `/v1/...` |
| **Endpoint type** | `"ollama"` | `"openai-compat"` | `"openai-compat"` |
| **Client** | **`OllamaClient`** | **`OpenAiCompatClient`** + `modelDiscovery: "lmstudio"`, `autoLoad: "lmstudio"` | **`OpenAiCompatClient`** |
| **Model list** | **`/api/tags`** | **`/api/v1/models`** (LM native via `modelDiscovery: "lmstudio"`) | **`GET /v1/models`** (via `modelDiscovery: "default"`) |

Other OpenAI-compat backends (**`openai`**, **`hf`**, **`nim`**) use the same `OpenAiCompatClient` with different `baseUrl` and behavior settings; see [OPENAI.md](OPENAI.md), [HUGGINGFACE.md](HUGGINGFACE.md), [NVIDIA_NIM.md](NVIDIA_NIM.md).

## Possible Future Use

- **Streaming to the channel** — Forward SSE deltas to Telegram or other channels as they arrive.
- **Per-request options** — Pass **`temperature`**, **`max_tokens`**, or vLLM **`extra_body`** fields from config.
- **Embeddings** — Call **`/v1/embeddings`** from a separate code path for RAG.
