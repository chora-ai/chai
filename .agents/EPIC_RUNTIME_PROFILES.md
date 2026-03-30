---
status: draft
---

# Epic: Runtime Profiles and Isolated Environments

**Summary** — Introduce a **NixOS-like switching model** for Chai: multiple **named runtime profiles** under **`~/.chai/profiles/`**, with **one active profile** at a time. **Switching profiles** means **restarting the gateway** (and optionally desktop attach) so the stack—models, **which skills are loaded**, tool allowlists, channel bindings, workspace—comes from that profile’s **`config.json`** and profile-local state. **Skills themselves** live in a **shared store at the `~/.chai` root** (e.g. **`skills/`**); **profiles do not nest duplicate skill trees**—they differ by **`skills.directory`**, **`skills.extraDirs`**, and (when implemented) **enablement** and **pins** defined by **[EPIC_SKILL_PACKAGES.md](EPIC_SKILL_PACKAGES.md)**. **Preferred posture:** **full isolation** for **identity, workspace, and channel state**; **one canonical skill store** with **per-profile composition** of what loads.

**Status** — **Draft.** No implementation commitment; complements **[EPIC_SIMULATIONS.md](EPIC_SIMULATIONS.md)** and **[`.testing/`](../.testing/)** playbooks.

## Problem Statement

Users need **different trust and capability boundaries** for different tasks:

- **Assistant mode** — Personal or work assistant with access to **sensitive files and context**, **local or self-hosted** models, and stable skills. Privacy-sensitive users configure this path deliberately.
- **Developer / maintenance mode** — **Skill generation**, **editing**, and **testing** (including scripts and tools) against **dummy or synthetic data**, with permission to use **more capable hosted models** (e.g. Claude Opus, Composer) **without** sending personal workspace content or credentials to those providers.

Today, a single **`~/.chai/config.json`** and default paths (**`skills.directory`**, workspace, pairing store) do not encode this split; users must manually swap configs or maintain separate machines. **Runtime profiles** make the boundary **first-class**, **auditable**, and **switchable** with a clear operational step (gateway restart).

## Goal

Multiple **named runtime profiles** under **`~/.chai/profiles/`** with **one active profile** per gateway process. Switching profiles means restarting the gateway into a different trust and capability environment. Skills live in a **shared store** at the `~/.chai` root; profiles differ by **which skills load**, **which model and provider** are configured, and **which workspace and channel state** are in scope. The design enables:

- **Named profiles** — At least **assistant** and **developer** (names user-defined); users may add **personal**, **work**, or other profiles as needed.
- **Deterministic activation** — One active profile per gateway process; switching = select profile + restart gateway (CLI and eventually desktop).
- **Isolation** — Separate per profile: config, workspace, pairing, channel stores, secrets (`.env`). Shared at `~/.chai` root: the skill store; which skills load and which revisions apply stay in each profile’s config (and lockfile, when [EPIC_SKILL_PACKAGES.md](EPIC_SKILL_PACKAGES.md) lands).
- **Developer profile as skill generation home** — The developer profile is the natural home for the skill generation workflow: discover CLI interface → design tool surface → generate `tools.json` → validate schema → simulate against target models → promote to assistant profile. Frontier models are used only in this profile for generation and testing — the assistant profile runs the produced artifacts on local/self-hosted models.
- **Capability-tier alignment** — Each skill declares a `capability_tier` in its `SKILL.md` frontmatter (`minimal`, `moderate`, `full`) indicating the minimum model capability it assumes. Profiles that specify a model or provider should be able to validate that enabled skills match the profile’s model capability. A `minimal`-tier skill (designed for 7B models) is safe in any profile; a `full`-tier skill enabled in a profile running a 7B model is a configuration error that can be surfaced at startup. This validation is informational in early phases (warn) and may become strict later. Related: skills may declare `model_variant_of` to link variant skills (e.g. `notesmd-daily` is a `minimal`-tier variant of `notesmd`); enabling both variants in the same profile creates tool overlap that should be warned about.
- **Pre-lock promotion** — Until the skill packages epic lands, promotion from developer to assistant is a manual operation: copy the validated `tools.json` (and any scripts) from the skill directory, update the assistant profile’s `config.json` if skill enablement changed, and restart the gateway. This is intentionally low-ceremony — the value of profiles in early phases is the data boundary (developer workspace never touches assistant workspace), not automated promotion. Document this path explicitly in Phase 2 so users aren’t left guessing.

## Scope

### In Scope

- Per-profile config, workspace, pairing, device identity, channel stores, and secrets
- Shared root-level skill store with per-profile composition (enablement, skill paths, `extraDirs`)
- CLI for profile management (`chai profile list | switch <name> | current`)
- Developer profile with restricted workspace and frontier provider use
- Desktop parity for profile switching

### Out of Scope

- **Hot reload** of full profile across an already-running gateway without restart (may revisit for subsets later).
- **OS-level sandboxing** (containers/VMs) — profiles are **runtime** and path isolation first; stronger isolation is a possible follow-on.
- **Skill revision format, lockfiles, and flake-style resolution** — Covered in [EPIC_SKILL_PACKAGES.md](EPIC_SKILL_PACKAGES.md). This epic only assumes profiles can point at a resolved skill set after restart.

## Design

### Relationship to Simulations and Model Testing

| Aspect | This epic (runtime profiles) | [EPIC_SIMULATIONS.md](EPIC_SIMULATIONS.md) / [`.testing/`](../.testing/) |
|--------|------------------------------|------------------------------------------------------------------------|
| **What it bounds** | **Who sees what data** and **which profile subtree** is live | **Repeatable scenarios**, fixtures, optional CI for gateway behavior |
| **Overlap** | A **developer profile** is the natural **home** for automated or scripted model/skill tests | Harness runs assume a **known config**—profiles supply that **without** contaminating the **assistant** profile |
| **Composition** | User switches to **developer** → runs playbooks or harness → promotes skill revisions for **assistant** (pins / locks; see **skill packages** epic) | Playbooks remain the **expectation** source; profiles define **safe** provider and path defaults |

**View:** Runtime profiles address **trust and layout**; simulations address **repeatability**. Both are needed for **safe iteration** on skills with frontier models.

### Design Axes (To Decide)

1. **Layout** — Per-profile subtrees under **`~/.chai/profiles/<name>/`** for **config + state + workspace** (see **Example layout**). **Skills** stay under **`~/.chai/`** root paths, not under **`profiles/<name>/`**.
2. **Shared vs. split** — **Default: split** for **pairing**, **device tokens**, **channel stores**, **workspace**, **profile `.env`**. **Default: shared** for **skill package trees** under **`~/.chai/skills/`** (and paths listed in **`skills.extraDirs`**). **Usually shared:** host **binaries** on **`PATH`**.
3. **Promotion** — Updating the **assistant** profile after testing in **developer** is a **pin / lock / enablement** change (see **[EPIC_SKILL_PACKAGES.md](EPIC_SKILL_PACKAGES.md)**), not a second on-disk skill tree under **`profiles/`**.
4. **CLI/UX** — `chai profile list | switch <name> | current`; gateway resolves **`CHAI_CONFIG_PATH`** via **active profile** (e.g. **`~/.chai/active`** → **`profiles/<name>`**), or **`CHAI_PROFILE`** + resolver. **`skills.*`** paths in config are typically **absolute or `~/.chai`-relative**, not relative to the profile dir—unless we standardize “profile-relative only for workspace/pairing/channels.”

### Decisions (PoC)

| Topic | Decision |
|-------|----------|
| **Pairing and device identity** | **Per profile** — **`paired.json`**, **device signing keys**, and **device token** material live **inside** each profile’s subtree. **Security over convenience:** switching profile switches **trust domain**, not only LLM config. |
| **Orchestration** | **Per profile** — Orchestrator settings (delegation, limits, whatever is config-driven) are read from **that profile’s** `config.json` (and profile-local state), not from a single global policy. |
| **Tool approval** | **Per profile** — Allowlists, approval modes, and related policy are **profile-scoped** so the **assistant** profile can stay strict while **developer** allows broader experimentation. See [EPIC_TOOL_APPROVAL.md](EPIC_TOOL_APPROVAL.md). |
| **Skill storage vs. profile** | **Root-level store** — Installed skills live under **`~/.chai/`** (e.g. **`skills/`**). Each profile’s **`config.json`** chooses **`skills.directory`**, **`skills.extraDirs`**, and (with the **skill packages** epic) **enablement** and **pins** / lockfile path. **No** per-profile **`profiles/<name>/skills/`** copy for the same package. |
| **Migration from single `~/.chai/config.json`** | **Out of scope for PoC** — No commitment to seamless upgrade paths; early adopters can **manually** adopt the profile layout or re-run **`chai init`**-style scaffolding per profile as needed. |

### Example `~/.chai` Layout (Illustrative)

The tree below shows **shared root-level skill storage** and **per-profile** config, workspace, pairing, and channel store layout. Exact filenames may evolve with implementation; the point is **skills are not nested under `profiles/<name>/`**.

```text
~/.chai/
├── profiles/
│   ├── assistant/
│   │   ├── matrix/
│   │   ├── workspace/
│   │   │   └── AGENTS.md            # assistant context — sensitive
│   │   ├── .env
│   │   ├── config.json              # assistant configuration — sensitive
│   │   ├── device.json
│   │   ├── device_token
│   │   └── paired.json
│   └── developer/
│       ├── matrix/
│       ├── workspace/
│       │   └── AGENTS.md            # developer context — not sensitive
│       ├── .env
│       ├── config.json              # developer configuration — not sensitive
│       ├── device.json
│       ├── device_token
│       └── paired.json
├── skills/
│   └── <skill-name>/
└── active -> profiles/assistant/    # symlink to active profile
```

**Reading the layout**

- **`active`** — Points at **`profiles/<name>`**; gateway loads **`config.json`** from there. **Workspace**, **pairing**, **device** material, **channel stores** resolve **under that profile subtree** (or as set in that config).
- **Skills** — **`~/.chai/skills/`** is the **default package store** for all profiles. **`profiles/assistant/config.json`** and **`profiles/developer/config.json`** differ by **which dirs** are on the skill path (**`directory`** + **`extraDirs`**), by **subset / enablement** (when implemented), and by **pins / lockfile** (when implemented)—not by duplicating **`skills/`** inside each profile.
- **Privacy** — Frontier-backed **developer** runs still **load skill definitions** from disk (shared store); they do **not** receive the **assistant** profile’s **workspace** or **channel history** if those paths stay profile-local. Skill **content** is not assumed to contain user secrets—**workspace** is where sensitive context lives.
- **Promotion** — After validating a skill revision in **developer**, update the **assistant** profile’s **pins or lock** (see **[EPIC_SKILL_PACKAGES.md](EPIC_SKILL_PACKAGES.md)**); revisions live **inside** each **`skills/<name>/`** tree, not in a separate drafts directory.

## Requirements

- [ ] **Profile layout** — Per-profile subtrees under `~/.chai/profiles/<name>/` for config, workspace, pairing, device identity, channel stores, and secrets (`.env`).
- [ ] **Shared skill store** — Skills live under `~/.chai/skills/`; profiles differ by `skills.directory`, `skills.extraDirs`, and enablement — not by duplicating skill trees per profile.
- [ ] **Active profile resolution** — Gateway resolves config from the active profile (e.g. `~/.chai/active` symlink → `profiles/<name>`), via `CHAI_PROFILE` env var, or CLI argument.
- [ ] **Profile switching** — CLI commands for profile management (`chai profile list | switch <name> | current`); switching requires gateway restart.
- [ ] **Per-profile pairing and device identity** — `paired.json`, device signing keys, and device token material live inside each profile's subtree.
- [ ] **Per-profile tool approval** — Allowlists, approval modes, and related policy are profile-scoped. See [EPIC_TOOL_APPROVAL.md](EPIC_TOOL_APPROVAL.md).
- [ ] **Developer profile template** — Scaffold developer profile with restricted workspace, optional dummy data location, and documentation for frontier provider use only in that profile.
- [ ] **Capability-tier validation** — Warn at startup when enabled skills have a `capability_tier` that exceeds the profile's configured model capability. Warn when variant skills (`model_variant_of`) are both enabled in the same profile.
- [ ] **Desktop parity** — Surface active profile and restart-to-switch in the desktop application.

## Phases (Tentative)

1. **Inventory** — Map every path and env var today (`CHAI_CONFIG_PATH`, `skills.directory`, workspace, `paired.json`, channel stores) to **profile-relative resolution** vs. **host-global**; document **privacy footguns** if anything still resolves outside the active profile subtree.
2. **Profile layout MVP** — **Per-profile** config + workspace + pairing + channels; **one** root **`skills/`**; profile configs differ by **skill paths and load rules**; **no** skill package lockfile yet—just **switch + restart**.
3. **Default + developer templates** — Scaffold **developer** profile with **restricted workspace**, optional **dummy data** location, and docs for **frontier** provider use **only** in that profile.
4. **Desktop** — Surface **active profile** and **restart to switch** (parity with CLI).

## Related Epics and Docs

- [EPIC_SKILL_PACKAGES.md](EPIC_SKILL_PACKAGES.md) — **Lockfiles**, **pins**, **rollback**, flake-style metaphor; **consumes** profile context for **which** lock is active.
- [EPIC_SIMULATIONS.md](EPIC_SIMULATIONS.md) — Harness and repeatable runs; complementary to profile-bound trust.
- [EPIC_TOOL_APPROVAL.md](EPIC_TOOL_APPROVAL.md) — Tool approval is **per profile** (see **Decisions**).
- [README.md](../README.md) — Current **`~/.chai`** layout and `CHAI_CONFIG_PATH`.
- [.testing/README.md](../.testing/README.md) — Model-comparison playbooks; natural fit for **developer** profile.
