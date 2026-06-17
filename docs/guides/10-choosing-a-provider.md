# Choosing a Provider and Model

Chai supports multiple model backends with different tradeoffs in cost, privacy, capability, and hardware requirements. This guide helps you choose the right provider and model for your use case.

## Decision Framework

The main factors to consider:

| Factor | Question |
|--------|----------|
| **Privacy** | Must all data stay on your machine, or can it leave for cloud inference? |
| **Hardware** | Do you have a GPU? How much VRAM? |
| **Capability** | Does your workflow need tool calling, long context, or complex reasoning? |
| **Cost** | Is there a budget for API usage, or is free/local preferred? |

## Local Providers

### Ollama (Default)

Ollama is the default provider — no configuration required. Best for:

- **Privacy-first** workflows where no data leaves your machine
- **Quick experimentation** with no API key setup
- **Offline use** once a model is downloaded

**Requirements:** Install [Ollama](https://ollama.com) and pull a model. Ollama supports CPU inference (slow) and GPU acceleration (fast). For comfortable performance, a GPU with 8 GB+ VRAM is recommended for 7B–8B models, 16 GB+ for 13B models, and 24 GB+ for 70B models.

```bash
ollama pull llama3.2:3b    # Small, fast, good for basic tasks
ollama pull qwen3:8b       # Mid-range, better reasoning
ollama pull llama3.1:70b   # Large, near cloud-level quality
```

**Model sizing rule of thumb:**

| Model size | VRAM needed | Quality | Speed |
|-----------|-------------|---------|-------|
| 3B | 2–4 GB | Basic | Fast |
| 7B–8B | 6–8 GB | Good | Fast |
| 13B–30B | 12–24 GB | Very good | Moderate |
| 70B+ | 40+ GB | Excellent | Slow |

For tool calling specifically, 7B+ models are recommended. Smaller models may struggle with structured tool arguments.

### LM Studio

LM Studio provides a GUI for downloading and running models locally. It exposes an OpenAI-compatible API that chai connects to. Best for:

- **GUI-driven model management** — browse, download, and load models from inside the app
- **Auto-loading** — chai can request an unloaded model and LM Studio will load it automatically
- **Same privacy guarantees** as Ollama (all data stays local)

**Requirements:** Install [LM Studio](https://lmstudio.ai), download a model, and start the local server.

Configuration:

```json
{
  "providers": [
    { "id": "lms", "endpointType": "openai-compat", "modelDiscovery": "lmstudio" }
  ],
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "lms",
      "defaultModel": "openai/gpt-oss-20b"
    }
  ]
}
```

## Cloud Providers

Cloud providers offer larger, more capable models without local hardware requirements. Data is sent to the provider's API — consider your privacy requirements before using cloud backends.

### NearAI

NearAI provides OpenAI-compatible cloud inference. Best for:

- **No local GPU** — run capable models in the cloud
- **Large model quality** without the hardware investment
- **Persistent API access** — set an API key and go

Configuration:

```json
{
  "providers": [
    { "id": "nearai", "endpointType": "openai-compat", "baseUrl": "https://cloud-api.near.ai/v1" }
  ],
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "nearai",
      "defaultModel": "zai-org/GLM-5.1-FP8"
    }
  ]
}
```

Set the API key via the `apiKey` field in the provider object.

### NVIDIA NIM

NVIDIA NIM provides optimized cloud inference for select models. It does not expose a `/v1/models` endpoint, so you must provide a static model list. Best for:

- **NVIDIA-optimized models** — fast inference on NVIDIA infrastructure
- **Specific model selection** — you know exactly which models you want

Configuration:

```json
{
  "providers": [
    {
      "id": "nim",
      "endpointType": "openai-compat",
      "baseUrl": "https://integrate.api.nvidia.com/v1",
      "modelDiscovery": "static",
      "staticModels": ["meta/llama-3.1-8b-instruct", "deepseek-ai/deepseek-v3.1"]
    }
  ],
  "agents": [
    {
      "id": "orchestrator",
      "role": "orchestrator",
      "defaultProvider": "nim",
      "defaultModel": "meta/llama-3.1-8b-instruct"
    }
  ]
}
```

Set `"apiKey"` to a literal key string or an environment variable reference like `"<NVIDIA_API_KEY>"` (the named variable is read from the shell environment or a `.env` file in the profile directory).

### Other OpenAI-Compatible APIs

Any service that exposes OpenAI-shaped routes (`/v1/chat/completions`, `/v1/models`) can be configured as an `"openai-compat"` provider with the appropriate `baseUrl` and `apiKey`. This includes OpenAI itself, Azure OpenAI, Together, Groq, vLLM, Hugging Face TGI, and more.

## Skill Capability Tiers

When choosing a model, consider the **capability tier** of the skills you plan to use. Each skill declares a tier that indicates the minimum model quality needed:

| Tier | Model size | Skills example |
|------|-----------|----------------|
| `minimal` | 7B+ | `files-read`, `git-read`, `git-remote`, `notes-daily`, `logs`, `skills-read` |
| `moderate` | 13B–30B | `git`, `notes`, `notes-frontmatter`, `notes-wikilink`, `rss` |
| `full` | 70B+ or cloud | `files`, `skills` |

The gateway warns at startup when an enabled skill's tier exceeds the likely capability of the configured model. If you see these warnings, consider either:

- Switching to a larger local model (if your hardware supports it)
- Using a cloud provider for the agent with high-tier skills
- Using a lower-tier variant of the skill (e.g., `files-read` instead of `files`)

See [Skills → Skill Variants](06-skills.md#skill-variants) for the full variant table.

## Multi-Provider Setups

You can configure multiple providers and assign different agents to different backends. A common pattern:

- **Orchestrator** uses a cloud provider for high-quality reasoning
- **Workers** use a local provider for fast, private, task-specific work

```json
{
  "providers": [
    { "id": "ollama", "endpointType": "ollama" },
    { "id": "nearai", "endpointType": "openai-compat", "baseUrl": "https://cloud-api.near.ai/v1" }
  ],
  "agents": [
    {
      "id": "assistant",
      "role": "orchestrator",
      "defaultProvider": "nearai",
      "enabledProviders": ["nearai", "ollama"]
    },
    {
      "id": "local-worker",
      "role": "worker",
      "defaultProvider": "ollama"
    }
  ]
}
```

## Recommendations by Use Case

| Use Case | Provider | Model | Reasoning |
|----------|----------|-------|-----------|
| First time / testing | Ollama | `llama3.2:3b` | Default, zero config, fast download |
| Privacy-critical work | Ollama or LM Studio | 7B–13B local | No data leaves the machine |
| Full skill suite (files, skills) | Cloud or 70B+ local | `full`-tier model | High-tier skills need strong reasoning |
| Low-power hardware | NearAI or NVIDIA NIM | Cloud model | No local GPU needed |
| Multi-agent with delegation | Mixed (cloud + local) | Orchestrator on cloud, workers local | Cost-effective delegation |

For systematic model and provider testing, see the [Testing Playbooks](../testing/README.md).
