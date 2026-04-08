---
status: complete
---

# Epic: Skill Packages (Revisions, Locks, and Derivation Metadata — Flake-Style)

**Summary** — Treat each directory under **`~/.chai/skills/<name>/`** as a **skill package**: a **content-addressed revision space** with immutable snapshot directories and an **active** pointer. A **lockfile** per profile records **exact content hashes** the gateway loads—**metaphorically** like **[Nix flakes](https://nixos.wiki/wiki/Flakes)** and **`flake.lock`**: **immutable inputs**, **pinned resolution**, **reproducible** restarts, **rollback** by pointing at a previous pin. **Switching pins** (and **restart**) parallels **activating** a new system configuration. This epic is **orthogonal** to **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)**: **runtime profiles** decide **which** config (and thus **which** lock / pin set) is live; this epic defines **how** skill packages expose **revisions**, **locks**, **derivation metadata**, and **rollback**.

**Status** — **Complete.** Phases 1–4 implemented. Phase 5 (CI/harness) is optional and deferred to the [SIMULATIONS.md](SIMULATIONS.md) epic.

## Problem Statement

Today, skills are **directories** on the skill path; there is **no first-class skill-package revision** or **reproducible pin**. The gateway loads whatever is on disk at **`~/.chai/skills/<name>/`** with no integrity verification, no version tracking, and no way to reproduce a previous state. Users who iterate with a **developer** profile and promote to the **assistant** profile need:

- **One** canonical path per skill (**`skills/foo/`**).
- **Explicit** “what ran” after a gateway restart: a **lock** or **pin set**, not whatever happened to be on disk last.
- **Rollback** without hunting arbitrary backups.
- **Integrity verification** — confidence that a skill's content has not been modified since it was pinned.

## Goal

Model each **`skills/<name>/`** tree as a **skill package** with reproducible resolution and rollback. A lockfile records exact revisions the gateway loads — the same skill set is guaranteed across restarts. The developer profile iterates on packages freely (dirty working tree); the assistant profile runs from pinned, validated revisions. Generations (complete lockfile snapshots) enable system-wide rollback.

## Scope

### In Scope

- **Content-addressed revisions** inside each skill package (`skills/<name>/versions/<hash>/`) with an `active` symlink
- **Per-profile lockfile** (`profiles/<name>/skills.lock`) mapping skill directory name → content hash
- Gateway startup resolution against lock, with configurable strictness (`warn` default, `strict` opt-in)
- Generation-level rollback — restoring the entire previous lockfile, not a single package’s revision
- Derivation metadata in `SKILL.md` frontmatter recording what produced each skill revision
- **Startup validation** of **`capability_tier`** and **`model_variant_of`** composition against the active profile’s effective model (warn first; see **Capability-Tier Validation (Startup)** under **Design**). This is independently deliverable ahead of the lockfile system.
- CLI commands (`chai skill lock`, `update`, `rollback`) for managing pins without hand-editing JSON

### Out of Scope

- **Bundling or fetching** skill packages from a global registry (like nixpkgs) unless explicitly scoped later.
- **Literal Nix or flakes integration** — Chai implements the flake metaphor natively; literal Nix integration is out of scope. The metaphor provides design vocabulary and UX guarantees without the adoption cost, platform constraints, or support burden of a Nix dependency.
- **Hot reload** of resolved package revisions without gateway restart (may revisit).
- **Per-version history metadata** — commit messages, changelogs, or parent-chain tracking within the version store. May revisit if the "why did this change?" question becomes a real friction point (see **Open Questions**).

## Inventory (Current State)

How **`lib`** loads skills today (Phase 1 findings):

### Skill Discovery and Loading

- **Entry point:** `load_skills(skills_root: &Path)` in `skills/loader.rs` — iterates immediate subdirectories of the given root
- **Per directory:** checks for `SKILL.md` → parses frontmatter → checks PATH for required binaries → loads `tools.json` → pushes `SkillEntry`
- **Data structures:** `SkillEntry { name, description, path, content, tool_descriptor }` → filtered per agent by `skillsEnabled` → `Skill { name, description, content }` for context injection
- **Frontmatter parsed:** Only `name`, `description`, `metadata.requires.bins`. No `deny_unknown_fields` — extra keys (`capability_tier`, `model_variant_of`, `generated_from`) are **silently ignored**
- **Failure handling:** Missing binary → skill silently skipped (`log::debug`). Bad `tools.json` → `log::warn`, skill has no tools. No skill errors cause startup failure

### Skill Sources

- **Single source:** `config::default_skills_dir(chai_home)` returns `<chai_home>/skills` — always `~/.chai/skills/`
- **No multi-root, no overlay, no config.json override** for the gateway. `CHAI_SKILLS_ROOT` env exists CLI-side only
- **Bundled skills** compiled in via `include_dir!` → extracted to `~/.chai/skills/` at `chai init` (skipped if directory exists)

### Profile and Agent Integration

- Skills directory is **shared across all profiles** — `~/.chai/skills/` is not profile-scoped
- Active profile determines `config.json`, which contains `agents` array with per-agent `skillsEnabled` and `contextMode`
- Each agent (orchestrator + workers) gets an independently filtered `Vec<SkillEntry>` from the shared pool
- `defaultModel` and `defaultProvider` are per-agent fields in config — available for capability-tier validation

### Existing CLI Surface

`chai skill` subcommands: `discover`, `list`, `read`, `init`, `write-skill-md`, `write-tools-json`, `write-script`, `validate`. No lock/update/rollback commands exist. No `skills.lock` or revision concept exists anywhere in the codebase.

### Where Revision Resolution Would Hook In

1. **`load_skills`** — currently reads skill files directly from `skills/<name>/`. With content-addressed versions, it would resolve `skills/<name>/active` symlink first, then read from the target `versions/<hash>/` directory
2. **Gateway startup** (`run_gateway` in `server.rs`) — after `load_skills` and config resolution, before building per-agent skill runtimes. This is where lockfile verification and capability-tier validation would run
3. **`chai init`** — bundled skill extraction would create the initial `versions/<hash>/` snapshot and `active` symlink instead of writing files directly into `skills/<name>/`

## Design

### Flake Metaphor (Mapping)

| Flake / Nix idea | Chai skill analogue |
|------------------|---------------------|
| **Flake** | A **skill package** at **`skills/<name>/`** with a **content-addressed version space** |
| **`flake.lock`** | **Per-profile `skills.lock`** fixing each skill to a **content hash** |
| **Nix store path** | **`versions/<hash>/`** — directory name is the integrity check |
| **Pinned input** | **Immutable snapshot** in `versions/<hash>/`; `active` symlink selects one |
| **New generation** | **Update lock** + **restart gateway** |
| **Rollback** | **Previous lock** still addressable; all version snapshots remain on disk |

### Version Storage (Content-Addressed Snapshots)

Each skill package stores immutable versioned snapshots under `versions/`, identified by a truncated content hash. An `active` symlink selects the current version:

```
skills/<name>/
  active -> versions/a1b2c3d/
  versions/
    a1b2c3d/
      SKILL.md
      tools.json
      scripts/
    f8e9d0b/
      SKILL.md
      tools.json
      scripts/
```

**Content hash computation** — deterministic hash (SHA-256, truncated) of the canonical skill content. Canonical form: sorted relative file paths, each entry as `<relative-path>\0<file-contents>`, concatenated and hashed. Line endings and trailing newlines are hashed as-is (no normalization) — the hash reflects the exact bytes. Script permissions are not included in the hash (they are a deployment concern, not a content concern).

**Why content-addressed:**
- **Self-verifying** — the directory name *is* the integrity check. If the content matches the hash, it is the correct version. No separate verification step.
- **Immutable by construction** — versions are never modified, only created. Eliminates the dirty working tree problem entirely.
- **No external dependency** — no git required on the system. Aligns with Chai’s minimal-dependency principle.
- **Atomic rollback** — changing the `active` symlink is a single filesystem operation.
- **Natural lock schema** — skill name → hash. The hash is both the address and the verification.

**Tradeoffs accepted:**
- Full copies per version (no delta compression). Skills are small (5–50KB each); disk cost is negligible in practice.
- No built-in history or "why" metadata. See **Open Questions** for potential future additions.

### Lockfile

Per-profile lockfile at **`profiles/<name>/skills.lock`** (JSON):

```json
{
  "version": 1,
  "skills": {
    "git": { "hash": "a1b2c3d" },
    "devtools": { "hash": "f8e9d0b" },
    "kb": { "hash": "9c8d7e6" }
  },
  "generation": 3
}
```

- **Skill identity** — keyed by **directory name** (not frontmatter `name`). Directory name is authoritative; frontmatter `name` is display/documentation only.
- **Generation** — monotonic integer incremented on each lock update. The lockfile itself is the generation; the integer provides ordering.
- **Strictness** — configurable per profile via `skillLockMode` in `config.json`:
  - **`"strict"`** (default) — refuse to start the gateway when any enabled skill’s active version does not match its locked hash. Appropriate for assistant profiles.
  - **`"warn"`** — log a warning when the `active` symlink target does not match the locked hash, but continue loading. Appropriate for developer profiles.

### Derivation Metadata

A skill package is not just a versioned artifact — it is a **derived** artifact. In the NixOS model, a **derivation** is a pure function: given the same inputs, it produces the same output. Chai’s skill generation has the same structure. A package is "derived" from:

- **CLI help output** — The source interface (e.g. `notesmd-cli --help`, subcommand help)
- **Skill format spec** — The `SKILL_FORMAT.md` rules governing directory layout, frontmatter, context modes
- **Tools schema spec** — The `TOOLS_SCHEMA.md` rules governing `tools.json` structure: tools array, allowlist, execution mapping, arg kinds, `resolveCommand`
- **Target model capability tier** — The `capability_tier` (`minimal`, `moderate`, `full`) determines schema complexity, tool count, and how much judgment the skill assumes from the model

**Formalizing derivation inputs** means each skill’s **`SKILL.md`** frontmatter records **what produced it**:

```yaml
# SKILL.md derivation metadata (already present on all 19 bundled skills)
generated_from:
  cli: notesmd-cli
  cli_version: "0.3.0"
  spec_version: "1.0"
  generator_model: claude-opus
  capability_tier: minimal
```

This connects **package revisions** to **reproducibility**: the lockfile records **what revision ran**; derivation metadata records **what produced that revision**. Together, they answer both "what changed?" and "can we rebuild it?" — the same guarantee NixOS derivations provide.

**Current state:** All 19 bundled skills already have `generated_from` blocks in SKILL.md frontmatter. The runtime does not parse these fields yet — `SkillFrontmatter` in `loader.rs` ignores unknown keys. Extending the struct to parse `capability_tier`, `model_variant_of`, and `generated_from` is the first implementation step (see **Capability-Tier Validation**).

**Model-specific variants** follow naturally: the same CLI source, cross-compiled for different capability tiers, produces different build outputs. `notesmd-daily` (minimal tier, 2 tools, focused scope) and `notesmd` (full tier, all CRUD operations, more judgment) are **variants of the same derivation** with different **`capability_tier`** targets. The **`model_variant_of`** field in `SKILL.md` frontmatter makes this relationship explicit.

**Context budget implications**: The capability tier also informs how much **context** a skill contributes to a session. A `minimal`-tier skill should default to **`readOnDemand`** context mode (load `SKILL.md` only when invoked) to preserve the limited context window of small models. A `full`-tier skill can use **`full`** context mode because the target model has the context budget for persistent instructions.

### Capability-Tier Validation (Startup)

**Independently deliverable** — this requires only extending `SkillFrontmatter` to parse `capability_tier` and `model_variant_of`, then adding validation logic after skill loading and config resolution. No dependency on the lockfile, versioning, or generation system.

The gateway validates skill composition at startup using the active profile’s per-agent config:

- **Tier vs model** — Warn when an **enabled** skill’s **`capability_tier`** assumes more capability than the agent’s effective model is likely to provide (e.g. **`full`** skill with a 7B local model). Exact mapping (catalog, heuristic, or operator override) is TBD; start with **informational** warnings and optional **strict** mode later.
- **Variant overlap** — Warn when **`model_variant_of`** links two skills that are **both** enabled for the same agent, creating redundant or overlapping tool surfaces (e.g. **`git`** and **`git-read`** both enabled).

**Why this epic:** Validation concerns **skill package declarations** and how they **compose** with **config**, not **profile layout**. Track it here alongside **derivation metadata**, **locks**, and **promotion**. **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** only determines **which** **`config.json`** is active—not the validation rules.

### Relationship to Runtime Profiles

| Concern | [RUNTIME_PROFILES.md](RUNTIME_PROFILES.md) | This epic (skill packages) |
|---------|-------------------|----------------------------|
| **Trust boundary** | **Workspace**, **pairing**, **channels**, **secrets** | **Skill source** revision only |
| **What switches on profile change** | **Active profile** → that profile’s **`config.json`** | That profile’s **skill pins** / **lockfile path** |
| **Shared disk** | **`~/.chai/skills/`** for all profiles | **Versions** live **inside** each **`skills/<name>/versions/`**; locks **differ per profile** |

**View:** **Two epics, one story**—profiles **isolate** user data and policy; **locks** **isolate** **which skill bits** ran.

### Promotion

**Developer → assistant promotion:** The developer profile iterates freely (creating new version snapshots as skills change). To promote a skill to the assistant profile:

1. `chai skill lock <name>` in the assistant profile — pins that skill to its current `active` hash
2. Or: copy the hash entry from the developer profile’s `skills.lock` into the assistant profile’s `skills.lock`

Both profiles reference the same `versions/<hash>/` directory — no file copying between profiles. The lock is the only thing that differs.

## Requirements

- [x] **Capability-tier validation** — Extend `SkillFrontmatter` in `loader.rs` to parse `capability_tier` and `model_variant_of`. At gateway startup, for each agent’s `skillsEnabled`: warn when an enabled skill’s `capability_tier` is a poor match for the agent’s `defaultModel` (informational first; optional strict mode later); warn when two enabled skills linked by `model_variant_of` overlap in tool surface. **Independently deliverable** — no dependency on lockfile or versioning.
- [x] **Content-addressed versions** — Each `skills/<name>/` contains `versions/<hash>/` snapshot directories and an `active` symlink. Hash is a truncated SHA-256 of canonical skill content (sorted paths + file bytes). `load_skills` resolves `active` before reading skill files.
- [x] **Lockfile** — Per-profile `skills.lock` at `profiles/<name>/skills.lock` mapping skill directory name → content hash + generation counter. `skillLockMode` config field (`"strict"` default, `"warn"` opt-in) controls startup behavior.
- [x] **Gateway resolution** — On startup, verify each enabled skill’s `active` symlink target matches the locked hash. Warn or refuse per `skillLockMode`. Unlocked skills load normally (no lock entry required).
- [x] **Generation tracking** — Each lockfile update increments the generation counter. Previous lockfiles are preserved as `skills.lock.<generation>` (or equivalent) to make each generation addressable.
- [x] **Rollback** — Restore a previous generation’s lockfile (generation-level, not per-package). `active` symlinks are updated to match the restored lock entries.
- [x] **Derivation metadata parsing** — Extend `SkillFrontmatter` to parse `generated_from` block. Surface in `chai skill list` output and lock metadata. (The frontmatter fields already exist on all 19 bundled skills.)
- [x] **CLI** — Extend `chai skill` with `lock`, `rollback`, `generations` subcommands. `lock` pins current `active` hashes to `skills.lock`. `rollback` restores a previous generation and updates `active` symlinks. `generations` lists available lockfile generations.
- [x] **Init migration** — `chai init` creates `versions/<hash>/` + `active` symlink per bundled skill instead of writing files directly into `skills/<name>/`.

## Phases

1. **Capability-tier validation** — ✅ Parse `capability_tier` and `model_variant_of` in `SkillFrontmatter`. Startup warnings for tier/model mismatch and variant overlap. Module: `skills/validation.rs`.
2. **Version storage** — ✅ Content-addressed snapshot directories (`versions/<hash>/`) and `active` symlink. Loader resolves `active` (backward compatible with flat layout). `chai init` creates versioned layout. CLI write commands create new snapshots. Module: `skills/versioning.rs`.
3. **Lockfile and resolution** — ✅ Per-profile `skills.lock` schema, gateway verification against lock at startup, `skillLockMode` config field (`strict`/`warn`), generation counter with backup files. Module: `skills/lockfile.rs`.
4. **Rollback and CLI** — ✅ `chai skill lock`, `rollback <generation>`, `generations` commands. Generation backup files (`skills.lock.<N>`). Rollback updates `active` symlinks to match restored lock.
5. **CI / harness** — Deferred. Simulations or tests record lockfile for repro (see [SIMULATIONS.md](SIMULATIONS.md)).

## Resolved Questions

- **Lock scope** — **Per-profile** at `profiles/<name>/skills.lock`. Different profiles legitimately pin different revisions (developer iterates, assistant pins stable). Aligns with existing layout where `config.json` is already per-profile.
- **Skill identity** — **Directory name is authoritative.** Current code falls back to directory name when frontmatter `name` is absent (`loader.rs`). Lockfile keys on directory name. Frontmatter `name` is display/documentation only.
- **Strictness** — **`skillLockMode` in `config.json`**: `"strict"` (default) refuses startup; `"warn"` logs mismatches and continues. Developer profile can opt into warn; assistant profile should remain strict.
- **Version storage model** — **Content-addressed snapshot directories.** Immutable, self-verifying, no git dependency. See **Version Storage** under Design for rationale and tradeoffs.

## Open Questions

- **Hash truncation length** — 7 characters (like short git hashes) may be sufficient for collision avoidance within a single skill’s version space. 12+ would be safer. TBD based on practical aesthetics and collision probability.
- **Garbage collection** — Old version snapshots accumulate. Need a pruning policy or `chai skill gc` command. Candidates: keep N most recent, keep all locked versions across profiles, prune unlocked versions older than N days.
- **Per-version history metadata** — The current design has no "why did this change?" record. A `history.json` per skill (ordered entries: `{hash, timestamp, note, parent_hash}`) could be added later if the need emerges. Currently out of scope.
- **Canonical hash stability** — The hash computation (sorted paths + raw bytes) is simple but means any whitespace change produces a new version. Acceptable for now; normalization could be reconsidered if it causes friction.

## Implementation Order (with Related Epics)

Both prerequisite epics are **complete**:

1. **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** — **Complete.** Active profile and `profileRoot`; lockfile path is profile-scoped.
2. **[AGENT_ISOLATION.md](AGENT_ISOLATION.md)** — **Complete.** Per-agent `skillsEnabled` and skill configuration in config; shared `~/.chai/skills/` with composition per agent.
3. **This epic** — **Unblocked.** Immutable pins and `skills.lock` on top of the shared store; rollback and CLI without re-negotiating profile or agent layout.

## Related Epics and Docs

- [RUNTIME_PROFILES.md](RUNTIME_PROFILES.md) — **Active profile** selects **which** lock/pin set applies after restart; implement **before** this epic (see **Implementation order** above).
- [AGENT_ISOLATION.md](AGENT_ISOLATION.md) — Per-agent skill enablement; implement **before** this epic so locks pin revisions for skills **actually loaded** per agent.
- [SIMULATIONS.md](SIMULATIONS.md) — Repeatable runs may **fix** a lockfile for **determinism**.
- [README.md](../../README.md) — Current **skills** layout under **`~/.chai/skills`**.
