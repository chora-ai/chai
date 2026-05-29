---
status: in-progress
---

# Epic: LLM Services and API Alignment

**Summary** — Align the gateway with multiple LLM services (local, self-hosted, third-party) via a consistent internal message/tool format and provider-specific clients or adapters.

**Status** — **Phase 1 complete** for all shipped backends (Ollama-native and OpenAI-compat families). **Phase 2** (official Anthropic and Google APIs) is **specified below**; implementation is **not** done. See [Phase 2: Anthropic and Google](#phase-2-anthropic-and-google).

## Problem Statement

The gateway needs to support a growing range of LLM backends — local, self-hosted, and third-party — each with its own HTTP API shape, authentication model, and tool-calling wire format. Without a consistent internal message and tool format and a per-provider adapter layer, adding a new backend would require changes to the agent loop itself, and the agent could not switch providers without code-level branching. This epic establishes the alignment layer so that the agent loop stays provider-agnostic.

## Goal

Support a variety of LLM backends so users can run models locally (Ollama, LM Studio), on their own infrastructure (LocalAI, vLLM, Hugging Face), or via third-party APIs (OpenAI, Anthropic, Google). The gateway should present a single agent interface while translating between each provider's request/response shape and the internal message and tool format.

## Current State

- **Backends implemented:** **Ollama** (native API), **LM Studio** (`lms`, OpenAI-compat), **vLLM** (`vllm`, OpenAI-compat), **NVIDIA NIM** (`nim`, hosted OpenAI-compat), **OpenAI** (`openai`, OpenAI API or compatible base URL), **Hugging Face** (`hf`, OpenAI-compat Inference Endpoints / TGI / similar). The agent uses a common **`Provider`** trait; **`agents.defaultProvider`** and **`agents.defaultModel`** select which client and model are used.
- **Shared HTTP layer:** LM Studio, vLLM, OpenAI, Hugging Face, and NIM chat paths use the shared **`openai_compat`** module (`OpenAiCompatClient` or equivalent) for `/v1/chat/completions` and `/v1/models` where applicable. **`openai.rs`** / **`hf.rs`** are thin wrappers with provider defaults and error types; they are not merged into **`openai_compat`** (see [OPENAI.md](../ref/OPENAI.md)).
- **Model discovery:** Gateway discovers models from configured backends at startup (per **`agents.enabledProviders`** rules) and exposes them under WebSocket **`status.payload.providers`** (per-provider **`models`** arrays).
- **Single backend per run:** One default backend and model for the entire agent loop; no per-request or per-step backend selection (orchestration delegation is separate; see [ORCHESTRATION.md](ORCHESTRATION.md)).

## Scope

### In Scope

- **Phase 1 (done):** Ollama-native and OpenAI-compat backends listed above; message/tool documentation; compatibility for LocalAI, llama.cpp, and Venice (OpenAI-compat) when they expose those API families; deployment and streaming notes in ref docs.
- **Phase 2 (tracked, not implemented):** Official **Anthropic** and **Google** APIs — see [Phase 2: Anthropic and Google](#phase-2-anthropic-and-google).

### Out of Scope

- Orchestrator–worker delegation as an API-alignment deliverable; see [ORCHESTRATION.md](ORCHESTRATION.md).

## Compatibility Targets (No Dedicated Provider Id)

These stacks are **tracked** here so expectations stay clear: Chai does **not** need a new canonical **`defaultProvider`** value when the server speaks an API family we already support.

- **LocalAI** — **Done (compatibility only).** **Ollama-compatible** deployment → use **`"ollama"`** and **`providers.ollama.baseUrl`**. **OpenAI-compatible** deployment → use **`"vllm"`** and **`providers.vllm.baseUrl`** pointing at that server’s **`/v1`** base (same client as vLLM). A future **`localai`** provider id would be optional UX only, not a new wire format.
- **llama.cpp** — **OpenAI-compatible** HTTP (e.g. **`llama-server`** with OpenAI-style routes when enabled) → use an existing OpenAI-compat path: typically **`"vllm"`** or **`"lms"`** with **`providers.*.baseUrl`** set to the server’s **`/v1`** origin, same as any other OpenAI-shaped endpoint. **Not** tracked as a separate shipped backend until we need one. A **non–OpenAI-compat**, **non–Ollama** HTTP API from llama.cpp would require **new adapter work** and would be a distinct epic item if prioritized (not current scope).
- **Venice** — **OpenAI-compatible** hosted API → use **`"openai"`** with **`providers.openai.baseUrl`** set to Venice’s base (e.g. **`https://api.venice.ai/api/v1`**) and a Venice API key via **`OPENAI_API_KEY`** / **`providers.openai.apiKey`**. No dedicated **`venice`** provider id; see [OPENAI.md](../ref/OPENAI.md) and [Venice docs](https://docs.venice.ai/overview/about-venice). Venice-specific request fields (e.g. **`venice_parameters`**) are **not** sent by Chai’s client today.

## Phase 1 Requirements

- [x] Ollama integrated (native API: `/api/tags`, `/api/chat`).
- [x] LM Studio integrated (OpenAI-compat chat: `/v1/chat/completions`; model list: native **`GET …/api/v1/models`** — see [LM_STUDIO.md](../ref/LM_STUDIO.md)).
- [x] LocalAI: **Ollama mode** — use **`agents.defaultProvider`**: **`"ollama"`** and optional **`providers.ollama.baseUrl`** (no code change). **OpenAI-compat mode** — use **`"vllm"`** and **`providers.vllm.baseUrl`** pointing at LocalAI’s `/v1` base (same shared adapter as vLLM); see [README.md](../../README.md) and [VLLM.md](../ref/VLLM.md).
- [x] vLLM: **`VllmClient`** + shared **`openai_compat`**; see [VLLM.md](../ref/VLLM.md).
- [x] Hugging Face (TGI / Inference Endpoints / OpenAI-compat): **`HfClient`** + **`openai_compat`**; **`providers.hf.baseUrl`**, **`HF_API_KEY`**; see [HUGGINGFACE.md](../ref/HUGGINGFACE.md).
- [x] OpenAI: **`OpenAiClient`** + **`openai_compat`**; **`OPENAI_API_KEY`**, optional **`providers.openai.baseUrl`**; see [OPENAI.md](../ref/OPENAI.md).
- [x] Message and tool format translation **implemented** for every **Phase 1** backend and **documented** in this epic (see [Message and Tool Translation](#message-and-tool-translation)) and per-backend [reference docs](../ref/).

## Phase 2: Anthropic and Google

**Status:** **Proposed.** Phase 2 has not yet begun; **`anthropic`** and **`gemini`** are not currently valid **`agents.defaultProvider`** values.

**Goal:** Add **`Provider`** implementations for official **Anthropic (Claude)** and **Google (Gemini)** HTTP APIs (not OpenAI-shaped proxies). Chai supports **`anthropic`** and **`gemini`** as valid **`agents.defaultProvider`** values. Each provider has a dedicated adapter that maps the internal agent contract (full history, system context, tools, **`tool_name`** on tool results) to and from the vendor API. Users targeting official Anthropic or Google endpoints get the same reliable routing and tool correlation as users on the OpenAI path.

### Problem Statement (Phase 2)

Chai's current provider support covers OpenAI-compatible APIs via the **`openai_compat`** module. Claude (Anthropic) and Gemini (Google) use distinct wire formats — different message layouts, tool-calling conventions, and model discovery mechanisms — that cannot be handled by the existing **`openai_compat`** path when users target official Anthropic or Google endpoints. First-class support requires dedicated adapter families for each vendor API.

### Scope (Phase 2)

#### In Scope

- **`anthropic` provider:** adapter, config, model discovery, tool-calling support
- **`gemini` provider:** adapter, config, model discovery, tool-calling support
- Config changes for **`providers.anthropic`** / **`providers.gemini`**, env vars, **`canonical_provider`**
- Orchestration changes: **`ProviderChoice`**, **`ProviderClients`**, **`resolve_model`** fallbacks
- Gateway server changes: client construction, discovery, **`status`** payload keys
- Desktop changes: provider allowlist, model reconciliation, info screen
- User docs and ref docs for both providers
- Tests for both providers

#### Out of Scope

- OpenAI-compatible routing for Anthropic or Google hosted endpoints (these can use the existing **`openai`** or **`vllm`** path with a custom base URL — this is a compatibility route, not a substitute for first-class support)

### Design (Phase 2)

#### Why a Separate Adapter Family

These APIs are **not** OpenAI-compatible chat completions in the narrow sense Chai's **`openai_compat`** module implements. Each has its own:

- Message and role layout (e.g. Anthropic system vs messages; Gemini system instruction and **`contents`** parts).
- Tool / function-calling wire format and IDs for tool invocations and results.
- List-models or catalog discovery (if any).

The internal agent contract stays the same: full history, system context, tools, **`tool_name`** on tool results in session storage; each new **`Provider`** maps that to and from the vendor API.

#### Relationship to OpenAI-Compat

Servers that expose **OpenAI-compatible** HTTP for Claude or Gemini (if hosted that way) could use the existing **`openai`** or **`vllm`** path with a provider base URL; that is a **compatibility** route, not a substitute for first-class Anthropic/Google APIs when users use official endpoints.

### Requirements (Phase 2, Not Yet Satisfied)

- [ ] **Anthropic:** Client + adapter + config + gateway **`status`** discovery (or documented static catalog) + desktop + docs.
- [ ] **Google (Gemini):** Same as above for the chosen Gemini API surface.
- [ ] **`crates/lib/src/config.rs`** — **`providers.anthropic`** / **`providers.gemini`** (or similar), env vars, **`canonical_provider`**
- [ ] **`crates/lib/src/providers/`** — New client modules and **`Provider`** impls for Anthropic and Google
- [ ] **`crates/lib/src/orchestration/`** — **`ProviderChoice`**, **`ProviderClients`**, **`resolve_model`** fallbacks
- [ ] **`crates/lib/src/gateway/server.rs`** — Client construction, discovery, **`status`** payload keys (e.g. **`anthropicModels`**, **`geminiModels`**)
- [ ] **`crates/desktop/`** — Provider allowlist, model reconciliation, info screen
- [ ] User docs — [README.md](../../README.md), ref docs under [`.agents/ref/`](../ref/), [spec/PROVIDERS.md](../spec/PROVIDERS.md), [spec/MODELS.md](../spec/MODELS.md)

Until Phase 2 ships, [spec/PROVIDERS.md](../spec/PROVIDERS.md) continues to list Claude and Gemini under **planned**.

### API Reference (Phase 2)

#### Anthropic (Claude)

- **Docs:** https://docs.anthropic.com/claude/reference/messages_post
- **Typical surface:** Messages API (`POST /v1/messages`), models like `claude-3-5-sonnet-latest`; tools and tool results use Anthropic's **`tool_use`** / **`tool_result`** blocks rather than OpenAI's **`tool_calls`** on the assistant message.
- **Adapter responsibilities:** Map internal **`ChatMessage`** list + tools to Anthropic **`messages`**, **`system`**, and **`tools`**; map assistant output and tool calls back to **`ChatResponse`**; map tool execution results by stable id ↔ internal **`tool_name`** as required by the agent loop.

#### Google (Gemini)

- **Docs:** https://ai.google.dev/api/generate-content (and current model list for your API version).
- **Typical surface:** **`generateContent`** / chat with **`contents`** and **`tools`**; schema differs from both OpenAI and Anthropic.
- **Adapter responsibilities:** Map internal messages and tools to Gemini **`contents`** and tool declarations; map model responses back to **`ChatResponse`**; preserve tool correlation for follow-up turns.

### Related Docs (Phase 2)

- [spec/PROVIDERS.md](../spec/PROVIDERS.md) — Provider configuration spec
- [spec/MODELS.md](../spec/MODELS.md) — Model resolution and fallback spec
- [ref/OPENAI.md](../ref/OPENAI.md) — Reference for the existing OpenAI-compat adapter (useful baseline for new adapters)
- [.testing/README.md](../../.testing/README.md) — Test index

## Technical Reference

### Concepts

- **Internal format** — The gateway uses a single message and tool shape for the agent (e.g. messages with `role`, `content`, optional `tool_calls`; tool results keyed by **`tool_name`**). Each backend adapter translates between this and the provider's request/response.
- **API families** — **Ollama-native**: `/api/chat`, `/api/tags`; tool results use `tool_name`. **OpenAI-compat**: `/v1/chat/completions`, `/v1/models`; tool results use **`tool_call_id`**; assistant `tool_calls` have `id`, `function.name`, `function.arguments`. **Provider-specific** (Claude, Gemini): own request/response and tool schema; need a dedicated adapter each ([Phase 2](#phase-2-anthropic-and-google)).
- **Statelessness** — All backends are stateless: the client sends full history and system prompt every call. The agent builds the message list from the session store and prepends system context; no backend-specific state.

### Message and Tool Translation

| Backend (`defaultProvider`) | API family | Where mapping lives | Documentation |
|---------------------------|------------|---------------------|---------------|
| `ollama` | Ollama-native | `OllamaClient` | [OLLAMA.md](../ref/OLLAMA.md) |
| `lms` | OpenAI-compat | `LmsClient` → `OpenAiCompatClient` | [LM_STUDIO.md](../ref/LM_STUDIO.md) |
| `vllm` | OpenAI-compat | `VllmClient` → `OpenAiCompatClient` | [VLLM.md](../ref/VLLM.md) |
| `hf` | OpenAI-compat | `HfClient` → `OpenAiCompatClient` | [HUGGINGFACE.md](../ref/HUGGINGFACE.md) |
| `nim` | OpenAI-compat (hosted) | `NimClient` (dedicated types; same wire ideas) | [NVIDIA_NIM.md](../ref/NVIDIA_NIM.md) |
| `openai` | OpenAI-compat | `OpenAiClient` → `OpenAiCompatClient` | [OPENAI.md](../ref/OPENAI.md) |

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
| **Provider-specific** | Claude (Anthropic), Gemini (Google) | Phase 2 | [Phase 2](#phase-2-anthropic-and-google). |

For providers, configuration, and API families, see [spec/PROVIDERS.md](../spec/PROVIDERS.md). For model families and named model ids, see [spec/MODELS.md](../spec/MODELS.md).

### Implementation Notes

- The **`Provider`** trait and a single **`agents.defaultProvider`** + **`agents.defaultModel`** keep one backend per run; each backend implements the same trait and performs its own message/tool translation.
- **Ollama or Ollama-compatible** servers: use **`OllamaClient`** and set default backend/model.
- **OpenAI-style** backends: shared **`openai_compat`** layer; per-provider config (base URL, API key) on **`ProvidersConfig`**.
- **Claude, Gemini (Phase 2):** New client and adapter each unless a common abstraction is introduced; internal format remains the contract for the agent.
- **Streaming:** The main agent path is non-streaming today. Streaming format differs by API family (Ollama: NDJSON; OpenAI-compat: SSE); see ref docs when extending streaming to the channel.
- **Backend-specific features:** Adapters need only support chat, tools, and system message. Features such as model load/unload (LM Studio), keep_alive or embeddings (Ollama), or provider-specific options are not required for the agent loop; see ref docs for what each API offers.
