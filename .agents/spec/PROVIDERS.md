---
status: stable
---

# Providers

Internal spec for **LLM backends** in Chai: canonical **`defaultProvider`** ids, configuration, discovery, API family (Ollama-native vs OpenAI-compatible), and compatibility targets. For **model** identifiers, repository inventory, and tool-fit notes, see [MODELS.md](MODELS.md).

## Relationship to Other Documents

- **[EPIC_API_ALIGNMENT.md](../EPIC_API_ALIGNMENT.md)** — Proposal and tracking for API alignment, message/tool mapping, and [Phase 2 (Anthropic/Google)](../EPIC_API_ALIGNMENT.md#phase-2-anthropic-and-google). This spec lists **which** backends exist and **how** to configure them; the epic defines **what “done” means** across backends.
- **[MODELS.md](MODELS.md)** — Model ids, repository inventory, deployment categories, and Chai tool compatibility.

## Categories of Providers

Providers are grouped into three categories by **where** the model runs (or who hosts the API). The distinction matters for privacy, cost, and operations.

### 1. Local (Personal Device)

**Description:** Models run directly on personal hardware (laptop, desktop).

**Also called:** Self-hosted (but "local" here implies "self-hosted on your own machine").

**Examples:** Running Llama 3, Qwen, or DeepSeek using Ollama, LM Studio, llama.cpp.

| Aspect | Details |
|--------|---------|
| **Key traits** | **Full data control and privacy**; constrained by device hardware (VRAM, CPU, cooling). |
| **Best for** | Development, experimentation, offline use. |

### 2. Self-Hosted (On-Premise or Private Cloud)

**Description:** Models run on your own infrastructure—physical servers, cloud VMs, or VPCs.

**Also called:** On-premise, private deployment; when on personal hardware, also called "local".

**Examples:** Running Llama 3, Qwen, or DeepSeek using Ollama, vLLM, LocalAI, or Hugging Face Inference Endpoints / TGI.

| Aspect | Details |
|--------|---------|
| **Key traits** | **Full data control and privacy**; upfront cost (hardware) or ongoing (cloud instance); supports fine-tuning and customization; requires ML/DevOps expertise. |
| **Best for** | High-volume usage, regulated industries, data-sensitive applications. |

### 3. Third-Party (Cloud / API-Based)

**Description:** Models hosted and managed by external providers (e.g. OpenAI, Anthropic, Google).

**Also called:** LLM-as-a-Service (LLMaaS), cloud APIs, hosted APIs.

**Examples:** Using GPT, Opus, or Gemini via OpenAI, Claude, or Google APIs.

| Aspect | Details |
|--------|---------|
| **Key traits** | **Data leaves your environment**; Pay-per-use or subscription pricing; no infrastructure to manage; fast setup and access to cutting-edge models. |
| **Best for** | Rapid prototyping, low-to-moderate usage, teams without dedicated AI/ML ops. |

### Summary of Approaches

| Approach     | Where it runs      | Data privacy        | Setup effort | Cost model                |
|-------------|--------------------|---------------------|--------------|---------------------------|
| **Local** (subset) | Your personal device | High (data stays) | Medium       | One-time hardware cost    |
| **Self-hosted** | Your infrastructure | High (data stays) | High         | Hardware + maintenance    |
| **Third-party** | Provider’s cloud   | Low (data leaves)   | Minimal      | Pay-per-token             |

### Hybrid Approaches Are Common

Many organizations combine both:

- **Self-hosted** providers for privacy, cost control, and customization when data must stay in your environment.
- **Third-party** providers for low-sensitivity, bursty workloads (e.g. experimentation, occasional heavy lifting).

Note: A **multi-agent management system** extends this idea: one agent or model acts as the **orchestrator**, delegating subtasks to other agents or models based on the task and each one’s abilities. The orchestrator chooses which agent and which model are best suited to complete a given step—e.g. route sensitive data only to local or self-hosted models, and send less sensitive or capability-heavy work to a third-party API when appropriate. That way, combining local, self-hosted, and third-party providers can be used with multi-agent workflows so that the right model and the right agent handle each part of the job.

## Status of Supported Providers

| Category | Current implementation | Planned / future |
|----------|-------------------------|------------------|
| **Local** (personal device) | **Ollama**, **LM Studio** | — |
| **Self-hosted** (on-prem / private cloud) | **vLLM**, **Hugging Face** (`hf`), **LocalAI** (via **`ollama`** or **`vllm`** paths; see below) | **llama.cpp** only via existing paths when OpenAI-compat (or Ollama-compatible) HTTP is enabled; no dedicated `defaultProvider` |
| **Hosted APIs** (privacy varies) | **NVIDIA NIM** (`nim`), **OpenAI** (`openai`) | Claude (Anthropic), Gemini (Google) |

**Configuration overview:** Set **`agents.defaultProvider`** to one of **`ollama`**, **`lms`**, **`vllm`**, **`nim`**, **`openai`**, **`hf`**. Set **`agents.defaultModel`** to the model id that backend expects. Top-level **`providers`** supplies base URLs and API keys. User-facing field names and env vars are documented in [README.md](../../README.md).

**Discovery:** When **`agents.enabledProviders`** is absent or empty, only the default provider is polled for models at startup. When set, only listed providers are polled. WebSocket **`status`** returns **`ollamaModels`**, **`lmsModels`**, **`vllmModels`**, **`nimModels`**, **`openaiModels`**, and **`hfModels`** (each a list of `{ "name": ... }` objects where applicable). For NIM, the list is a static catalog plus optional **`providers.nim.extraModels`** (NVIDIA does not expose **`/v1/models`** on the hosted API).

**Ollama-compatible backends:** If a server exposes the native Ollama API (`/api/chat`, `/api/tags`), use **`"ollama"`** and optional **`providers.ollama.baseUrl`**. LocalAI in Ollama mode is an example.

**OpenAI-compatible backends:** If a server exposes OpenAI-shaped routes (`/v1/chat/completions`, and optionally `/v1/models`), use **`"lms"`**, **`"vllm"`**, **`"openai"`**, **`"hf"`**, or **`"nim"`** depending on product and config. LocalAI in OpenAI-compat mode can use **`"vllm"`** with **`providers.vllm.baseUrl`** set to that server’s `/v1` base.

### Compatibility: LocalAI, llama.cpp, and Venice

None of these uses a dedicated **`defaultProvider`** id in Chai today; they are **compatibility** stories (see [EPIC_API_ALIGNMENT.md](../EPIC_API_ALIGNMENT.md) — **Compatibility Targets**).

| Product | How to use Chai |
|---------|-----------------|
| **LocalAI** (Ollama-compatible API) | **`"ollama"`** + optional **`providers.ollama.baseUrl`**. |
| **LocalAI** (OpenAI-compatible API) | **`"vllm"`** + **`providers.vllm.baseUrl`** → LocalAI’s **`/v1`** base. |
| **llama.cpp** (OpenAI-compatible server, e.g. `llama-server` with `/v1/...`) | **`"vllm"`** or **`"lms"`** (or **`"hf"`** if that matches your deployment) + matching **`providers.*.baseUrl`**. |
| **llama.cpp** (custom or legacy HTTP not matching Ollama or OpenAI chat) | Not supported until a dedicated adapter is added; treat as future epic work, not documentation-only. |
| **Venice** (hosted OpenAI-compatible API) | **`"openai"`** + **`providers.openai.baseUrl`** → **`https://api.venice.ai/api/v1`** (or the current base from [Venice docs](https://docs.venice.ai/overview/about-venice)); Venice API key via **`OPENAI_API_KEY`** / **`providers.openai.apiKey`**. See [OPENAI_REFERENCE.md](../ref/OPENAI_REFERENCE.md). |

## API Comparison (Current Implementation)

Canonical comparison of what the gateway uses vs what each API offers. For endpoint details and shapes, see the per-backend references under [.agents/ref/](../ref/).

**Ollama: current usage vs full API vs hosted**

| Area | Current | Ollama full API | Hosted (OpenAI/Anthropic) |
|------|---------|-----------------|---------------------------|
| **Base** | `OllamaClient`, default local URL | Same | Remote URL + API key |
| **Chat** | `/api/chat` with model, messages, stream, tools | + options, keep_alive, format, think, logprobs | Different URL/params, similar roles/messages/tools |
| **Streaming** | Implemented but not used to channel; NDJSON | Same | SSE or similar, different format |
| **Models** | `GET /api/tags` at startup; one default model from config | + pull, delete, copy, show | Model ID only, no local lifecycle |
| **Generate** | Not used | `/api/generate` (prompt-only) | Often "completions" vs "chat" |
| **Embed** | Not used | `/api/embed` | Separate embedding APIs |
| **State** | Client sends full history + system each time | N/A (stateless) | Same (stateless) |

**Ollama vs LM Studio: current usage**

| Area | Ollama (current) | LM Studio OpenAI (current) | LM Studio native (current) |
|------|------------------|---------------------------|----------------------------|
| **Base** | `OllamaClient`, default local URL | `LmsClient(base_url)` (OpenAI-compat only) | — |
| **Chat** | `POST /api/chat`, messages + tools | `POST /v1/chat/completions`, messages + tools; errors returned (load+retry only on 500 "unloaded") | — |
| **Streaming** | NDJSON, not used to channel | SSE (OpenAI shape) | — |
| **Models** | `GET /api/tags` at startup | `GET /api/v1/models` | — |
| **Config** | `defaultProvider: "ollama"`, `defaultModel` | `defaultProvider: "lms"`, `defaultModel`, `providers.lms.baseUrl` | — |

**OpenAI-compat family (vLLM, OpenAI, Hugging Face `hf`, NIM):** Shared patterns in **`openai_compat`** — `POST /v1/chat/completions`, `GET /v1/models` where supported. See [VLLM_REFERENCE.md](../ref/VLLM_REFERENCE.md), [HUGGINGFACE_REFERENCE.md](../ref/HUGGINGFACE_REFERENCE.md), [NVIDIA_NIM_REFERENCE.md](../ref/NVIDIA_NIM_REFERENCE.md), [OPENAI_REFERENCE.md](../ref/OPENAI_REFERENCE.md).

## Providers at a Glance

| Provider       | Type        | Hosting        | API / integration | Status    |
|----------------|-------------|----------------|-------------------|-----------|
| **Ollama**     | Local       | Your machine   | Native Ollama (`/api/chat`, `/api/tags`) | Supported |
| **LM Studio**  | Local       | Your machine   | OpenAI-compat (`/v1/chat/completions`, `/v1/models`); `defaultProvider` **`"lms"`** | Supported |
| **vLLM**       | Self-hosted | Your infra     | OpenAI-compat; `defaultProvider` **`"vllm"`** | Supported |
| **LocalAI**    | Self-hosted | Your infra     | Ollama-style and/or OpenAI-compat | Compatibility (see [Compatibility: LocalAI, llama.cpp, and Venice](#compatibility-localai-llamacpp-and-venice)) |
| **llama.cpp**  | Self-hosted | Your infra     | OpenAI-compat **`/v1/...`** when enabled; else not integrated | Compatibility when OpenAI-compat; custom API not implemented |
| **Venice**     | Hosted      | Venice         | OpenAI-compat via **`openai`** + **`providers.openai.baseUrl`** | Compatibility (see [Compatibility: LocalAI, llama.cpp, and Venice](#compatibility-localai-llamacpp-and-venice)) |
| **Hugging Face** | Self-hosted / HF cloud | Your endpoint | OpenAI-compat; `defaultProvider` **`"hf"`**, **`providers.hf.baseUrl`** | Supported |
| **NVIDIA NIM** | Hosted      | NVIDIA         | OpenAI-compat; `defaultProvider` **`"nim"`** | Supported |
| **OpenAI**     | Third-party | OpenAI | OpenAI API; `defaultProvider` **`"openai"`** | Supported |
| **Claude**     | Third-party | Anthropic      | Anthropic API     | Planned   |
| **Gemini**     | Third-party | Google         | Google API        | Planned   |
