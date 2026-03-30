# Reference Document Conventions

Reference documents summarize external systems, APIs, and projects for agent use during implementation. They live in `.agents/ref/` (e.g. `ref/OLLAMA_REFERENCE.md`). Reference docs exist so the implementing agent has the context it needs without fetching external documentation mid-session.

## Frontmatter

Every reference document must include YAML frontmatter with the following fields:

```yaml
---
status: current
---
```

### Required Fields

| Field | Values | Description |
|-------|--------|-------------|
| `status` | `current`, `outdated` | Whether the document reflects the current external system |

### States

| State | Meaning |
|-------|---------|
| `current` | Content reflects the external system as last verified. |
| `outdated` | The external system has changed and the document needs updating. |

## Structure

### Required Sections

**Title** — First heading: `# <System Name> Reference` (e.g. `# Ollama Reference`)

**Purpose** — One to three sentences describing what external system this covers and how Chai uses it.

### Recommended Patterns

- **API surface** — Endpoints, request/response shapes, authentication, and headers relevant to Chai's integration
- **Configuration** — How the external system is configured in Chai's `config.json` and environment variables
- **Comparison tables** — When documenting multiple related systems (e.g. Claw ecosystem), use tables for feature comparison
- **Links** — Include official documentation URLs for the external system

## Naming

- File name: `<SYSTEM_NAME>_REFERENCE.md` (uppercase, underscores) for single-system references
- File name: `<TOPIC>.md` for comparison or ecosystem documents (e.g. `CLAW_ECOSYSTEM.md`)
- Place in `ref/` directory

## Maintenance

- When an external API changes, update the reference document and verify Chai's integration still matches
- Set `status: outdated` when the external system has changed but the document has not been updated yet
