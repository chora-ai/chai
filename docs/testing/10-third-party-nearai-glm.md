# Third-Party NearAI GLM

## Scope

Third-party NearAI API runs for zai-org-family models.

## Setup

Follow the [NearAI setup](PROVIDER_SETUP.md#nearai) instructions, then set `agents.defaultModel` to one of the models below.

Example configuration:

```json
{
  "id": "nearai",
  "endpointType": "openai-compat",
  "baseUrl": "https://cloud-api.near.ai/v1",
  "apiKey": "<NEARAI_API_KEY>"
}
```

## Models

The following models support tools:

- `z-ai/glm-5.2` - [source (NearAI)](https://near.ai)

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure.

## See Also

- [Provider setup](PROVIDER_SETUP.md#nearai) · [Configuration → Providers](../guides/03-configuration.md#configuring-providers) · [Model selection](../guides/10-choosing-a-provider.md)
