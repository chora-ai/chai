# Test - Local Models

This document describes a repeatable test procedure for comparing local LLM performance with the Chai agent. The same context and message sequence are used across models so results can be compared.

**Scope**

- **Current:** Models supported by **Ollama** only.
- **Planned:** Add models supported by **LM Studio** and **Hugging Face** when those backends are available.

---

## Models Under Test (Ollama)

| Model | Notes |
|-------|--------|
| `llama3:latest` | Default |
| `deepseek-1:7b` | |
| `qwen3:8b` | |
| `gemma2:9b` | |

Each model is tested **three times** with the same message sequence. Same gateway config (skills, workspace) and same conversation context for all runs.

---

## Test Message Sequence

Messages are sent in order. Each block below is one user message (separators `---` are for readability only).

**Message 1 — Greeting**

```
Hello
```

**Message 2 — Create daily note**

````
Can you create a daily note that includes the following:

```markdown
# 2026-02-25

## Action Items

- [ ] test creating a daily note from a Telegram chat

```
````

**Message 3 — Update note (mark first item complete)**

```
I would like you to now update the note so that the the first action item is marked as complete.
```

**Message 4 — Add task (marked complete)**

```
Can you add another task for "test updating a daily note from a Telegram chat" that is marked as complete?
```

**Message 5 — Praise**

```
Nice job!
```

---

## Expected Behavior

| Message | Expectation |
|--------|-------------|
| **1 — Greeting** | Agent does **not** attempt a tool call; replies with a greeting only. |
| **2 — Create daily note** | Agent calls the appropriate tool(s) to create the daily note and returns the latest version of the content. |
| **3 — Update note** | Agent calls the appropriate tool(s) to update the note (first action item checked) and returns the latest content. |
| **4 — Add task** | Agent calls the appropriate tool(s) to add the new task (marked complete) and returns the latest content. |
| **5 — Praise** | Agent does **not** attempt another tool call; acknowledges briefly. |

---

## Test Procedure

1. Set gateway config `agents.defaultModel` to the model under test (use exact name from `ollama list`).
2. Start a **new** session (e.g. `/new` in Telegram or a fresh conversation) so context is clean.
3. Send the five messages in order, one at a time.
4. For each message, record:
   - Whether the agent used tools (yes/no; if yes, which).
   - The agent’s reply (summary or full text as needed).
   - Any errors or unexpected behavior.
5. Repeat steps 1–4 for three full runs with the same model.
6. Repeat the full process for each model in the table above.

---

## Results

Use the sections below to record outcomes. Format: **Run** (1–3), **Model**, **Message** (1–5), then notes (tools used, reply summary, pass/fail).

### Ollama — llama3:latest

| Run | Message | Tools used? | Reply / Notes |
|-----|---------|-------------|---------------|
| 1 | 1 | | |
| 1 | 2 | | |
| 1 | 3 | | |
| 1 | 4 | | |
| 1 | 5 | | |
| 2 | 1 | | |
| … | | | |

*(Duplicate the table for runs 2–3 and for each model.)*

### Ollama — deepseek-1:7b

*(Same table structure.)*

### Ollama — qwen3:8b

*(Same table structure.)*

### Ollama — gemma2:9b

*(Same table structure.)*

---

## Future: LM Studio and Hugging Face

When LM Studio and/or Hugging Face backends are supported, add corresponding model lists and result tables here, reusing the same message sequence and expectations above.
