# Agents

This is the `AGENTS.md` file in the root of the `base` directory.

## Directory

The `base` directory is the root of the chai knowledge base and also contains ad-hoc working notes for bugs and improvements being tracked. [README.md](README.md)** is the entry point for this directory's structured documentation.

## Conventions

- **Issue tracking**: Bugs, features, and audits are tracked in files prefixed with `BUG_`, `FEAT_`, and `AUDIT_` respectively. Summaries are maintained in this file under "Active Work"; full details are in the individual files.

## Working Notes

The `AUDIT_*`/`BUG_*`/`FEAT_*` files in the root of the `base` directory are a **lighter-weight tracking layer**. They're for small bugs, improvements, and reviews being worked on through the agent before they're ready for the formal structured documentation. The relationship is:

- **Working notes** (`AUDIT_*`/`BUG_*`/`FEAT_*`) = active tracking, ad-hoc. For agent-driven discovery and quick iteration.
- **Structured docs** (`adr/`, `epic/`, `spec/`, etc.) = canonical, versioned, shared. Formal frontmatter and structure. For design decisions and project-wide reference.

When a working note matures (e.g. a bug fix is verified, or a feature grows into a design decision), its substance should **graduate into structured documentation**:
- A significant change in architecture → new ADR (`adr/`)
- A feature that grows in scope → new epic (`epic/`)
- A spec that needs updating → update existing spec (`spec/`)
- A change to channels or providers → update existing ref (`ref/`)

## Active Work

- **[AUDIT_SKILLS.md](AUDIT_SKILLS.md)** — Cross-skill audit of all bundled skills: identify redundancies between SKILL.md and tool schemas, classify directives by enforceability, evaluate examples, review frontmatter field purposes and context costs, and find cross-skill patterns. Round 3 in progress.
- **[BUG_FILES_REPLACE.md](BUG_FILES_REPLACE.md)** — fixed multiple issues with `files_replace`; continuing to test and monitor.
- **[BUG_FILES_WRITE_LINES.md](BUG_FILES_WRITE_LINES.md)** — fixed multiple issues with `files_write_lines`; continuing to test and monitor.
- **[FEAT_SKILL_CARGO.md](FEAT_SKILL_CARGO.md)** — Expose cargo check/test/build as a chai skill so the agent can verify code changes compile and pass tests.
- **[FEAT_SKILL_LOGS.md](FEAT_SKILL_LOGS.md)** — Expose chai process logs as a chai skill so the agent can read diagnostic output (finish_reason, usage tokens).
- **[RELEASE.md](RELEASE.md)** — Release process design: how releases are tagged, tracked, documented, and distributed.
- **[RELEASE_V0_1_0.md](RELEASE_V0_1_0.md)** — Requirements and resolved decisions for v0.1.0. Messaging channels epic complete; both Matrix and Signal are experimental opt-in features.
- **[SECURITY.md](SECURITY.md)** — Known security considerations and vulnerabilities in Chai's agent sandboxing model.
