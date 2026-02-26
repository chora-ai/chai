# Ollama Reference

Reference for how the Ollama API is used in this codebase, what the full API offers, and how Ollama differs from hosted APIs (OpenAI, Anthropic). Use this when extending LLM support or aligning with Ollama’s full capabilities.

## Purpose and How to Use

- **Purpose:** Document current Ollama usage in Chai, list Ollama API capabilities we do not yet use, and summarize differences from hosted chat APIs.
- **How to use:** When adding features (e.g. generation options, embeddings, model lifecycle, streaming to the channel), consult this doc and the [official Ollama API docs](https://docs.ollama.com/api).

## Official Ollama API

- **Docs:** https://docs.ollama.com/api  
- **Base URL (local):** `http://localhost:11434` (default); configurable via `OllamaClient::new(base_url)`.  
- **Chat:** https://docs.ollama.com/api/chat  
- **Generate (prompt-only):** https://docs.ollama.com/api/generate  
- **Embeddings:** https://docs.ollama.com/api/embed  
- **Model lifecycle:** https://docs.ollama.com/api/pull, https://docs.ollama.com/api/tags, https://docs.ollama.com/api/delete, https://docs.ollama.com/api/copy  

---

## Current Usage in the Codebase

### Client and Configuration

- **`crates/lib/src/llm/ollama.rs`** — Single Ollama HTTP client.
- **`OllamaClient::new(base_url: Option<String>)`** — Default base URL `http://127.0.0.1:11434`; no auth (local only).
- **Config** — `agents.defaultModel` in config (e.g. `llama3.2:latest`, `smollm2:1.7b`). Model name must match `ollama list` exactly (no extra segments like `:latest` unless that tag exists). See `resolve_model()` in the gateway and fallback in the agent when the configured value is empty.

### Endpoints Used

| Endpoint | Method | Use in Chai |
|----------|--------|-------------|
| **`/api/tags`** | GET | `list_models()` — Discover available models at gateway startup; result stored in gateway state and exposed in WebSocket `status` as `ollamaModels`. |
| **`/api/chat`** | POST | `chat()` (non-streaming) and `chat_stream()` — Agent turn: messages (system + history), optional `tools`, `stream: true/false`. No `options`, `keep_alive`, `format`, or `think` sent. |

### Request/Response Shapes (What We Send)

- **Chat request:** `model`, `messages` (array of `{ role, content?, tool_calls?, tool_name? }`), `stream`, `tools` (optional array of function definitions). We do **not** send: `options`, `keep_alive`, `format`, `think`, `logprobs`, `top_logprobs`.
- **Messages:** System message is built in the agent and inserted at index 0; then session history (user, assistant, tool). Tool results use `role: "tool"` and `tool_name` (Ollama’s name for the tool this result is for).
- **Streaming:** When `on_chunk` is provided, we use `chat_stream()`: POST with `stream: true`, parse NDJSON line-by-line, call `on_chunk` for each content delta, and collect tool_calls from the last chunk that contains them. Non-streaming is used for the main agent path (no streaming to the channel in the current POC).

### Where Ollama Is Referenced

- **Gateway server** — Holds `OllamaClient` and `ollama_models`; resolves model via `resolve_model(config.agents.default_model)`; calls `agent::run_turn(..., ollama_client, model, ...)` for inbound messages and WebSocket `agent` requests.
- **Agent** — `run_turn()` builds messages, strips optional `ollama/` prefix from model name, then calls `ollama.chat()` or `ollama.chat_stream()` with the chosen model; handles tool_calls and re-calls up to a fixed max iterations.
- **Tools** — Obsidian and notesmd-cli skills expose Ollama-format `ToolDefinition` (type, function with name, description, parameters) and tool results are sent back as assistant/tool messages.

---

## Full Ollama API (Relevant to Chai)

### Chat (`POST /api/chat`)

- **What we use:** `model`, `messages`, `stream`, `tools`.  
- **What we don’t use:**
  - **`options`** — Runtime controls: `temperature`, `top_p`, `top_k`, `num_ctx`, `num_predict`, `seed`, `stop`, etc. Would allow per-request or per-model tuning (e.g. lower temperature for tool-heavy flows).
  - **`keep_alive`** — How long to keep the model in memory (e.g. `5m`, `0` to unload). We never send it; Ollama uses its default. Useful to reduce memory when switching models or to keep a model warm.
  - **`format`** — `"json"` or a JSON schema for structured output. Could be used for stricter tool/output shapes.
  - **`think`** — For reasoning models: `true` or `"high"`/`"medium"`/`"low"` to get separate thinking output. Not used today.
  - **`logprobs` / `top_logprobs`** — Token-level log probabilities. Not used.

### Generate (`POST /api/generate`)

- **Not used.** Single-prompt completion (no multi-turn messages). Different from chat: `prompt`, optional `system`, `context` (for follow-ups), `images`, `format`, `options`, `keep_alive`, `stream`. Could support simple “one prompt → one response” flows or backward compatibility with prompt-only clients; Chai is message-based and uses `/api/chat` only.

### Embeddings (`POST /api/embed`)

- **Not used.** Request: `model`, `input` (string or array of strings); optional `truncate`, `dimensions`, `keep_alive`, `options`. Returns `embeddings` (array of vectors). Would enable semantic search, RAG, or similarity over local data if we add those features.

### Model Lifecycle

- **`GET /api/tags`** — **Used.** Lists local models (name, size, etc.).
- **`POST /api/pull`** — **Not used.** Download a model (streaming or not). Could add “ensure model exists” or “install model from UI/CLI.”
- **`DELETE /api/delete`** — **Not used.** Remove a model. Could be used from an admin or config UI.
- **`POST /api/copy`** — **Not used.** Copy a model to a new name. Niche for workflows that duplicate models.
- **`POST /api/show`** — **Not used.** Model details (e.g. parameters, template). Could power model info in the desktop or docs.

---

## Possible Future Use (Alignment With Ollama)

- **Per-request or per-model `options`** — Pass `temperature`, `num_ctx`, `num_predict`, etc., from config or from the client to better control tool use and length.
- **`keep_alive`** — Config or UI to unload model after idle (`keep_alive: 0`) or keep it warm for a period; helps when switching models or saving memory.
- **Streaming to the channel** — Use `chat_stream()` and send deltas to Telegram (or other channels) as they arrive instead of waiting for the full reply.
- **Structured output** — Use `format` (e.g. JSON schema) where we need strict tool or response shape.
- **Thinking/reasoning** — If we support reasoning models, expose `think` and optionally show or store “thinking” separately from the final reply.
- **Embeddings** — Add `POST /api/embed` for local RAG, semantic search over notes, or similarity features.
- **Model management** — Optional use of pull/delete/show (e.g. “add model” from desktop, or show model details) for operators who manage models via Chai.

---

## Ollama vs Hosted APIs (OpenAI, Anthropic)

### Deployment and Auth

- **Ollama:** Local (or self-hosted) server; default no API key; base URL typically `http://localhost:11434`. No per-token billing; models are pulled and run on the same machine.
- **OpenAI / Anthropic:** Remote APIs; auth via API keys (or similar); pay per token/request. Different base URLs and headers.

### Statelessness

- **Both** are stateless: each request is independent. The client (Chai) is responsible for sending full conversation history and system prompt on every call. We do that in the agent by building the message list from the session store and prepending the system context.

### Streaming

- **Ollama:** `stream: true` returns **NDJSON** (newline-delimited JSON): each line is a JSON object with fields like `message`, `message.content`, `done`. Content is incremental; tool_calls may appear in the last chunk(s). Default is streaming in the API; we explicitly set `stream: false` for the main chat path.
- **OpenAI:** SSE (Server-Sent Events) with a different chunk shape (e.g. `choices[0].delta.content`). Stream must be requested explicitly.
- **Anthropic:** Similar idea with their own streaming format. So: same idea (stream tokens), different wire format and field names.

### Message and Tool Format

- **Ollama chat:** `messages[]` with `role`, `content`, optional `tool_calls`, and for tool results `tool_name` (Ollama’s name; OpenAI uses a different structure for tool results). Supports multimodal via `images` on a message.
- **OpenAI:** `messages[]` with `role` and `content` (can be array of content blocks for text + images). Tool results use a different schema (e.g. `tool_call_id`). Function/tool definitions are similar in spirit but with different naming (e.g. `function` vs `tools`).
- **Anthropic:** Again similar roles and content; tools and tool_use have their own schema. So when adding multiple backends, we’ll need a small adapter layer: internal message/tool format → Ollama shape vs OpenAI shape vs Anthropic shape.

### Model Selection

- **Ollama:** Model is a string that must match a **local** model name (e.g. from `GET /api/tags`). No separate “model list” from a cloud; models are pulled and then appear in `/api/tags`.
- **OpenAI / Anthropic:** Model is an identifier for a **hosted** model (e.g. `gpt-4o`, `claude-3-5-sonnet`). No local “load/unload”; different models are just different endpoints/models in the same API.

### Capabilities We Rely On (Ollama)

- **Chat with history** — Same idea as hosted APIs.
- **Tool/function calling** — Ollama supports it for compatible models; we send `tools` and parse `tool_calls` in the response. Behavior is model-dependent (e.g. which models support tools).
- **System message** — We send it as the first message; Ollama doesn’t have a separate “system” parameter in the same way as some hosted APIs, but the first message with role `system` is supported.

### Capabilities That Differ or Are Ollama-Specific

- **`keep_alive`** — Controls in-process model lifetime. No direct equivalent in hosted APIs (they don’t “load” a model on your machine).
- **`/api/pull`** — Download models; no analogue for hosted APIs (you don’t download their models).
- **`/api/embed`** — Local embeddings; hosted APIs have their own embedding endpoints and pricing.
- **No API key** — By default Ollama doesn’t require auth; hosted APIs do. If we add remote Ollama or ollama.com, auth may be required.

---

## Summary Table

| Area | Chai (current) | Ollama full API | Hosted (OpenAI/Anthropic) |
|------|----------------|-----------------|---------------------------|
| **Base** | `OllamaClient`, default local URL | Same | Remote URL + API key |
| **Chat** | `/api/chat` with model, messages, stream, tools | + options, keep_alive, format, think, logprobs | Different URL/params, similar roles/messages/tools |
| **Streaming** | Implemented but not used to channel; NDJSON | Same | SSE or similar, different format |
| **Models** | `GET /api/tags` at startup; one default model from config | + pull, delete, copy, show | Model ID only, no local lifecycle |
| **Generate** | Not used | `/api/generate` (prompt-only) | Often “completions” vs “chat” |
| **Embed** | Not used | `/api/embed` | Separate embedding APIs |
| **State** | Client sends full history + system each time | N/A (stateless) | Same (stateless) |

This reference should be updated when we add new Ollama options, support another backend (e.g. LM Studio, Hugging Face), or change how we call the Ollama API.
