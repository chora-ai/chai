# Local LMS Deepseek

## Scope

Local LM Studio runs for Deepseek-family models.

## Provider

LM Studio has the following gotchas:

- LM Studio must be installed and running to use latest `lms`
- LM Studio developer settings must be on with runtime set to CPU
- LM Studio models must be manually loaded (e.g. `lms load <model path>`)
- All models support tools but some models are not trained on tool use

## Models

The following models support tools (*and they are trained on tool use*):

- NA

The following models support tools (*but they are not trained on tool use*):

- `deepseek/deepseek-r1-0528-qwen3-8b` - [source (LM Studio)](https://lmstudio.ai/models/deepseek/deepseek-r1-0528-qwen3-8b)
- `deepseek/deepseek-r1-distill-llama-8b` - [source (LM Studio)](https://lmstudio.ai/models/deepseek/deepseek-r1-distill-llama-8b)
- `deepseek/deepseek-r1-distill-qwen-7b` - [source (LM Studio)](https://lmstudio.ai/models/deepseek/deepseek-r1-distill-qwen-7b)

The following models do not support tools:

- NA

## Setup

- `agents.defaultProvider`: `lms`
- `agents.defaultModel`: one model from the list above

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure.
