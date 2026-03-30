---
status: current
---

# LM Studio Reference

Reference for how the LM Studio API is used in this codebase, what the full API offers, and how it aligns with the OpenAI-compatible endpoints. Use this when extending LLM support or aligning with LM Studio’s capabilities.

## Purpose and How to Use

- **Purpose:** Document current LM Studio usage (via the OpenAI-compatible API), list LM Studio API capabilities we do not yet use, and summarize how it compares to Ollama and hosted OpenAI.
- **How to use:** When adding features (e.g. streaming to the channel, model load/unload), consult this doc.

## Official LM Studio API

- **REST API overview:** https://lmstudio.ai/docs/developer/rest  
- **OpenAI-compatible endpoints:** https://lmstudio.ai/docs/developer/openai-compat/chat-completions (chat), plus list models, responses, etc.  
- **Streaming events (native API):** https://lmstudio.ai/docs/developer/rest/streaming-events  
- **Model lifecycle:** https://lmstudio.ai/docs/developer/rest/load (load), https://lmstudio.ai/docs/developer/rest/unload (unload).  
- **Base URL (local, OpenAI-compat):** Typically `http://localhost:1234/v1`; configurable via **`providers.lms.baseUrl`** (default when absent). The `/v1` path is used for OpenAI-compatible endpoints.

## Current Usage in the Codebase

### Client and Configuration

- **LM Studio client** — Uses only the OpenAI-compatible API: **`GET /api/v1/models`** for model list and **`POST /v1/chat/completions`** for chat (with tools). All API errors (400, 500, etc.) are returned to the caller; the only automatic retry is when chat returns 500 "Model is unloaded" — we call POST `/api/v1/models/load` then retry chat once (aligns with Ollama).
- **Config** — LM Studio is selected when **`agents.defaultProvider`** is **`"lms"`** (see [EPIC_API_ALIGNMENT.md](../EPIC_API_ALIGNMENT.md) for the full provider list). **`agents.defaultModel`** is the model id passed as-is (e.g. `llama-3.2-3B-instruct`, `openai/gpt-oss-20b`). Under **`providers`** use **`lms`** for LM Studio settings. Only **`baseUrl`** is supported (default `http://127.0.0.1:1234/v1`).

### Base URL

**Base URL** — **`providers.lms.baseUrl`** (default `http://127.0.0.1:1234/v1` when absent). Points at the `/v1` path for chat; `/api/v1` is used only for model list and load.

### Endpoints Used

| Endpoint | Method | Use |
|----------|--------|-----|
| **`/api/v1/models`** | GET | `list_models()` — model list; returned `key` is the model id for chat. |
| **`/api/v1/models/load`** | POST | Called when chat returns 500 "Model is unloaded"; we load then retry chat once. Request body **`{ "model": "<id>" }`** only. If load fails (e.g. VRAM), use `lms load <model> --gpu 0.5` then chat. |
| **`/v1/chat/completions`** | POST | Agent turn with messages and optional tools. All errors (400, 500, etc.) are returned; we only retry after load on "unloaded". |

### Request/Response Shapes (What We Send)

- **Chat request:** `model`, `messages` (array of `{ role, content }` or tool messages with `tool_call_id`), `stream`, `tools` (optional array of function definitions in OpenAI shape). We map our internal messages (with `tool_name` for tool results) to OpenAI format (tool results keyed by `tool_call_id` from the previous assistant `tool_calls`).
- **Streaming:** When `stream: true`, the response is SSE (Server-Sent Events). We parse `data:` lines and accumulate content and `tool_calls` to build a single `ChatResponse` for the agent loop.
- **Auth:** LM Studio local server often accepts any API key (e.g. `lm-studio`); we send a placeholder if the API requires a key. Optional config for API key when using a remote or secured LM Studio instance.

### Where LM Studio Is Referenced

- **Gateway server** — Builds the LM Studio client with **`resolve_lms_base_url`** from config. When **`defaultProvider`** is **`"lms"`**, runs the agent turn via **`run_turn_dyn`** with the LM Studio **`Provider`** and **`agents.defaultModel`** (passed as-is).
- **Agent** — **`run_turn_dyn`** uses the **`Provider`** trait; the gateway passes the resolved client and model id.
- **Tools** — The same skill `tools.json` and `ToolDefinition` list are converted to OpenAI tool format when calling LM Studio.

## LM Studio API Overview

### Chat and Model List

- **`POST /v1/chat/completions`** — Agent turn with `model`, `messages` (including tool messages), `stream: true` or `false`, optional `tools`. We map our internal messages to OpenAI format and back. Response: SSE when streaming; we parse `data:` lines and accumulate content and `tool_calls` into a single `ChatResponse`. All errors are returned; we only retry once after load when the server returns "Model is unloaded".
- **`GET /api/v1/models`** — Model discovery; response shape uses `models[].key`. We expose the result in the gateway WebSocket **status** as **`lmsModels`**.

### Model Lifecycle

- **`GET /api/v1/models`** — **Used.** List models at startup; gateway exposes result in WebSocket status as `lmsModels`.
- **`POST /api/v1/models/load`** — Called when chat returns "Model is unloaded"; we load then retry once. Not used proactively.
- **`POST /api/v1/models/unload`** — Not used. Could add "unload model" from desktop or CLI to free memory.

### Possible Future Use

- **Streaming to the channel** — Send chat deltas to Telegram (or other channels) as they arrive instead of waiting for the full reply.
- **Anthropic-compatible `/v1/messages`** — Not used; we use chat completions only.
- **Per-request or per-model options** — Pass `temperature`, `max_tokens`, etc., from config when supported.
- **Model management** — Optional load/unload from desktop or CLI for operators.
