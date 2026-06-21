# Agents

This is the `AGENTS.md` file in the root of the `base` directory.

## Directory

The `base` directory is the root of the chai notes and also contains ad-hoc working notes for bugs and improvements being tracked.

## Conventions

- **Issue tracking**: Audits, bugs, and features are tracked in working notes (prefixed `AUDIT_`, `BUG_`, `FEAT_`) that live **only on feature branches** — they are never merged into `main`. Release requirements (`RELEASE_*`) are committed directly to `main` per the release process.

## Working Notes

The `AUDIT_*`/`BUG_*`/`FEAT_*` files in the root of the `base` directory are a **lighter-weight tracking layer**. They are being worked on through the agent before being migrated to structured documentation. The relationship is:

- **Working notes** (`AUDIT_*`/`BUG_*`/`FEAT_*`) = active tracking notes within a feature branch. Live on feature branches only.
- **Release notes** (`RELEASE_*`) = release-specific tracking. Live on `main` temporarily and are deleted before the release commit.
- **Structured docs** (`adr/`, `epic/`, `ref/`, `spec/`, `tag/`) = canonical, versioned, shared. For design decisions and formal reference.

When a working note matures (e.g. a bug fix is verified, or a feature grows into a design decision), its substance should **graduate into structured documentation** before the feature branch is squash-merged:
- A significant change in architecture → new ADR (`adr/`)
- A feature that grows in scope → new epic (`epic/`)
- A spec that needs updating → update existing spec (`spec/`)
- A change to channels or providers → update existing ref (`ref/`)
- A completed feature → update the changelog, then delete the working note

After graduation, the working note is deleted on the feature branch. The squash-merge into `main` includes only the permanent changes — the working note's creation and deletion cancel out.

Always read [README.md](README.md) and the relevant `meta/` file **before updating structured documentation**.
