---
status: current
---

# IronClaw Reference

Reference extracted from [IronClaw](https://www.ironclaw.com/) marketing material and the [IronClaw repository](https://github.com/nearai/ironclaw) for alignment when comparing gateways, security models, or LLM integration patterns. This project (**Chai**) is not IronClaw; use this doc to understand IronClaw on its own terms and to spot ideas worth borrowing or contrasting.

## Purpose and How to Use

- **Purpose:** Summarize IronClaw’s stated goals (security-first, Rust), high-level architecture, how it names **LLM providers**, and where configuration lives—without vendoring the whole upstream design.
- **How to use:** When discussing OpenClaw-adjacent systems, TEE/cloud positioning, WASM tool sandboxes, or provider env vars, consult this doc and the official links below.

## Official IronClaw Resources

- **Website:** https://www.ironclaw.com/ — product positioning (NEAR AI Cloud, TEE, encrypted vault, Wasm tools, comparison vs OpenClaw).
- **Repository:** https://github.com/nearai/ironclaw — source, `README`, `providers.json`, `docs/` (e.g. provider guides referenced from the README).
- **Feature parity:** Upstream tracks OpenClaw-style behavior in **`FEATURE_PARITY.md`** (named in the repo README).

The website and the open-source README overlap in theme (security, Rust, OpenClaw heritage) but emphasize different deployment stories: the site highlights **NEAR AI Cloud** and **Trusted Execution Environments**; the README focuses on **local/self-hosted** install (`ironclaw onboard`, PostgreSQL, optional provider selection). Treat them as complementary, not identical product definitions.

## Chai vs IronClaw (Feature Summary)

Minimal scan. **Yes** = supported, **Partial** = subset or different shape, **No** = not present, **N/A** = not applicable. IronClaw’s own OpenClaw parity is tracked upstream in [`FEATURE_PARITY.md`](https://github.com/nearai/ironclaw/blob/staging/FEATURE_PARITY.md) (IronClaw ↔ OpenClaw only—not Chai).

| Feature | Chai | IronClaw |
|---------|------|----------|
| Rust implementation | Yes | Yes |
| PostgreSQL + pgvector workspace memory | No | Yes |
| WASM-isolated tools + leak scanning | No | Yes |
| MCP servers | No | Yes |
| Docker / worker orchestration for jobs | No | Yes |
| Broad **LLM provider** surface (env + catalog) | Partial (fixed backends) | Yes |
| Web gateway + control-style UI | Partial (desktop app) | Yes |
| Telegram channel | Yes | Yes |
| OpenClaw **FEATURE_PARITY**-style matrix in-repo | No | Yes (vs OpenClaw) |

## What IronClaw Is (From Upstream)

### Positioning

- **OpenClaw heritage:** Described as a **Rust reimplementation inspired by OpenClaw**, with a tracking matrix vs OpenClaw (`FEATURE_PARITY.md`).
- **Security narrative:** Emphasis on **defense in depth**, **WASM sandboxing** for untrusted tools, **credential injection at the host boundary** (secrets not exposed to tool code), **endpoint allowlisting**, **leak detection**, and **prompt-injection defenses** (see repository README and [ironclaw.com](https://www.ironclaw.com/)).
- **Stack:** **Rust** codebase; **PostgreSQL** with **pgvector** for persistence; channels include REPL, HTTP, WASM-packaged channels (e.g. Telegram), and a **web gateway** (SSE/WebSocket) per the architecture section of the README.

### Architecture (High Level)

The README’s architecture diagram centers **Channels** → **Agent Loop** (with router, scheduler, routines) → **Orchestrator** (including Docker sandbox) → **Tool Registry** (built-in, MCP, WASM). Core components named there include **Agent Loop**, **Router**, **Scheduler**, **Worker**, **Orchestrator**, **Web Gateway**, **Routines Engine**, **Workspace**, and **Safety Layer**.

### LLM Providers and Naming

IronClaw’s README refers to **“LLM providers”** and **“Alternative LLM Providers”** in configuration. It defaults to **NEAR AI** but documents built-in integrations (e.g. **Anthropic**, **OpenAI**, **Google Gemini**, **MiniMax**, **Mistral**, **Ollama**) and **OpenAI-compatible** endpoints (e.g. **OpenRouter**, **Together AI**, **Fireworks AI**, self-hosted **vLLM**, **LiteLLM**). Selection is described via **`ironclaw onboard`** and/or environment variables such as **`LLM_BACKEND`**, **`LLM_BASE_URL`**, **`LLM_API_KEY`**, **`LLM_MODEL`** (examples appear in the README). The repo also contains **`providers.json`** at the root for provider metadata.

**Terminology:** Upstream uses **“providers”** for LLM backends (aligned with OpenClaw’s “model providers” vocabulary), not “clients” as the primary user-facing term.

### Configuration and State

- **Bootstrap:** `ironclaw onboard` wizard; settings persisted in the database; bootstrap variables (e.g. `DATABASE_URL`, `LLM_BACKEND`) are described as written to **`~/.ironclaw/.env`** for availability before DB connect.
- **Secrets:** README describes **AES-256-GCM** encryption and **system keychain** use during setup.

### Tools and Sandboxing

- **WASM:** Untrusted tools run in **WebAssembly** with capability-based permissions, allowlisted HTTP, credential injection at the host, and response leak scanning (README “Security” / “WASM Sandbox” sections).
- **MCP:** **Model Context Protocol** servers can extend capabilities.
- **Dynamic tools:** README mentions **dynamic tool building** (describe need → build as WASM tool).

## Product Site vs Repository Emphasis

| Aspect | [ironclaw.com](https://www.ironclaw.com/) | [GitHub README](https://github.com/nearai/ironclaw) |
|--------|-------------------------------------------|-----------------------------------------------------|
| **Deploy** | “One-click” on **NEAR AI Cloud**, TEE, encrypted enclave framing | Clone, `cargo build`, PostgreSQL, `ironclaw onboard` |
| **Secrets** | Encrypted vault, “never touch the LLM” messaging | Credential vault, host-boundary injection, leak detection |

For implementation details, prefer the repository and `docs/`; the site is useful for positioning and security storytelling.

## Possible Future Use

- **Vocabulary:** If Chai user-facing copy adopts **“model providers”** (OpenClaw style), IronClaw’s **`LLM_BACKEND` / `provider`** language is a useful parallel.
- **Sandboxing:** If Chai adds isolated tool execution, IronClaw’s WASM + allowlist + leak-scan pipeline is a concrete benchmark to read in upstream `docs/` and code.
- **Parity tracking:** When scoping features “like OpenClaw,” checking **`FEATURE_PARITY.md`** in IronClaw can show what a Rust sibling project prioritized.
