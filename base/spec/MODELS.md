---
status: stable
---

# Models

Internal spec for **model identifiers** in Chai: examples per endpoint type, cross-provider **families**, a **repository inventory** of every model id named in code and docs, and **tool-calling** fit for the agent loop. For **backend** configuration and wire protocols, see [PROVIDERS.md](PROVIDERS.md).

## Relationship to Other Documents

- **[PROVIDERS.md](PROVIDERS.md)** — Provider `id`, endpoint types, discovery, and compatibility; pair with **MODELS.md** for **model** strings.
- **Testing procedures** live under **`/docs/testing/`** and are operational runbooks; this spec remains the canonical model taxonomy and inventory.

## Models by Endpoint Type

### `"ollama"` — Ollama (Supported)

Models are identified by the name used in `ollama list`. Set `defaultProvider` to the `id` of a provider with `endpointType: "ollama"` and `defaultModel` to the model name (e.g. `llama3.2:3b`).

| Model | Notes |
|-------|-------|
| `llama3.2:3b` | Default (same weight class as LM Studio/NIM defaults below) |
| `qwen3:8b` | |

*Any other model you run in Ollama (or an Ollama-compatible server) can be used the same way. Runtime defaults in code use **`llama3.2:3b`**.*

### `"openai-compat"` — OpenAI-Compatible Servers (Supported)

This endpoint type covers LM Studio, NVIDIA NIM, NEAR AI, and any other server speaking the OpenAI chat completions protocol. Models are identified by the id expected by the server. The specific model list depends on which product and deployment you are using.

#### LM Studio

Models are identified by the model id shown in LM Studio (e.g. from the in-app list or `GET /v1/models`). Provider config: `endpointType: "openai-compat"`, `modelDiscovery: "lmstudio"`. Optional `baseUrl` (default `http://127.0.0.1:1234/v1`). Retry on "unloaded" error is automatic with `modelDiscovery: "lmstudio"`.

| Model id (example) | Notes |
|--------------------|-------|
| `llama-3.2-3B-instruct` | Example model id |
| `openai/gpt-oss-20b` | Larger alternative |

*Any model loaded in LM Studio can be used; the id is shown in the LM Studio UI or via the API (and may include a provider prefix like `openai/`).*

#### NVIDIA NIM (Hosted)

Provider config: `endpointType: "openai-compat"`, `baseUrl: "https://integrate.api.nvidia.com/v1"`, `modelDiscovery: "static"`, `staticModels` array, `apiKey` / `NVIDIA_API_KEY`. Not a private deployment; see [NVIDIA_NIM.md](../ref/NVIDIA_NIM.md).

| Model | Notes |
|-------|-------|
| `meta/llama-3.2-3b-instruct` | Example model id |

*Static models list is user-curated via the `staticModels` config field — any model id from the [NVIDIA LLM APIs](https://docs.api.nvidia.com/nim/reference/llm-apis) docs works when added to the array.*

#### NearAI

Provider config: `endpointType: "openai-compat"`, `baseUrl: "https://cloud-api.near.ai/v1"`, `apiKey`.

| Model | Notes |
|-------|-------|
| `zai-org/GLM-5.1-FP8` | Example model id |

*Model ids depend on the NearAI model catalog. Standard OpenAI-compat discovery (`GET /v1/models`) is used by default.*

#### Other OpenAI-Compatible Servers

Any server exposing OpenAI-shaped routes can be configured as an `"openai-compat"` provider with the appropriate `baseUrl` and `apiKey`. This includes vLLM, Hugging Face TGI, OpenAI, and others. No special behavior fields are needed. See [OPENAI.md](../ref/OPENAI.md) for the `openai-compat` endpoint type reference.

## Repository Model Inventory

This section lists **every concrete model id** named in this repository (code, tests, agent docs, and journey guides), **where** it appears, and how it fits **local** (personal hardware), **self-hosted** (your infra), and **third-party / hosted API** deployment. It also records **Chai tool compatibility**: the agent loop expects the model to accept **function / tool calls** over the provider wire; models without tool support are usable only for **non-tool** roles (e.g. a worker that does not run `delegate_task` or skills that require tools).

### Local Device Eligibility (Parameter Budget)

For this document, **local** means a model that is **realistic to run on a typical personal laptop or desktop** for development. Treat **more than 8B parameters** (or an obviously larger / MoE variant such as 17B active, 70B, 120B+, 480B) as **not** "local"—use **self-hosted** (dedicated GPU / server) or **third-party API** instead. **8B and smaller** (including 7B, 3B-class, and "micro" small models) may be listed as local when used via **Ollama** or **LM Studio** on your machine.

### Context Index (Where Each Id Appears)

| Model id | Primary references |
|----------|-------------------|
| **`llama3.2:3b`** | Runtime default for `"ollama"` endpoint type: [`crates/lib/src/orchestration/model.rs`](../../crates/lib/src/orchestration/model.rs), [`crates/lib/src/agent.rs`](../../crates/lib/src/agent.rs), [README.md](../../README.md), journey guides (`docs/journey/`), [`crates/lib/src/config.rs`](../../crates/lib/src/config.rs) |
| **`llama-3.2-3B-instruct`** | LM Studio fallback when model unset: [`crates/lib/src/orchestration/model.rs`](../../crates/lib/src/orchestration/model.rs), [`crates/lib/src/config.rs`](../../crates/lib/src/config.rs) |
| **`meta/llama-3.2-3b-instruct`** | NIM example model id in docs; runtime default for NIM is provider-configured via `staticModels` |
| **`qwen3:8b`** | [OLLAMA.md](../ref/OLLAMA.md) |
| **`openai/gpt-oss-20b`** | [LM Studio](#lm-studio), [LM_STUDIO.md](../ref/LM_STUDIO.md) |
| **`gpt-4o-mini`**, **`gpt-4o`** | [OPENAI.md](../ref/OPENAI.md), [`crates/lib/src/config.rs`](../../crates/lib/src/config.rs) |
| **`zai-org/GLM-5.1-FP8`** | NearAI example model id in docs |

### Master Table: Deployment Category and Chai Fit

**Legend:** **Local** = suitable on a personal device under the **≤8B** (or small / micro) rule above. **Self-hosted** = your servers or private endpoint (any size). **Third-party API** = vendor-hosted HTTP API (NearAI, NVIDIA NIM hosted, OpenAI, etc.). A given architecture can use the **same** open-weight id locally, on your GPU cluster, or via a cloud API—columns indicate where that id is **named in this repo**, not every possible deployment.

| Model id | ~Params (guide) | Named as local (≤8B rule) | Self-hosted in docs/code | Third-party API | Tool calling (for Chai agent + skills) |
|----------|-----------------|----------------------------|---------------------------|-----------------|----------------------------------------|
| `llama3.2:3b` | ~3B class (Ollama tag) | Yes (Ollama) | If you serve the same weights | — | Yes on Ollama when the tag supports tools |
| `llama3.2:latest` | Ollama tag (size varies) | Yes (Ollama) | If you serve the same weights | — | Yes when the tag supports tools |
| `qwen3:8b` | 8B | Yes | Yes | — | Yes if tool-capable build |
| `openai/gpt-oss-20b` | 20B | **No** | Yes (LM Studio / openai-compat) | If offered | Depends; test |
| `gpt-4o-mini`, `gpt-4o` | — | **No** (API-only in repo) | — | Yes (OpenAI) | Yes |
| `zai-org/GLM-5.1-FP8` | — | **No** | — | Yes (NearAI) | Yes (OpenAI-compat) |
| NIM static models: `meta/llama-3.1-8b-instruct` | 8B | Yes *if* you run equivalent locally | Yes | Yes (NIM) | Yes via OpenAI-compat |
| NIM static models: `meta/llama-3.1-70b-instruct`, etc. | Large | **No** | Yes | Yes (NIM) | Varies; large instruct models usually support tools on NIM |
