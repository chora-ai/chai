# Third-Party NIM Llama

## Scope

Third-party NVIDIA NIM API runs for Llama-family models.

## Models

The following models support tools:

- `meta/llama3-70b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama3-70b)
- `meta/llama-3.1-8b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama-3_1-8b-instruct)
- `meta/llama-3.1-70b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama-3_1-70b-instruct)
- `meta/llama-3.1-405b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama-3_1-405b-instruct)
- `meta/llama-3.2-3b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama-3.2-3b-instruct)
- `meta/llama-3.3-70b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama-3_3-70b-instruct)
- `meta/llama-4-maverick-17b-128e-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama-4-maverick-17b-128e-instruct)

The following models do not support tools:

- `meta/llama3-8b-instruct` - [source (NVIDIA)](https://build.nvidia.com/meta/llama3-8b)

## Setup

- `agents.defaultProvider`: `nim`
- `agents.defaultModel`: one model from the list above
- `NVIDIA_API_KEY` or `providers.nim.apiKey`

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure.
