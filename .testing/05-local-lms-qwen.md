# Local LMS Qwen

## Scope

Local LM Studio runs for Qwen-family models.

## Provider

LM Studio has the following gotchas:

- LM Studio must be installed and running to use latest `lms`
- LM Studio developer settings must be on with runtime set to CPU
- LM Studio models must be manually loaded (e.g. `lms load <model path>`)
- All models support tools but some models are not trained on tool use

## Models

The following models support tools (*and they are trained on tool use*):

- `qwen/qwen3-4b-instruct-2507` - [source (LM Studio)](https://lmstudio.ai/models/qwen/qwen3-4b-2507)
- `qwen/qwen3-4b-thinking-2507` - [source (LM Studio)](https://lmstudio.ai/models/qwen/qwen3-4b-thinking-2507)
- `qwen/qwen3.5-4b` - [source (LM Studio)](https://lmstudio.ai/models/qwen/qwen3.5-4b)
- `qwen/qwen3.5-9b` - [source (LM Studio)](https://lmstudio.ai/models/qwen/qwen3.5-9b)

The following models support tools (*but they are not trained on tool use*):

- NA

The following models do not support tools:

- NA

## Setup

- `agents.defaultProvider`: `lms`
- `agents.defaultModel`: one model from the list above

## Procedure

Follow the shared protocol in [README.md](README.md): message sequence, expectations, and run procedure.
