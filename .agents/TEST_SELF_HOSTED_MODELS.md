# Test - Self-Hosted Models

This document describes a repeatable test procedure for comparing self-hosted LLM performance with the Chai agent. The same context and message sequence are used across models so results can be compared.

**Scope**

- **Current:** Models served by a self-hosted **Hugging Face** backend.
- **Planned:** Add models served by **LocalAI** and **llama.cpp**.

---

## Models Under Test (Hugging Face)

Update the models in this table to match the Hugging Face models you are currently running (e.g. via Text Generation Inference, Inference Endpoints, or another self-hosted deployment).

| Model | Notes |
|-------|--------|
| `meta-llama/Llama-3.1-8B-Instruct` | Default |
| `mistralai/Mistral-7B-Instruct-v0.3` | |
| `google/gemma-2-9b-it` | |
| `Qwen/Qwen2.5-7B-Instruct` | |

Each model is tested **in both skill context modes** (**full** and **readOnDemand**), **three runs per mode**, with the same message sequence. Use the same gateway config (skills, workspace) except for `skills.contextMode`; restart the gateway when switching mode so the system prompt and tool list match.

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
Can you mark the first action item as complete?
```

**Message 4 — Add task (marked complete)**

```
Can you add another action item for "test updating a daily note from a Telegram chat" and mark it as complete?
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

1. Configure the gateway so that **`agents.defaultModel`** (or the provider-specific equivalent) points to the Hugging Face model under test (for example `meta-llama/Llama-3.1-8B-Instruct` or whatever identifier your deployment expects).
2. Set **`skills.contextMode`** to **`"full"`** or **`"readOnDemand"`** for the current test batch. Restart the gateway so the system prompt and tools (including any optional `read_skill`) match the mode.
3. Start a **new** session (e.g. `/new` in Telegram or a fresh conversation) so context is clean.
4. Send the five messages in order, one at a time.
5. For each message, record:
   - Whether the agent used tools (yes/no; if yes, which). In readOnDemand, note if the model called `read_skill` before using a skill’s tools.
   - The agent’s reply (summary or full text as needed).
   - Any errors or unexpected behavior.
6. Repeat steps 3–5 for **three full runs** with the same model and same context mode.
7. Switch **`skills.contextMode`** to the other mode, restart the gateway, then repeat steps 3–6 for that mode (three runs).
8. Repeat the full process (both modes, three runs each) for each model in the table above.

---

## Results

Record outcomes per **context mode** and **model**. For each cell: **Run** (1–3), **Message** (1–5), tools used (in readOnDemand note if `read_skill` was used), reply summary, pass/fail.

### Full Mode (`skills.contextMode: "full"`)

#### Hugging Face — meta-llama/Llama-3.1-8B-Instruct

| Run | Message | Tools used? | Reply / Notes |
|-----|---------|-------------|---------------|
| 1 | 1 | | |
| 1 | 2 | | |
| 1 | 3 | | |
| 1 | 4 | | |
| 1 | 5 | | |
| 2 | 1 | | |
| … | | | |

*(Duplicate for runs 2–3.)*

#### Hugging Face — mistralai/Mistral-7B-Instruct-v0.3

*(Same table structure.)*

#### Hugging Face — google/gemma-2-9b-it

*(Same table structure.)*

#### Hugging Face — Qwen/Qwen2.5-7B-Instruct

*(Same table structure.)*

### Read-On-Demand Mode (`skills.contextMode: "readOnDemand"`)

#### Hugging Face — meta-llama/Llama-3.1-8B-Instruct

| Run | Message | Tools used? (incl. read_skill?) | Reply / Notes |
|-----|---------|----------------------------------|---------------|
| 1 | 1 | | |
| 1 | 2 | | |
| … | | | |

*(Same structure: three runs per model; note whether model called read_skill before skill tools.)*

#### Hugging Face — mistralai/Mistral-7B-Instruct-v0.3

*(Same table structure.)*

#### Hugging Face — google/gemma-2-9b-it

*(Same table structure.)*

#### Hugging Face — Qwen/Qwen2.5-7B-Instruct

*(Same table structure.)*

---

## Future: LocalAI And llama.cpp

When LocalAI and llama.cpp backends are supported, add corresponding model lists and result tables here, reusing the same message sequence and expectations above. Group results by backend (e.g. LocalAI vs. llama.cpp) and by model, and keep the same **full** vs **readOnDemand** structure so results can be compared consistently across all providers.

