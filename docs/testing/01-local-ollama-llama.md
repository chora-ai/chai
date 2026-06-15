# Local Ollama Llama

## Scope

Local Ollama runs for the Llama family.

## Setup

Follow the [Ollama setup](PROVIDER_SETUP.md#ollama) instructions, then set `agents.defaultModel` to one of the models below.

Example configuration:

```json
{ "id": "ollama", "endpointType": "ollama" }
```

## Models

The following models support tools:

- `llama3.1:8b` - [source (Ollama)](https://ollama.com/library/llama3.1:8b)
- `llama3.2:3b` - [source (Ollama)](https://ollama.com/library/llama3.2:3b)

The following models do not support tools:

- `llama3:8b` - [source (Ollama)](https://ollama.com/library/llama3:8b)

> **Note:** Models without tool support should be tested with [20-conversation-no-tools.md](20-conversation-no-tools.md) instead of the shared tool-use message sequence.

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure.

## See Also

- [Provider setup](PROVIDER_SETUP.md#ollama) · [Configuration → Providers](../guides/03-configuration.md#configuring-providers) · [Model selection](../guides/10-choosing-a-provider.md)
