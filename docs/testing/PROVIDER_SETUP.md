# Provider Setup

Setup instructions for each provider used in the testing playbooks. Each playbook references this file instead of duplicating provider-specific steps.

## Ollama

Ollama runs locally and requires no API key.

1. **Install Ollama** from [ollama.com](https://ollama.com) or via package manager.
2. **Pull the model** you want to test:

   ```sh
   ollama pull llama3.1:8b
   ```

3. **Verify the model is available:**

   ```sh
   ollama list
   ```

4. **Configure the provider** — Ollama is the default; an empty config works:

   ```json
   {}
   ```

   Or explicitly:

   ```json
   {
     "providers": [{ "id": "ollama", "endpointType": "ollama" }],
     "agents": [{ "id": "orchestrator", "role": "orchestrator", "defaultProvider": "ollama", "defaultModel": "llama3.1:8b" }]
   }
   ```

5. **Default base URL:** `http://127.0.0.1:11434` (no configuration needed unless Ollama is on a different host/port).

## LM Studio

LM Studio runs locally and uses the `"openai-compat"` endpoint type with LM Studio–specific behavior fields.

1. **Install LM Studio** from [lmstudio.ai](https://lmstudio.ai).
2. **Enable developer mode** — open LM Studio → Settings → Developer → toggle "Enable Developer Mode".
3. **Set the runtime** — in Developer settings, set the runtime to CPU (or GPU if available).
4. **Download a model** — use the LM Studio UI search to download a model (e.g. `llama-3.2-3B-instruct`).
5. **Load the model** — either via the LM Studio UI or the CLI:

   ```sh
   lms load <model path>
   ```

   If the model fails to load due to VRAM, try:

   ```sh
   lms load <model path> --gpu 0.5
   ```

6. **Configure the provider:**

   ```json
   {
     "providers": [{ "id": "lms", "endpointType": "openai-compat", "modelDiscovery": "lmstudio" }],
     "agents": [{ "id": "orchestrator", "role": "orchestrator", "defaultProvider": "lms", "defaultModel": "llama-3.2-3B-instruct" }]
   }
   ```

**Gotchas:**

- LM Studio must be running before starting the gateway.
- Developer settings must be enabled with the runtime set appropriately.
- Models must be manually loaded before the gateway sends a chat request (automatic retry on unload handles this for one model when `modelDiscovery: "lmstudio"` is set).
- `modelDiscovery: "lmstudio"` uses LM Studio's native `GET /api/v1/models` endpoint (filters by `type == "llm"`).
- Automatic retry on unload: when `modelDiscovery: "lmstudio"` is set, the gateway automatically loads an unloaded model and retries when LM Studio returns an "unloaded" error.
- All LM Studio models accept the tools parameter, but some models are not trained on tool use — results may be unreliable.

## NVIDIA NIM

NVIDIA NIM is a remote OpenAI-compatible API (free tier) that requires an API key.

1. **Create an NVIDIA account** at [build.nvidia.com](https://build.nvidia.com).
2. **Generate an API key** from the build.nvidia.com dashboard.
3. **Configure the provider** — NIM does not expose a model list endpoint, so use `modelDiscovery: "static"` with a curated `staticModels` list:

   ```json
   {
     "providers": [{
       "id": "nim",
       "endpointType": "openai-compat",
       "baseUrl": "https://integrate.api.nvidia.com/v1",
       "modelDiscovery": "static",
       "staticModels": ["meta/llama-3.1-8b-instruct", "meta/llama-3.1-70b-instruct"],
       "apiKey": "<NVIDIA_API_KEY>"
     }],
     "agents": [{ "id": "orchestrator", "role": "orchestrator", "defaultProvider": "nim", "defaultModel": "meta/llama-3.1-8b-instruct" }]
   }
   ```

4. **API key:** The `apiKey` value `<NVIDIA_API_KEY>` is an environment variable reference — the gateway reads the `NVIDIA_API_KEY` variable at startup. Set it in your shell environment or in a `.env` file in the profile directory (e.g. `~/.chai/profiles/assistant/.env`). You can also use a literal key string instead: `"apiKey": "nvapi-..."`.

**Gotchas:**

- **Privacy:** NVIDIA NIM hosted API is not a privacy-preserving option. All requests and conversation data are sent to NVIDIA's servers. Use it as a free scratchpad to try open-source models before investing in local or self-hosted hardware.
- **Rate limits:** The free tier allows approximately 40 requests per minute. Expect 429 responses under heavier use.
- **Model ids:** Must match the NIM catalog exactly (e.g. `meta/llama-3.1-8b-instruct`). See the [NVIDIA LLM APIs reference](https://docs.api.nvidia.com/nim/reference/llm-apis) for the full catalog.

## NearAI

NearAI is a remote OpenAI-compatible API that requires an API key.

1. **Create a NearAI account** at [near.ai](https://near.ai).
2. **Generate an API key** from the NearAI dashboard.
3. **Configure the provider:**

   ```json
   {
     "providers": [{ "id": "nearai", "endpointType": "openai-compat", "baseUrl": "https://cloud-api.near.ai/v1", "apiKey": "<NEAR_API_KEY>" }],
     "agents": [{ "id": "orchestrator", "role": "orchestrator", "defaultProvider": "nearai", "defaultModel": "zai-org/GLM-5.1-FP8" }]
   }
   ```

4. **API key:** The `apiKey` value `<NEAR_API_KEY>` is an environment variable reference — the gateway reads the `NEAR_API_KEY` variable at startup. Set it in your shell environment or in a `.env` file in the profile directory (e.g. `~/.chai/profiles/assistant/.env`). You can also use a literal key string instead: `"apiKey": "sk-..."`.
5. **Model catalog:** Check [near.ai](https://near.ai) for available models. Default model discovery uses `GET /v1/models`.

**Notes:**

- NearAI is a cloud service — conversation data is sent to NearAI's servers.
- Other OpenAI-compatible servers (vLLM, Hugging Face TGI, OpenAI, Azure OpenAI, etc.) follow the same `"openai-compat"` pattern with a different `baseUrl` and `apiKey`.
