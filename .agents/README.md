# Additional Resources

This directory includes additional resources for agents (and humans) working on the codebase. Use it to find additional context and to add and update documents for future reference.

## Directory Layout

| Location | Purpose |
|----------|---------|
| **`adr/`** | Architecture Decision Records: why we chose X, alternatives considered. |
| **`ref/`** | External systems: summaries of other systems or specs (e.g. OpenClaw, Ollama) for alignment. |
| **`spec/`** | Internal specs and design summaries: how this project works (e.g. agent context, skill format and loader). |
| **root** | Deliverables and working documents: what was built, how it was built, and what's next. |
## Current Documents

### `/adr`

- **[PROGRAMMING_LANGUAGE.md](adr/PROGRAMMING_LANGUAGE.md)** — Why Rust was chosen for this project.
- **[DESKTOP_FRAMEWORK.md](adr/DESKTOP_FRAMEWORK.md)** — Why egui/eframe was chosen for the desktop UI.

### `/ref`

- **[OPENCLAW_REFERENCE.md](ref/OPENCLAW_REFERENCE.md)** — OpenClaw concepts, protocol, and design reference for alignment.

### `/spec`

- **[AGENT_CONTEXT.md](spec/AGENT_CONTEXT.md)** — How context is built and provided to the model each turn.
- **[SKILL_FORMAT.md](spec/SKILL_FORMAT.md)** — The format for skills, frontmatter, metadata, and loaders.

### root

- **[POC_IMPLEMENTATION.md](POC_IMPLEMENTATION.md)** — Record of the POC deliverable and how it differs from OpenClaw.

## Adding Documents

- **Decisions** (rationale, alternatives, “why we chose X”) → add under **`adr/`**. Use a clear filename (e.g. `PROGRAMMING_LANGUAGE.md`, `DESKTOP_FRAMEWORK.md`). Follow the general format and tone of other documents in the directory.
- **Reference** (external system or spec summary for alignment) → add under **`ref/`**. Use a clear filename (e.g. `OLLAMA_REFERENCE.md`, `LM_STUDIO_REFERENCE.md`). Follow the general format and tone of other documents in the directory.
- **Specifcation** (internal: how this project works—context shape, format, loader behavior) → add under **`spec/`**. Use a clear filename (e.g. `AGENT_CONTEXT.md`, `SKILL_FORMAT.md`). Follow the general format and tone of other documents in the directory.
- **Deliverable / Working Document** (what was built, what’s next) → add at **root**. Use a clear filename (e.g. `POC_IMPLEMENTATION.md`, `MVP_IMPLEMENTATION.md`). Follow the general format and tone of other documents in the directory.
