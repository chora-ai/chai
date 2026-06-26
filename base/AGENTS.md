# Agents

This is the `AGENTS.md` file in the root of the `base` directory.

## Directory

The `base` directory serves two roles:

1. **Structured Knowledge Base** — Canonical, versioned documentation for design decisions and formal reference (`adr/`, `epic/`, `ref/`, `spec/`, `tag/`). Defined and tracked in `README.md` and governed by conventions in `meta/`.
2. **Working Notes Layer** — Ephemeral tracking of audits, bugs, and features on feature branches (`AUDIT_*`, `BUG_*`, `FEAT_*`). Working notes are lighter-weight and graduate into structured documentation when they mature.

## Conventions

- Always read [README.md](README.md) and the relevant `meta/` file **before updating structured documentation**.
- **Issue tracking**: Audits, bugs, and features are tracked in working notes (prefixed `AUDIT_*`, `BUG_*`, `FEAT_*`). Working note prefixes describe the *motivation* (why the work exists), while conventional commit types describe the *nature of the change* (what the code change is). These are independent — for example, a `BUG_*` note tracking a test flakiness issue may result in a `test/` branch and commit type, just as a `FEAT_*` note may result in a `refactor/` type.

## Working Notes

The `AUDIT_*`/`BUG_*`/`FEAT_*` files in the root of the `base` directory are a **lighter-weight tracking layer**. They are being worked on through the agent before being migrated to structured documentation. The relationship is:

- **Working notes** (`AUDIT_*`/`BUG_*`/`FEAT_*`) = active tracking notes within a feature branch. Live on feature branches only — they are never merged into `main`.
- **Release notes** (`RELEASE_*`) = release-specific tracking. Live on `main` temporarily and are deleted before the release commit.
- **Structured docs** (`adr/`, `epic/`, `ref/`, `spec/`, `tag/`) = canonical, versioned, shared. For design decisions and formal reference.

### Graduation

When a working note matures (e.g. a bug fix is verified, or a feature grows into a design decision), its substance should **graduate into structured documentation** before the feature branch is squash-merged:

- A significant change in architecture → new ADR (`adr/`)
- A feature that grows in scope → new epic (`epic/`)
- A spec that needs updating → update existing spec (`spec/`)
- A change to channels or providers → update existing ref (`ref/`)

Always update root directory `CHANGELOG.md` and user documentation (`docs/`) when graduating working notes that include changes to user-facing features. Conventions for updating user documentation can be found in the root directory `AGENTS.md`, `docs/AGENTS.md`, and `AGENTS.md` files in each `docs/` subdirectory.

### Lifecycle

Working notes live **only on feature branches** — they are never merged into `main`. The lifecycle:

1. **First commit** on the feature branch: add working note (e.g. `FEAT_GIT_MERGE.md`).
2. **Subsequent commits**: the actual implementation, unit tests, updates to the working note, etc.
3. **Graduation commit**: update structured documentation and **delete the working note**.
4. **Squash-merge**: the working note (e.g. `FEAT_GIT_MERGE.md`) is not included in the diff.

`RELEASE_*` working notes are the only exception — they are committed directly to `main` per the release process in `RELEASE.md`, and then deleted before the release commit.
