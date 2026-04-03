# Spec Conventions

Specs are behavioral contracts that describe how the system works. They live in `.agents/spec/` (e.g. `spec/SKILL_FORMAT.md`). Specs are reference documents for the implementing agent — they define the behavior the code must produce, not the code itself.

**Epics** in **`epic/`** propose and track work; **specs** document behavior. When epic-driven features land, the spec for that area should reflect the new behavior.

**Same base name as an epic** (e.g. `ORCHESTRATION.md` in both **`epic/`** and **`spec/`**) — The **spec** is the runtime contract; the **epic** is roadmap, phases, and rationale. In prose, say **orchestration spec** vs **orchestration epic** (or **spec `ORCHESTRATION`** vs **epic `ORCHESTRATION`**) so readers follow the correct link.

## Frontmatter

Every spec must include YAML frontmatter with the following fields:

```yaml
---
status: draft
---
```

### Required Fields

| Field | Values | Description |
|-------|--------|-------------|
| `status` | `draft`, `stable` | Current state of the spec |

### States

| State | Meaning |
|-------|---------|
| `draft` | Content is being developed or is subject to significant change. |
| `stable` | Content reflects implemented behavior. Changes are incremental updates, not rewrites. |

## Structure

Specs are less prescriptive in structure than epics. The content should be organized for an implementing agent to find what it needs quickly.

### Required Sections

**Title** — First heading: `# <Name>` (no prefix convention; use a descriptive name)

**Purpose** — One to three sentences at the top describing what this spec covers and when to reference it.

### Recommended Patterns

- **Definitions** — Define terms used in the spec, especially when they have specific technical meanings in the project
- **Behavior descriptions** — Describe what the system does, not how the code is structured. Focus on inputs, outputs, and invariants.
- **Tables** — Use tables for field definitions, enum values, format mappings, and comparison matrices
- **Code examples** — Use fenced code blocks for wire formats, JSON shapes, config snippets, and schema examples
- **Cross-references** — Link to related specs, epics, and reference documents

## Naming

- File name: `<NAME>.md` (uppercase, underscores)
- Place in `spec/` directory

## Maintenance

- Update the frontmatter `status` from `draft` to `stable` when the spec reflects implemented behavior
- When implementation changes behavior, update the spec in the same change or session when practical
- Prefer keeping specs accurate over leaving known drift; link to the relevant **`epic/*.md`** when history or motivation helps
