---
status: draft
---

# Epic: Skill Packages (Revisions, Locks, and Derivation Metadata — Flake-Style)

**Summary** — Treat each directory under **`~/.chai/skills/<name>/`** as a **skill package**: a **revision space** inside that tree (e.g. **git** history, tags, or an internal **`versions/`** layout with an **active** pointer). A **lockfile** (or embedded pins in profile **`config.json`**) records **exact revisions** the gateway loads—**metaphorically** like **[Nix flakes](https://nixos.wiki/wiki/Flakes)** and **`flake.lock`**: **immutable inputs**, **pinned resolution**, **reproducible** restarts, **rollback** by pointing at a previous pin. **Switching pins** (and **restart**) parallels **activating** a new system configuration. This epic is **orthogonal** to **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)**: **runtime profiles** decide **which** config (and thus **which** lock / pin set) is live; this epic defines **how** skill packages expose **revisions**, **locks**, **derivation metadata**, and **rollback**.

**Status** — **Draft.** No implementation commitment.

## Problem Statement

Today, skills are **directories** on the skill path; there is **no first-class skill-package revision** or **reproducible pin**. Users who iterate with a **developer** profile and promote to the **assistant** profile need:

- **One** canonical path per skill (**`skills/foo/`**).
- **Explicit** “what ran” after a gateway restart: a **lock** or **pin set**, not whatever happened to be on disk last.
- **Rollback** without hunting arbitrary backups.

## Goal

Model each **`skills/<name>/`** tree as a **skill package** with reproducible resolution and rollback. A lockfile records exact revisions the gateway loads — the same skill set is guaranteed across restarts. The developer profile iterates on packages freely (dirty working tree); the assistant profile runs from pinned, validated revisions. Generations (complete lockfile snapshots) enable system-wide rollback.

## Scope

### In Scope

- **Revisions** inside each skill package (`skills/<name>/`): git rev/tag or `versions/<id>/` snapshots
- Lockfile or equivalent (`skills.lock`) mapping skill id → resolved revision (+ optional content hash)
- Gateway startup resolution against pins/lock, with configurable strictness (refuse or warn on dirty tree)
- Generation-level rollback — restoring the entire previous lockfile, not a single package’s revision
- Derivation metadata in `SKILL.md` frontmatter recording what produced each skill revision
- **Startup validation** of **`capability_tier`** and **`model_variant_of`** composition against the active profile’s effective model (warn first; see **Capability-Tier Validation (Startup)** under **Design**)
- CLI commands (`chai skills lock`, `update`, `rollback`) for managing pins without hand-editing JSON

### Out of Scope

- **Bundling or fetching** skill packages from a global registry (like nixpkgs) unless explicitly scoped later.
- **Literal Nix or flakes integration** — Chai implements the flake metaphor natively; literal Nix integration is out of scope. The metaphor provides design vocabulary and UX guarantees without the adoption cost, platform constraints, or support burden of a Nix dependency.
- **Hot reload** of resolved package revisions without gateway restart (may revisit).

## Design

### Flake Metaphor (Mapping)

| Flake / Nix idea | Chai skill analogue |
|------------------|---------------------|
| **Flake** | A **skill package** at **`skills/<name>/`** with a defined **revision space** |
| **`flake.lock`** | **`skills.lock`** (or pins in **`config.json`**) fixing **each** skill to a **rev** / **hash** |
| **Pinned input** | **Git** commit, tag, or **frozen `versions/<id>`** snapshot |
| **New generation** | **Update lock** + **restart gateway** |
| **Rollback** | **Previous lock** or **previous tag** still addressable |

### Derivation Metadata

A skill package is not just a versioned artifact — it is a **derived** artifact. In the NixOS model, a **derivation** is a pure function: given the same inputs, it produces the same output. Chai's skill generation has the same structure. A package is "derived" from:

- **CLI help output** — The source interface (e.g. `notesmd-cli --help`, subcommand help)
- **Skill format spec** — The `SKILL_FORMAT.md` rules governing directory layout, frontmatter, context modes
- **Tools schema spec** — The `TOOLS_SCHEMA.md` rules governing `tools.json` structure: tools array, allowlist, execution mapping, arg kinds, `resolveCommand`
- **Target model capability tier** — The `capability_tier` (`minimal`, `moderate`, `full`) determines schema complexity, tool count, and how much judgment the skill assumes from the model

**Formalizing derivation inputs** means each skill's **`SKILL.md`** frontmatter (or a sibling metadata file) records **what produced it**:

```yaml
# SKILL.md derivation metadata (proposed)
generated_from:
  cli: notesmd-cli
  cli_version: "0.3.0"          # or commit hash
  spec_version: "1.0"           # SKILL_FORMAT + TOOLS_SCHEMA version
  generator_model: claude-opus   # model used for generation (if applicable)
  capability_tier: minimal
```

This connects **package revisions** to **reproducibility**: the lockfile records **what revision ran**; derivation metadata records **what produced that revision**. Together, they answer both "what changed?" and "can we rebuild it?" — the same guarantee NixOS derivations provide.

**Model-specific variants** follow naturally: the same CLI source, cross-compiled for different capability tiers, produces different build outputs. `notesmd-daily` (minimal tier, 2 tools, focused scope) and `notesmd` (full tier, all CRUD operations, more judgment) are **variants of the same derivation** with different **`capability_tier`** targets. The **`model_variant_of`** field in `SKILL.md` frontmatter makes this relationship explicit.

**Context budget implications**: The capability tier also informs how much **context** a skill contributes to a session. A `minimal`-tier skill should default to **`readOnDemand`** context mode (load `SKILL.md` only when invoked) to preserve the limited context window of small models. A `full`-tier skill can use **`full`** context mode because the target model has the context budget for persistent instructions.

### Capability-Tier Validation (Startup)

Once **`capability_tier`** and optional **`model_variant_of`** are **stable in package metadata** and the **active profile** supplies an **effective default model** (and provider), the gateway can **validate composition** at startup:

- **Tier vs model** — Warn (initially) when an **enabled** skill’s **`capability_tier`** **assumes** more capability than the profile’s effective model is likely to provide (e.g. **`full`** skill with a small local model). Exact mapping (catalog, heuristic, or operator override) is TBD; start with **informational** warnings and optional **strict** mode later.
- **Variant overlap** — Warn when **`model_variant_of`** links two skills that are **both** enabled in the same profile, creating redundant or overlapping tool surfaces (e.g. **`notesmd`** and **`notesmd-daily`**).

**Why this epic:** Validation concerns **skill package declarations** and how they **compose** with **config**, not **profile layout**. Track it here alongside **derivation metadata**, **locks**, and **promotion**. **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** only determines **which** **`config.json`** is active—not the validation rules.

### Relationship to Runtime Profiles

| Concern | [RUNTIME_PROFILES.md](RUNTIME_PROFILES.md) | This epic (skill packages) |
|---------|-------------------|----------------------------|
| **Trust boundary** | **Workspace**, **pairing**, **channels**, **secrets** | **Skill source** revision only |
| **What switches on profile change** | **Active profile** → that profile’s **`config.json`** | That profile’s **skill pins** / **lockfile path** |
| **Shared disk** | **`~/.chai/skills/`** for all profiles | **Revisions** live **inside** each **`skills/<name>/`**; locks **differ per profile** |

**View:** **Two epics, one story**—profiles **isolate** user data and policy; **locks** **isolate** **which skill bits** ran.

### Design Options (Tentative)

1. **Git-native** — Each **`skills/<name>/`** package is a **git** repo (or submodule); lock stores **`rev`** (and **branch** only as **input** to `update`, not for **assistant**-profile stability). **Rollback** = pin to **tag** or **commit**.
2. **Opaque snapshots** — **`versions/20250329-abc/`** trees + **`active`** file; lock stores **snapshot id**. Heavier on disk; no git dependency.
3. **Hybrid** — **Git** for authors who have it; **fallback** **versioned subdirs** or **tar** snapshots for simple packages.

**Promotion** from **developer** to **assistant**: **merge** or **tag** inside **`skills/<name>/`**, then **copy or merge** the **developer** profile’s lock entry for that skill into the **assistant** profile’s lock (exact UX TBD).

## Requirements

- [ ] **Package revisions** — Revision identity inside `skills/<name>/` (git rev/tag or `versions/<id>/` + active pointer). Working tree can be dirty during dev; locks record clean pins for the assistant profile.
- [ ] **Lockfile** — Machine-readable `skills.lock` (next to profile `config.json` or at `~/.chai/` referenced by config) listing skill id → resolved revision (+ optional content hash). Developer and assistant profiles may differ only in this lock.
- [ ] **Gateway resolution** — On startup, resolve skill packages according to pins/lock. Configurable strictness: refuse or warn on dirty tree when strict pin mode is set.
- [ ] **Generation tracking** — Each lockfile state is a generation. Lockfile history (git commits or internal `generations/` log) makes each generation addressable.
- [ ] **Rollback** — Restore the entire previous lockfile (generation-level, not per-package). Developer profile simulation testing can validate a new generation before it becomes the assistant profile's active state.
- [ ] **Derivation metadata** — `SKILL.md` frontmatter records what produced each skill revision: CLI source, CLI version, spec version, generator model, capability tier.
- [ ] **Capability-tier validation** — At gateway startup, using the **active profile**’s effective default model/provider and each agent’s **`skillsEnabled`**: warn when an enabled skill’s **`capability_tier`** is a poor match for that model (informational first; optional strict mode later); warn when two enabled skills linked by **`model_variant_of`** overlap in tool surface. **Depends on** stable frontmatter fields and profile **`agents`** defaults (**[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** provides **which** profile is live, not this epic’s validation logic).
- [ ] **CLI** — `chai skills lock`, `update`, `rollback` commands to manage pins without hand-editing JSON. `rollback` operates on generations (whole lockfile).

## Phases (Tentative)

1. **Inventory** — How **`lib`** loads skills today (**`~/.chai/skills`**, discovery of **`SKILL.md`** / **`tools.json`**); where a **rev** would be **read** and **validated**; how **`capability_tier`** / **`model_variant_of`** would feed **startup validation** (this epic).
2. **Pin model** — Minimal schema: **skill name** → **rev** (+ optional **hash**); **profile-local** lockfile path in **`config.json`**.
3. **Resolver MVP** — Given lock + **`skills/<name>/`**, resolve **working tree** to **pinned** content (git **checkout** or **read snapshot path**); integrate with **gateway** startup.
4. **UX** — Document **dirty** vs **pinned** behavior; optional CLI to **bump** and **rollback** locks.
5. **CI / harness** — Optional: simulations or tests **record** lockfile for **repro** (see [SIMULATIONS.md](SIMULATIONS.md)).

## Open Questions

- **Strictness** — Fail startup if **pin** does not match **working tree**, vs **warn** only in **developer**.
- **Lock scope** — One **global** lock under **`~/.chai/`** vs **one lock per profile** under **`profiles/<name>/`** (latter aligns with **per-profile** composition).
- **Skill identity** — **Directory name** vs **manifest `name`** in **`SKILL.md`** when they **differ**.

## Implementation order (with related epics)

When implementing **runtime profiles**, **agent isolation**, and **skill packages** together, use this sequence:

1. **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** — **First.** Active profile and **`profileRoot`**; lockfile path is profile-scoped (see **Decisions** and **Example layout** in that epic).
2. **[AGENT_ISOLATION.md](AGENT_ISOLATION.md)** — **Second.** Per-agent **`skillsEnabled`** and skill configuration in config; shared **`~/.chai/skills/`** with composition per agent.
3. **This epic** — **Third.** Immutable pins and **`skills.lock`** (or equivalent) on top of the shared store; **rollback** and CLI **without** re-negotiating profile or agent layout.

**Note:** Implementing this epic **before** profiles forces awkward lockfile placement; implementing **before** agent isolation complicates which skill names are in scope per agent. See **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** for the full ordering rationale.

## Related Epics and Docs

- [RUNTIME_PROFILES.md](RUNTIME_PROFILES.md) — **Active profile** selects **which** lock/pin set applies after restart; implement **before** this epic (see **Implementation order** above).
- [AGENT_ISOLATION.md](AGENT_ISOLATION.md) — Per-agent skill enablement; implement **before** this epic so locks pin revisions for skills **actually loaded** per agent.
- [SIMULATIONS.md](SIMULATIONS.md) — Repeatable runs may **fix** a lockfile for **determinism**.
- [README.md](../../README.md) — Current **skills** layout under **`~/.chai/skills`**.
