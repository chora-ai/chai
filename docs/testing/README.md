# User Testing

Files are grouped by deployment category, then provider, then model family. A separate non-tool playbook covers models that lack tool calling.

For provider setup instructions, see [PROVIDER_SETUP.md](PROVIDER_SETUP.md). For provider configuration details, see [Configuration](../guides/03-configuration.md). For model ids and choosing a provider, see [Choosing a Provider and Model](../guides/10-choosing-a-provider.md). For hands-on channel and skill walkthroughs, see the [User Journeys](../journey/README.md).

## Coverage Matrix

| Provider | Llama | Qwen | DeepSeek | No-Tool | 
|----------|-------|------|----------|---------|
| Ollama   | 01    | 02   | 03 → 20  | 20      |
| LM Studio| 04    | 05   | 06       | 20      |
| NIM      | 07    | 08   | 09       | 20      |
| NearAI   | 10    | —    | —        | —       |

→ 20 = all models lack tool support; use [20-conversation-no-tools.md](20-conversation-no-tools.md) instead. Playbooks with mixed tool/no-tool models (06, 09) offer both paths with notes.

## Playbook Order

### Tool-Use Playbooks

These playbooks use the shared message sequences that exercise tool calls. Only use these with models that support tool calling.

| Order | Category | Provider | Model Family | Sequence | File |
|------:|----------|----------|--------------|----------|------|
| 01 | local | ollama | llama | A | [01-local-ollama-llama.md](01-local-ollama-llama.md) |
| 02 | local | ollama | qwen | A | [02-local-ollama-qwen.md](02-local-ollama-qwen.md) |
| 03 | local | ollama | deepseek | — | [03-local-ollama-deepseek.md](03-local-ollama-deepseek.md) |
| 04 | local | lms | llama | A | [04-local-lms-llama.md](04-local-lms-llama.md) |
| 05 | local | lms | qwen | A | [05-local-lms-qwen.md](05-local-lms-qwen.md) |
| 06 | local | lms | deepseek | A | [06-local-lms-deepseek.md](06-local-lms-deepseek.md) |
| 07 | third-party | nim | llama | A | [07-third-party-nim-llama.md](07-third-party-nim-llama.md) |
| 08 | third-party | nim | qwen | A | [08-third-party-nim-qwen.md](08-third-party-nim-qwen.md) |
| 09 | third-party | nim | deepseek | A | [09-third-party-nim-deepseek.md](09-third-party-nim-deepseek.md) |
| 10 | third-party | nearai | zai-org | A | [10-third-party-nearai-glm.md](10-third-party-nearai-glm.md) |

### Non-Tool Playbook

For models that do not support tool calling, use the conversation-only playbook instead of any tool-use sequence.

| Order | Category | Sequence | File |
|------:|----------|----------|------|
| 20 | conversation (no tools) | B | [20-conversation-no-tools.md](20-conversation-no-tools.md) |

### Multi-Tool and Error-Handling Playbooks

For testing multi-tool selection and error recovery with models that support tool calling.

| Order | Category | Sequence | File |
|------:|----------|----------|------|
| 21 | multi-tool | C | [21-multi-tool.md](21-multi-tool.md) |
| 22 | error-handling | D | [22-error-handling.md](22-error-handling.md) |

## Sequence A: Daily Note Tool Use

Use this sequence for playbooks 01–10 (models that support tool calling). It exercises the daily note skill: create, append, and replace.

1. Greeting

```
Hello. What's your name?
```

2. Create daily note

```
Can you create a daily note with the following checklist item?

- [ ] create daily note
```

3. Update daily note (append)

```
Can you update the daily note with the following checklist item?

- [ ] update daily note
```

4. Update daily note (replace)

```
Can you mark both checklist items as complete?
```

5. Praise and appreciation

```
Nice work! Thanks for all your help.
```

### Sequence A Expectations

| Message | Expectation |
|--------|-------------|
| 1 | No tool call; greeting and response. |
| 2 | Tool call(s) to create daily note; return latest content. |
| 3 | Tool call(s) to mark first item complete; return latest content. |
| 4 | Tool call(s) to add second item completed; return latest content. |
| 5 | No new tool call; acknowledgement. |

## Sequence B: Conversation Only

Use this sequence for models that do not support tool calling ([20-conversation-no-tools.md](20-conversation-no-tools.md)). No tool calls are expected on any message.

1. Greeting and identity

```
Hello! Can you tell me a bit about yourself?
```

2. Reasoning task

```
If I have 3 apples and give 1 to each of my 2 friends, how many apples do I have left?
```

3. Instruction following

```
List three primary colors. Format your answer as a numbered list with exactly one word per line.
```

4. Consistency check

```
Earlier I asked about apples. How many did I start with, and how many did I give away?
```

5. Closing

```
Thanks for the conversation. Goodbye!
```

### Sequence B Expectations

| Message | Expectation |
|--------|-------------|
| 1 | No tool call; introduces itself and responds conversationally. |
| 2 | No tool call; gives the correct answer (1 apple left) with reasoning. |
| 3 | No tool call; follows the formatting instruction (numbered list, one word per line). |
| 4 | No tool call; correctly recalls earlier context (3 apples started, 2 given away). |
| 5 | No tool call; friendly sign-off, no hallucinated tool use. |

## Sequence C: Multi-Tool Use

Use this sequence to verify that a model can invoke different tools across turns and correctly choose which tool to use. Requires an agent with multiple skills enabled (e.g., daily note + files).

1. Greeting

```
Hello! I need some help today.
```

2. Create a daily note

```
Can you create a daily note with the item "review playbook"?
```

3. Read a file

```
Can you read the file called README.md?
```

4. Append to daily note

```
Now add "read README" to the daily note.
```

5. Multi-step request

```
Can you read the daily note and tell me everything that's on it?
```

### Sequence C Expectations

| Message | Tool Category | Expectation |
|--------|---------------|-------------|
| 1 | — | No tool call; greeting and response. |
| 2 | Daily note | Tool call to create daily note with "review playbook"; return content. |
| 3 | Files | Tool call to read README.md; return file content. |
| 4 | Daily note | Tool call to append "read README" to daily note; return content. |
| 5 | Daily note | Tool call to read daily note; summarize all items. |

## Sequence D: Error Handling

Use this sequence to verify that a model handles tool failures gracefully — acknowledging the error and either recovering or explaining the problem.

1. Greeting

```
Hi there! I'd like to try some file operations.
```

2. Read a nonexistent file

```
Can you read the file called does_not_exist.md?
```

3. Acknowledge and move on

```
That's okay. Can you create a daily note with the item "error recovery test"?
```

4. Attempt to write outside the sandbox

```
Can you write a file to /etc/test.txt?
```

5. Wrap up

```
Thanks for handling those errors gracefully!
```

### Sequence D Expectations

| Message | Expectation |
|--------|-------------|
| 1 | No tool call; greeting and response. |
| 2 | Tool call that returns an error; model acknowledges the file was not found and explains. |
| 3 | Tool call to create daily note; succeeds despite previous error; returns content. |
| 4 | Tool call that returns a sandbox/permission error; model explains it cannot write there. |
| 5 | No new tool call; acknowledgement. |

## Shared Run Procedure

1. Set the playbook model in `agents.defaultModel`.
2. Run all five messages in `skills.contextMode: "full"` for three runs.
3. Repeat in `skills.contextMode: "readOnDemand"` for three runs.
4. Record tool usage, response summary, pass/fail, and any errors.

## Pass/Fail Rubric

A run **passes** when all of the following are true:

| Criterion | Tool-Use Sequences (A, C, D) | Non-Tool Sequence (B) |
|-----------|-------------------------------|-----------------------|
| Tool invocation | Correct tool called for each tool-expected message (or correct tool category for Sequence C) | No tool calls on any message |
| Tool arguments | Arguments match the user's intent (correct filename, correct content) | N/A |
| Error recovery | On error messages, model acknowledges the error and does not hallucinate success | N/A |
| Response quality | Reply is relevant and coherent; no repeated or garbled text | Reply is relevant and coherent |
| Consistency | No contradiction with earlier turns | Correct recall of prior context (Sequence B, message 4; Sequence C, message 5) |
| Format compliance | N/A | Formatting instructions followed when given (Sequence B, message 3) |

A run **fails** when any criterion is not met. Record the specific failure in the Notes column.

## Results

Results are stored in `results/` with one file per playbook per run date. Filenames follow the pattern `YYYY-MM-DD-<playbook>-<model>.md` (e.g., `2025-06-07-01-llama3.1-8b.md`).

Each result file contains a filled-in result table (see template below) and a summary of observations. When a playbook is re-run for the same model, create a new dated file — do not overwrite previous results.

### Last Run

Playbooks track the last-run date in their model list comments. A model list preceded by `<!-- Last verified: YYYY-MM-DD -->` indicates that the model ids were confirmed available on that date. This is separate from the test results — a model may be verified available but not yet tested.

### Result Template

Copy this template into a new file under `results/` for each run.

```
# <Playbook Name> — <Model> — <Date>

## Configuration

- Provider: <provider id>
- Model: <model id>
- Context mode: full / readOnDemand
- Skills enabled: <comma-separated list>

## Results

| Run | Mode | Message | Tools Used? | Reply Summary | Pass/Fail | Notes |
|-----|------|---------|-------------|---------------|-----------|-------|
| 1 | full | 1 | | | | |
| 1 | full | 2 | | | | |
| 1 | full | 3 | | | | |
| 1 | full | 4 | | | | |
| 1 | full | 5 | | | | |
| 2 | full | 1 | | | | |
| ... | ... | ... | | | | |

## Observations

<Free-form notes: any patterns across runs, unexpected behavior, model-specific quirks.>
```

## Provider Setup

See [PROVIDER_SETUP.md](PROVIDER_SETUP.md) for step-by-step setup instructions for each provider (Ollama, LM Studio, NVIDIA NIM, NearAI), including installation, model loading, API keys, gotchas, and example configurations.
