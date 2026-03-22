# Services and Models

This document provides an introduction and overview of the LLM services and models that can be used—both what the current implementation supports and what is planned for a full implementation.

## Relationship to Other Documents

This section describes how this document is related to other working documents.

### API Alignment Epic

This document is the **working list** of services, model families, and configuration: what is supported today, what is planned, and how each fits the Ollama-native or OpenAI-compat API family. [EPIC_API_ALIGNMENT.md](EPIC_API_ALIGNMENT.md) is the **proposal and tracking** epic: internal format, API families, adapters, and Phase 1 vs Phase 2 (Anthropic/Google). Use this doc for **which** services exist and **how to configure** them; use the epic for **what “done” means** and **message/tool mapping** across backends. Phase 2 provider-specific APIs are specified in [EPIC_API_ALIGNMENT_PHASE_2.md](EPIC_API_ALIGNMENT_PHASE_2.md).

### Model Testing Documents

- [TEST_LOCAL_MODELS.md](TEST_LOCAL_MODELS.md) — Procedure and result tables for **Ollama** and **LM Studio**. Runnable with the current implementation.
- [TEST_SELF_HOSTED_MODELS.md](TEST_SELF_HOSTED_MODELS.md) — Procedure for **Hugging Face** (`hf`), LocalAI, llama.cpp, and similar. The **`hf`** backend is implemented; run against a configured OpenAI-compatible endpoint.
- [TEST_THIRD_PARTY_MODELS.md](TEST_THIRD_PARTY_MODELS.md) — Procedure for **OpenAI** (`openai`) and (when added) Claude, Gemini. The **`openai`** backend is implemented; run with a valid API key and model id.

All three use the same message sequence and expectations so that behavior can be compared across services and models once multiple backends exist.

## Categories of LLM Services

Providers are grouped into three service categories (or approaches). The distinction matters for privacy, cost, and operations.

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

**Also called:** LLM-as-a-Service (LLMaaS), cloud APIs, hosted services.

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

- **Self-hosted** services for privacy, cost control, and customization when data must stay in your environment.
- **Third-party** services for low-sensitivity, bursty workloads (e.g. experimentation, occasional heavy lifting).

Note: A **multi-agent management system** extends this idea: one agent or model acts as the **orchestrator**, delegating subtasks to other agents or models based on the task and each one’s abilities. The orchestrator chooses which agent and which model are best suited to complete a given step—e.g. route sensitive data only to local or self-hosted models, and send less sensitive or capability-heavy work to a third-party API when appropriate. That way, combining local, self-hosted, and third-party services can be used with multi-agent workflows so that the right model and the right agent handle each part of the job.

## Status of Supported Services

| Category | Current implementation | Planned / future |
|----------|-------------------------|------------------|
| **Local** (personal device) | **Ollama**, **LM Studio** | — |
| **Self-hosted** (on-prem / private cloud) | **vLLM**, **Hugging Face** (`hf`), **LocalAI** (via **`ollama`** or **`vllm`** paths; see below) | **llama.cpp** only via existing paths when OpenAI-compat (or Ollama-compatible) HTTP is enabled; no dedicated `defaultProvider` |
| **Hosted APIs** (privacy varies) | **NVIDIA NIM** (`nim`), **OpenAI** (`openai`) | Claude (Anthropic), Gemini (Google) |

**Configuration overview:** Set **`agents.defaultProvider`** to one of **`ollama`**, **`lms`**, **`vllm`**, **`nim`**, **`openai`**, **`hf`**. Set **`agents.defaultModel`** to the model id that backend expects. Top-level **`providers`** supplies base URLs and API keys. User-facing field names and env vars are documented in [README.md](../README.md).

**Discovery:** When **`agents.enabledProviders`** is absent or empty, only the default provider is polled for models at startup. When set, only listed providers are polled. WebSocket **`status`** returns **`ollamaModels`**, **`lmsModels`**, **`vllmModels`**, **`nimModels`**, **`openaiModels`**, and **`hfModels`** (each a list of `{ "name": ... }` objects where applicable).

**Ollama-compatible backends:** If a server exposes the native Ollama API (`/api/chat`, `/api/tags`), use **`"ollama"`** and optional **`providers.ollama.baseUrl`**. LocalAI in Ollama mode is an example.

**OpenAI-compatible backends:** If a server exposes OpenAI-shaped routes (`/v1/chat/completions`, and optionally `/v1/models`), use **`"lms"`**, **`"vllm"`**, **`"openai"`**, **`"hf"`**, or **`"nim"`** depending on product and config. LocalAI in OpenAI-compat mode can use **`"vllm"`** with **`providers.vllm.baseUrl`** set to that server’s `/v1` base.

### Compatibility: LocalAI, llama.cpp, and Venice

None of these uses a dedicated **`defaultProvider`** id in Chai today; they are **compatibility** stories (see [EPIC_API_ALIGNMENT.md](EPIC_API_ALIGNMENT.md) — **Compatibility Targets**).

| Product | How to use Chai |
|---------|-----------------|
| **LocalAI** (Ollama-compatible API) | **`"ollama"`** + optional **`providers.ollama.baseUrl`**. |
| **LocalAI** (OpenAI-compatible API) | **`"vllm"`** + **`providers.vllm.baseUrl`** → LocalAI’s **`/v1`** base. |
| **llama.cpp** (OpenAI-compatible server, e.g. `llama-server` with `/v1/...`) | **`"vllm"`** or **`"lms"`** (or **`"hf"`** if that matches your deployment) + matching **`providers.*.baseUrl`**. |
| **llama.cpp** (custom or legacy HTTP not matching Ollama or OpenAI chat) | Not supported until a dedicated adapter is added; treat as future epic work, not documentation-only. |
| **Venice** (hosted OpenAI-compatible API) | **`"openai"`** + **`providers.openai.baseUrl`** → **`https://api.venice.ai/api/v1`** (or the current base from [Venice docs](https://docs.venice.ai/overview/about-venice)); Venice API key via **`OPENAI_API_KEY`** / **`providers.openai.apiKey`**. See [OPENAI_REFERENCE.md](ref/OPENAI_REFERENCE.md). |

## API Comparison (Current Implementation)

Canonical comparison of what the gateway uses vs what each API offers. For endpoint details and shapes, see the per-backend references under [.agents/ref/](ref/).

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

**OpenAI-compat family (vLLM, OpenAI, Hugging Face `hf`, NIM):** Shared patterns in **`openai_compat`** — `POST /v1/chat/completions`, `GET /v1/models` where supported. See [VLLM_REFERENCE.md](ref/VLLM_REFERENCE.md), [HUGGINGFACE_REFERENCE.md](ref/HUGGINGFACE_REFERENCE.md), [NVIDIA_NIM_REFERENCE.md](ref/NVIDIA_NIM_REFERENCE.md), [OPENAI_REFERENCE.md](ref/OPENAI_REFERENCE.md).

## Services at a Glance

| Service        | Type        | Hosting        | API / integration | Status    |
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

## Models by Service

### Local — Ollama (supported)

Models are identified by the name used in `ollama list`. Set **`agents.defaultProvider`** to **`"ollama"`** and **`agents.defaultModel`** to the model name (e.g. `llama3.2:latest`).

| Model            | Notes     |
|------------------|-----------|
| `llama3:latest`  | Default   |
| `deepseek-1:7b`  |           |
| `qwen3:8b`       |           |

*Any other model you run in Ollama (or an Ollama-compatible server) can be used the same way; the table above reflects the set used in TEST_LOCAL_MODELS.md.*

### Local — LM Studio (supported)

Models are identified by the model id shown in LM Studio (e.g. from the in-app list or `GET /v1/models`). Set **`agents.defaultProvider`** to **`"lms"`** and **`defaultModel`** to the model id (e.g. `openai/gpt-oss-20b`, `ibm/granite-4-micro`). Optional **`providers.lms.baseUrl`** (default `http://127.0.0.1:1234/v1`).

| Model id (example) | Notes     |
|--------------------|-----------|
| `openai/gpt-oss-20b` | Use as `defaultModel` when `defaultProvider` is `"lms"` |
| `ibm/granite-4-micro` | Same |

*Any model loaded in LM Studio can be used; the id is shown in the LM Studio UI or via the API (and may include a provider prefix like `openai/` or `ibm/`).*

### Self-hosted — vLLM (supported)

Set **`agents.defaultProvider`** to **`"vllm"`** and **`defaultModel`** to the same id as `vllm serve`. Optional **`providers.vllm.baseUrl`** (default `http://127.0.0.1:8000/v1`), optional **`providers.vllm.apiKey`** / **`VLLM_API_KEY`**. See [TEST_SELF_HOSTED_MODELS.md](TEST_SELF_HOSTED_MODELS.md) and [VLLM_REFERENCE.md](ref/VLLM_REFERENCE.md).

### Self-hosted — Hugging Face (supported)

Set **`agents.defaultProvider`** to **`"hf"`**, **`providers.hf.baseUrl`** to your OpenAI-compatible base including **`/v1`**, and **`defaultModel`** to the id your server expects. Optional **`HF_API_KEY`** / **`providers.hf.apiKey`**. See [HUGGINGFACE_REFERENCE.md](ref/HUGGINGFACE_REFERENCE.md) and [TEST_SELF_HOSTED_MODELS.md](TEST_SELF_HOSTED_MODELS.md).

| Model                               | Notes     |
|-------------------------------------|-----------|
| `meta-llama/Llama-3.1-8B-Instruct`  | Default fallback in gateway when model unset |
| `Qwen/Qwen2.5-7B-Instruct`          |           |

### Hosted — NVIDIA NIM (supported)

Set **`agents.defaultProvider`** to **`"nim"`** and **`defaultModel`** to a NIM catalog id. Not a private deployment; see [NVIDIA_NIM_REFERENCE.md](ref/NVIDIA_NIM_REFERENCE.md).

### Third-party — OpenAI (supported)

Set **`agents.defaultProvider`** to **`"openai"`**, **`OPENAI_API_KEY`** or **`providers.openai.apiKey`**, and **`defaultModel`** to an OpenAI model id (e.g. `gpt-4o-mini`). Optional **`providers.openai.baseUrl`** for Azure-compatible gateways or proxies. See [OPENAI_REFERENCE.md](ref/OPENAI_REFERENCE.md) and [TEST_THIRD_PARTY_MODELS.md](TEST_THIRD_PARTY_MODELS.md).

| Model        | Notes              |
|--------------|--------------------|
| `gpt-4o-mini` | Gateway fallback when model unset |
| `gpt-4o`     |                    |

*Use current OpenAI model ids from their documentation; the table in TEST_THIRD_PARTY_MODELS.md may list additional examples.*

## Model Families Across Services

Cross-reference by family. **Supported** columns include backends that are implemented today; **Planned** lists APIs not yet integrated as dedicated providers.

| Family    | Local — Ollama | OpenAI-compat (lms, vllm, openai, hf, nim) | Planned (Claude / Gemini) |
|-----------|----------------|-------------------------------------------|---------------------------|
| **Llama** | `llama3:latest` | `meta-llama/Llama-3.1-8B-Instruct` (example on `hf`) | — |
| **Qwen**  | `qwen3:8b`      | Various via vLLM / HF | — |
| **DeepSeek** | `deepseek-1:7b` | — | — |
| **GPT**   | —               | `gpt-4o-mini`, etc. (`openai`) | — |

When new services or models are added, extend this table so that popular models and services remain comparable in one place.
