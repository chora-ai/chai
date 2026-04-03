---
status: complete
---

# Epic: Runtime Profiles and Isolated Environments

**Summary** — Chai uses a **NixOS-like switching model**: multiple **named runtime profiles** under **`~/.chai/profiles/`**, with **one active profile** at a time. The **persistent** choice is **`~/.chai/active`** → **`profiles/<name>/`**, overridable per process by **`CHAI_PROFILE`** and **`chai gateway --profile`** (ephemeral). **Changing** the persistent profile requires **stopping the gateway**, **`chai profile switch`**, and **restart** — **no** switch while the gateway is running (CLI error; desktop disables the profile control). **Switching the live stack** means **restarting the gateway** (and optionally desktop attach) so models, **per-agent skill configuration** (**`skillsEnabled`**, **`contextMode`** in that profile’s **`config.json`**; see **[AGENT_ISOLATION.md](AGENT_ISOLATION.md)**), delegation policy, channel bindings, and **on-disk agent context** (**`agents/<id>/`**) come from that profile. **Skill packages** live in a **shared store** at **`~/.chai/skills/`**; profiles do **not** duplicate package trees—they differ by **per-agent** enablement lists and (when **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)** lands) **pins / lockfile**. **Preferred posture:** **full isolation** for **identity, pairing, device, channel stores**, and **profile-local files**; **one canonical skill store** with **per-profile, per-agent composition** of what loads. Default profile names (**`assistant`**, **`developer`**) are **labels** for two **equivalent** scaffolds; **motivations** for using a second profile appear in **Problem statement** and **Goal** but are **not** enforced by different defaults or developer-only runtime features.

**Status** — **Complete.** Layout, **`chai profile`** CLI, **`gateway.lock`** with **advisory exclusive lock** (**`fs2`**, portable **`flock` / `LockFileEx`** semantics), shared **`skills/`**, **[AGENT_ISOLATION.md](AGENT_ISOLATION.md)** paths, and **desktop** header profile switcher (same lock rule as CLI) are **shipped**. **Historical** references in **`.journey/`**, **`.agents/ref/CLAW_ECOSYSTEM.md`**, **[DESKTOP_APP.md](DESKTOP_APP.md)**, and **[IMPLEMENTATION.md](../poc/IMPLEMENTATION.md)** / **[CHANGELOG.md](../poc/CHANGELOG.md)** under **`.agents`** were updated to the profile layout (or labeled historical). Optional **persistent vs effective** profile hint in the desktop UI remains in **[DESKTOP_APP.md](DESKTOP_APP.md)**.

## Shipped Model

The shipped layout **replaces** the flat **`~/.chai/config.json`** model; there is **no** backwards compatibility. **Removed:** **`CHAI_CONFIG_PATH`** and flat config resolution. **Wired:** all durable runtime state (except the shared skill store) under **profile roots** — **no** shims, **no** migration mode, and **no** “deprecated” callouts in code or user-facing docs.

## Problem Statement

**Motivation:** operators often want **different trust and capability boundaries** for different tasks. These are **reasons people reach for multiple profiles**, not requirements the product imposes on any named profile:

- **Assistant-style use** — Personal or work assistant with access to **sensitive files and context**, **local or self-hosted** models, and stable skills. Privacy-sensitive users may want this **isolated** from experimental configuration.
- **Skill work and experimentation** — **Skill generation**, **editing**, and **testing** (including scripts and tools), sometimes against **dummy or synthetic data**, and sometimes with **more capable hosted models** (e.g. Claude Opus, Composer) **without** mixing that with **personal** context or credentials kept in another profile.

Before profiles, a single flat config and undifferentiated paths did not encode this split; users had to manually swap configs or maintain separate machines. **Runtime profiles** make **path isolation** **first-class**, **auditable**, and **switchable** with a clear operational step (gateway restart). **What** each profile is *for* stays **user-defined** (config, **`AGENTS.md`**, enabled skills)—the runtime does **not** ship stricter rules for one default name than the other.

## Goal

Multiple **named runtime profiles** under **`~/.chai/profiles/`** with **one active profile** per gateway process. Switching profiles means restarting the gateway so **a different profile subtree** (config, agent dirs, pairing, channels) is live. Skill **packages** live in a **shared store** at **`~/.chai/skills`**; profiles differ by per-agent **skillsEnabled** and **contextMode** (see **[AGENT_ISOLATION.md](AGENT_ISOLATION.md)**), **which model and provider** are configured, **delegation policy**, and **which profile-local paths** (**`agents/<id>/`**, pairing, Matrix store) are in scope. The design enables:

- **Named profiles** — **Initialization** creates default profiles **`assistant`** and **`developer`** with the **same** scaffold (empty-equivalent defaults); users may **rename them**, add more (`personal`, `work`, …), or adjust layout after init. The **`~/.chai/active`** symlink initially points at **`profiles/assistant/`**. The names are **mnemonics** for the motivations above, **not** different built-in policies.
- **Deterministic activation** — One active profile per gateway process. **Changing the persistent active profile** requires **stopping the gateway**, running **`chai profile switch`**, then restarting. **No switching** (CLI or desktop) **while the gateway is running** — attempts fail with a **clear error**; desktop **disables** profile switching when the gateway is up.
- **Isolation** — Separate per profile: **config**, **`agents/<id>/`** (on-disk agent context), pairing, channel stores, secrets (`.env`). Shared at **`~/.chai` root**: the skill **package** store; **which packages each agent uses** and **how** skill text is presented are set in that profile’s **`config.json`** (and lockfile, when [SKILL_PACKAGES.md](SKILL_PACKAGES.md) lands).
- **Illustrative second-profile workflow (motivation, not enforcement)** — Many teams will use **one profile** for day-to-day assistant use (local models, sensitive **`AGENTS.md`**) and **another** for skill iteration: discover CLI → design tool surface → generate **`tools.json`** → validate → run simulations → later adjust pins or enablement on the first profile. **Where** frontier vs local models run is **configuration and discipline**, not a rule tied to the string **`developer`**. A **modular** way to capture that workflow is **skills** (e.g. a skill-authoring package—name illustrative) enabled only on the profile where you want that guidance, instead of developer-specific runtime options or templates.
- **Promotion across profiles (motivation)** — When skill packages and lockfiles land, updating **one** profile after validating work in **another** is expected to be a **pin / lock / enablement** change (see **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)**), not a second skill tree under **`profiles/`**. Until then, operators may still **manually** copy artifacts and edit **`config.json`**—the **value** is **data isolation** between profiles, not a commitment to automate promotion in this epic. User-facing docs may describe the pattern; it is **not** a separate product requirement for a special **developer** template.

## Scope

### In Scope

- Per-profile **config**, **`agents/<id>/`**, pairing, device identity, channel stores, and secrets
- Shared root-level skill store at **`~/.chai/skills`** and **per-agent** skill configuration in each profile’s **`config.json`** (**[AGENT_ISOLATION.md](AGENT_ISOLATION.md)**)
- CLI for profile management (`chai profile list | switch <name> | current`)
- **Second default profile** — **`chai init`** creates **`developer`** alongside **`assistant`** with **parity** (same scaffold and defaults). **No** distinct developer template, synthetic-context bundles, frontier-only config, or runtime rules keyed on profile name.
- Desktop **core** profile switching (active name + switch when gateway stopped) — **shipped**; optional **persistent vs effective** hint in **[DESKTOP_APP.md](DESKTOP_APP.md)**

### Out of Scope

- **Hot reload** of full profile across an already-running gateway without restart (may revisit for subsets later).
- **OS-level sandboxing** (containers/VMs) — profiles are **runtime** and path isolation first; stronger isolation is a possible follow-on.
- **Skill revision format, lockfiles, and flake-style resolution** — Covered in [SKILL_PACKAGES.md](SKILL_PACKAGES.md). This epic only assumes profiles can point at a resolved skill set after restart.

## Design

### Relationship to Simulations and Model Testing

| Aspect | This epic (runtime profiles) | [SIMULATIONS.md](SIMULATIONS.md) / [`.testing/`](../../.testing/) |
|--------|------------------------------|------------------------------------------------------------------------|
| **What it bounds** | **Who sees what data** and **which profile subtree** is live | **Repeatable scenarios**, fixtures, optional CI for gateway behavior |
| **Overlap** | *Motivation:* a **second profile** (e.g. the one named **`developer`**) is a common place to run automated or scripted model/skill tests **without** touching another profile’s state | Harness runs assume a **known config**—profiles let you keep that config **isolated** from a profile you use for something else |
| **Composition** | *Illustrative:* switch to a **non-production** profile → run playbooks or harness → later update pins or enablement elsewhere (see **skill packages** epic) | Playbooks remain the **expectation** source; **you** choose providers and paths per profile—the epic does **not** mandate “safe defaults” per profile name |

**View:** Runtime profiles address **trust and layout**; simulations address **repeatability**. Together they support **safe iteration** when **you** separate experimental runs from sensitive profiles by configuration and habit, not by enforced profile personas.

### Design Axes

1. **Layout** — **Everything except skill packages** lives under **`~/.chai/profiles/<name>/`**: **config**, **`agents/<id>/`** (on-disk agent context), **pairing**, **device identity**, **channel stores**, **`.env`**, and any other durable runtime state (see **Example layout**). **Only** the **shared skill store** **`~/.chai/skills/`** sits **outside** profile trees (fixed path under **`~/.chai`**).
2. **Host** — **Binaries** on **`PATH`** remain host-global; they are not profile data.
3. **Promotion** — *When using two profiles for isolation,* updating one profile after testing in another is a **pin / lock / enablement** change (see **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)**), not a second on-disk skill tree under **`profiles/`**.
4. **CLI/UX** — `chai profile list | switch <name> | current`; gateway loads **`config.json`** from the **resolved profile root** (see **Active profile resolution**). Non-skill paths in config default **profile-relative** (matrix store, device files, etc.). **Skill discovery** is **`~/.chai/skills`** only. **Which packages each agent uses** is **`skillsEnabled`** / **`contextMode`** on **`agents`** entries (**[AGENT_ISOLATION.md](AGENT_ISOLATION.md)**)—not top-level **`skills.enabled`**.

### Active Profile Resolution

**Persistent default** — Symlink **`~/.chai/active`** → **`profiles/<name>/`** (target is the profile directory). The name **`active`** is fixed; it means “which profile is selected by default,” not a user profile name.

**Precedence (this process)** — **Highest first:**

1. **CLI** profile override for the running command (**`chai gateway --profile`**, **`chai chat --profile`**) — does **not** update **`active`**.
2. **`CHAI_PROFILE`** — profile **name** (e.g. `assistant`); does **not** update **`active`**.
3. **`~/.chai/active`** — read symlink; must yield a **valid** **`profiles/<name>/`** directory.

**Errors (clear messages, same class of failure)** — When resolution **falls through to the symlink** (no CLI override and no **`CHAI_PROFILE`**): if **`active`** is **missing**, **broken**, or points at a **non-existent or invalid** profile path, **fail** — **do not** assume **`assistant`**. When the profile name comes from **CLI** or **`CHAI_PROFILE`**, validate that **`~/.chai/profiles/<name>/`** exists (or equivalent); invalid name → same style of error. **One-shot** invocations that **fully specify** the profile via CLI or **`CHAI_PROFILE`** need not read **`active`**; they still require a valid **`profiles/<name>/`** on disk.

**Updating the persistent default** — **`chai profile switch <name>`** rewrites **`~/.chai/active`** **only when the gateway is not running**; if the gateway **is** running, **fail** with a **clear error**. **Desktop:** profile switching is **disabled** while the gateway is running; same rule.

**Implementation note** — “Gateway running” uses **`~/.chai/gateway.lock`**: the file holds profile name + PID for humans; **`gateway_is_running`** and a second **`chai gateway`** take a **non-blocking advisory exclusive lock** on that file (**`fs2`**) so concurrent starts do not race a check-then-write pattern. The lock releases when the gateway process exits (including **`kill -9`** on Unix once the fd closes).

**`chai profile current`** — Show the **persistent** profile: the **name** implied by **`~/.chai/active`** (the thing **`chai profile switch`** updates). If **`CHAI_PROFILE`** is set in the **current environment** and selects a **different** profile than the symlink, print **both** **persistent** and **effective**, and label **effective** as coming from **`CHAI_PROFILE`** (same pattern if this subcommand ever accepts an ephemeral **`--profile`**). When they match, **one line** is enough. That matches common CLI practice (default vs override) and avoids hiding a mismatch between “what I last switched to” and “what a gateway would use from this shell.”

### Decisions (Shipped)

| Topic | Decision |
|-------|----------|
| **Pairing and device identity** | **Per profile** — **`paired.json`**, **device signing keys**, and **device token** material live **inside** each profile’s subtree. **Security over convenience:** switching profile switches **trust domain**, not only LLM config. |
| **Orchestration** | **Per profile** — Orchestrator settings (delegation, limits, whatever is config-driven) are read from **that profile’s** `config.json` (and profile-local state), not from a single global policy. |
| **Skill storage vs. profile** | **Only skill packages outside profiles** — Packages live under **`~/.chai/skills/`**. **No** per-profile duplicate skill tree for the same package. Per-agent **skillsEnabled** in that profile’s **`config.json`** selects subsets (**[AGENT_ISOLATION.md](AGENT_ISOLATION.md)**). |
| **Active profile** | **`~/.chai/active`** symlink → **`profiles/<name>/`**. Override order: **CLI** (ephemeral) → **`CHAI_PROFILE`** → **symlink**. Init creates **`assistant`** and **`developer`** and sets **`active` → `profiles/assistant/`**. User may rename profiles after init. |
| **Switching while gateway runs** | **Not allowed** — **`chai profile switch`** errors if the gateway is running; desktop **disables** the profile control in that state. User **stops** the gateway, **switches**, **restarts**. |
| **Legacy layout** | **Not supported** — Flat **`~/.chai/config.json`** and **`CHAI_CONFIG_PATH`** are removed; no compatibility layer. **`chai init`** creates the profile layout. |

### Example `~/.chai` Layout (Illustrative)

The tree below shows **shared root-level skill storage** and **per-profile** config, **agent context dirs**, pairing, and channel store layout. Exact filenames may evolve; the point is **skill packages are not nested under `profiles/<name>/`**, and **orchestrator `AGENTS.md`** lives under **`agents/<orchestratorId>/`**.

```text
~/.chai/
├── profiles/
│   ├── assistant/                   # default assistant profile directory
│   │   ├── agents/
│   │   │   └── orchestrator/
│   │   │       └── AGENTS.md
│   │   ├── .env
│   │   ├── config.json
│   │   ├── device.json
│   │   ├── device_token
│   │   └── paired.json
│   └── developer/                   # default developer profile directory
│       ├── agents/
│       │   └── orchestrator/
│       │       └── AGENTS.md
│       ├── .env
│       ├── config.json
│       ├── device.json
│       ├── device_token
│       └── paired.json
├── skills/
│   └── <skill-name>/
└── active -> profiles/assistant/    # symlink to active profile (default assistant)
```

**Reading the layout**

- **`active`** — Symlink at **`~/.chai/active`** points at **`profiles/<name>/`**; see **Active profile resolution** for overrides (**`CHAI_PROFILE`**, CLI) and error rules. Gateway loads **`config.json`** from the resolved profile root. **Agent context dirs** (**`agents/<id>/`**), **pairing**, **device** material, and **channel stores** resolve **under that profile subtree** (or as set in config).
- **Skills** — **`~/.chai/skills/`** is the **only** package store. Profiles differ by per-agent **skillsEnabled** / **`contextMode`** in **`config.json`**; **pins / lockfile** when **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)** ships—not by duplicating **`skills/`** under **`profiles/`**.
- **Privacy** — *Motivation:* if you point **one** profile at frontier providers and keep **another** for sensitive use, skill **definitions** still load from the shared store; **profile-local** **`agents/`**, **channel history**, and pairing **do not** cross profiles. Put sensitive **orchestrator instructions** in **`agents/<orchestratorId>/AGENTS.md`** for the profile that should see them.
- **Promotion** — *Illustrative workflow:* after validating a skill revision in **profile A**, update **profile B**’s **pins or lock** (see **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)**); revisions live **inside** each **`skills/<name>/`** tree, not in a separate drafts directory.

## Requirements (Shipped)

- [x] **Profile layout** — Per-profile subtrees under `~/.chai/profiles/<name>/` for `config.json`, **`agents/<id>/`**, `paired.json`, device identity, Matrix store default, etc. Users may add a profile-local **`.env`**; the runtime does **not** auto-create or load it yet.
- [x] **Shared skill store** — Skills under `~/.chai/skills/` only; each profile’s `config.json` selects subsets per agent via **`skillsEnabled`** on orchestrator/worker entries (see **[AGENT_ISOLATION.md](AGENT_ISOLATION.md)**).
- [x] **Active profile resolution** — Symlink `~/.chai/active` → `profiles/<name>/`; overrides: **`chai gateway --profile` / `chai chat --profile`**, then **`CHAI_PROFILE`**, then symlink. Clear errors when the symlink is missing, broken, or invalid (no silent default to `assistant` when the symlink is required); invalid override targets fail similarly.
- [x] **Initialization defaults** — **`chai init`** creates **`assistant`** and **`developer`**, writes default **`config.json`**, **`agents/orchestrator/AGENTS.md`**, extracts shared **`skills/`**, sets **`active` → `profiles/assistant/`**.
- [x] **Profile switching (CLI)** — **`chai profile list`**, **`current`** (persistent + effective when **`CHAI_PROFILE`** differs), **`switch`** (refuses when the advisory lock on **`gateway.lock`** indicates a live gateway). Live stack still **stop → switch → restart**.
- [x] **Gateway lock** — **`~/.chai/gateway.lock`**: profile name + PID for humans; **`gateway_is_running`** and second **`chai gateway`** use a **non-blocking advisory exclusive lock** (**`fs2`**) so concurrent starts do not race.
- [x] **Per-profile pairing and device identity** — `paired.json`, **`device.json`**, **`device_token`** under each profile directory; gateway uses profile-local paths.
- [x] **Desktop profile switching (core)** — Header shows **persistent** active profile name; **ComboBox** switches **`~/.chai/active`** when the gateway is **not** running (same lock rule as CLI). Optional **persistent vs effective** hint when env overrides exist — **[DESKTOP_APP.md](DESKTOP_APP.md)**.
- [x] **Historical doc alignment** — **`.journey/`**, **`.agents/ref/CLAW_ECOSYSTEM.md`**, **[IMPLEMENTATION.md](../poc/IMPLEMENTATION.md)** / **[CHANGELOG.md](../poc/CHANGELOG.md)** (where still useful as history), and stray **`CHAI_CONFIG_PATH`** / flat **`config.json`** references brought in line with the profile layout.

## Delivery Phases (Retrospective)

| Phase | Outcome |
|-------|---------|
| **Inventory** | Paths mapped; **`CHAI_CONFIG_PATH`** and flat config removed from code and user docs. |
| **Profile layout** | Per-profile config, **`agents/<id>/`**, pairing, channels, shared **`skills/`**, switch + restart. |
| **Second default profile** | **`developer`** scaffold has **parity** with **`assistant`**. |
| **Desktop** | Header profile switcher + lock rule (**`crates/desktop`**). |
| **Lock + docs** | Advisory **`gateway.lock`**; journey, **`.agents`** implementation archive, and ecosystem docs updated. |

## Implementation Order (with Related Epics)

When adding **agent isolation**, **skill packages**, or adjacent work on top of profiles, use this sequence:

1. **This epic** — **First.** Establishes **`profileRoot`**, active profile resolution, and trust boundaries (config, **`agents/<agentId>/`**, pairing, channels).
2. **[AGENT_ISOLATION.md](AGENT_ISOLATION.md)** — **Second.** Per-agent context directories under the profile, per-agent skill configuration in **`config.json`**, and orchestrator vs worker prompts.
3. **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)** — **Third.** Package revisions, **`skills.lock`** / pins, rollback, and **skill-package concerns** such as **capability-tier / variant validation** at startup (see that epic)—on top of the shared **`~/.chai/skills/`** store and profile-local config.

**Note:** **[AGENT_ISOLATION.md](AGENT_ISOLATION.md)** assumes **`profileRoot`** is always a profile directory (shipped layout); there is no flat **`~/.chai/agents/`** tree.

## Related Epics and Docs

- [AGENT_ISOLATION.md](AGENT_ISOLATION.md) — Per-agent **`agents/<id>/`** under the profile; **Implementation order** aligns with this epic **first**.
- [SKILL_PACKAGES.md](SKILL_PACKAGES.md) — **Lockfiles**, **pins**, **rollback**, flake-style metaphor, and **startup validation** of **`capability_tier`** / **`model_variant_of`** against the active profile’s effective model (not a requirement of this epic); **consumes** profile context for **which** lock is active; implement **after** agent isolation (see **Implementation order** above).
- [DESKTOP_APP.md](DESKTOP_APP.md) — Desktop **profile** UX beyond this epic’s core switcher (optional **persistent vs effective** hint).
- [SIMULATIONS.md](SIMULATIONS.md) — Harness and repeatable runs; complementary to profile-bound trust.
- [README.md](../../README.md) — Describes profile layout, **`CHAI_PROFILE`**, and **`chai profile`** commands (keep in sync with code).
- [.testing/README.md](../../.testing/README.md) — Model-comparison playbooks; natural fit for a **second** or **non-production** profile when you want isolation from daily assistant state.
