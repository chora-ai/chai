# FEAT: Improve User Guides

Improve the user guides at `docs/guides/` so they form a coherent, progressive user journey rather than a mix of stubs and reference dumps.

## Problem

- **01-introduction** and **02-getting-started** are stubs (heading + one sentence each). New users have no guided path from install to first interaction.
- **03-configuration** is a 14KB reference document disguised as a guide — flat tables with no narrative or progressive disclosure.
- **05-agents** mixes conceptual content with configuration details that duplicate 03, and includes `base/spec/...` links that don't resolve from the guides context.
- **06-skills** covers only the versioning/revision workflow despite being titled "Skills." There is no introductory content about what skills are or how to create one.
- **04-connections** and **07-sandbox** are reasonably good (07 is the quality target).
- Cross-cutting: no end-to-end journey, broken internal links, duplicated content, inconsistent heading hierarchy, and missing topics (CLI commands, desktop app, troubleshooting, provider/model selection guidance).

## Recommendations

### R1 — Fill the stubs (01 and 02) [high priority] ✅

- **01-introduction** — What chai is, key concepts (orchestrator, workers, providers, channels, skills, sandbox), how the pieces fit together, mental model.
- **02-getting-started** — Prerequisites, install, `chai init`, configure a first provider, start the gateway, send a first message, next steps.

### R2 — Restructure 03 into guide + reference [high priority] ✅

Split 03-configuration.md into:
- A **narrative guide** (~3–4 pages) walking through common config tasks in order of complexity (minimal → providers → channels → multi-agent → security).
- A **configuration reference** with the full tables (clearly separated section in the same file, or a separate file if it grows).

### R3 — Broaden 06-skills into a full skills guide [medium priority] ✅

Add front matter covering what skills are, directory layout (SKILL.md, tools.json, scripts/), creating a skill from scratch, enabling skills on an agent, inspecting and validating. Keep existing versioning content as a later section.

### R4 — Fix cross-references [medium priority] ✅

Audit all internal links. Options:
- Point to published doc paths if a doc site exists.
- Convert to inline references that don't rely on repo-relative paths.
- Acknowledge as source-tree references with a footnote convention.

### R5 — Add missing guide topics [lower priority]

- **CLI reference** — `chai init`, `chai gateway`, `chai profile`, `chai skill`, etc.
- **Desktop app guide** — GUI usage, pairing, profile switching.
- **Choosing a provider and model** — Decision guide based on hardware, privacy, use case.
- **Troubleshooting** — Common errors and fixes.

### R6 — Clean up consistency [low priority]

- Normalize heading hierarchy (all guides start at `#` for the title).
- Apply title-case heading convention per code style guidelines.
- Deduplicate agent configuration content between 03 and 05.
- Standardize code block language hints and formatting.

### R7 — Enhance guide README [low priority]

Add a one-line description per guide in `docs/guides/README.md` so readers can choose where to start without opening each file. Currently just a flat link list.

### R8 — Document `chai file` subcommands [low priority]

The CLI has a full `file` subcommand group (read, write, append, patch lines, delete file/dir, read/write/remove frontmatter, rename with wikilink update). These are primarily skill-tool-facing but are part of the CLI surface. Consider adding at least a reference entry, possibly within the CLI reference (R5) or a dedicated section.

### R9 — Cross-reference journey docs [low priority]

`docs/journey/` has step-by-step test procedures that complement the guides (e.g., "set up Telegram", "use the notesmd skill"). The guides don't mention them and the journeys don't link back. Add directional links between the two so readers can find the hands-on procedures from the conceptual guides and vice versa.

### R10 — Anchor link stability note [maintenance]

Inter-guide links now use fragment anchors (e.g., `03-configuration.md#agents`, `03-configuration.md#securing-the-gateway`). These depend on heading text and render differently across GitHub, mdbook, Hugo, etc. If the guides move to a static site generator, anchors will need auditing. Note this as a review item for any docs-publishing migration.

### R11 — Architecture diagram maintenance [maintenance]

01-introduction now includes an ASCII diagram of the system architecture. This is useful but will drift as the architecture changes. Flag as something to review when significant structural changes land (new providers, new channel types, major agent model changes, etc.).

### R12 — Consider splitting 03 reference section [future]

The configuration reference in 03-configuration.md is currently in the same file, separated by `---`. This works at current size but may become unwieldy. If it grows substantially, splitting into a separate `03-configuration-reference.md` would keep the guide scannable.

## Progress

| Session | Work Done |
|---------|-----------|
| 1 | Initial audit and FEAT file created |
| 2 | R1 complete: rewrote 01-introduction and 02-getting-started. R3 complete: rewrote 06-skills with full introductory content. |
| 3 | R2 complete: restructured 03-configuration into narrative guide + reference section. |
| 4 | R4 complete: fixed cross-references in 05-agents and 04-connections. R6 partial: all guides now use `#` title and title-case headings. Added R7–R12. |
| 5 | R9 complete: added cross-references from guides to journeys and testing. Each guide now links to relevant journeys ("Try It" sections) and the testing playbooks. Updated 01, 02, 03, 04, 05, 06, 07. Added "How the Docs Relate" section to docs/README.md. |
