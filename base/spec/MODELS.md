---
status: stable
---

# Models

Internal spec for **model identifiers** in Chai: examples per backend, cross-provider **families**, a **repository inventory** of every model id named in code and docs, and **tool-calling** fit for the agent loop. For **backend** configuration and wire protocols, see [PROVIDERS.md](PROVIDERS.md).

## Relationship to Other Documents

- **[API_ALIGNMENT.md](../epic/API_ALIGNMENT.md)** — Roadmap for backends and API families; this document is the canonical catalog of **named** model ids in this repository.
- **[PROVIDERS.md](PROVIDERS.md)** — `defaultProvider` ids, discovery, and compatibility; pair with **MODELS.md** for **model** strings.
- **Testing procedures** live under **`/.testing/`** and are operational runbooks; this spec remains the canonical model taxonomy and inventory.

## Models by Provider

### Local — Ollama (supported)

Models are identified by the name used in `ollama list`. Set **`agents.defaultProvider`** to **`"ollama"`** and **`agents.defaultModel`** to the model name (e.g. `llama3.2:3b`).

| Model            | Notes     |
|------------------|-----------|
| `llama3.2:3b`  | Default (same weight class as LMS/NIM defaults below)   |
| `qwen3:8b`       |           |

*Any other model you run in Ollama (or an Ollama-compatible server) can be used the same way. Runtime defaults in code use **`llama3.2:3b`**.*

### Local — LM Studio (supported)

Models are identified by the model id shown in LM Studio (e.g. from the in-app list or `GET /v1/models`). Set **`agents.defaultProvider`** to **`"lms"`** and **`defaultModel`** to the model id (e.g. `llama-3.2-3B-instruct`, `openai/gpt-oss-20b`). Optional **`providers.lms.baseUrl`** (default `http://127.0.0.1:1234/v1`).

| Model id (example) | Notes     |
|--------------------|-----------|
| `llama-3.2-3B-instruct` | Default (same weight class as Ollama/NIM defaults above) |
| `openai/gpt-oss-20b` | Larger alternative |
| `ibm/granite-4-micro` | Same |

*Any model loaded in LM Studio can be used; the id is shown in the LM Studio UI or via the API (and may include a provider prefix like `openai/` or `ibm/`).*

### Self-hosted — vLLM (supported)

Set **`agents.defaultProvider`** to **`"vllm"`** and **`defaultModel`** to the same id as `vllm serve`. Optional **`providers.vllm.baseUrl`** (default `http://127.0.0.1:8000/v1`), optional **`providers.vllm.apiKey`** / **`VLLM_API_KEY`**. See [VLLM.md](../ref/VLLM.md).

### Self-hosted — Hugging Face (supported)

Set **`agents.defaultProvider`** to **`"hf"`**, **`providers.hf.baseUrl`** to your OpenAI-compatible base including **`/v1`**, and **`defaultModel`** to the id your server expects. Optional **`HF_API_KEY`** / **`providers.hf.apiKey`**. See [HUGGINGFACE.md](../ref/HUGGINGFACE.md).

| Model                               | Notes     |
|-------------------------------------|-----------|
| `meta-llama/Llama-3.1-8B-Instruct`  | Default fallback in gateway when model unset |
| `Qwen/Qwen2.5-7B-Instruct`          |           |

### Hosted — NVIDIA NIM (supported)

Set **`agents.defaultProvider`** to **`"nim"`** and **`defaultModel`** to a NIM catalog id (default fallback **`meta/llama-3.2-3b-instruct`**, same weight class as Ollama/LMS defaults above). Not a private deployment; see [NVIDIA_NIM.md](../ref/NVIDIA_NIM.md).

### Third-party — OpenAI (supported)

Set **`agents.defaultProvider`** to **`"openai"`**, **`OPENAI_API_KEY`** or **`providers.openai.apiKey`**, and **`defaultModel`** to an OpenAI model id (e.g. `gpt-4o-mini`). Optional **`providers.openai.baseUrl`** for Azure-compatible gateways or proxies. See [OPENAI.md](../ref/OPENAI.md).

| Model        | Notes              |
|--------------|--------------------|
| `gpt-4o-mini` | Gateway fallback when model unset |
| `gpt-4o`     |                    |

*Use current OpenAI model ids from their documentation.*

## Model Families Across Providers

Cross-reference by family. **Supported** columns include backends that are implemented today; **Planned** lists APIs not yet integrated as dedicated providers.

| Family    | Local — Ollama | OpenAI-compat (lms, vllm, openai, hf, nim) | Planned (Claude / Gemini) |
|-----------|----------------|-------------------------------------------|---------------------------|
| **Llama** | `llama3.2:3b` | `llama-3.2-3B-instruct` (lms), `meta/llama-3.2-3b-instruct` (nim), `meta-llama/Llama-3.1-8B-Instruct` (example on `hf`) | — |
| **Qwen**  | `qwen3:8b`      | Various via vLLM / HF | — |
| **GPT**   | —               | `gpt-4o-mini`, etc. (`openai`) | — |

When new providers or models are added, extend this table so that popular models and backends remain comparable in one place.

## Repository Model Inventory

This section lists **every concrete model id** named in this repository (code, tests, agent docs, and journey guides), **where** it appears, and how it fits **local** (personal hardware), **self-hosted** (your infra), and **third-party / hosted API** deployment. It also records **Chai tool compatibility**: the agent loop expects the model to accept **function / tool calls** over the provider wire; models without tool support are usable only for **non-tool** roles (e.g. a worker that does not run `delegate_task` or skills that require tools).

### Local Device Eligibility (Parameter Budget)

For this document, **local** means a model that is **realistic to run on a typical personal laptop or desktop** for development. Treat **more than 8B parameters** (or an obviously larger / MoE variant such as 17B active, 70B, 120B+, 480B) as **not** “local”—use **self-hosted** (dedicated GPU / server) or **third-party API** instead. **8B and smaller** (including 7B, 3B-class, and “micro” small models) may be listed as local when used via **Ollama** or **LM Studio** on your machine. Names like `:latest` can resolve to different sizes over time; confirm with `ollama show` / your UI.

### Context Index (Where Each Id Appears)

| Model id | Primary references |
|----------|-------------------|
| **`llama3.2:3b`** | Runtime default for Ollama: [`crates/lib/src/orchestration/model.rs`](../../crates/lib/src/orchestration/model.rs), [`crates/lib/src/agent.rs`](../../crates/lib/src/agent.rs), [README.md](../../README.md), journey guides (`.journey/`), [`crates/lib/src/config.rs`](../../crates/lib/src/config.rs) |
| **`llama-3.2-3B-instruct`** | LM Studio fallback when model unset: [`crates/lib/src/orchestration/model.rs`](../../crates/lib/src/orchestration/model.rs), [`crates/lib/src/config.rs`](../../crates/lib/src/config.rs) |
| **`meta/llama-3.2-3b-instruct`** | NIM fallback when model unset: [`crates/lib/src/orchestration/model.rs`](../../crates/lib/src/orchestration/model.rs), [`crates/lib/src/providers/nim.rs`](../../crates/lib/src/providers/nim.rs) `static_model_list()` |
| **`llama3:latest`** | [Local — Ollama](#local--ollama-supported) — historical test matrix id; codebase defaults use **`llama3.2:3b`**. |
| **`qwen3:8b`** | [OLLAMA.md](../ref/OLLAMA.md), [README.md](../../README.md), [Local — Ollama](#local--ollama-supported) |
| **`gpt-oss-20b`** | LM Studio example in docs; runtime default is **`llama-3.2-3B-instruct`**: [`crates/lib/src/orchestration/model.rs`](../../crates/lib/src/orchestration/model.rs) |
| **`openai/gpt-oss-20b`** | [Local — LM Studio](#local--lm-studio-supported), [LM_STUDIO.md](../ref/LM_STUDIO.md), [README.md](../../README.md) |
| **`ibm/granite-4-micro`** | [Local — LM Studio](#local--lm-studio-supported), [README.md](../../README.md), [ORCHESTRATION.md](../epic/ORCHESTRATION.md), [`crates/lib/src/config.rs`](../../crates/lib/src/config.rs) tests |
| **`granite`** | Short id in [`crates/lib/src/orchestration/policy.rs`](../../crates/lib/src/orchestration/policy.rs) tests only |
| **`Qwen/Qwen2.5-7B-Instruct`** | vLLM fallback and docs: [`crates/lib/src/orchestration/model.rs`](../../crates/lib/src/orchestration/model.rs), [`crates/lib/src/config.rs`](../../crates/lib/src/config.rs), [VLLM.md](../ref/VLLM.md), [README.md](../../README.md). |
| **`meta-llama/Llama-3.1-8B-Instruct`** | HF fallback: [`crates/lib/src/orchestration/model.rs`](../../crates/lib/src/orchestration/model.rs), [HUGGINGFACE.md](../ref/HUGGINGFACE.md). |
| **`qwen/qwen3.5-122b-a10b`** | NIM catalog example; [`crates/lib/src/providers/nim.rs`](../../crates/lib/src/providers/nim.rs) `static_model_list()` — runtime default for NIM is **`meta/llama-3.2-3b-instruct`**. |
| **NIM static catalog entries** | [`crates/lib/src/providers/nim.rs`](../../crates/lib/src/providers/nim.rs) `static_model_list()`; gateway **`nimModels`** also merges optional **`providers.nim.extraModels`** (see [README.md](../../README.md)) |
| **`gpt-4o-mini`**, **`gpt-4o`** | [Third-party — OpenAI](#third-party--openai-supported), [OPENAI.md](../ref/OPENAI.md), [`crates/lib/src/config.rs`](../../crates/lib/src/config.rs) |
| **`gpt-5.2`**, **`gpt-5.1`**, **`gpt-5.1-mini`**, **`gpt-5-mini`** | OpenAI test-matrix ids used in operational testing docs under `/.testing/`. |
| **`llama3.2:latest`**, **`ibm/granite-4-micro`** (delegation) | Orchestrator/worker examples: [`crates/lib/src/config.rs`](../../crates/lib/src/config.rs) tests, [ORCHESTRATION.md](../epic/ORCHESTRATION.md) |
| **`nvidia/nemotron-3-super-120b-a12b`** | [NEMOCLAW.md](../ref/NEMOCLAW.md) (external NemoClaw / OpenShell stack, not a Chai default) |

### Master Table: Deployment Category and Chai Fit

**Legend:** **Local** = suitable on a personal device under the **≤8B** (or small / micro) rule above. **Self-hosted** = your servers or private endpoint (any size). **Third-party API** = vendor-hosted HTTP API (OpenAI, NVIDIA NIM hosted, Venice, etc.). A given architecture can use the **same** open-weight id locally, on your GPU cluster, or via a cloud API—columns indicate where that id is **named in this repo**, not every possible deployment.

| Model id | ~Params (guide) | Named as local (≤8B rule) | Self-hosted in docs/code | Third-party API | Tool calling (for Chai agent + skills) |
|----------|-----------------|----------------------------|---------------------------|-----------------|----------------------------------------|
| `llama3.2:3b` | ~3B class (Ollama tag) | Yes (Ollama) | If you serve the same weights | — | Yes on Ollama when the tag supports tools |
| `llama3.2:latest` | Ollama tag (size varies) | Yes (Ollama) | If you serve the same weights | — | Yes when the tag supports tools |
| `qwen3:8b` | 8B | Yes | Yes | — | Yes if tool-capable build |
| `ibm/granite-4-micro` | Small / micro | Yes (LM Studio) | Yes | — | Depends on build; test |
| `openai/gpt-oss-20b` / `gpt-oss-20b` | 20B | **No** | Yes (LM Studio / vLLM) | If offered | Depends; test |
| `Qwen/Qwen2.5-7B-Instruct` | 7B | Yes (if run locally via Ollama/LMS) | Yes (vLLM/HF default examples) | — | Typically yes (OpenAI-compat tools) |
| `meta-llama/Llama-3.1-8B-Instruct` | 8B | Yes | Yes (HF / TGI / vLLM) | — | Typically yes |
| `gpt-4o-mini`, `gpt-4o` | — | **No** (API-only in repo) | — | Yes (OpenAI) | Yes |
| `gpt-5.2`, `gpt-5.1`, `gpt-5.1-mini`, `gpt-5-mini` | — | **No** | — | Yes (tests assume OpenAI) | Yes (flagship / mini variants) |
| NIM list: `meta/llama-3.1-8b-instruct` | 8B | Yes *if* you run equivalent locally | Yes | Yes (NIM) | Yes via OpenAI-compat |
| NIM list: `meta/llama-3.1-70b-instruct`, `meta/llama-4-*`, `deepseek-ai/deepseek-v3_*`, `qwen/qwen3.5-122b-a10b`, `qwen/qwen3-coder-480b-*`, `qwen/qwen3-next-80b-*` | Large | **No** | Yes | Yes (NIM) | Varies; large instruct models usually support tools on NIM |

### Maintenance

When you add a new **default**, **test**, or **example** model id anywhere in the repo, add a row to the **Context Index** and **Master Table** above (or a bullet under **Models Not Named in Repo** if it is only discussed narratively).
