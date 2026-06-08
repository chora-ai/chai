# Agents

This is the `AGENTS.md` file in the root of the `base` directory.

## Directory

The `base` directory is the root of the chai knowledge base and also contains ad-hoc working notes for bugs and improvements being tracked. [README.md](README.md)** is the entry point for this directory's structured documentation.

## Conventions

- **Issue tracking**: Bugs, features, and audits are tracked in files prefixed with `BUG_`, `FEAT_`, and `AUDIT_` respectively (e.g. `FEAT_SKILL_LOGS.md`, `AUDIT_SKILLS.md`). Summaries are maintained in this file under "Active Work"; full details are in the individual files.

## Working Notes

The `BUG_*`/`FEAT_*`/`AUDIT_*` files in the root of the `base` directory are a **lighter-weight tracking layer**. They're for small bugs, improvements, and reviews being worked on through the agent before they're ready for the formal structured documentation. The relationship is:

- **Working notes** (`BUG_*`/`FEAT_*`/`AUDIT_*`) = active tracking, ad-hoc. For agent-driven discovery and quick iteration.
- **Structured docs** (`adr/`, `epic/`, etc.) = canonical, versioned, shared. Formal frontmatter and structure. For design decisions and project-wide reference.

When a working note matures (e.g. a bug fix is verified, or a feature grows into a design decision), its substance should **graduate into structured documentation**:
- A significant change in architecture → new ADR (`adr/`)
- A feature that grows in scope → new epic (`epic/`)
- A spec that needs updating → update existing spec (`spec/`)
- A change to channels or providers → update existing ref (`ref/`)

## Active Work

- **[AUDIT_SKILLS.md](AUDIT_SKILLS.md)** — Cross-skill audit of all bundled skills: identify redundancies between SKILL.md and tool schemas, classify directives by enforceability, evaluate examples, review frontmatter field purposes and context costs, and find cross-skill patterns. `files` and `skills-design` have initial findings; frontmatter question absorbed from former `FEAT_SKILL_MODE_FRONTMATTER.md`.
- **[FEAT_SKILL_LOGS.md](FEAT_SKILL_LOGS.md)** — Expose chai process logs as a chai skill so the agent can read diagnostic output (finish_reason, usage tokens).
- **[FEAT_SKILL_CARGO.md](FEAT_SKILL_CARGO.md)** — Expose cargo check/test/build as a chai skill so the agent can verify code changes compile and pass tests.
