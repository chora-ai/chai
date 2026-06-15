# Agents

This is the `AGENTS.md` file in the root of the `base` directory.

## Directory

The `base` directory is the root of the chai knowledge base and also contains ad-hoc working notes for bugs and improvements being tracked.

## Conventions

- **Issue tracking**: Audits, bugs, features, and release requirements are tracked in files prefixed with `AUDIT_`, `BUG_`, `FEAT_`, and `RELEASE_` respectively. Summaries are maintained in this file under "Active Work"; full details are in the individual files.

## Working Notes

The `AUDIT_*`/`BUG_*`/`FEAT_*`/`RELEASE_*` files in the root of the `base` directory are a **lighter-weight tracking layer**. They are being worked on through the agent before being migrated to structured documentation. The relationship is:

- **Working notes** (`AUDIT_*`/`BUG_*`/`FEAT_*`/`RELEASE_*`) = active tracking, ad-hoc. For agent-driven discovery and quick iteration.
- **Structured docs** (`adr/`, `epic/`, `ref/`, `spec/`, `tag/`) = canonical, versioned, shared. For design decisions and formal reference.

When a working note matures (e.g. a bug fix is verified, or a feature grows into a design decision), its substance should **graduate into structured documentation**:
- A significant change in architecture → new ADR (`adr/`)
- A feature that grows in scope → new epic (`epic/`)
- A spec that needs updating → update existing spec (`spec/`)
- A change to channels or providers → update existing ref (`ref/`)
- A completed release tracking document → delete the working document

Always read [README.md](README.md) and the relevant `meta/` file **before updating structured documentation**.

## Active Work

- **[AUDIT_SKILLS.md](AUDIT_SKILLS.md)** — Cross-skill audit of all bundled skills: identify redundancies between SKILL.md and tool schemas, classify directives by enforceability, evaluate examples, review frontmatter field purposes and context costs, and find cross-skill patterns. Round 3 in progress.
- **[BUG_FILES_REPLACE.md](BUG_FILES_REPLACE.md)** — fixed multiple issues with `files_replace`; continuing to test and monitor.
- **[BUG_FILES_WRITE_LINES.md](BUG_FILES_WRITE_LINES.md)** — fixed multiple issues with `files_write_lines`; continuing to test and monitor.
- **[FEAT_SKILL_CARGO.md](FEAT_SKILL_CARGO.md)** — Expose cargo check/test/build as a chai skill so the agent can verify code changes compile and pass tests.
- **[RELEASE_V0_1_0.md](RELEASE_V0_1_0.md)** — Requirements and resolved decisions for v0.1.0. Messaging channels epic complete; both Matrix and Signal are experimental opt-in features.

## Structured Documentation

- **[README.md](README.md)** is the entry point for this directory's structured documentation.
- **[RELEASE.md](RELEASE.md)** — Official release process: how releases are planned, tagged, documented, and distributed.
- **[SECURITY.md](SECURITY.md)** — Known security considerations and vulnerabilities in Chai's agent sandboxing model.
- **[VISION.md](VISION.md)** — Known security considerations and vulnerabilities in Chai's agent sandboxing model.
