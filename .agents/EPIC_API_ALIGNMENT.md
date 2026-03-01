# Epic: LLM Services and API Alignment

**Summary** — Align the gateway with multiple LLM services (local, self-hosted, third-party) via a consistent internal message/tool format and provider-specific clients or adapters.

**Status** — Partially done (Ollama and LM Studio implemented; OpenAI and others planned).

## Goal

Support a variety of LLM backends so users can run models locally (Ollama, LM Studio), on their own infrastructure (LocalAI, vLLM, Hugging Face), or via third-party APIs (OpenAI, Anthropic, Google). The gateway should present a single agent interface while translating between each provider's request/response shape and the internal message and tool format.

## Current State

- **Backends implemented:** Ollama (native API), LM Studio (OpenAI-compat or native API). The agent uses a common **`LlmBackend`** trait; **`agents.defaultBackend`** and **`agents.default_model`** select which client and model are used.
- **Model discovery:** Gateway discovers models from both backends at startup and exposes `ollamaModels` and `lmStudioModels` in WebSocket `status`.
- **Single backend per run:** One default backend and model for the entire agent loop; no per-request or per-step backend selection.

## Scope

- **In scope:** Adding support for additional backends (OpenAI, Hugging Face TGI/Inference Endpoints, LocalAI, vLLM, Claude, Gemini) via clients or adapters; documenting deployment, streaming, message/tool format, and model selection by service; ensuring config and implementation implications are clear.
- **Out of scope:** Orchestrator–worker delegation (multiple models per turn); see [EPIC_ORCHESTRATION.md](EPIC_ORCHESTRATION.md).

## Requirements

- [x] Ollama integrated (native API: `/api/tags`, `/api/chat`).
- [x] LM Studio integrated (OpenAI-compat or native: `/v1/*` or `/api/v1/*`).
- [ ] LocalAI: works in Ollama mode with no code change; OpenAI-compat mode via shared adapter.
- [ ] vLLM: document or implement via OpenAI-compat adapter path.
- [ ] Hugging Face (TGI / Inference Endpoints) via same adapter path as OpenAI where applicable.
- [ ] OpenAI (or shared OpenAI-style adapter) for chat + tools; API key and base URL config.
- [ ] Claude (Anthropic), Gemini (Google): provider-specific client and adapter, or common abstraction.
- [ ] Message/tool format translation documented and implemented for each new backend.

## Technical Reference

### Concepts

- **Internal format** — The gateway uses a single message and tool shape for the agent (e.g. messages with `role`, `content`, optional `tool_calls`; tool results keyed by **`tool_name`**). Each backend adapter translates between this and the provider's request/response.
- **API families** — **Ollama-native**: `/api/chat`, `/api/tags`; tool results use `tool_name`. **OpenAI-compat**: `/v1/chat/completions`, `/v1/models`; tool results use **`tool_call_id`**; assistant `tool_calls` have `id`, `function.name`, `function.arguments`. **Provider-specific** (Claude, Gemini): own request/response and tool schema; need a dedicated adapter each.
- **Statelessness** — All backends are stateless: the client sends full history and system prompt every call. The agent builds the message list from the session store and prepends system context; no backend-specific state.

### Why Alignment Matters

- **Single agent interface** — One `run_turn` and one `LlmBackend` trait regardless of provider; the agent loop does not branch on "which API." Adding a backend is a new client + adapter, not a new agent path.
- **Capabilities we rely on** — Chat with history, tool/function calling, and a system message as the first message. Backends that support these (Ollama, LM Studio openai, OpenAI, many self-hosted) fit the same adapter pattern; provider-specific APIs (Claude, Gemini) need a translation layer from our internal format.
- **Deployment and auth** — Local/self-hosted (Ollama, LM Studio, LocalAI, vLLM) often need only base URL; remote (OpenAI, Anthropic) require API key and pay-per-token. The adapter hides this behind config (base URL, API key, endpoint type).

### By API Family

| Family | Examples | Status | Adapter / notes |
|--------|----------|--------|-----------------|
| **Ollama-native** | Ollama, LocalAI (Ollama mode), llama.cpp if same API | Implemented (Ollama) | Internal format matches; no translation. Others: same client path when server exposes `/api/chat`, `/api/tags`. |
| **OpenAI-compat** | LM Studio, OpenAI, Hugging Face TGI, vLLM, LocalAI (OpenAI mode) | Implemented (LM Studio); rest planned | Translate `tool_name` ↔ `tool_call_id`; one shared client/adapter with provider config (base URL, API key). |
| **Provider-specific** | Claude (Anthropic), Gemini (Google) | Planned | Separate client and adapter per provider; internal format → provider shape. |

For the full list of services, model families, and test procedures, see [SERVICES_AND_MODELS.md](SERVICES_AND_MODELS.md).

### Implementation Notes

- The **`LlmBackend`** trait and a single **`agents.defaultBackend`** + **`agents.default_model`** keep one backend per run; each backend implements the same trait and performs its own message/tool translation.
- **Ollama or Ollama-compatible** servers: no gateway code change; use existing Ollama client and set default backend/model.
- **OpenAI-style** backends: one shared adapter for chat + tools; add config (base URL, API key) per provider; implement `tool_name` ↔ `tool_call_id` and any streaming differences in that adapter.
- **Claude, Gemini:** New client and adapter each unless a common abstraction is introduced; internal format remains the contract for the agent.
- **Streaming:** The main agent path is non-streaming today. Streaming format differs by API family (Ollama: NDJSON; OpenAI-compat: SSE); see ref docs (e.g. [OLLAMA_REFERENCE.md](ref/OLLAMA_REFERENCE.md), [LM_STUDIO_REFERENCE.md](ref/LM_STUDIO_REFERENCE.md)) when adding streaming support.
- **Backend-specific features:** Adapters need only support chat, tools, and system message. Features such as model load/unload (LM Studio), keep_alive or embeddings (Ollama), or provider-specific options are not required for the agent loop; see ref docs for what each API offers.
