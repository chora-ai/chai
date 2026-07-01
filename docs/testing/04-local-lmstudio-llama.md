# Local LMS Llama

## Scope

Local LM Studio runs for Llama-family models.

## Setup

Follow the [LM Studio setup](PROVIDER_SETUP.md#lm-studio) instructions, then set `agents.defaultModel` to one of the models below.

Example configuration:

```json
{ "id": "lmstudio", "endpointType": "openai-compat", "modelDiscovery": "lmstudio" }
```

## Models

The following models support tools (*and they are trained on tool use*):

- `llama-3.2-3b-instruct` - [source (Hugging Face / LM Studio Community)](https://huggingface.co/lmstudio-community/Llama-3.2-3B-Instruct-GGUF)
- `meta-llama-3.1-8b-instruct` - [source (Hugging Face / LM Studio Community)](https://huggingface.co/lmstudio-community/Meta-Llama-3.1-8B-Instruct-GGUF)

The following models support tools (*but they are not trained on tool use*):

- `meta-llama-3-8b-instruct` - [source (Hugging Face / LM Studio Community)](https://huggingface.co/lmstudio-community/Meta-Llama-3-8B-Instruct-GGUF)

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure.

## See Also

- [Provider setup](PROVIDER_SETUP.md#lm-studio) · [Configuration → Providers](../guides/03-configuration.md#configuring-providers) · [Model selection](../guides/10-choosing-a-provider.md)
