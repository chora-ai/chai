# Third-Party NIM DeepSeek

## Scope

Third-party NVIDIA NIM API runs for DeepSeek-family models.

## Setup

Follow the [NVIDIA NIM setup](PROVIDER_SETUP.md#nvidia-nim) instructions, then set `agents.defaultModel` to one of the models below.

Example configuration:

```json
{ "id": "nim", "endpointType": "openai-compat", "baseUrl": "https://integrate.api.nvidia.com/v1", "modelDiscovery": "static", "staticModels": [...] }
```

## Models

The following models support tools:

- `deepseek-ai/deepseek-v3.1` - [source (NVIDIA)](https://build.nvidia.com/deepseek-ai/deepseek-v3_1)
- `deepseek-ai/deepseek-v3.1-terminus` - [source (NVIDIA)](https://build.nvidia.com/deepseek-ai/deepseek-v3_1-terminus)
- `deepseek-ai/deepseek-v3.2` - [source (NVIDIA)](https://build.nvidia.com/deepseek-ai/deepseek-v3_2)

The following models do not support tools:

- `deepseek-ai/deepseek-r1-distill-llama-8b` - [source (NVIDIA)](https://build.nvidia.com/deepseek-ai/deepseek-r1-distill-llama-8b)
- `deepseek-ai/deepseek-r1-distill-qwen-7b` - [source (NVIDIA)](https://build.nvidia.com/deepseek-ai/deepseek-r1-distill-qwen-7b)

> **Note:** Models without tool support should be tested with [20-conversation-no-tools.md](20-conversation-no-tools.md) instead of the shared tool-use message sequence.

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure — or use the [non-tool conversation playbook](20-conversation-no-tools.md) for models that lack tool support.

## See Also

- [Provider setup](PROVIDER_SETUP.md#nvidia-nim) · [Configuration → Providers](../guides/03-configuration.md#configuring-providers) · [Model selection](../guides/10-choosing-a-provider.md)
