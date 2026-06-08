# Third-Party NIM Qwen

## Scope

Third-party NVIDIA NIM API runs for Qwen-family models.

## Setup

Follow the [NVIDIA NIM setup](PROVIDER_SETUP.md#nvidia-nim) instructions, then set `agents.defaultModel` to one of the models below.

Example configuration:

```json
{ "id": "nim", "endpoint": "openai-compat", "baseUrl": "https://integrate.api.nvidia.com/v1", "modelDiscovery": "static", "staticModels": [...] }
```

## Models

The following models support tools:

- `qwen/qwen3-coder-480b-a35b-instruct` - [source (NVIDIA)](https://build.nvidia.com/qwen/qwen3-coder-480b-a35b-instruct)
- `qwen/qwen3-next-80b-a3b-instruct` - [source (NVIDIA)](https://build.nvidia.com/qwen/qwen3-next-80b-a3b-instruct)
- `qwen/qwen3-next-80b-a3b-thinking` - [source (NVIDIA)](https://build.nvidia.com/qwen/qwen3-next-80b-a3b-thinking)
- `qwen/qwen3.5-122b-a10b` - [source (NVIDIA)](https://build.nvidia.com/qwen/qwen3.5-122b-a10b)

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure.

## See Also

- [Provider setup](PROVIDER_SETUP.md#nvidia-nim) · [Configuration → Providers](../guides/03-configuration.md#configuring-providers) · [Provider spec](../../base/spec/PROVIDERS.md) · [Model spec](../../base/spec/MODELS.md)
