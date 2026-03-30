---
status: current
---

# OpenAI API Reference

Reference for the **OpenAI** HTTP API as used by Chai’s **`openai`** provider (`agents.defaultProvider: "openai"`). Data is sent to OpenAI (or to a compatible URL you configure); this is not a local-first option.

**Scope:** Chat Completions and List Models—the same routes used by other OpenAI-compatible backends in this repo. See [LM_STUDIO_REFERENCE.md](LM_STUDIO_REFERENCE.md) for internal message and tool mapping (`tool_name` ↔ `tool_call_id`).

## Purpose and How to Use

- **Purpose:** Document base URL, auth, discovery, and official API references for the **`OpenAiClient`** integration.
- **How to use:** Configure **`providers.openai`** and set **`OPENAI_API_KEY`** (or **`providers.openai.apiKey`**). Optional **`providers.openai.baseUrl`** overrides the default API origin (e.g. Azure OpenAI–compatible gateways or HTTP proxies).

## Venice (OpenAI-Compatible Hosted API)

**Venice** exposes an OpenAI-shaped HTTP API; use the **`openai`** provider with **`providers.openai.baseUrl`** set to Venice’s base URL (commonly **`https://api.venice.ai/api/v1`**) and a Venice API key in **`OPENAI_API_KEY`** or **`providers.openai.apiKey`**. Official overview: [About Venice](https://docs.venice.ai/overview/about-venice). Venice-specific extensions on requests (e.g. **`venice_parameters`**) are **not** emitted by Chai’s client today.

## Official Documentation

- **API overview:** https://platform.openai.com/docs/api-reference
- **Chat Completions:** https://platform.openai.com/docs/api-reference/chat
- **List models:** https://platform.openai.com/docs/api-reference/models/list

## Configuration in Chai

| Setting | Description |
|---------|-------------|
| **`agents.defaultProvider`** | **`"openai"`** |
| **`agents.defaultModel`** | OpenAI model id (e.g. **`gpt-4o-mini`**); gateway fallback when unset: **`gpt-4o-mini`** |
| **`providers.openai.baseUrl`** | Optional. Default **`https://api.openai.com/v1`** (no trailing slash; gateway trims as needed). |
| **`providers.openai.apiKey`** | Optional if **`OPENAI_API_KEY`** is set. |
| **`OPENAI_API_KEY`** | Overrides **`providers.openai.apiKey`** when set and non-empty. |

## Endpoints Used

| Path | Method | Use |
|------|--------|-----|
| **`/v1/models`** | GET | Model discovery when **`openai`** is in **`enabledProviders`** or is the default provider; WebSocket **`status`** → **`openaiModels`**. |
| **`/v1/chat/completions`** | POST | Agent turns; optional tools; non-streaming path today for the main loop (streaming supported by client for future use). |

## Codebase

- **`crates/lib/src/providers/openai.rs`** — **`OpenAiClient`**: thin wrapper with OpenAI default base URL and provider-specific error type; implements **`Provider`**.
- **`crates/lib/src/providers/openai_compat.rs`** — Shared **`OpenAiCompatClient`** (HTTP, serde types, streaming). Also used by LM Studio, vLLM, Hugging Face **`hf`**, etc. Not OpenAI-specific.

## Comparison With `openai_compat`

| | **`openai_compat` module** | **`openai` module (`OpenAiClient`)** |
|--|---------------------------|-------------------------------------|
| **Role** | Shared library: one implementation of OpenAI-shaped HTTP for all compat backends | Named provider: defaults, **`Provider`** impl, **`OpenAiError`** |
| **Merge?** | Keep separate: one **`openai_compat`** avoids duplicating wire logic across **`lms`**, **`vllm`**, **`openai`**, **`hf`**, **`nim`**-style paths | Stays a thin wrapper only |
