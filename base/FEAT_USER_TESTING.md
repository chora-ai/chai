# FEAT: Improve User Testing

Improve the user testing playbooks at `docs/testing/` so they provide broader coverage, a clearer structure, and actionable results rather than boilerplate that requires significant manual effort to execute and record.

## Problem

- **All nine playbooks share the same procedure verbatim.** Each individual file (01–09) is essentially a model list + setup line + "follow the shared protocol in README.md." There is almost no provider- or model-specific guidance beyond the model names themselves.
- **Provider-specific gotchas are duplicated.** The LMS provider notes ("developer settings must be on," "models must be manually loaded") are copy-pasted across 04, 05, and 06 rather than factored out.
- **The shared message sequence is Obsidian-centric.** All five messages exercise the Obsidian/notesmd skill (daily note create, append, replace). There is no coverage for other skills, basic agent conversation without tools, multi-agent routing, channel delivery, or gateway management.
- **No results are recorded.** The README defines a result template but the repository contains no filled-in results. The playbooks are aspirational — there is no evidence they have ever been run, and no guidance on where to store or commit results.
- **Only three providers covered.** The spec (`base/spec/PROVIDERS.md`) documents seven supported providers: Ollama, LM Studio, vLLM, Hugging Face, LocalAI, NVIDIA NIM, and OpenAI. The testing playbooks cover only Ollama, LM Studio, and NIM. No playbooks exist for vLLM, Hugging Face, LocalAI, or OpenAI — including OpenAI, which is a supported first-party provider.
- **Only three model families per provider.** The playbooks test Llama, Qwen, and DeepSeek. While these are the dominant families, the spec references other models (e.g., Mistral, Gemma) that are not covered.
- **Model lists will drift.** The model names and sources are hand-maintained and will become stale as new models are released and old ones are deprecated. There is no convention for marking last-verified dates or deprecated entries.
- **No playbook for non-tool scenarios.** The DeepSeek playbooks (03, 06, 09) list models that do not support tools, but the shared message sequence requires tool use (daily note creation). These playbooks are effectively unrunnable as written — the procedure and the model capabilities are in conflict.
- **README is the only substantial document.** The README carries all the weight (message sequence, expectations, run procedure, result template). The individual playbooks add almost nothing beyond model names. This creates a fragile structure: if the README is updated, every playbook's implicit contract changes, but there's no way to tell which playbooks have been re-validated.
- **AGENTS.md is a stub.** The testing directory's AGENTS.md has "NA" for guidelines and conventions — unlike the journey directory which has filename conventions and writing guidelines.
- **No cross-references.** The testing playbooks don't link to the provider spec (`base/spec/PROVIDERS.md`), the model spec (`base/spec/MODELS.md`), the journey docs (`docs/journey/`), or the configuration guide (`docs/guides/03-configuration.md`). Readers have to discover these independently.

## Recommendations

### R1 — Add provider-specific setup guidance [high priority]

Each provider has unique setup steps that the current playbooks reduce to a single `agents.defaultProvider` line. Add real setup guidance:
- **Ollama:** Install, `ollama pull <model>`, verify with `ollama list`, default base URL.
- **LM Studio:** Install, enable developer mode, load model via `lms load`, default base URL, CPU/GPU runtime setting.
- **NVIDIA NIM:** API key setup (`NVIDIA_API_KEY` or config), rate limits, base URL.
- **vLLM:** Install, serve a model, default base URL, OpenAI-compat endpoint.
- **Hugging Face:** API token, model ID format, base URL.
- **OpenAI:** API key setup, model ID format, base URL, rate limits.

This can live either in the individual playbook files (replacing the current "Setup" one-liners) or in a shared `PROVIDER_SETUP.md` referenced from each playbook. The latter avoids duplication and is recommended.

### R2 — Resolve the DeepSeek / no-tool conflict [high priority]

Playbooks 03, 06, and 09 list models that do not support tools, but the shared message sequence requires tool calls (messages 2–4 exercise daily note operations). This means these playbooks cannot be run as written. Options:

- **Option A:** Add an alternative message sequence for non-tool models (pure conversational turns with no tool expectations). Mark it as a "non-tool variant" in the README.
- **Option B:** Remove the no-tool models from the current playbooks (they are not runnable) and create a separate "non-tool conversation" playbook that covers them with an appropriate message sequence.
- **Option C:** Update the model lists to only include tool-supporting DeepSeek variants (e.g., `deepseek-v3.1` on NIM supports tools) and note that non-tool models are excluded.

Option B is recommended: it keeps the tool playbooks consistent and gives non-tool models a purpose-built test with appropriate expectations.

### R3 — Create playbooks for missing providers [high priority]

Add playbooks for the four supported providers that have no coverage:
- **`10-local-vllm-llama.md`** (and qwen, deepseek variants)
- **`11-self-hosted-hf-llama.md`**
- **`12-local-openai-compat.md`** (covers LocalAI and any OpenAI-compatible server)
- **`13-third-party-openai.md`** (OpenAI GPT models — this is a first-party supported provider with no playbook)

Follow the same `NN-category-provider-family.md` naming convention.

### R4 — Add a non-tool conversational playbook [medium priority]

Create a playbook (e.g., `20-conversation-no-tools.md`) for models that do not support tool calling. The message sequence should test pure conversation quality:
- Multi-turn reasoning without tools.
- Following instructions (formatting, constraints).
- Consistency across turns (referring back to earlier messages).

This gives the DeepSeek no-tool models and any future non-tool models a meaningful test to run.

### R5 — Factor out shared provider content [medium priority]

- Extract the LM Studio gotchas from 04/05/06 into a shared `PROVIDER_SETUP.md` or a provider-specific reference.
- Consider whether the current per-family playbook structure is the right granularity. An alternative: one file per provider (e.g., `01-local-ollama.md`) covering all model families for that provider, rather than three separate files per provider with near-identical content. This would reduce the file count from 9+ to 5–6 and make maintenance easier.

An intermediate option: keep per-family files but have them be thin references that point to a shared provider file for setup and a shared family file for model lists, leaving only the unique combination (provider × family) in the numbered file.

### R6 — Expand the message sequence [medium priority]

The current five-message sequence only exercises the Obsidian/notesmd skill. Add additional message sequences to cover:
- **Basic conversation** (no tools) — greeting, reasoning, follow-up, consistency.
- **Multi-tool use** — an agent with multiple skills enabled, triggering different tools across turns.
- **Channel delivery** — verify agents reply correctly through a channel (could reference the journey docs).
- **Error handling** — send a message that triggers a tool failure, verify the model recovers or explains the error.

These can be defined in the README as alternative sequences (like "Sequence A: Obsidian tool use", "Sequence B: Conversation only", etc.) so that individual playbooks can reference the appropriate one.

### R7 — Establish a results convention [medium priority]

The README defines a result template but no results exist. Make the playbooks actionable:
- Decide where results live (e.g., `docs/testing/results/` with files named by date or run-id, or inline in each playbook with a collapsible section, or in a separate repository).
- Add a "last run" date to each playbook so readers know currency.
- Define pass/fail criteria more precisely — currently "pass/fail" is left to the runner's judgment with no rubric.

### R8 — Enrich the AGENTS.md [low priority]

Replace the stub AGENTS.md with real guidelines and conventions:
- **Guidelines:** How to write a new playbook (model list format, setup section, procedure reference, tool support note).
- **Conventions:** Filename convention (confirm or refine `NN-category-provider-family.md`), model list format (supported/not-supported/trained-on-tool-use), link style for model sources.
- **Maintenance:** When to update model lists (new release, deprecation), when to add a new playbook (new provider or model family), when to re-validate.

### R9 — Add cross-references [low priority] ✅

- Link from each playbook to the provider spec (`base/spec/PROVIDERS.md`) and model spec (`base/spec/MODELS.md`) for authoritative configuration and model details.
- Link from the README to the configuration guide (`docs/guides/03-configuration.md`) for provider setup.
- Link from the relevant journey docs to the testing playbooks (e.g., after completing the Ollama journey, "for systematic model testing, see `docs/testing/01-local-ollama-llama.md`").

### R10 — Add model list freshness markers [maintenance]

Model lists will drift as new models are released and old ones are deprecated. Add a convention:
- A comment at the top of each model list: `<!-- Last updated: YYYY-MM-DD -->`
- A note in the README or AGENTS.md that model lists should be reviewed quarterly or after a major model release from a provider.
- Mark deprecated models with a note (e.g., "⚠️ superseded by llama3.1:8b") rather than removing them immediately, so historical test results remain meaningful.

### R11 — Consider a summary matrix in the README [future]

As the number of playbooks grows (with R3 adding 4+ more), the README's linear table will become harder to scan. Consider adding a summary matrix:

| Provider | Llama | Qwen | DeepSeek | No-Tool | Last Run |
|----------|-------|------|----------|---------|----------|
| Ollama   | 01 ✅ | 02   | 03       | —       | 2025-05  |
| LM Studio| 04   | 05   | 06       | —       | —        |
| NIM      | 07   | 08   | 09 ✅    | —       | 2025-05  |
| vLLM     | 10   | ...  |          |         |          |
| OpenAI   | 13   | —    | —        | —       | —        |

This gives at-a-glance coverage status. Only worthwhile once the playbook count exceeds ~12.

## Progress

| Session | Work Done |
|---------|-----------|
| 1 | Initial audit and FEAT file created |
| 2 | R9 complete: added "See Also" sections to all 9 playbooks linking to configuration guide, provider spec, and model spec. Updated testing README with cross-references to guides, journeys, and specs. |
