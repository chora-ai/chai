# Third-Party NIM DeepSeek

## Scope

Third-party NVIDIA NIM API runs for DeepSeek-family models.

## Models

The following models support tools:

- `deepseek-ai/deepseek-v3.1` - [source (NVIDIA)](https://build.nvidia.com/deepseek-ai/deepseek-v3_1)
- `deepseek-ai/deepseek-v3.1-terminus` - [source (NVIDIA)](https://build.nvidia.com/deepseek-ai/deepseek-v3_1-terminus)
- `deepseek-ai/deepseek-v3.2` - [source (NVIDIA)](https://build.nvidia.com/deepseek-ai/deepseek-v3_2)

The following models do not support tools:

- (excluded from list) `deepseek-ai/deepseek-r1-distill-llama-8b` - [source (NVIDIA)](https://build.nvidia.com/deepseek-ai/deepseek-r1-distill-llama-8b)
- (excluded from list) `deepseek-ai/deepseek-r1-distill-qwen-7b` - [source (NVIDIA)](https://build.nvidia.com/deepseek-ai/deepseek-r1-distill-qwen-7b)

## Setup

- Provider: `endpoint: "openai-compat"` with `modelDiscovery: "static"` and `baseUrl: "https://integrate.api.nvidia.com/v1"` (e.g. `{ "id": "nim", "endpoint": "openai-compat", "baseUrl": "https://integrate.api.nvidia.com/v1", "modelDiscovery": "static", "staticModels": [...] }`)
- `agents.defaultProvider`: provider `id` (e.g. `"nim"`)
- `agents.defaultModel`: one model from the list above
- `NVIDIA_API_KEY` or provider `apiKey`

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure.


## See Also

- [Configuration → Providers](../guides/03-configuration.md#configuring-providers) · [Provider spec](../../base/spec/PROVIDERS.md) · [Model spec](../../base/spec/MODELS.md)
