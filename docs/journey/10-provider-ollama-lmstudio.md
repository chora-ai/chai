# Journey: Provider — local and cloud

**Goal:** Configure chai to use different model providers beyond the default Ollama setup, verify model discovery works for each, and confirm chat succeeds. Covers switching models, adding a second local provider (LM Studio), and connecting to cloud providers (NearAI and NVIDIA NIM).

**Background:** [Configuration → Providers](../guides/03-configuration.md#configuring-providers) · [Choosing a Provider and Model](../guides/10-choosing-a-provider.md)

Journey 00 uses the default Ollama configuration (empty `config.json`). This journey goes deeper: switching the default model, adding LM Studio as a second local provider, and configuring cloud providers for when you don't have local GPU hardware or want to test larger models.

Each part is independent — complete only the ones relevant to your setup.

## Prerequisites

- **Setup complete** — You have installed chai, run `chai init`, and verified the gateway works with defaults (see [00-setup-init.md](00-setup-init.md)).
- **Ollama** running with at least one model (e.g. `ollama pull llama3.2:3b`). For Part A, pull a second model:
  ```bash
  ollama pull qwen3:8b
  ```

Additional prerequisites per part:
- **Part B (LM Studio):** Install from [lmstudio.ai](https://lmstudio.ai), download a model, and start the local server.
- **Part C (NearAI):** A NearAI account and API key from [near.ai](https://near.ai).
- **Part D (NVIDIA NIM):** A NVIDIA build account and API key from [build.nvidia.com](https://build.nvidia.com).

## Part A: Switch the Default Model (Ollama)

1. **Edit config.json**
   - Open `~/.chai/profiles/assistant/config.json` and set the default model:
   ```json
   {
     "agents": [
       {
         "id": "orchestrator",
         "role": "orchestrator",
         "defaultModel": "qwen3:8b"
       }
     ]
   }
   ```
   - The `defaultProvider` is omitted because `"ollama"` is the built-in default.

2. **Start the gateway**
   - Stop any running gateway first, then:
   ```bash
   chai gateway
   ```
   - **Expect:** Log line showing `provider ollama discovered N model(s)` including `qwen3:8b`.

3. **Verify the model is used**
   - In another terminal:
   ```bash
   chai chat
   ```
   - Send: "What model are you? Reply with just your model name."
   - **Expect:** A reply. The model may or may not know its own name, but the gateway logs will confirm which model was called.

4. **Revert to the default model (optional)**
   - Remove `defaultModel` from config or set it back to `"llama3.2:3b"`, then restart the gateway.

## Part B: Add LM Studio as a Provider

5. **Start LM Studio**
   - Open LM Studio, load a model, and start the local server (default: `http://127.0.0.1:1234/v1`).

6. **Edit config.json**
   - Add LM Studio as an OpenAI-compatible provider:
   ```json
   {
     "providers": [
       { "id": "ollama", "endpointType": "ollama" },
       { "id": "lms", "endpointType": "openai-compat", "modelDiscovery": "lmstudio" }
     ],
     "agents": [
       {
         "id": "orchestrator",
         "role": "orchestrator",
         "defaultProvider": "lms",
         "defaultModel": "openai/gpt-oss-20b",
         "enabledProviders": ["ollama", "lms"]
       }
     ]
   }
   ```
   - `modelDiscovery: "lmstudio"` uses LM Studio's native model list endpoint (`GET /api/v1/models`). When this is set, the gateway also automatically retries chat requests on "unloaded" errors.
   - `enabledProviders` tells the gateway to poll both providers for model discovery.

7. **Start the gateway**
   ```bash
   chai gateway
   ```
   - **Expect:** Log lines showing `provider ollama discovered N model(s)` and `provider lms discovered N model(s)`.

8. **Chat via LM Studio**
   ```bash
   chai chat
   ```
   - Send: "Say hello in one short sentence."
   - **Expect:** A reply from the LM Studio model.

9. **Revert config** before continuing to the next part.

## Part C: Add NearAI as a Cloud Provider

NearAI provides OpenAI-compatible cloud inference. Unlike the local providers, data is sent to NearAI's servers — consider your privacy requirements before using cloud backends.

10. **Set up your API key**
    - Create an account at [near.ai](https://near.ai) and generate an API key.
    - Either add it to the provider config (see step 11) or set it as an environment variable:
      ```bash
      export NEARAI_API_KEY="your-key-here"
      ```

11. **Edit config.json**
    ```json
    {
      "providers": [
        { "id": "ollama", "endpointType": "ollama" },
        { "id": "nearai", "endpointType": "openai-compat", "baseUrl": "https://cloud-api.near.ai/v1", "apiKey": "your-key-here" }
      ],
      "agents": [
        {
          "id": "orchestrator",
          "role": "orchestrator",
          "defaultProvider": "nearai",
          "defaultModel": "zai-org/GLM-5.1-FP8",
          "enabledProviders": ["ollama", "nearai"]
        }
      ]
    }
    ```
    - `baseUrl` must be set explicitly — the default for `"openai-compat"` is `http://127.0.0.1:1234/v1` (a local address).
    - `apiKey` can be omitted if you set the `NEARAI_API_KEY` environment variable (though chai doesn't have a provider-specific env var for NearAI — use the config field or a generic env var approach).
    - `defaultModel` must be a model id from the [NearAI model catalog](https://near.ai). The exact id format differs from Ollama (e.g. `zai-org/GLM-5.1-FP8` instead of `llama3.2:3b`).

12. **Start the gateway**
    ```bash
    chai gateway
    ```
    - **Expect:** Log lines showing `provider ollama discovered N model(s)` and `provider nearai discovered N model(s)`. NearAI uses `GET /v1/models` for discovery (the default for `"openai-compat"`).

13. **Chat via NearAI**
    ```bash
    chai chat
    ```
    - Send: "Say hello in one short sentence."
    - **Expect:** A reply from the NearAI model. First inference may take a few seconds while the model loads on the remote server.

14. **Revert config** before continuing to the next part.

## Part D: Add NVIDIA NIM as a Cloud Provider

NVIDIA NIM provides optimized cloud inference for select models. Like NearAI, data is sent to NVIDIA's servers — this is not a privacy-preserving option. NIM's free tier allows approximately 40 requests per minute.

NIM does **not** expose a `/v1/models` endpoint, so you must provide a static model list in config.

15. **Set up your API key**
    - Create an account at [build.nvidia.com](https://build.nvidia.com) and generate an API key.
    - Set the environment variable in your shell:
      ```bash
      export NVIDIA_API_KEY="your-key-here"
      ```
      Or create a `.env` file in your profile directory:
      ```bash
      echo 'NVIDIA_API_KEY=your-key-here' >> ~/.chai/profiles/assistant/.env
      ```

16. **Edit config.json**
    ```json
    {
      "providers": [
        { "id": "ollama", "endpointType": "ollama" },
        {
          "id": "nim",
          "endpointType": "openai-compat",
          "baseUrl": "https://integrate.api.nvidia.com/v1",
          "modelDiscovery": "static",
          "staticModels": [
            "meta/llama-3.1-8b-instruct",
            "meta/llama-3.1-70b-instruct",
            "deepseek-ai/deepseek-v3.1"
          ],
          "apiKey": "<NVIDIA_API_KEY>"
        }
      ],
      "agents": [
        {
          "id": "orchestrator",
          "role": "orchestrator",
          "defaultProvider": "nim",
          "defaultModel": "meta/llama-3.1-8b-instruct",
          "enabledProviders": ["ollama", "nim"]
        }
      ]
    }
    ```
    - `modelDiscovery: "static"` — NIM has no model list endpoint. Instead, you curate the list in `staticModels`.
    - `staticModels` — Model ids from the [NVIDIA LLM APIs reference](https://docs.api.nvidia.com/nim/reference/llm-apis). Add or remove models as the catalog changes.
    - `apiKey: "<NVIDIA_API_KEY>"` — An environment variable reference. The gateway reads `NVIDIA_API_KEY` at startup. You can also use a literal key string instead: `"apiKey": "nvapi-..."`.

17. **Start the gateway**
    ```bash
    chai gateway
    ```
    - **Expect:** Log lines showing `provider ollama discovered N model(s)` and `provider nim discovered N model(s)`. For NIM, "discovered" means the static models are loaded from config (no HTTP polling occurs).
    - **Expect:** A startup warning if the NIM provider is the default — this reminds you that data is sent to NVIDIA's servers and the free tier has rate limits.

18. **Chat via NIM**
    ```bash
    chai chat
    ```
    - Send: "Say hello in one short sentence."
    - **Expect:** A reply from the NIM model. If you hit the rate limit, you will see an error — wait a minute and retry.

19. **Revert config** when done.

## If Something Fails

### General

- **Gateway exits immediately after config change** — The config JSON may be malformed. Validate: `cat ~/.chai/profiles/assistant/config.json | python3 -m json.tool`.
- **Agent turn failed after switching models** — The model id in `defaultModel` doesn't match any available model at the provider. Double-check spelling and format.

### Ollama

- **"provider ollama discovered 0 model(s)"** — Ollama is not running or has no models pulled. Run `ollama list` to verify. Pull a model: `ollama pull llama3.2:3b`.

### LM Studio

- **"provider lms discovered 0 model(s)"** — LM Studio local server is not running or no model is loaded. Start the server and load a model. Verify: `curl http://127.0.0.1:1234/v1/models`.
- **Model not found / "unloaded" error** — The `defaultModel` value must match what LM Studio reports. Check `GET /api/v1/models` or the LM Studio UI for the exact model id format. If the model exists but is unloaded, ensure `modelDiscovery: "lmstudio"` is set (the gateway automatically retries on "unloaded" errors).

### NearAI

- **"provider nearai discovered 0 model(s)"** — The base URL may be wrong. Confirm it is `https://cloud-api.near.ai/v1` (with `/v1` suffix). Check network connectivity: `curl https://cloud-api.near.ai/v1/models`.
- **401 / authentication error** — The API key is missing or invalid. Ensure `apiKey` references a set environment variable (e.g. `<NEAR_API_KEY>` with `NEAR_API_KEY` exported or in `.env`), or use a literal key. Test: `curl -H "Authorization: Bearer YOUR_KEY" https://cloud-api.near.ai/v1/models`.
- **Model not found** — The model id must match a model in the NearAI catalog. Check [near.ai](https://near.ai) for the current list. The format uses org/model (e.g. `zai-org/GLM-5.1-FP8`), not the Ollama `model:tag` format.

### NVIDIA NIM

- **401 / authentication error** — The API key is missing or invalid. Ensure `apiKey` references a set environment variable (e.g. `<NVIDIA_API_KEY>` with `NVIDIA_API_KEY` exported or in `.env`), or use a literal key. Test: `curl -H "Authorization: Bearer YOUR_KEY" https://integrate.api.nvidia.com/v1/chat/completions -d '{"model":"meta/llama-3.1-8b-instruct","messages":[{"role":"user","content":"hi"}]}' -H "Content-Type: application/json"`.
- **429 / rate limit** — The NIM free tier allows ~40 requests per minute. Wait and retry. If sustained throughput is needed, consider a local provider instead.
- **402 / payment required** — Credits or free-tier quota exceeded. Check your NVIDIA build account for usage status.
- **Model not found with `staticModels`** — The model id must exactly match a NIM catalog id (e.g. `meta/llama-3.1-8b-instruct`). Check the [LLM APIs reference](https://docs.api.nvidia.com/nim/reference/llm-apis) for current model ids. Unlike Ollama, there is no discovery endpoint — you must curate the list manually.
- **Startup warning about NIM as default provider** — This is informational, not an error. It reminds you that conversation data is sent to NVIDIA and the free tier has rate limits. Acknowledge and continue, or switch to a local provider.

## Summary

| Part | Provider | Key Config | Discovery | Auth |
|------|----------|------------|-----------|------|
| A | Ollama | `defaultModel` change | `GET /api/tags` (default) | None |
| B | LM Studio | `modelDiscovery: "lmstudio"` | `GET /api/v1/models` (LM Studio native) | None |
| C | NearAI | `baseUrl: "https://cloud-api.near.ai/v1"` | `GET /v1/models` (default) | `apiKey` (literal or `<ENV_VAR>`) |
| D | NVIDIA NIM | `baseUrl`, `modelDiscovery: "static"`, `staticModels` | Static list in config | `apiKey` (literal or `<NVIDIA_API_KEY>`) |

**Next:** [11 — Agent: multi-agent configuration](11-agent-multi.md) · [12 — Gateway: auth](12-gateway-auth.md)
