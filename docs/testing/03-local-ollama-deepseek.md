# Local Ollama DeepSeek

## Scope

Local Ollama runs for DeepSeek models.

## Setup

Follow the [Ollama setup](PROVIDER_SETUP.md#ollama) instructions, then set `agents.defaultModel` to one of the models below.

Example configuration:

```json
{ "id": "ollama", "endpointType": "ollama" }
```

## Models

The following models do not support tools:

- `deepseek-r1:7b` - [source (Ollama)](https://ollama.com/library/deepseek-r1:7b)
- `deepseek-r1:8b` - [source (Ollama)](https://ollama.com/library/deepseek-r1:8b)

> **Note:** All listed models lack tool support. Use [20-conversation-no-tools.md](20-conversation-no-tools.md) instead of the shared tool-use message sequence.

## Procedure

Use the [non-tool conversation playbook](20-conversation-no-tools.md) — the shared tool-use message sequence in [README.md](README.md) is not suitable for these models.

## See Also

- [Provider setup](PROVIDER_SETUP.md#ollama) · [Configuration → Providers](../guides/03-configuration.md#configuring-a-provider) · [Provider spec](../../base/spec/PROVIDERS.md) · [Model spec](../../base/spec/MODELS.md)
