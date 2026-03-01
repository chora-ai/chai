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
- **Base URL (local, OpenAI-compat):** Typically `http://localhost:1234/v1`; configurable via **`agents.backends.lmStudio.baseUrl`** (default when absent). The `/v1` path is used for OpenAI-compatible endpoints.

## Current Usage in the Codebase

### Client and Configuration

- **LM Studio client** — Supports two endpoint types. **OpenAI** (`endpointType: "openai"`, default): talks to `/v1/chat/completions` and `/v1/models` with full tool/function calling and message-based chat. **Native** (`endpointType: "native"`): talks to `/api/v1/chat` and `/api/v1/models`; message content only (no custom tools in this implementation).
- **Config** — Backend is set by **`agents.defaultBackend`** (`"ollama"` or `"lmstudio"`; default `"ollama"`). When `defaultBackend` is `"lmstudio"`, **`agents.defaultModel`** is the model id passed as-is (e.g. `openai/gpt-oss-20b`, `ibm/granite-4-micro`). Base URL and endpoint type: **`agents.backends.lmStudio.baseUrl`** and optional **`agents.backends.lmStudio.endpointType`** (`"openai"` | `"native"`; default `"openai"`).

### Base URL and Endpoint Type

**Base URL** — **`agents.backends.lmStudio.baseUrl`** (default `http://127.0.0.1:1234/v1` when absent).

**Endpoint type** — LM Studio exposes two API shapes; the gateway supports both via **`agents.backends.lmStudio.endpointType`**:

- **`openai`** (default) — Use OpenAI-compatible `/v1/models` and `/v1/chat/completions`. Full tool calling and multi-turn message history. Base URL should point at the `/v1` path (e.g. `http://127.0.0.1:1234/v1`).
- **`native`** — Use LM Studio native `/api/v1/models` and `/api/v1/chat`. The server root is derived from the configured base URL (if it ends with `/v1`, that suffix is stripped). **Tool calling is not supported in native mode** in this implementation: the agent sends only message content and receives only message content; skill tools are not sent to the model. Suitable for simple chat when you prefer the native API or use native-only features (e.g. MCP) outside the gateway.

LM Studio does **not** expose Ollama-compatible endpoints; only `openai` and `native` are available for LM Studio.

### Endpoints Used

| Endpoint type | Endpoint | Method | Use |
|---------------|----------|--------|-----|
| **OpenAI** | `/v1/models` | GET | `list_models()` — Discover models at startup; gateway exposes result in WebSocket status as `lmStudioModels`. |
| **OpenAI** | `/v1/chat/completions` | POST | `chat()` and `chat_stream()` — Agent turn with messages and optional tools; map to/from internal `ChatMessage` / `ChatResponse`. |
| **Native** | `/api/v1/models` | GET | `list_models()` — Same discovery; response shape uses `models[].key`. |
| **Native** | `/api/v1/chat` | POST | `chat()` — Message content only (system_prompt + input array); tools not sent; response `output[]` parsed for type `"message"` content. `chat_stream()` in native mode performs one non-streaming call and passes full content to the callback. |

### Request/Response Shapes (What We Send)

- **Chat request:** `model`, `messages` (array of `{ role, content }` or tool messages with `tool_call_id`), `stream`, `tools` (optional array of function definitions in OpenAI shape). We map our internal messages (with `tool_name` for tool results) to OpenAI format (tool results keyed by `tool_call_id` from the previous assistant `tool_calls`).
- **Streaming:** When `stream: true`, the response is SSE (Server-Sent Events). We parse `data:` lines and accumulate content and `tool_calls` to build a single `ChatResponse` for the agent loop.
- **Auth:** LM Studio local server often accepts any API key (e.g. `lm-studio`); we send a placeholder if the API requires a key. Optional config for API key when using a remote or secured LM Studio instance.

### Where LM Studio Is Referenced

- **Gateway server** — Builds the LM Studio client with **`resolve_lm_studio_base_url`** and **`resolve_lm_studio_endpoint_type`** from config, so either OpenAI-compat or native endpoint is used. Resolves backend from **`agents.defaultBackend`** (`"lmstudio"` → LM Studio) and calls agent `run_turn` with that client and **`agents.default_model`** (passed as-is).
- **Agent** — `run_turn` uses a common trait so it can call either Ollama or LM Studio; the gateway passes the right client and the model id.
- **Tools** — The same skill `tools.json` and `ToolDefinition` list are converted to OpenAI tool format when calling LM Studio.

## LM Studio API Overview

### Native v1 API (`/api/v1/*`)

Used when **`agents.backends.lmStudio.endpointType`** is `"native"`. The native API does not accept our skill tools as function definitions; LM Studio’s native “tools” are MCP integrations, which this codebase does **not** support. We send only message content and receive only message content.

- **`POST /api/v1/chat`** — We call this when endpoint type is native for the agent turn. Request: `model`, `system_prompt`, `input` (array of message-like items), `stream: false`. We do not send skill tools or the `integrations` (MCP) parameter. Response: we parse `output[]` for items of type `"message"` and take the content as the reply. `chat_stream()` in native mode performs one non-streaming call and passes the full content to the callback. This endpoint also supports stateful chats (e.g. `previous_response_id`); we do not use that.
- **`GET /api/v1/models`** — We call this when endpoint type is native for model discovery at startup; response shape uses `models[].key`. We store the result and expose it in the gateway’s WebSocket **status** response to clients under **`lmStudioModels`** (same key as when endpoint type is openai).
- **Streaming** — The native API supports SSE (e.g. `chat.start`, `message.delta`, `chat.end`). We do not parse it: native SSE uses a different event format than OpenAI’s `data:` JSON chunks, so it would require a separate parser. We use a single non-streaming call for native and pass the full reply to the callback for the agent loop.

### OpenAI-Compatible Endpoints (`/v1/*`)

Used when **`agents.backends.lmStudio.endpointType`** is `"openai"` (default).

- **`POST /v1/chat/completions`** — We call this when endpoint type is openai for the agent turn. Request: `model`, `messages` (array of `{ role, content }` or tool messages with `tool_call_id`), `stream: true` or `false`, optional `tools` (function definitions in OpenAI shape). We map our internal messages (with `tool_name` for tool results) to OpenAI format and back. Response: SSE when streaming; we parse `data:` lines and accumulate content and `tool_calls` into a single `ChatResponse`. Tools / function calling: payload matches OpenAI (assistant `tool_calls` with `id`, `function.name`, `function.arguments`; tool role messages with `tool_call_id`).
- **`GET /v1/models`** — We call this when endpoint type is openai for model discovery at startup; response includes model `id` and optionally other fields. We store the result and expose it in the gateway’s WebSocket **status** response to clients under the key **`lmStudioModels`** (to distinguish from `ollamaModels` and other backends).
- **Streaming** — We use `stream: true` for chat when endpoint type is openai; the response is SSE and we accumulate chunks into one `ChatResponse` for the agent loop.

### Model Lifecycle

- **`GET /v1/models`** (openai) / **`GET /api/v1/models`** (native) — **Used.** List models at startup; gateway exposes result in WebSocket status as `lmStudioModels`.
- **`POST /api/v1/models/load`** — **Not used.** Load a model (e.g. `model`, optional `context_length`, `flash_attention`). Could add “load model” from desktop or CLI for operators who manage models.
- **`POST /api/v1/models/unload`** — **Not used.** Unload a model by `instance_id`. Could add “unload model” from desktop or CLI to free memory or switch models.

### Possible Future Use

- **Streaming to the channel** — Use `chat_stream()` and send deltas to Telegram (or other channels) as they arrive instead of waiting for the full reply. For native endpoint, would require implementing a parser for native SSE events (`chat.start`, `message.delta`, `chat.end`).
- **Native streaming** — Parse the native API’s SSE events for streaming when endpoint type is native; would need a dedicated parser (different format from OpenAI’s `data:` JSON chunks).
- **Native stateful chats and MCP** — When `endpointType: "native"`, we do not send `previous_response_id` (stateful continuation) or `integrations` (MCP). MCP is not supported in this codebase; stateful continuation could be added if needed.
- **Anthropic-compatible `/v1/messages`** — Not used; we use chat completions only. Could add if we need compatibility with Anthropic-style clients.
- **Per-request or per-model options** — Pass `temperature`, `max_tokens`, etc., from config or from the client when supported by the endpoint to better control tool use and length.
- **Model management** — Optional use of load/unload API (e.g. “load model” from desktop, or unload to free memory) for operators who manage models via the desktop or CLI.
