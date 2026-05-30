## Agents

In Chai, **agents** hold **configuration** for the assistant the gateway runs: they name the **orchestrator** that owns the conversation, optionally define **workers** for delegated subtasks, and set **defaults** for which **provider** and **model** to use, how **model discovery** is scoped, and per-role **skills** (**`skillsEnabled`**, **`contextMode`**). On disk, each agent’s **`AGENT.md`** lives in that agent’s **context directory** at **`<active-profile>/agents/<agentId>/AGENT.md`** (no path override in config). An agent is not a separate service or binary—the **`agents`** block is **configuration** the gateway reads to route each turn and assemble **context**. **Skills** supply instructions and optional tools; top-level **`providers`** supply URLs and API keys; the **`agents`** block ties those inputs to one orchestrator and any workers you define. **Delegation** allowlists, caps, and routes are **policy** on top of that configuration (see [base/spec/ORCHESTRATION.md](base/spec/ORCHESTRATION.md)).

### Agent Orchestration

Each entry in **`agents`** has a unique **`id`**, a **`role`** (`orchestrator` or `worker`), and the optional fields listed under **Configuration → Agents** above. The gateway uses this to route turns to the right backend, pass model ids to each provider, decide which APIs to poll for model discovery, and load **`AGENT.md`** from **`<active-profile>/agents/<id>/`**. With workers configured, the orchestrator can delegate subtasks using the built-in **`delegate_task`** tool.

**`chai init`** creates **`agents/orchestrator/AGENT.md`** for the default orchestrator id. Edit that file (or add **`agents/<workerId>/AGENT.md`** for workers) to customize on-disk agent context; see **Agent Context On Disk** below.

**Multi-agent example** — only the **`agents`** array; orchestration agent and worker agents:

```json
"agents": [
  {
    "id": "assistant",
    "role": "orchestrator",
    "defaultProvider": "ollama",
    "defaultModel": "llama3.2:3b",
    "enabledProviders": ["ollama", "lms"]
  },
  {
    "id": "engineer",
    "role": "worker",
    "defaultProvider": "lms",
    "defaultModel": "ibm/granite-4-micro",
    "enabledProviders": ["lms"]
  },
  {
    "id": "researcher",
    "role": "worker",
    "defaultProvider": "lms",
    "defaultModel": "ibm/granite-4-micro",
    "enabledProviders": ["lms"]
  }
]
```

### Providers and Models

The gateway integrates **six** model **backends** (named by **`agents.defaultProvider`**): **Ollama** (native Ollama API), **LM Studio** (`lms`, OpenAI-compatible local server), **vLLM** (OpenAI-compatible **`vllm serve`** for self-hosted inference), **Hugging Face** (`hf`, OpenAI-compatible Inference Endpoints, TGI, or similar), **NVIDIA NIM** (`nim`, hosted NVIDIA catalog API), **OpenAI** (`openai`, and OpenAI HTTP API or compatible base URL). They differ in **where** the model runs (your machine, your infrastructure, or a cloud API), **which** wire protocol and discovery endpoints Chai uses, and **whether** an API key or fixed base URL applies.

For **provider** taxonomy, configuration, and API comparisons, see [base/spec/PROVIDERS.md](base/spec/PROVIDERS.md). For **model** ids, repository inventory, and tool-fit notes, see [base/spec/MODELS.md](base/spec/MODELS.md). For the **API alignment** roadmap, see [base/epic/API_ALIGNMENT.md](base/epic/API_ALIGNMENT.md). To run **repeatable model tests** by deployment category, see [testing](docs/testing/README.md). Endpoint-level detail and how Chai calls each API are in the per-backend references:

| Backend | Document |
|---------|----------|
| Ollama (`ollama`) | [base/ref/OLLAMA.md](base/ref/OLLAMA.md) |
| LM Studio (`lms`) | [base/ref/LM_STUDIO.md](base/ref/LM_STUDIO.md) |
| vLLM (`vllm`) | [base/ref/VLLM.md](base/ref/VLLM.md) |
| Hugging Face (`hf`) | [base/ref/HUGGINGFACE.md](base/ref/HUGGINGFACE.md) |
| NVIDIA NIM (`nim`) | [base/ref/NVIDIA_NIM.md](base/ref/NVIDIA_NIM.md) |
| OpenAI (`openai`) | [base/ref/OPENAI.md](base/ref/OPENAI.md) |

Set **`defaultProvider`** on the orchestrator entry to **`ollama`**, **`lms`**, **`vllm`**, **`hf`**, **`nim`**, or **`openai`** when no per-request override is used. Optional **`enabledProviders`** on the orchestrator entry lists which providers to poll for model discovery at startup (e.g. `["ollama", "lms", "vllm", "hf", "nim", "openai"]`). When absent or empty, only the default provider (`ollama`) is discovered.


Use the exact model id expected by the selected provider for **`defaultModel`**:

- For `ollama`, use the name from `ollama list` (e.g. `llama3.2:3b`, `qwen3:8b`).
- For `lms`, use the id from the LM Studio UI or **`GET …/api/v1/models`** on the LM Studio server (e.g. `llama-3.2-3B-instruct`, `openai/gpt-oss-20b`).
- For `vllm`, use the same id you pass to `vllm serve` (e.g. `Qwen/Qwen2.5-7B-Instruct`).
- For `hf`, use the model id your endpoint expects (e.g. `meta-llama/Llama-3.1-8B-Instruct`).
- For `nim`, use a NIM catalog id (e.g. `meta/llama-3.2-3b-instruct`); see [LLM APIs reference](https://docs.api.nvidia.com/nim/reference/llm-apis).
- For `openai`, use an OpenAI model id (e.g. `gpt-4o-mini`); see [OpenAI models](https://platform.openai.com/docs/models).

### Agent Context On Disk

Each profile stores per-agent instructions under **`agents/<agentId>/`** (the **agent context directory** for that **`id`**). The file is always **`AGENT.md`** in that directory. **`chai init`** creates **`agents/orchestrator/AGENT.md`** for the default orchestrator id.

- **`AGENT.md`** — Agent-level context for that role; the gateway prepends it to the skills block on each turn.
