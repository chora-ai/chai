# ADR Conventions

Architecture Decision Records document why significant technical choices were made. They live in `.agents/adr/` (e.g. `adr/PROGRAMMING_LANGUAGE.md`). ADRs are retrospective — they record decisions already made, not proposals for future work (use epics for that).

## Frontmatter

Every ADR must include YAML frontmatter with the following fields:

```yaml
---
status: accepted
---
```

### Required Fields

| Field | Values | Description |
|-------|--------|-------------|
| `status` | `accepted`, `superseded` | Current state of the decision |

### States

| State | Meaning |
|-------|---------|
| `accepted` | The decision is in effect and reflected in the codebase. |
| `superseded` | A later decision replaced this one. Link to the superseding ADR or epic. |

## Structure

### Required Sections

**Title** — First heading: `# <Decision Subject>` (e.g. `# Programming Language`, `# Desktop Framework`)

**Context** — What situation or question prompted the decision. What constraints existed.

**Decision** — What was chosen and a concise statement of why.

**Alternatives Considered** — What other options were evaluated and why they were not chosen. Use a table or list.

### Optional Sections

**Consequences** *(when applicable)* — Known tradeoffs or implications of the decision.

**References** *(when applicable)* — Links to relevant external resources, documentation, or discussions.

## Naming

- File name: `<DECISION_SUBJECT>.md` (uppercase, underscores)
- Place in `adr/` directory

## Maintenance

- ADRs are generally stable after writing — they record a point-in-time decision
- If a decision is reversed or replaced, set `status: superseded` and add a note linking to the new decision
