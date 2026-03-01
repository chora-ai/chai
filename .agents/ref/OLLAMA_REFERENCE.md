# Ollama Reference

Reference for how the Ollama API is used in this codebase, what the full API offers, and how Ollama differs from hosted APIs (OpenAI, Anthropic). Use this when extending LLM support or aligning with Ollama’s full capabilities.

## Purpose and How to Use

- **Purpose:** Document current Ollama usage, list Ollama API capabilities we do not yet use, and summarize differences from hosted chat APIs.
- **How to use:** When adding features (e.g. generation options, embeddings, model lifecycle, streaming to the channel), consult this doc.

## Official Ollama API

- **Docs:** https://docs.ollama.com/api  
- **Base URL (local):** `http://localhost:11434` (default); configurable via `OllamaClient::new(base_url)`.  
- **Chat:** https://docs.ollama.com/api/chat  
- **Generate (prompt-only):** https://docs.ollama.com/api/generate  
- **Embeddings:** https://docs.ollama.com/api/embed  
- **Model lifecycle:** https://docs.ollama.com/api/pull, https://docs.ollama.com/api/tags, https://docs.ollama.com/api/delete, https://docs.ollama.com/api/copy  

## Current Usage in the Codebase

### Client and Configuration

- **`crates/lib/src/llm/ollama.rs`** — Single Ollama HTTP client.
- **`OllamaClient::new(base_url: Option<String>)`** — Default base URL `http://127.0.0.1:11434`; no auth (local only).
- **Config** — `agents.defaultModel` in config (e.g. `llama3.2:latest`, `qwen3:8b`). Model name must match `ollama list` exactly (no extra segments like `:latest` unless that tag exists). See `resolve_model()` in the gateway and fallback in the agent when the configured value is empty.

### Endpoints Used

| Endpoint | Method | Use |
|----------|--------|-------------|
| **`/api/tags`** | GET | `list_models()` — Discover available models at gateway startup; result stored in gateway state and exposed in WebSocket `status` as `ollamaModels`. |
| **`/api/chat`** | POST | `chat()` (non-streaming) and `chat_stream()` — Agent turn: messages (system + history), optional `tools`, `stream: true/false`. No `options`, `keep_alive`, `format`, or `think` sent. |

### Request/Response Shapes (What We Send)

- **Chat request:** `model`, `messages` (array of `{ role, content?, tool_calls?, tool_name? }`), `stream`, `tools` (optional array of function definitions). We do **not** send: `options`, `keep_alive`, `format`, `think`, `logprobs`, `top_logprobs`.
- **Messages:** System message is built in the agent and inserted at index 0; then session history (user, assistant, tool). Tool results use `role: "tool"` and `tool_name` (Ollama’s name for the tool this result is for).
- **Streaming:** When `on_chunk` is provided, we use `chat_stream()`: POST with `stream: true`, parse NDJSON line-by-line, call `on_chunk` for each content delta, and collect tool_calls from the last chunk that contains them. Non-streaming is used for the main agent path (no streaming to the channel in the current implementation).

### Where Ollama Is Referenced

- **Gateway server** — Holds `OllamaClient` and `ollama_models`; resolves model via `resolve_model(config.agents.default_model)`; calls `agent::run_turn(..., ollama_client, model, ...)` for inbound messages and WebSocket `agent` requests.
- **Agent** — `run_turn()` builds messages and calls `ollama.chat()` or `ollama.chat_stream()` with the model name from config (backend is chosen via `agents.defaultBackend`; model id from `agents.default_model` is passed as-is). Handles tool_calls and re-calls up to a fixed max iterations.
- **Tools** — Skills with a `tools.json` descriptor (e.g. notesmd-cli, obsidian) expose Ollama-format `ToolDefinition` (type, function with name, description, parameters); the generic executor runs tool calls via the descriptor allowlist (and optional scripts when `skills.allowScripts`). Tool results are sent back as assistant/tool messages.

## Ollama API Overview

### Chat (`POST /api/chat`)

- **What we use:** `model`, `messages`, `stream`, `tools`.  
- **What we don’t use:**
  - **`options`** — Runtime controls: `temperature`, `top_p`, `top_k`, `num_ctx`, `num_predict`, `seed`, `stop`, etc. Would allow per-request or per-model tuning (e.g. lower temperature for tool-heavy flows).
  - **`keep_alive`** — How long to keep the model in memory (e.g. `5m`, `0` to unload). We never send it; Ollama uses its default. Useful to reduce memory when switching models or to keep a model warm.
  - **`format`** — `"json"` or a JSON schema for structured output. Could be used for stricter tool/output shapes.
  - **`think`** — For reasoning models: `true` or `"high"`/`"medium"`/`"low"` to get separate thinking output. Not used in the current implementation.
  - **`logprobs` / `top_logprobs`** — Token-level log probabilities. Not used.

### Generate (`POST /api/generate`)

- **Not used.** Single-prompt completion (no multi-turn messages). Different from chat: `prompt`, optional `system`, `context` (for follow-ups), `images`, `format`, `options`, `keep_alive`, `stream`. Could support simple “one prompt → one response” flows or backward compatibility with prompt-only clients; the implementation is message-based and uses `/api/chat` only.

### Embeddings (`POST /api/embed`)

- **Not used.** Request: `model`, `input` (string or array of strings); optional `truncate`, `dimensions`, `keep_alive`, `options`. Returns `embeddings` (array of vectors). Would enable semantic search, RAG, or similarity over local data if we add those features.

### Model Lifecycle

- **`GET /api/tags`** — **Used.** Lists local models (name, size, etc.).
- **`POST /api/pull`** — **Not used.** Download a model (streaming or not). Could add “ensure model exists” or “install model from UI/CLI.”
- **`DELETE /api/delete`** — **Not used.** Remove a model. Could be used from an admin or config UI.
- **`POST /api/copy`** — **Not used.** Copy a model to a new name. Niche for workflows that duplicate models.
- **`POST /api/show`** — **Not used.** Model details (e.g. parameters, template). Could power model info in the desktop or docs.

### Possible Future Use

- **Per-request or per-model `options`** — Pass `temperature`, `num_ctx`, `num_predict`, etc., from config or from the client to better control tool use and length.
- **`keep_alive`** — Config or UI to unload model after idle (`keep_alive: 0`) or keep it warm for a period; helps when switching models or saving memory.
- **Streaming to the channel** — Use `chat_stream()` and send deltas to Telegram (or other channels) as they arrive instead of waiting for the full reply.
- **Structured output** — Use `format` (e.g. JSON schema) where we need strict tool or response shape.
- **Thinking/reasoning** — If we support reasoning models, expose `think` and optionally show or store “thinking” separately from the final reply.
- **Embeddings** — Add `POST /api/embed` for local RAG, semantic search over notes, or similarity features.
- **Model management** — Optional use of pull/delete/show (e.g. “add model” from desktop, or show model details) for operators who manage models via the desktop or CLI.
