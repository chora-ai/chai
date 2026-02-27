# Test - Third-Party Models

This document describes a repeatable test procedure for comparing third-party hosted LLM performance with the Chai agent. The same context and message sequence are used across models so results can be compared.

**Scope**

- **Current:** Models served by **OpenAI**.
- **Planned:** Add models served by **Claude** (Anthropic) and **Gemini** (Google).

---

## Models Under Test (OpenAI)

Update the models in this table to match the OpenAI GPT-5 models you are currently using in the gateway configuration.

| Model | Notes |
|-------|--------|
| `gpt-5.2` | Default (flagship) |
| `gpt-5.1` | |
| `gpt-5.1-mini` | |
| `gpt-5-mini` | |

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

1. Configure the gateway so that **`agents.defaultModel`** (or the provider-specific equivalent) points to the OpenAI model under test (for example `gpt-5.2` or the identifier your configuration expects, such as `openai:gpt-5.2`).
2. Ensure the OpenAI API configuration (API key, base URL, organization or project if applicable) is valid and reachable from the gateway.
3. Set **`skills.contextMode`** to **`"full"`** or **`"readOnDemand"`** for the current test batch. Restart the gateway so the system prompt and tools (including any optional `read_skill`) match the mode.
4. Start a **new** session (e.g. `/new` in Telegram or a fresh conversation) so context is clean.
5. Send the five messages in order, one at a time.
6. For each message, record:
   - Whether the agent used tools (yes/no; if yes, which). In readOnDemand, note if the model called `read_skill` before using a skill’s tools.
   - The agent’s reply (summary or full text as needed).
   - Any errors or unexpected behavior.
7. Repeat steps 4–6 for **three full runs** with the same model and same context mode.
8. Switch **`skills.contextMode`** to the other mode, restart the gateway, then repeat steps 4–7 for that mode (three runs).
9. Repeat the full process (both modes, three runs each) for each model in the table above.

---

## Results

Record outcomes per **context mode** and **model**. For each cell: **Run** (1–3), **Message** (1–5), tools used (in readOnDemand note if `read_skill` was used), reply summary, pass/fail.

### Full Mode (`skills.contextMode: "full"`)

#### OpenAI — gpt-5.2

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

#### OpenAI — gpt-5.1

*(Same table structure.)*

#### OpenAI — gpt-5.1-mini

*(Same table structure.)*

#### OpenAI — gpt-5-mini

*(Same table structure.)*

### Read-On-Demand Mode (`skills.contextMode: "readOnDemand"`)

#### OpenAI — gpt-5.2

| Run | Message | Tools used? (incl. read_skill?) | Reply / Notes |
|-----|---------|----------------------------------|---------------|
| 1 | 1 | | |
| 1 | 2 | | |
| … | | | |

*(Same structure: three runs per model; note whether model called read_skill before skill tools.)*

#### OpenAI — gpt-5.1

*(Same table structure.)*

#### OpenAI — gpt-5.1-mini

*(Same table structure.)*

#### OpenAI — gpt-5-mini

*(Same table structure.)*

---

## Future: Claude And Gemini

When Anthropic Claude and Google Gemini backends are supported, add corresponding model lists and result tables here, reusing the same message sequence and expectations above. Group results by provider (e.g. Claude vs Gemini) and by model, and keep the same **full** vs **readOnDemand** structure so results can be compared consistently across all providers.
