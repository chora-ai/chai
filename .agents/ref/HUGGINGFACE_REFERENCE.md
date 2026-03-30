---
status: current
---

# Hugging Face OpenAI-Compat Reference

Reference for **Hugging Face** deployments that expose an **OpenAI-compatible** HTTP API (`/v1/chat/completions`, `/v1/models`), used by Chai’s **`hf`** provider (`agents.defaultProvider: "hf"`). Typical cases: **Inference Endpoints** (managed) or **Text Generation Inference (TGI)** / similar stacks on your infrastructure.

**Scope:** What Chai expects on the wire matches [OpenAI Chat Completions](https://platform.openai.com/docs/api-reference/chat) and List Models. Your deployment must expose a base URL whose **`/v1`** routes behave like OpenAI’s. See [LM_STUDIO_REFERENCE.md](LM_STUDIO_REFERENCE.md) for how Chai maps messages and tools internally.

## Purpose and How to Use

- **Purpose:** Document **`providers.hf`**, env vars, and discovery for self-hosted or private HF-style endpoints.
- **How to use:** Set **`providers.hf.baseUrl`** to your service root **including `/v1`** (e.g. `https://<deployment>.endpoints.huggingface.cloud/v1`). Set **`HF_API_KEY`** or **`providers.hf.apiKey`** when the endpoint requires a bearer token.

## Official Documentation (examples)

- **Inference Endpoints (product):** https://huggingface.co/docs/inference-endpoints
- **TGI / OpenAI API:** https://huggingface.co/docs/text-generation-inference/en/basic_tutorials/consuming_tgi#openai-compatibility

Exact URLs and features depend on your product and version; Chai only requires compatible **`/v1/chat/completions`** and (for discovery) **`/v1/models`** when available.

## Configuration in Chai

| Setting | Description |
|---------|-------------|
| **`agents.defaultProvider`** | **`"hf"`** |
| **`agents.defaultModel`** | Model id your server expects (e.g. **`meta-llama/Llama-3.1-8B-Instruct`**); gateway fallback when unset: **`meta-llama/Llama-3.1-8B-Instruct`** |
| **`providers.hf.baseUrl`** | Required for real deployments. If omitted, default **`http://127.0.0.1:8080/v1`** is used and the gateway logs a warning when **`hf`** is the default provider. |
| **`providers.hf.apiKey`** | Optional if **`HF_API_KEY`** is set. |
| **`HF_API_KEY`** | Overrides **`providers.hf.apiKey`** when set and non-empty. |

## Endpoints Used

| Path | Method | Use |
|------|--------|-----|
| **`/v1/models`** | GET | Model discovery when **`hf`** is in **`enabledProviders`** or is the default provider (may be empty if the server omits this route). WebSocket **`status`** → **`hfModels`**. |
| **`/v1/chat/completions`** | POST | Agent turns; optional tools. |

## Codebase

- **`crates/lib/src/providers/hf.rs`** — **`HfClient`**: thin wrapper with a placeholder default base URL; implements **`Provider`**.
- **`crates/lib/src/providers/openai_compat.rs`** — Shared **`OpenAiCompatClient`** (same as LM Studio / vLLM / OpenAI **`openai`** provider).

## LocalAI (OpenAI-Compatible Mode)

For **LocalAI** in OpenAI-compat mode, you can instead use **`agents.defaultProvider: "vllm"`** and set **`providers.vllm.baseUrl`** to LocalAI’s **`/v1`** base—the same shared client stack. Ollama-compatible LocalAI uses **`ollama`** and **`providers.ollama.baseUrl`**. See **`VllmProviderEntry`** rustdoc in **`crates/lib/src/config.rs`**.
