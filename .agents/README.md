# Additional Resources

This directory includes additional resources for agents (and humans) working on the codebase. Use it to find additional context and to add and update documents for future reference.

## Directory Layout

| Location | Purpose |
|----------|---------|
| **`adr/`** | Architecture Decision Records: why we chose X, alternatives considered. |
| **`ref/`** | External systems: summaries of other systems or specs (e.g. OpenClaw, Ollama, LM Studio) for alignment. |
| **`spec/`** | Internal specs and design summaries: how this project works (e.g. agent context, skill format and loader). |
| **root** | Deliverables and working documents: what was built, how it was built, and what's next. |
## Current Documents

### `/adr`

- **[PROGRAMMING_LANGUAGE.md](adr/PROGRAMMING_LANGUAGE.md)** — Why Rust was chosen for this project.
- **[DESKTOP_FRAMEWORK.md](adr/DESKTOP_FRAMEWORK.md)** — Why egui/eframe was chosen for the desktop UI.

### `/ref`

- **[OPENCLAW_REFERENCE.md](ref/OPENCLAW_REFERENCE.md)** — OpenClaw concepts, protocol, and design reference for alignment.
- **[OLLAMA_REFERENCE.md](ref/OLLAMA_REFERENCE.md)** — Ollama API references, how it is used, and additional API capabilities.
- **[LM_STUDIO_REFERENCE.md](ref/LM_STUDIO_REFERENCE.md)** — LM Studio API reference, how it is used, and additional API capabilities

### `/spec`

- **[AGENT_CONTEXT.md](spec/AGENT_CONTEXT.md)** — How context is built and provided to the model.
- **[SKILL_FORMAT.md](spec/SKILL_FORMAT.md)** — The format for skills, frontmatter, metadata, and loaders.
- **[TOOLS_SCHEMA.md](spec/TOOLS_SCHEMA.md)** — The tools.json schema for declarative skill tools.

### root

- **[POC_CHANGELOG.md](POC_CHANGELOG.md)** — Changelog of features added in the proof-of-concept implementation.
- **[POC_DELIVERABLE.md](POC_DELIVERABLE.md)** — High-level summary of the proof-of-concept implementation and next steps.
- **[POC_IMPLEMENTATION.md](POC_IMPLEMENTATION.md)** — Detailed technical reference for the proof-of-concept implementation.
- **[EPIC_API_ALIGNMENT.md](EPIC_API_ALIGNMENT.md)** — Epic: LLM services and API alignment (proposal and tracking for multiple backends).
- **[EPIC_ORCHESTRATION.md](EPIC_ORCHESTRATION.md)** — Epic: Orchestrators and workers (proposal and tracking for multi-model flows).
- **[SERVICES_AND_MODELS.md](SERVICES_AND_MODELS.md)** — Working document for comparing LLM services and models.
- **[TEST_LOCAL_MODELS.md](TEST_LOCAL_MODELS.md)** — Working document for testing the performance of local models.
- **[TEST_SELF_HOSTED_MODELS.md](TEST_SELF_HOSTED_MODELS.md)** — Working document for testing the performance of self-hosted models.
- **[TEST_THIRD_PARTY_MODELS.md](TEST_THIRD_PARTY_MODELS.md)** — Working document for testing the performance of third-party models.

## Adding Documents

- **Decisions** (rationale, alternatives, “why we chose X”) → add under **`adr/`**. Use a clear filename (e.g. `PROGRAMMING_LANGUAGE.md`, `DESKTOP_FRAMEWORK.md`). Follow the general format and tone of other documents in the directory.
- **Reference** (external system or spec summary for alignment) → add under **`ref/`**. Use a clear filename (e.g. `OPENCLAW_REFERENCE.md`, `OLLAMA_REFERENCE.md`). Follow the general format and tone of other documents in the directory.
- **Specifcation** (internal: how this project works—context shape, format, loader behavior) → add under **`spec/`**. Use a clear filename (e.g. `AGENT_CONTEXT.md`, `SKILL_FORMAT.md`). Follow the general format and tone of other documents in the directory.
- **Work In Progress** (what's built, what’s next, comparing and testing different models) → add at **root**. Use a clear filename (e.g. `POC_DELIVERABLE.md`, `POC_IMPLEMENTATION.md`). Follow the general format and tone of other documents in the directory.
