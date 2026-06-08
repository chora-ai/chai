# Local Ollama Qwen

## Scope

Local Ollama runs for the Qwen family.

## Setup

Follow the [Ollama setup](PROVIDER_SETUP.md#ollama) instructions, then set `agents.defaultModel` to one of the models below.

Example configuration:

```json
{ "id": "ollama", "endpoint": "ollama" }
```

## Models

The following models support tools:

- `qwen3:4b` - [source (Ollama)](https://ollama.com/library/qwen3:4b)
- `qwen3:8b` - [source (Ollama)](https://ollama.com/library/qwen3:8b)
- `qwen3.5:4b` - [source (Ollama)](https://ollama.com/library/qwen3.5:4b)
- `qwen3.5:9b` - [source (Ollama)](https://ollama.com/library/qwen3.5:9b)

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure.

## See Also

- [Provider setup](PROVIDER_SETUP.md#ollama) · [Configuration → Providers](../guides/03-configuration.md#configuring-a-provider) · [Provider spec](../../base/spec/PROVIDERS.md) · [Model spec](../../base/spec/MODELS.md)
