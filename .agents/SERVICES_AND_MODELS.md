# Services and Models

This document provides an introduction and overview of the LLM services and models that can be used—both what the current implementation supports and what is planned for a full implementation.

## Relationship to Other Documents

This section describes how this document is related to other working documents.

### API Alignment Epic

This document is the **working list** of services, model families, and configuration: what is supported today (Ollama, LM Studio), what is planned (self-hosted, third-party), and how each fits the Ollama-native or OpenAI-compat API family. [EPIC_API_ALIGNMENT.md](EPIC_API_ALIGNMENT.md) is the **proposal and tracking** epic for implementing additional backends: it defines the internal format, API families, and implementation notes (adapters, streaming, backend-specific features). Use this doc to see which services and models are in scope and how to configure them; use the epic to see what “done” means for each planned backend and how new backends plug into the gateway.

### Model Testing Documents

- [TEST_LOCAL_MODELS.md](TEST_LOCAL_MODELS.md) — Procedure and result tables for **Ollama** and **LM Studio** models. **Runnable with the current implementation** (both backends are implemented).
- [TEST_SELF_HOSTED_MODELS.md](TEST_SELF_HOSTED_MODELS.md) — Procedure and result tables for **Hugging Face** (and later LocalAI, llama.cpp). Defines the test flow for when those backends are implemented; not runnable with the current implementation.
- [TEST_THIRD_PARTY_MODELS.md](TEST_THIRD_PARTY_MODELS.md) — Procedure and result tables for **OpenAI** (and later Claude, Gemini). Defines the test flow for when those backends are implemented; not runnable with the current implementation.

All three use the same message sequence and expectations so that behavior can be compared across services and models once multiple backends exist. This overview summarizes which services and models are in scope, what is actually implemented, and what API alignment would be required for each planned service.

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

**Examples:** Running Llama 3, Qwen, or DeepSeek using Ollama, vLLM, or LocalAI.

| Aspect | Details |
|--------|---------|
| **Key traits** | **Full data control and privacy**; upfront cost (hardware) or ongoing (cloud instance); supports fine-tuning and customization; requires ML/DevOps expertise. |
| **Best for** | High-volume usage, regulated industries, data-sensitive applications. |

### 3. Third-Party (Cloud / API-Based)

**Description:** Models hosted and managed by external providers (e.g. OpenAI, Anthropic, Google).

**Also called:** LLM-as-a-Service (LLMaaS), cloud APIs, hosted services.

**Examples:** Using GPT-5, Opus, or Gemini via OpenAI, Claude, or Google APIs.

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
|----------|-----------------|------------------|
| **Local** (personal device) | **Ollama** (native API) and **LM Studio** (OpenAI-compatible API at e.g. `http://localhost:1234/v1`) | — |
| **Self-hosted** (on-prem / private cloud) | — | **Hugging Face** (TGI, Inference Endpoints), LocalAI, llama.cpp, vLLM |
| **Third-party** (cloud / API) | — | **OpenAI**, Claude (Anthropic), Gemini (Google) |

**Current implementation:** The gateway supports two local backends (see [POC_IMPLEMENTATION.md](POC_IMPLEMENTATION.md)): **Ollama** (native API) and **LM Studio** (OpenAI-compatible API). Which backend is used is set by **`agents.defaultBackend`** (`"ollama"` or `"lmstudio"`; default `"ollama"`). The **`agents.default_model`** is the model id for that backend and is passed as-is (e.g. for Ollama `llama3.2:latest`; for LM Studio `openai/gpt-oss-20b` or `ibm/granite-4-micro`, matching LM Studio’s model identifiers). The gateway discovers models from both backends at startup and exposes them in WebSocket `status` as `ollamaModels` and `lmStudioModels`.

**Ollama-compatible backends:** Whether Ollama (or an equivalent) runs on your **personal machine** (local) or on **your server** (self-hosted), if it exposes the native Ollama API, the current integration works without code changes. For example, LocalAI configured to expose Ollama-style `/api/chat` and `/api/tags` on your infra is self-hosted but uses the same client path as local Ollama.

**OpenAI-compatible backends:** The same idea applies to any server that exposes the OpenAI-compatible API (`/v1/chat/completions`, `/v1/models`). Whether it runs **locally** (e.g. LM Studio), on **your server** (e.g. vLLM, LocalAI in OpenAI mode, Hugging Face TGI), or as a **third-party** service (OpenAI), one shared client or adapter can be used with provider-specific config (base URL, and API key where required). For example, vLLM or LocalAI in OpenAI-compat mode on your infra would use the same client path as local LM Studio, with a different base URL and optional API key.

## API Comparison (Current Implementation)

The following tables are the **canonical** comparison of what the gateway uses today vs what each API offers and how backends differ. For endpoint details, request/response shapes, and possible future work, see [OLLAMA_REFERENCE.md](ref/OLLAMA_REFERENCE.md) and [LM_STUDIO_REFERENCE.md](ref/LM_STUDIO_REFERENCE.md).

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
| **Base** | `OllamaClient`, default local URL | `LmStudioClient(base_url)`, endpoint type openai | Same, endpoint type native |
| **Chat** | `POST /api/chat`, messages + tools | `/v1/chat/completions`, messages + tools | `/api/v1/chat`, messages only (no tools) |
| **Streaming** | NDJSON, not used to channel | SSE (OpenAI shape) | One-shot then callback |
| **Models** | `GET /api/tags` at startup | `GET /v1/models` | `GET /api/v1/models` |
| **Config** | `defaultBackend: "ollama"`, `default_model` | `defaultBackend: "lmstudio"`, `default_model`, `backends.lmStudio.baseUrl`, `endpointType: "openai"` | Same, `endpointType: "native"` |

## Services at a Glance

| Service        | Type        | Hosting        | API / integration | Status    |
|----------------|-------------|----------------|-------------------|-----------|
| **Ollama**     | Local       | Your machine   | Native Ollama (`/api/chat`, `/api/tags`) | Supported |
| **LM Studio**  | Local       | Your machine   | OpenAI-compat (`/v1/chat/completions`, `/v1/models`); set `agents.defaultBackend` to `"lmstudio"` and `agents.default_model` to the model id (e.g. `openai/gpt-oss-20b`) | Supported |
| **LocalAI**    | Self-hosted | Your infra     | Can expose Ollama or OpenAI-compat | Planned (Ollama mode = no code change) |
| **llama.cpp**  | Self-hosted | Your infra     | —                 | Planned   |
| **vLLM**       | Self-hosted | Your infra     | OpenAI-compat or custom | Planned   |
| **Hugging Face** | Self-hosted | Your infra / HF endpoints | OpenAI-compat (`/v1/chat/completions`) | Planned   |
| **OpenAI**     | Third-party | OpenAI         | OpenAI API        | Planned   |
| **Claude**     | Third-party | Anthropic      | Anthropic API     | Planned   |
| **Gemini**     | Third-party | Google         | Google API        | Planned   |

## Models by Service

### Local — Ollama (supported)

Models are identified by the name used in `ollama list`. Set **`agents.defaultBackend`** to `"ollama"` and **`agents.default_model`** to the model name (e.g. `llama3.2:latest`).

| Model            | Notes     |
|------------------|-----------|
| `llama3:latest`  | Default   |
| `deepseek-1:7b`  |           |
| `qwen3:8b`       |           |
| `gemma2:9b`      |           |

*Any other model you run in Ollama (or an Ollama-compatible server) can be used the same way; the table above reflects the set used in TEST_LOCAL_MODELS.md.*

### Local — LM Studio (supported)

Models are identified by the model id shown in LM Studio (e.g. from the in-app list or `GET /v1/models`). Set **`agents.defaultBackend`** to `"lmstudio"` and **`agents.defaultModel`** to the model id (e.g. `openai/gpt-oss-20b`, `ibm/granite-4-micro`). Optional **`agents.backends.lmStudio.baseUrl`** (default `http://127.0.0.1:1234/v1`) and **`agents.backends.lmStudio.endpointType`** (`"openai"` | `"native"`).

| Model id (example) | Notes     |
|--------------------|-----------|
| `openai/gpt-oss-20b` | Use as `default_model` when `defaultBackend` is `"lmstudio"` |
| `ibm/granite-4-micro` | Same |

*Any model loaded in LM Studio can be used; the id is shown in the LM Studio UI or via the API (and may include a provider prefix like `openai/` or `ibm/`).*

### Self-hosted — Hugging Face (planned)

Not implemented yet. When supported, models would be identified by Hugging Face model IDs (e.g. `org/model-name`). The table below matches TEST_SELF_HOSTED_MODELS.md for when that backend is added.

| Model                               | Notes     |
|-------------------------------------|-----------|
| `meta-llama/Llama-3.1-8B-Instruct`  | Default   |
| `mistralai/Mistral-7B-Instruct-v0.3`|           |
| `google/gemma-2-9b-it`              |           |
| `Qwen/Qwen2.5-7B-Instruct`          |           |

### Third-party — OpenAI (planned)

Not implemented yet. When supported, models would be identified by the OpenAI model ID (e.g. `gpt-5.2`). The table below matches TEST_THIRD_PARTY_MODELS.md for when that backend is added.

| Model        | Notes              |
|--------------|--------------------|
| `gpt-5.2`    | Default (flagship) |
| `gpt-5.1`    |                    |
| `gpt-5.1-mini` |                  |
| `gpt-5-mini` |                    |

## Model Families Across Services

The same or similar model families appear in more than one service. Below is a quick cross-reference by family. **The Ollama and LM Studio (local) columns are supported in the current implementation;** the other columns are planned (test procedures exist in the TEST_* docs).

| Family    | Local — Ollama (supported) | Self-hosted — Hugging Face (planned) | Third-party — OpenAI (planned) |
|-----------|----------------------------|--------------------------------------|---------------------------------|
| **Llama** | `llama3:latest` | `meta-llama/Llama-3.1-8B-Instruct` | —          |
| **Qwen**  | `qwen3:8b`      | `Qwen/Qwen2.5-7B-Instruct` | —          |
| **Gemma** | `gemma2:9b`     | `google/gemma-2-9b-it`     | —          |
| **DeepSeek** | `deepseek-1:7b` | —                        | —          |
| **Mistral** | —               | `mistralai/Mistral-7B-Instruct-v0.3` | —     |
| **GPT**   | —               | —                          | `gpt-5.2`, `gpt-5.1`, `gpt-5.1-mini`, `gpt-5-mini` |

When new services or models are added, extend this table so that popular models and services remain comparable in one place.
