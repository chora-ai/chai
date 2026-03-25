# Epic: API Alignment — Phase 2 (Anthropic and Google)

This document **specifies** what a future integration of **Claude (Anthropic)** and **Gemini (Google)** would entail. It is **not** an implementation: Chai does not yet expose `anthropic` or `gemini` as **`agents.defaultProvider`** values.

Use this when picking up Phase 2 work from [EPIC_API_ALIGNMENT.md](EPIC_API_ALIGNMENT.md).

## Why a Separate Adapter Family

These APIs are **not** OpenAI-compatible chat completions in the narrow sense Chai’s **`openai_compat`** module implements. Each has its own:

- Message and role layout (e.g. Anthropic system vs messages; Gemini system instruction and `contents` parts).
- Tool / function-calling wire format and IDs for tool invocations and results.
- List-models or catalog discovery (if any).

The internal agent contract stays the same: full history, system context, tools, **`tool_name`** on tool results in session storage; each new **`Provider`** maps that to and from the vendor API.

## Anthropic (Claude)

- **Docs:** https://docs.anthropic.com/claude/reference/messages_post
- **Typical surface:** Messages API (`POST /v1/messages`), models like `claude-3-5-sonnet-latest`; tools and tool results use Anthropic’s **`tool_use`** / **`tool_result`** blocks rather than OpenAI’s `tool_calls` on the assistant message.
- **Adapter responsibilities:** Map internal `ChatMessage` list + tools to Anthropic `messages`, `system`, and `tools`; map assistant output and tool calls back to **`ChatResponse`**; map tool execution results by stable id ↔ internal **`tool_name`** as required by the agent loop.

## Google (Gemini)

- **Docs:** https://ai.google.dev/api/generate-content (and current model list for your API version).
- **Typical surface:** `generateContent` / chat with `contents` and `tools`; schema differs from both OpenAI and Anthropic.
- **Adapter responsibilities:** Map internal messages and tools to Gemini **`contents`** and tool declarations; map model responses back to **`ChatResponse`**; preserve tool correlation for follow-up turns.

## Implementation Checklist (Gateway)

When implementing, expect to touch at least:

- **`crates/lib/src/config.rs`** — `providers.anthropic` / `providers.gemini` (or similar), env vars, **`canonical_provider`**.
- **`crates/lib/src/providers/`** — New client modules + **`Provider`** impls.
- **`crates/lib/src/orchestration/`** — **`ProviderChoice`**, **`ProviderClients`**, **`resolve_model`** fallbacks.
- **`crates/lib/src/gateway/server.rs`** — Client construction, discovery, **`status`** payload keys (e.g. `anthropicModels`, `geminiModels`).
- **`crates/desktop/`** — Provider allowlist, model reconciliation, info screen.
- **User docs** — [README.md](../README.md), reference docs under [`.agents/ref/`](ref/), [spec/PROVIDERS.md](spec/PROVIDERS.md), [spec/MODELS.md](spec/MODELS.md).
- **Tests** — [10-third-party-openai-gpt.md](../.testing/10-third-party-openai-gpt.md) and the index at [.testing/README.md](../.testing/README.md) when applicable.

## Relationship to OpenAI-Compat

Servers that expose **OpenAI-compatible** HTTP for Claude or Gemini (if hosted that way) could use the existing **`openai`** or **`vllm`** path with provider **base URL**; that is a **compatibility** route, not a substitute for first-class Anthropic/Google APIs when users use official endpoints.
