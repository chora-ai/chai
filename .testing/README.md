# Testing Playbooks

Files are grouped by deployment category, then provider, then model family.

## Playbook Order

| Order | Category | Provider | Model Family | File |
|------:|----------|----------|--------------|------|
| 01 | local | ollama | llama | [01-local-ollama-llama.md](01-local-ollama-llama.md) |
| 02 | local | ollama | qwen | [02-local-ollama-qwen.md](02-local-ollama-qwen.md) |
| 03 | local | ollama | deepseek | [03-local-ollama-deepseek.md](03-local-ollama-deepseek.md) |
| 04 | local | lms | llama | [04-local-lms-llama.md](04-local-lms-llama.md) |
| 05 | local | lms | qwen | [05-local-lms-qwen.md](05-local-lms-qwen.md) |
| 06 | local | lms | deepseek | [06-local-lms-deepseek.md](06-local-lms-deepseek.md) |
| 07 | third-party | nim | llama | [07-third-party-nim-llama.md](07-third-party-nim-llama.md) |
| 08 | third-party | nim | qwen | [08-third-party-nim-qwen.md](08-third-party-nim-qwen.md) |
| 09 | third-party | nim | deepseek | [09-third-party-nim-deepseek.md](09-third-party-nim-deepseek.md) |

## Shared Message Sequence

Use this exact sequence in every playbook run.

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

## Shared Expectations

| Message | Expectation |
|--------|-------------|
| 1 | No tool call; greeting and response. |
| 2 | Tool call(s) to create daily note; return latest content. |
| 3 | Tool call(s) to mark first item complete; return latest content. |
| 4 | Tool call(s) to add second item completed; return latest content. |
| 5 | No new tool call; acknowledgement. |

## Shared Run Procedure

1. Set the playbook model in `agents.defaultModel`.
2. Run all five messages in `skills.contextMode: "full"` for three runs.
3. Repeat in `skills.contextMode: "readOnDemand"` for three runs.
4. Record tool usage, response summary, pass/fail, and any errors.

## Result Template

| Run | Mode | Message | Tools Used? | Reply Summary | Pass/Fail | Notes |
|-----|------|---------|-------------|---------------|-----------|-------|
| 1 | full | 1 | | | | |
| 1 | full | 2 | | | | |
| ... | ... | ... | | | | |
