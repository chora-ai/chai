# Local LMS DeepSeek

## Scope

Local LM Studio runs for DeepSeek-family models.

## Setup

Follow the [LM Studio setup](PROVIDER_SETUP.md#lm-studio) instructions, then set `agents.defaultModel` to one of the models below.

Example configuration:

```json
{ "id": "lms", "endpointType": "openai-compat", "modelDiscovery": "lmstudio", "autoLoad": "lmstudio" }
```

## Models

The following models support tools (*but they are not trained on tool use*):

- `deepseek/deepseek-r1-0528-qwen3-8b` - [source (LM Studio)](https://lmstudio.ai/models/deepseek/deepseek-r1-0528-qwen3-8b)
- `deepseek/deepseek-r1-distill-llama-8b` - [source (LM Studio)](https://lmstudio.ai/models/deepseek/deepseek-r1-distill-llama-8b)
- `deepseek/deepseek-r1-distill-qwen-7b` - [source (LM Studio)](https://lmstudio.ai/models/deepseek/deepseek-r1-distill-qwen-7b)

> **Note:** These models are not trained on tool use. For a pure conversational test, use [20-conversation-no-tools.md](20-conversation-no-tools.md) instead of the shared tool-use message sequence.

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure — or use the [non-tool conversation playbook](20-conversation-no-tools.md) for a more appropriate test.

## See Also

- [Provider setup](PROVIDER_SETUP.md#lm-studio) · [Configuration → Providers](../guides/03-configuration.md#configuring-a-provider) · [Provider spec](../../base/spec/PROVIDERS.md) · [Model spec](../../base/spec/MODELS.md)
