# Local LMS Qwen

## Scope

Local LM Studio runs for Qwen-family models.

## Setup

Follow the [LM Studio setup](PROVIDER_SETUP.md#lm-studio) instructions, then set `agents.defaultModel` to one of the models below.

Example configuration:

```json
{ "id": "lms", "endpointType": "openai-compat", "modelDiscovery": "lmstudio" }
```

## Models

The following models support tools (*and they are trained on tool use*):

- `qwen/qwen3-4b-instruct-2507` - [source (LM Studio)](https://lmstudio.ai/models/qwen/qwen3-4b-2507)
- `qwen/qwen3-4b-thinking-2507` - [source (LM Studio)](https://lmstudio.ai/models/qwen/qwen3-4b-thinking-2507)
- `qwen/qwen3.5-4b` - [source (LM Studio)](https://lmstudio.ai/models/qwen/qwen3.5-4b)
- `qwen/qwen3.5-9b` - [source (LM Studio)](https://lmstudio.ai/models/qwen/qwen3.5-9b)

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure.

## See Also

- [Provider setup](PROVIDER_SETUP.md#lm-studio) · [Configuration → Providers](../guides/03-configuration.md#configuring-providers) · [Model selection](../guides/10-choosing-a-provider.md)
