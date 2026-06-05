---
status: current
---

# OpenAI API Reference

Reference for the **OpenAI** HTTP API as used by Chai's `"openai-compat"` endpoint type with the OpenAI base URL. Data is sent to OpenAI (or to a compatible URL you configure); this is not a local-first option.

**Scope:** Chat Completions and List Models — the same routes used by other OpenAI-compatible backends in this repo. See [LM_STUDIO.md](LM_STUDIO.md) for internal message and tool mapping (`tool_name` ↔ `tool_call_id`).

## Purpose and How to Use

- **Purpose:** Document base URL, auth, discovery, and official API references for the OpenAI integration.
- **How to use:** Configure a provider with `endpoint: "openai-compat"` and `baseUrl: "https://api.openai.com/v1"`. Set `OPENAI_API_KEY` (or provider `apiKey`).

## Venice (OpenAI-Compatible Hosted API)

**Venice** exposes an OpenAI-shaped HTTP API; use `endpoint: "openai-compat"` with `baseUrl` set to Venice's base URL (commonly **`https://api.venice.ai/api/v1`**) and a Venice API key via `apiKey` or `OPENAI_API_KEY`. Official overview: [About Venice](https://docs.venice.ai/overview/about-venice). Venice-specific extensions on requests (e.g. **`venice_parameters`**) are **not** emitted by Chai's client today.

## Official Documentation

- **API overview:** https://platform.openai.com/docs/api-reference
- **Chat Completions:** https://platform.openai.com/docs/api-reference/chat
- **List models:** https://platform.openai.com/docs/api-reference/models/list

## Configuration in Chai

| Setting | Description |
|---------|-------------|
| `endpoint` | `"openai-compat"` |
| `id` (example) | `"openai"` (user-chosen) |
| `baseUrl` | Optional. Default **`https://api.openai.com/v1`** must be set explicitly (the `"openai-compat"` default is `http://127.0.0.1:1234/v1`). |
| `apiKey` | Optional if **`OPENAI_API_KEY`** is set. |
| `modelDiscovery` | Optional. Default `"default"` (uses `GET /v1/models`). |
| `defaultModel` | OpenAI model id (e.g. **`gpt-4o-mini`**); gateway fallback when unset: **`gpt-4o-mini`** |

Example:

```json
{ "id": "openai", "endpoint": "openai-compat", "baseUrl": "https://api.openai.com/v1" }
```

## Endpoints Used

| Path | Method | Use |
|------|--------|-----|
| **`/v1/models`** | GET | Model discovery (when `modelDiscovery: "default"`); WebSocket **`status`** includes models for this provider. |
| **`/v1/chat/completions`** | POST | Agent turns; optional tools; non-streaming path today for the main loop (streaming supported by client for future use). |

## Codebase

- **`crates/lib/src/providers/openai_compat.rs`** — Shared **`OpenAiCompatClient`** (HTTP, serde types, streaming). Used by all `"openai-compat"` providers (LM Studio, vLLM, OpenAI, Hugging Face, NIM, etc.). LM Studio–specific model discovery and auto-load are methods on this client.
- No separate `openai.rs` module — OpenAI is just another `"openai-compat"` provider with a specific base URL.
