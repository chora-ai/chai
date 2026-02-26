# Agent Resources

This directory holds resources for agents (and humans) working on the codebase. Use it to find context and to add new documents in the right place.

## Layout

| Location | Purpose | When to Add |
|----------|---------|-------------|
| **`adr/`** | Architecture Decision Records: why we chose X, alternatives considered. | When recording a significant technical or product decision (e.g. language, framework, packaging, local model integration). |
| **`reference/`** | Reference material: external systems, protocols, or design summaries for continuation work. | When adding a doc that summarizes another system or spec (e.g. OpenClaw, Ollama) so the project can extend or align without keeping the original source. |
| **root** | Status and deliverable documents: what was built, what’s next, how we differ from others. | When adding project-level status or deliverable records (e.g. POC record). |

## Current Documents

#### `/adr`

- **[PROGRAMMING_LANGUAGE.md](adr/PROGRAMMING_LANGUAGE.md)** — Why Rust was chosen for CLI and desktop.
- **[DESKTOP_FRAMEWORK.md](adr/DESKTOP_FRAMEWORK.md)** — Why egui/eframe was selected for the desktop UI.

#### `/reference`

- **[OPENCLAW_REFERENCE.md](reference/OPENCLAW_REFERENCE.md)** — OpenClaw concepts, protocol, and design reference for continuation work (gateway, pairing, skills, exec, etc.).

#### root

- **[AGENT_CONTEXT.md](AGENT_CONTEXT.md)** — Working doc: exact context provided to the model each turn (system message, session messages, tools); turn vs session; what is sent every turn; efficiency notes.
- **[POC_IMPLEMENTATION.md](POC_IMPLEMENTATION.md)** — Record of the POC deliverable and how it differs from OpenClaw.

## For Agents

- **Decisions** (rationale, alternatives, “why we chose X”) → add under **`adr/`**. Use a clear filename (e.g. `PACKAGING.md`, `LOCAL_MODELS.md`). Keep content project-agnostic where possible; link to project-specific details only where relevant.
- **Reference** (external system or protocol summary for future work) → add under **`reference/`**. Use a clear filename (e.g. `OLLAMA_REFERENCE.md`, `LM_STUDIO_REFERENCE.md`).
- **Status / deliverable** (what was built, what’s next) → add at **root**.
- **AGENTS.md** (repo root) lists these resources and links to them; update its table when you add or move documents.
