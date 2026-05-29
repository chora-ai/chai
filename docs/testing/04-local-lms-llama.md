# Local LMS Llama

## Scope

Local LM Studio runs for Llama-family models.

## Provider

LM Studio has the following gotchas:

- LM Studio must be installed and running to use latest `lms`
- LM Studio developer settings must be on with runtime set to CPU
- LM Studio models must be manually loaded (e.g. `lms load <model path>`)
- All models support tools but some models are not trained on tool use

## Models

The following models support tools (*and they are trained on tool use*):

- `llama-3.2-3b-instruct` - [source (Hugging Face / LM Studio Community)](https://huggingface.co/lmstudio-community/Llama-3.2-3B-Instruct-GGUF)
- `meta-llama-3.1-8b-instruct` - [source (Hugging Face / LM Studio Community)](https://huggingface.co/lmstudio-community/Meta-Llama-3.1-8B-Instruct-GGUF)

The following models support tools (*but they are not trained on tool use*):

- `meta-llama-3-8b-instruct` - [source (Hugging Face / LM Studio Community)](https://huggingface.co/lmstudio-community/Meta-Llama-3-8B-Instruct-GGUF)

The following models do not support tools:

- NA

## Setup

- `agents.defaultProvider`: `lms`
- `agents.defaultModel`: one model from the list above

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure.
