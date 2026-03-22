# vLLM Reference

Reference for the **vLLM** inference and serving stack: OpenAI-compatible HTTP APIs, vLLM-specific options, and how it is used in this codebase. Use this when configuring or extending the vLLM backend.

**Scope:** This document focuses on the **OpenAI-compatible HTTP server** (`vllm serve`) and the integration path shared with LM Studio (OpenAI chat completions). It does not cover every vLLM feature (tensor parallelism, disaggregated serving, etc.); see the official docs for those topics.

## Purpose and How to Use

- **Purpose:** Summarize vLLM’s serving APIs relevant to chat agents, note behaviors that differ from plain OpenAI clients, and document how the gateway uses **`VllmClient`**.
- **How to use:** When configuring **`agents.defaultProvider: "vllm"`** or **`enabledProviders`**, consult this doc alongside [LM_STUDIO_REFERENCE.md](LM_STUDIO_REFERENCE.md) (same request/response shapes for chat) and [EPIC_API_ALIGNMENT.md](../EPIC_API_ALIGNMENT.md).

## Official vLLM Documentation

- **Docs home:** https://docs.vllm.ai/
- **OpenAI-compatible server (endpoints, chat template, extra parameters):** https://docs.vllm.ai/en/latest/serving/openai_compatible_server.html
- **Quickstart:** https://docs.vllm.ai/en/latest/getting_started/quickstart/
- **Installation:** https://docs.vllm.ai/en/latest/getting_started/installation/
- **`vllm serve` / configuration:** https://docs.vllm.ai/en/latest/configuration/serve_args/
- **Docker deployment:** https://docs.vllm.ai/en/latest/deployment/docker/
- **OpenAI Chat Completions (request/response shape):** https://platform.openai.com/docs/api-reference/chat — vLLM’s chat server aims to match this API.

## Typical Deployment and Base URL

- **Process:** Install vLLM, then run `vllm serve <model_id>` (see [serve arguments](https://docs.vllm.ai/en/latest/configuration/serve_args/)). Docker images are available for server deployments.
- **Base URL (OpenAI client):** The docs commonly use **`http://localhost:8000/v1`**. In config, set **`providers.vllm.baseUrl`** (default when absent: **`http://127.0.0.1:8000/v1`**). The path **`/v1`** is where OpenAI-compatible routes live.
- **Auth:** Optional. If you start the server with **`--api-key <token>`**, set **`providers.vllm.apiKey`** or **`VLLM_API_KEY`** so the gateway sends `Authorization: Bearer <token>`.
- **Privacy:** Self-hosted vLLM keeps inference on **your** hardware (unlike hosted cloud APIs). Network placement and who can reach the server still matter for operational security.

## Current Usage in the Codebase

### Client and Configuration

- **`crates/lib/src/providers/vllm.rs`** — **`VllmClient`**: OpenAI-compatible **`POST /v1/chat/completions`** and **`GET /v1/models`**; optional bearer auth.
- **`crates/lib/src/providers/openai_compat.rs`** — Shared OpenAI wire types and HTTP logic (also used by LM Studio for chat).
- **Config** — **`agents.defaultProvider`**: **`"vllm"`**; **`agents.defaultModel`**: model id as served (e.g. **`Qwen/Qwen2.5-7B-Instruct`**). **`providers.vllm.baseUrl`** (optional), **`providers.vllm.apiKey`** (optional; overridden by **`VLLM_API_KEY`**).
- **Fallback model** when **`default_model`** is empty: **`Qwen/Qwen2.5-7B-Instruct`** (set **`defaultModel`** explicitly to match your `vllm serve` model).

### Endpoints Used

| Endpoint | Method | Use |
|----------|--------|-----|
| **`/v1/models`** | GET | **`list_models()`** — Model discovery at gateway startup when vLLM is in **`enabledProviders`** or is the default backend; exposed in WebSocket **`status`** as **`vllmModels`**. |
| **`/v1/chat/completions`** | POST | **`chat()`** / **`chat_stream()`** — Agent turn with messages and optional tools (same mapping as LM Studio: internal **`tool_name`** ↔ **`tool_call_id`**). |

### Request/Response Shapes

- **Chat request:** **`model`**, **`messages`**, **`stream`**, optional **`tools`** — OpenAI-compatible; see LM Studio reference for internal message mapping.
- **Streaming:** SSE; parsed like LM Studio into a single **`ChatResponse`** for the agent loop.

### Where vLLM Is Referenced

- **Gateway server** — Builds **`VllmClient`** with **`resolve_vllm_base_url`** and **`resolve_vllm_api_key`**; when **`defaultProvider`** is **`"vllm"`**, runs **`run_turn_dyn`** with that **`Provider`** and **`agents.defaultModel`**.
- **WebSocket `status`** — Includes **`vllmModels`** (from **`GET /v1/models`**).
- **Agent** — Implements **`Provider`** for **`VllmClient`**; same tool loop as other OpenAI-compat providers.
- **Desktop** — Chat backend dropdown and model list use **`vllmModels`** when **`vllm`** is selected.

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
- **Extra parameters:** Parameters not in OpenAI’s API (e.g. **`top_k`**) can be passed via **`extra_body`** in OpenAI clients; the Rust client does not send these unless we add them later.

## Comparison With Other Backends in This Repo

| Aspect | Ollama | LM Studio | vLLM |
|--------|--------|-----------|------|
| **API style** | Native Ollama REST | OpenAI-compatible `/v1/...` | OpenAI-compatible `/v1/...` |
| **Client** | **`OllamaClient`** | **`LmsClient`** | **`VllmClient`** |
| **Model list** | **`/api/tags`** | **`/api/v1/models`** (LM native) | **`GET /v1/models`** |

Other OpenAI-compat backends in this repo (**`openai`**, **`hf`**, **`nim`**) use the same chat/list patterns via **`OpenAiCompatClient`** (or **`NimClient`** for NIM); see [OPENAI_REFERENCE.md](OPENAI_REFERENCE.md), [HUGGINGFACE_REFERENCE.md](HUGGINGFACE_REFERENCE.md), [NVIDIA_NIM_REFERENCE.md](NVIDIA_NIM_REFERENCE.md).

## Possible Future Use

- **Streaming to the channel** — Forward SSE deltas to Telegram or other channels as they arrive.
- **Per-request options** — Pass **`temperature`**, **`max_tokens`**, or vLLM **`extra_body`** fields from config.
- **Embeddings** — Call **`/v1/embeddings`** from a separate code path for RAG.
