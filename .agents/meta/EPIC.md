# Epic Conventions

Epics are proposals and tracking documents for significant features or architectural changes. They live in the `.agents/` root directory with the `EPIC_` prefix (e.g. `EPIC_ORCHESTRATION.md`).

## Frontmatter

Every epic must include YAML frontmatter with the following fields:

```yaml
---
status: draft
---
```

### Required Fields

| Field | Values | Description |
|-------|--------|-------------|
| `status` | `draft`, `proposed`, `in-progress`, `complete` | Current lifecycle state |

### Lifecycle States

| State | Meaning |
|-------|---------|
| `draft` | Idea captured; no implementation commitment. Design and scope may still be open. |
| `proposed` | Specified with enough detail to evaluate and begin implementation. |
| `in-progress` | Active implementation underway; phases tracked in the document. |
| `complete` | All phases delivered. Follow-ups (if any) are tracked in the document or in separate epics. |

## Structure

Epics follow a standard section order. Sections marked *(when applicable)* may be omitted when they do not apply.

### Required Sections

**Title** — First heading: `# Epic: <Name>`

**Summary** — Bold paragraph immediately after the title. One to three sentences describing what this epic delivers.

**Status** — Bold line after the summary. States the lifecycle state and brief context (e.g. which phases are complete, what remains). Must be consistent with the frontmatter `status` field.

**Problem Statement** — Why this work is needed. What is missing, broken, or insufficient today. Motivates the epic before describing the solution.

**Goal** — What success looks like. Describes the desired end state, not the implementation path.

**Scope** — What is in scope and what is out of scope. Use two subsections (`### In Scope`, `### Out of Scope`) or a combined description when the boundary is simple. Out of scope items should reference where they are tracked if applicable (e.g. another epic).

**Requirements** — Checklist of discrete deliverables. Use `- [ ]` for pending items and `- [x]` for completed items. Requirements should be specific enough that completion is unambiguous.

**Phases** — Ordered delivery plan. Use a table with columns for phase number/name, focus, and status. Phases should be incrementally deliverable.

**Related Epics and Docs** — Cross-references to other epics, specs, ADRs, reference documents, and external files (e.g. `.testing/`, `.journey/`). Use relative links.

### Optional Sections

**Current State** *(when applicable)* — Baseline before this epic. What exists today that this epic changes or builds on. Useful for epics that modify existing behavior.

**Dependencies** *(when applicable)* — What must exist before this epic can begin or what must be coordinated with. Reference the specific epic or spec.

**Design** — Options considered, decisions made, and axes still open. Use subsections as needed (e.g. `### Design Options`, `### Decisions`, `### Design Axes`). Record resolved decisions with rationale. Tables work well for decision records and option comparisons.

**Technical Reference** *(when applicable)* — Definitions, implementation notes, comparison tables, and other context the implementing agent needs. This section exists for the agent's benefit during implementation. Include wire formats, API shapes, code paths, and architectural notes as appropriate.

**Open Questions** *(when applicable)* — Unresolved decisions. Each question should be specific enough to resolve (not open-ended). Mark resolved questions inline (e.g. **Resolved:** with the answer) or move them to Design decisions.

**Follow-ups** *(when applicable)* — Post-completion backlog items that do not block treating the epic as complete. Use when the epic is `in-progress` or `complete` and there are known improvements or extensions. Distinguish clearly from Requirements (which block completion).

## Section Order

When present, sections appear in this order:

1. Title
2. Summary
3. Status
4. Problem Statement
5. Goal
6. Current State
7. Scope
8. Dependencies
9. Design
10. Requirements
11. Technical Reference
12. Phases
13. Open Questions
14. Follow-ups
15. Related Epics and Docs

## Naming

- File name: `EPIC_<NAME>.md` (uppercase, underscores)
- Title: `# Epic: <Human Readable Name>`
- Name should be concise and descriptive of the feature or change

## Maintenance

- Update the frontmatter `status` field when the lifecycle state changes
- Keep the Status line consistent with the frontmatter
- Mark Requirements as complete (`[x]`) as they are delivered
- Update Phase status as phases complete
- Move resolved Open Questions to Design decisions or remove them
- Update stale references to other documents (e.g. "not yet implemented" for features that have shipped)
