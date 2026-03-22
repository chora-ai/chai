# Epic: LLM Services and API Alignment

**Summary** — Align the gateway with multiple LLM services (local, self-hosted, third-party) via a consistent internal message/tool format and provider-specific clients or adapters.

**Status** — **Phase 1 complete** for all shipped backends (Ollama-native and OpenAI-compat families). **Phase 2** (Anthropic and Google first-party APIs) is **specified and tracked**; implementation is **not** done. See [Phase 2: Anthropic and Google](#phase-2-anthropic-and-google).

## Goal

Support a variety of LLM backends so users can run models locally (Ollama, LM Studio), on their own infrastructure (LocalAI, vLLM, Hugging Face), or via third-party APIs (OpenAI, Anthropic, Google). The gateway should present a single agent interface while translating between each provider's request/response shape and the internal message and tool format.

## Current State

- **Backends implemented:** **Ollama** (native API), **LM Studio** (`lms`, OpenAI-compat), **vLLM** (`vllm`, OpenAI-compat), **NVIDIA NIM** (`nim`, hosted OpenAI-compat), **OpenAI** (`openai`, OpenAI API or compatible base URL), **Hugging Face** (`hf`, OpenAI-compat Inference Endpoints / TGI / similar). The agent uses a common **`Provider`** trait; **`agents.defaultProvider`** and **`agents.defaultModel`** select which client and model are used.
- **Shared HTTP layer:** LM Studio, vLLM, OpenAI, Hugging Face, and NIM chat paths use the shared **`openai_compat`** module (`OpenAiCompatClient` or equivalent) for `/v1/chat/completions` and `/v1/models` where applicable. **`openai.rs`** / **`hf.rs`** are thin wrappers with provider defaults and error types; they are not merged into **`openai_compat`** (see [OPENAI_REFERENCE.md](ref/OPENAI_REFERENCE.md)).
- **Model discovery:** Gateway discovers models from configured backends at startup (per **`agents.enabledProviders`** rules) and exposes **`ollamaModels`**, **`lmsModels`**, **`vllmModels`**, **`nimModels`**, **`openaiModels`**, and **`hfModels`** in WebSocket **`status`**.
- **Single backend per run:** One default backend and model for the entire agent loop; no per-request or per-step backend selection (orchestration delegation is separate; see [EPIC_ORCHESTRATION.md](EPIC_ORCHESTRATION.md)).

## Scope

- **Phase 1 (done):** Ollama-native and OpenAI-compat backends listed above; message/tool documentation; compatibility for LocalAI, llama.cpp, and Venice (OpenAI-compat) when they expose those API families; deployment and streaming notes in ref docs.
- **Phase 2 (tracked, not implemented):** First-party **Anthropic** and **Google** APIs — see below and [EPIC_API_ALIGNMENT_PHASE_2.md](EPIC_API_ALIGNMENT_PHASE_2.md).
- **Out of scope:** Orchestrator–worker delegation as an API-alignment deliverable; see [EPIC_ORCHESTRATION.md](EPIC_ORCHESTRATION.md).

## Compatibility Targets (No Dedicated Provider Id)

These stacks are **tracked** here so expectations stay clear: Chai does **not** need a new canonical **`defaultProvider`** value when the server speaks an API family we already support.

- **LocalAI** — **Done (compatibility only).** **Ollama-compatible** deployment → use **`"ollama"`** and **`providers.ollama.baseUrl`**. **OpenAI-compatible** deployment → use **`"vllm"`** and **`providers.vllm.baseUrl`** pointing at that server’s **`/v1`** base (same client as vLLM). A future **`localai`** provider id would be optional UX only, not a new wire format.
- **llama.cpp** — **OpenAI-compatible** HTTP (e.g. **`llama-server`** with OpenAI-style routes when enabled) → use an existing OpenAI-compat path: typically **`"vllm"`** or **`"lms"`** with **`providers.*.baseUrl`** set to the server’s **`/v1`** origin, same as any other OpenAI-shaped endpoint. **Not** tracked as a separate shipped backend until we need one. A **non–OpenAI-compat**, **non–Ollama** HTTP API from llama.cpp would require **new adapter work** and would be a distinct epic item if prioritized (not current scope).
- **Venice** — **OpenAI-compatible** hosted API → use **`"openai"`** with **`providers.openai.baseUrl`** set to Venice’s base (e.g. **`https://api.venice.ai/api/v1`**) and a Venice API key via **`OPENAI_API_KEY`** / **`providers.openai.apiKey`**. No dedicated **`venice`** provider id; see [OPENAI_REFERENCE.md](ref/OPENAI_REFERENCE.md) and [Venice docs](https://docs.venice.ai/overview/about-venice). Venice-specific request fields (e.g. **`venice_parameters`**) are **not** sent by Chai’s client today.

## Phase 1 Requirements

- [x] Ollama integrated (native API: `/api/tags`, `/api/chat`).
- [x] LM Studio integrated (OpenAI-compat: `/v1/*`).
- [x] LocalAI: **Ollama mode** — use **`agents.defaultProvider`**: **`"ollama"`** and optional **`providers.ollama.baseUrl`** (no code change). **OpenAI-compat mode** — use **`"vllm"`** and **`providers.vllm.baseUrl`** pointing at LocalAI’s `/v1` base (same shared adapter as vLLM); see [README.md](../README.md) and [HUGGINGFACE_REFERENCE.md](ref/HUGGINGFACE_REFERENCE.md).
- [x] vLLM: **`VllmClient`** + shared **`openai_compat`**; see [VLLM_REFERENCE.md](ref/VLLM_REFERENCE.md).
- [x] Hugging Face (TGI / Inference Endpoints / OpenAI-compat): **`HfClient`** + **`openai_compat`**; **`providers.hf.baseUrl`**, **`HF_API_KEY`**; see [HUGGINGFACE_REFERENCE.md](ref/HUGGINGFACE_REFERENCE.md).
- [x] OpenAI: **`OpenAiClient`** + **`openai_compat`**; **`OPENAI_API_KEY`**, optional **`providers.openai.baseUrl`**; see [OPENAI_REFERENCE.md](ref/OPENAI_REFERENCE.md).
- [x] Message and tool format translation **implemented** for every **Phase 1** backend and **documented** in this epic (see [Message and Tool Translation](#message-and-tool-translation)) and per-backend [ref/](ref/) docs.

## Phase 2: Anthropic and Google

**Goal:** Add **`Provider`** implementations for first-party **Anthropic (Claude)** and **Google (Gemini)** HTTP APIs (not OpenAI-shaped proxies).

**Specification:** [EPIC_API_ALIGNMENT_PHASE_2.md](EPIC_API_ALIGNMENT_PHASE_2.md) — API families, adapter responsibilities, implementation checklist.

**Requirements (not yet satisfied):**

- [ ] **Anthropic:** Client + adapter + config + gateway **`status`** discovery (or documented static catalog) + desktop + docs.
- [ ] **Google (Gemini):** Same as above for the chosen Gemini API surface.

Until Phase 2 ships, [SERVICES_AND_MODELS.md](SERVICES_AND_MODELS.md) continues to list Claude and Gemini under **planned**.

## Technical Reference

### Concepts

- **Internal format** — The gateway uses a single message and tool shape for the agent (e.g. messages with `role`, `content`, optional `tool_calls`; tool results keyed by **`tool_name`**). Each backend adapter translates between this and the provider's request/response.
- **API families** — **Ollama-native**: `/api/chat`, `/api/tags`; tool results use `tool_name`. **OpenAI-compat**: `/v1/chat/completions`, `/v1/models`; tool results use **`tool_call_id`**; assistant `tool_calls` have `id`, `function.name`, `function.arguments`. **Provider-specific** (Claude, Gemini): own request/response and tool schema; need a dedicated adapter each (Phase 2).
- **Statelessness** — All backends are stateless: the client sends full history and system prompt every call. The agent builds the message list from the session store and prepends system context; no backend-specific state.

### Message and Tool Translation

| Backend (`defaultProvider`) | API family | Where mapping lives | Documentation |
|-----------------------------|------------|---------------------|---------------|
| `ollama` | Ollama-native | `OllamaClient` | [OLLAMA_REFERENCE.md](ref/OLLAMA_REFERENCE.md) |
| `lms` | OpenAI-compat | `LmsClient` → `OpenAiCompatClient` | [LM_STUDIO_REFERENCE.md](ref/LM_STUDIO_REFERENCE.md) |
| `vllm` | OpenAI-compat | `VllmClient` → `OpenAiCompatClient` | [VLLM_REFERENCE.md](ref/VLLM_REFERENCE.md) |
| `hf` | OpenAI-compat | `HfClient` → `OpenAiCompatClient` | [HUGGINGFACE_REFERENCE.md](ref/HUGGINGFACE_REFERENCE.md) |
| `nim` | OpenAI-compat (hosted) | `NimClient` (dedicated types; same wire ideas) | [NVIDIA_NIM_REFERENCE.md](ref/NVIDIA_NIM_REFERENCE.md) |
| `openai` | OpenAI-compat | `OpenAiClient` → `OpenAiCompatClient` | [OPENAI_REFERENCE.md](ref/OPENAI_REFERENCE.md) |

OpenAI-compat adapters map internal tool results (**`tool_name`**) to OpenAI **`tool_call_id`** on the wire and reverse for assistant tool calls. Ollama uses **`tool_name`** end-to-end on its native API.

### Why Alignment Matters

- **Single agent interface** — One `run_turn` and one `Provider` trait regardless of provider; the agent loop does not branch on "which API." Adding a backend is a new client + adapter, not a new agent path.
- **Capabilities we rely on** — Chat with history, tool/function calling, and a system message as the first message. Backends that support these (Ollama, OpenAI-compat family) fit the same adapter pattern; provider-specific APIs (Claude, Gemini) need a translation layer from our internal format.
- **Deployment and auth** — Local/self-hosted often need only base URL; remote APIs need API keys and correct base URLs. The adapter hides this behind config.

### By API Family

| Family | Examples | Status | Adapter / notes |
|--------|----------|--------|-----------------|
| **Ollama-native** | Ollama, LocalAI (Ollama mode), llama.cpp if same API | Phase 1 done | **`OllamaClient`**. Same path when server exposes `/api/chat`, `/api/tags`. |
| **OpenAI-compat** | LM Studio, OpenAI, Hugging Face TGI/IE, vLLM, LocalAI (OpenAI mode), NIM | Phase 1 done | **`OpenAiCompatClient`** (+ thin provider wrappers); `tool_name` ↔ `tool_call_id`. |
| **Provider-specific** | Claude (Anthropic), Gemini (Google) | Phase 2 | See [EPIC_API_ALIGNMENT_PHASE_2.md](EPIC_API_ALIGNMENT_PHASE_2.md). |

For the full list of services, model families, and configuration, see [SERVICES_AND_MODELS.md](SERVICES_AND_MODELS.md).

### Implementation Notes

- The **`Provider`** trait and a single **`agents.defaultProvider`** + **`agents.defaultModel`** keep one backend per run; each backend implements the same trait and performs its own message/tool translation.
- **Ollama or Ollama-compatible** servers: use **`OllamaClient`** and set default backend/model.
- **OpenAI-style** backends: shared **`openai_compat`** layer; per-provider config (base URL, API key) on **`ProvidersConfig`**.
- **Claude, Gemini (Phase 2):** New client and adapter each unless a common abstraction is introduced; internal format remains the contract for the agent.
- **Streaming:** The main agent path is non-streaming today. Streaming format differs by API family (Ollama: NDJSON; OpenAI-compat: SSE); see ref docs when extending streaming to the channel.
- **Backend-specific features:** Adapters need only support chat, tools, and system message. Features such as model load/unload (LM Studio), keep_alive or embeddings (Ollama), or provider-specific options are not required for the agent loop; see ref docs for what each API offers.
