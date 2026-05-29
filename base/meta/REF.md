# Reference Document Conventions

Reference documents summarize external systems, APIs, and projects for agent use during implementation. They live in `.agents/ref/` (e.g. `ref/OLLAMA.md`). Reference docs exist so the implementing agent has the context it needs without fetching external documentation mid-session.

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
| `status` | `current`, `outdated` | Whether the document matches the external system and Chai's integration (see **States**) |

### States

| State | Meaning |
|-------|---------|
| `current` | Default. Content reflects the external system and Chai's integration as last verified. |
| `outdated` | Optional. Use only when drift is **known** and an update is **deferred** (e.g. upstream changed and the fix is not done yet). Prefer updating the document when you change Chai's integration of that system in the same change or session. |

Most of the time you should **edit the doc and leave `status: current`** rather than marking **`outdated`**. **`outdated`** is an honest bookmark for incomplete follow-up, not a substitute for fixing the reference.

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

- File name: `<SYSTEM_NAME>.md` (uppercase, underscores) for single-system references
- File name: `<TOPIC>.md` for comparison or ecosystem documents (e.g. `CLAW_ECOSYSTEM.md`)
- Place in `ref/` directory; the **`ref/`** path identifies these as reference documents.

## Maintenance

- When an external API changes or you change how Chai calls it, **update this reference** and verify the integration still matches
- If you cannot update immediately, set `status: outdated` until the doc matches reality again, then return to `current`
