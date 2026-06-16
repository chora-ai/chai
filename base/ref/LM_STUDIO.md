---
status: current
---

# LM Studio Reference

Reference for how the LM Studio API is used in this codebase, what the full API offers, and how it aligns with the OpenAI-compatible endpoints. Use this when extending LLM support or aligning with LM Studio's capabilities.

## Purpose and How to Use

- **Purpose:** Document current LM Studio usage (via the `"openai-compat"` endpoint type with LM Studio–specific behavior fields), list LM Studio API capabilities we do not yet use, and summarize how it compares to Ollama and hosted OpenAI.
- **How to use:** When adding features (e.g. streaming to the channel, model load/unload), consult this doc.

## Official LM Studio API

- **REST API overview:** https://lmstudio.ai/docs/developer/rest  
- **OpenAI-compatible endpoints:** https://lmstudio.ai/docs/developer/openai-compat/chat-completions (chat), plus list models, responses, etc.  
- **Streaming events (native API):** https://lmstudio.ai/docs/developer/rest/streaming-events  
- **Model lifecycle:** https://lmstudio.ai/docs/developer/rest/load (load), https://lmstudio.ai/docs/developer/rest/unload (unload).  
- **Base URL (local, OpenAI-compat):** Typically `http://localhost:1234/v1`; configurable via provider `baseUrl` field (default when absent). The `/v1` path is used for OpenAI-compatible endpoints.

## Current Usage in the Codebase

### Client and Configuration

- **LM Studio is configured as an `"openai-compat"` provider** with LM Studio–specific behavior fields. Example:

  ```json
  { "id": "lms", "endpointType": "openai-compat", "modelDiscovery": "lmstudio" }
  ```

- **`modelDiscovery: "lmstudio"`** — Uses LM Studio's native `GET /api/v1/models` endpoint to discover models (filters by `type == "llm"`, uses `key` as model id), instead of the standard OpenAI-compat `GET /v1/models`.
- **Automatic retry on unload** — When `modelDiscovery: "lmstudio"` is set and a chat request returns a 500 "Model is unloaded" error, the client automatically calls `POST /api/v1/models/load` with the model id and retries the chat request once. This behavior is always enabled for LM Studio providers — no separate configuration field is needed.
- **Config** — A provider with `endpointType: "openai-compat"` and `id` such as `"lms"` (user-chosen). `agents.defaultProvider` references this `id`. `agents.defaultModel` is the model id passed as-is (e.g. `llama-3.2-3B-instruct`, `openai/gpt-oss-20b`). Optional `baseUrl` (default `http://127.0.0.1:1234/v1`).
- **Client** — Uses `OpenAiCompatClient` (shared with all `"openai-compat"` providers). LM Studio–specific model discovery and retry on unload are implemented as methods on `OpenAiCompatClient`.

### Base URL

**Base URL** — Provider `baseUrl` field (default `http://127.0.0.1:1234/v1` when absent). Points at the `/v1` path for chat; `/api/v1` is used only for the native model list and model load endpoints.

### Endpoints Used

| Endpoint | Method | Use |
|----------|--------|-----|
| **`/api/v1/models`** | GET | `modelDiscovery: "lmstudio"` — model list; returned `key` is the model id for chat. Filter `type == "llm"`. |
| **`/api/v1/models/load`** | POST | Automatically called when `modelDiscovery: "lmstudio"` and chat returns 500 "Model is unloaded"; the client loads the model then retries the chat request once. Request body **`{ "model": "<id>" }`** only. If load fails (e.g. VRAM), use `lms load <model> --gpu 0.5` then chat. |
| **`/v1/chat/completions`** | POST | Agent turn with messages and optional tools. All errors (400, 500, etc.) are returned; we only retry after load on "unloaded". |

### Request/Response Shapes (What We Send)

- **Chat request:** `model`, `messages` (array of `{ role, content }` or tool messages with `tool_call_id`), `stream`, `tools` (optional array of function definitions in OpenAI shape). We map our internal messages (with `tool_name` for tool results) to OpenAI format (tool results keyed by `tool_call_id` from the previous assistant `tool_calls`).
- **Streaming:** When `stream: true`, the response is SSE (Server-Sent Events). We parse `data:` lines and accumulate content and `tool_calls` to build a single `ChatResponse` for the agent loop.
- **Auth:** LM Studio local server often accepts any API key (e.g. `lm-studio`); we send a placeholder if the API requires a key. Optional config for API key when using a remote or secured LM Studio instance.

### Where LM Studio Is Referenced

- **Gateway server** — Builds the `OpenAiCompatClient` for the provider with `retry_on_unload: true` (derived from `modelDiscovery: "lmstudio"`). When the provider `id` is referenced in `defaultProvider`, runs the agent turn via `run_turn_dyn` with that `Provider` and `agents.defaultModel` (passed as-is).
- **Agent** — `run_turn_dyn` uses the `Provider` trait; the gateway passes the resolved client and model id.
- **Tools** — The same skill `tools.json` and `ToolDefinition` list are converted to OpenAI tool format when calling LM Studio.

## LM Studio API Overview

### Chat and Model List

- **`POST /v1/chat/completions`** — Agent turn with `model`, `messages` (including tool messages), `stream: true` or `false`, optional `tools`. We map our internal messages to OpenAI format and back. Response: SSE when streaming; we parse `data:` lines and accumulate content and `tool_calls` into a single `ChatResponse`. All errors are returned; we only retry once after load when the server returns "Model is unloaded".
- **`GET /api/v1/models`** — Model discovery when `modelDiscovery: "lmstudio"`; response shape uses `models[].key`. We filter by `type == "llm"` and expose the result in the gateway WebSocket `status`.

### Model Lifecycle

- **`GET /api/v1/models`** — **Used.** List models when `modelDiscovery: "lmstudio"`; gateway exposes result in WebSocket status.
- **`POST /api/v1/models/load`** — Automatically called when `modelDiscovery: "lmstudio"` and chat returns "Model is unloaded"; the client loads the model then retries once. Not used proactively.
- **`POST /api/v1/models/unload`** — Not used. Could add "unload model" from desktop or CLI to free memory.

### Possible Future Use

- **Streaming to the channel** — Send chat deltas to Telegram (or other channels) as they arrive instead of waiting for the full reply.
- **Anthropic-compatible `/v1/messages`** — Not used; we use chat completions only.
- **Per-request or per-model options** — Pass `temperature`, `max_tokens`, etc., from config when supported.
- **Model management** — Optional load/unload from desktop or CLI for operators.
