# Local Ollama DeepSeek

## Scope

Local Ollama runs for DeepSeek models.

## Models

The following models support tools:

- NA

The following models do not support tools:

- `deepseek-r1:7b` - [source (Ollama)](https://ollama.com/library/deepseek-r1:7b)
- `deepseek-r1:8b` - [source (Ollama)](https://ollama.com/library/deepseek-r1:8b)

## Setup

- Provider: `endpoint: "ollama"` (e.g. `{ "id": "ollama", "endpoint": "ollama" }`)
- `agents.defaultProvider`: provider `id` (e.g. `"ollama"`)
- `agents.defaultModel`: one model from the list above

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure.


## See Also

- [Configuration → Providers](../guides/03-configuration.md#configuring-a-provider) · [Provider spec](../../base/spec/PROVIDERS.md) · [Model spec](../../base/spec/MODELS.md)
