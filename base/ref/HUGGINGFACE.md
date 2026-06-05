---
status: current
---

# Hugging Face OpenAI-Compat Reference

Reference for **Hugging Face** deployments that expose an **OpenAI-compatible** HTTP API (`/v1/chat/completions`, `/v1/models`), used by Chai's `"openai-compat"` endpoint type. Typical cases: **Inference Endpoints** (managed) or **Text Generation Inference (TGI)** / similar stacks on your infrastructure.

**Scope:** What Chai expects on the wire matches [OpenAI Chat Completions](https://platform.openai.com/docs/api-reference/chat) and List Models. Your deployment must expose a base URL whose **`/v1`** routes behave like OpenAI's. See [LM_STUDIO.md](LM_STUDIO.md) for how Chai maps messages and tools internally.

## Purpose and How to Use

- **Purpose:** Document the `"openai-compat"` provider configuration for self-hosted or private HF-style endpoints.
- **How to use:** Configure a provider with `endpoint: "openai-compat"` and `baseUrl` set to your service root **including `/v1`** (e.g. `https://<deployment>.endpoints.huggingface.cloud/v1`). Set `HF_API_KEY` or provider `apiKey` when the endpoint requires a bearer token.

## Official Documentation (examples)

- **Inference Endpoints (product):** https://huggingface.co/docs/inference-endpoints
- **TGI / OpenAI API:** https://huggingface.co/docs/text-generation-inference/en/basic_tutorials/consuming_tgi#openai-compatibility

Exact URLs and features depend on your product and version; Chai only requires compatible **`/v1/chat/completions`** and (for discovery) **`/v1/models`** when available.

## Configuration in Chai

| Setting | Description |
|---------|-------------|
| `endpoint` | `"openai-compat"` |
| `id` (example) | `"hf"` (user-chosen) |
| `baseUrl` | Required for real deployments. If omitted, default **`http://127.0.0.1:8080/v1`** is used and the gateway logs a warning when this provider is the default. |
| `apiKey` | Optional if **`HF_API_KEY`** is set. |
| `HF_API_KEY` | Overrides provider `apiKey` when set and non-empty. |
| `defaultModel` | Model id your server expects (e.g. **`meta-llama/Llama-3.1-8B-Instruct`**); gateway fallback when unset: **`meta-llama/Llama-3.1-8B-Instruct`** |

Example:

```json
{ "id": "hf", "endpoint": "openai-compat", "baseUrl": "https://your-deployment.endpoints.huggingface.cloud/v1" }
```

## Endpoints Used

| Path | Method | Use |
|------|--------|-----|
| **`/v1/models`** | GET | Model discovery when `modelDiscovery: "default"` (the default); may return empty if the server omits this route. WebSocket **`status`** includes models for this provider. |
| **`/v1/chat/completions`** | POST | Agent turns; optional tools. |

## Codebase

- **`crates/lib/src/providers/openai_compat.rs`** — Shared **`OpenAiCompatClient`** (same as LM Studio / vLLM / OpenAI / NIM providers). No separate `hf.rs` module.
