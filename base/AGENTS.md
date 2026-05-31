# Agents

This is the `AGENTS.md` file in the root of the `base` directory.

## Directory

The `base` directory is the root of the chai knowledge base and also contains ad-hoc working notes for bugs and improvements being tracked. [README.md](README.md)** is the entry point for this directory's structured documentation.

## Conventions

- **Issue tracking**: Bugs and feature requests are tracked in files prefixed with `BUG_` and `FEAT_` respectively (e.g. `FEAT_AUDIT_LARGE_FILES.md`). Summaries are maintained in this file under "Active Work"; full details are in the individual files.

## Working Notes

The `BUG_*`/`FEAT_*` files in the root of the `base` directory are a **lighter-weight tracking layer**. They're for small bugs and improvements being worked on through the agent before they're ready for the formal structured documentation. The relationship is:

- **Working notes** (`BUG_*`/`FEAT_*`) = active tracking, ad-hoc. For agent-driven discovery and quick iteration.
- **Structured docs** (`adr/`, `epic/`, etc.) = canonical, versioned, shared. Formal frontmatter and structure. For design decisions and project-wide reference.

When a working note matures (e.g. a bug fix is verified, or a feature grows into a design decision), its substance should **graduate into structured documentation**:
- A fix that changes architecture → new ADR (e.g., `successExitCodes` in `tools.json` → `adr/`)
- A feature that grows in scope → new epic (e.g., tool approval workflow → `epic/`)
- A spec that needs updating → update existing spec (e.g., `spec/TOOLS_SCHEMA.md`)
- Reference material → `ref/`

Completed working notes where the substance is captured in source files or structured docs should be deleted to keep the working layer clean.

## Active Work

*(No active working notes.)*
