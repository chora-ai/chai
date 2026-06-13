# Third-Party NIM Llama

## Scope

Third-party NVIDIA NIM API runs for Llama-family models.

## Setup

Follow the [NVIDIA NIM setup](PROVIDER_SETUP.md#nvidia-nim) instructions, then set `agents.defaultModel` to one of the models below.

Example configuration:

```json
{ "id": "nim", "endpointType": "openai-compat", "baseUrl": "https://integrate.api.nvidia.com/v1", "modelDiscovery": "static", "staticModels": [...] }
```

## Models

The following models support tools:

- `meta/llama3-70b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama3-70b)
- `meta/llama-3.1-8b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama-3_1-8b-instruct)
- `meta/llama-3.1-70b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama-3_1-70b-instruct)
- `meta/llama-3.1-405b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama-3_1-405b-instruct)
- `meta/llama-3.2-3b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama-3.2-3b-instruct)
- `meta/llama-3.3-70b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama-3_3-70b-instruct)
- `meta/llama-4-maverick-17b-128e-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama-4-maverick-17b-instruct)

The following models do not support tools:

- `meta/llama3-8b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama3-8b)

> **Note:** Models without tool support should be tested with [20-conversation-no-tools.md](20-conversation-no-tools.md) instead of the shared tool-use message sequence.

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure.

## See Also

- [Provider setup](PROVIDER_SETUP.md#nvidia-nim) · [Configuration → Providers](../guides/03-configuration.md#configuring-providers) · [Provider spec](../../base/spec/PROVIDERS.md) · [Model spec](../../base/spec/MODELS.md)
