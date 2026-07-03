---
status: stable
---

# Runtime Profiles

This document specifies how **named runtime profiles** under `~/.chai/profiles/` organize Chai's durable state, how the active profile is resolved, and how profile switching works. For the architectural decision, see [adr/RUNTIME_PROFILES.md](../adr/RUNTIME_PROFILES.md). For on-disk `config.json` fields within a profile, see [CONFIGURATION.md](CONFIGURATION.md).

## Profile Directory Structure

Each profile is a directory under `~/.chai/profiles/<name>/` containing all trust-sensitive and profile-local runtime state:

```text
~/.chai/
├── active -> profiles/assistant/     # persistent active profile symlink
├── profiles/
│   ├── assistant/                    # profile directory
│   │   ├── agents/
│   │   │   └── orchestrator/
│   │   │       ├── AGENT.md
│   │   │       └── sessions/         # persisted sessions and bindings
│   │   ├── sandbox/
│   │   ├── .env
│   │   ├── config.json
│   │   ├── device.json
│   │   ├── device_token
│   │   ├── gateway.lock              # per-profile advisory lock
│   │   ├── paired.json
│   │   └── skills.lock
│   └── developer/                    # profile directory
│       ├── agents/
│       │   └── orchestrator/
│       │       ├── AGENT.md
│       │       └── sessions/        # persisted sessions and bindings
│       ├── sandbox/
│       ├── .env
│       ├── config.json
│       ├── device.json
│       ├── device_token
│       ├── gateway.lock              # per-profile advisory lock
│       ├── paired.json
│       └── skills.lock
└── skills/                           # shared skill package store
```

### Per-Profile Resources

The following resources are **isolated per profile** — each profile has its own independent copy:

| Resource | Location under profile | Notes |
|----------|----------------------|-------|
| Agent context | `agents/<agentId>/AGENT.md` | On-disk instructions per agent (see [AGENTS.md](AGENTS.md)) |
| Agent sessions | `agents/<agentId>/sessions/` | Persisted session files and bindings (see [CONTEXT.md](CONTEXT.md)) |
| Write sandbox | `sandbox/` | Per-profile write boundary (see [SANDBOX.md](SANDBOX.md)) |
| Secrets | `.env` | Optional profile-local environment file |
| Configuration | `config.json` | Agent entries, providers, channels, gateway settings |
| Device identity | `device.json`, `device_token` | Signing keys and device material |
| Channel stores | (varies by channel) | Matrix store default, etc. |
| Gateway lock | `gateway.lock` | Advisory lock file (see Gateway Lock below) |
| Pairing | `paired.json` | Pairing state for this profile's trust domain |
| Skill lockfile | `skills.lock` | Pinned skill versions for reproducible restarts (see Skill Lockfile below) |

### Shared Resources

The following resources are **shared across all profiles**:

| Resource | Location | Notes |
|----------|----------|-------|
| Active symlink | `~/.chai/active` | Points to the persistent default profile |
| Skill packages | `~/.chai/skills/` | Only package store; per-agent enablement selects subsets (see [AGENTS.md](AGENTS.md)) |
| Desktop config | `~/.chai/desktop.json` | Desktop appearance and log settings (see [DESKTOP.md](DESKTOP.md)) |

## Active Profile Resolution

The **active profile** determines which profile directory the gateway loads at startup. Resolution uses the following precedence (highest first):

| Precedence | Source | Scope | Updates `active`? |
|------------|--------|-------|-------------------|
| 1 | CLI `--profile` flag (`chai gateway --profile <name>`, `chai chat --profile <name>`) | Current process only | No |
| 2 | `~/.chai/active` symlink → `profiles/<name>/` | Persistent default | Set by `chai profile switch` |

### Error Rules

- When resolution falls through to the symlink (no CLI override): if `active` is **missing**, **broken**, or points at a **non-existent or invalid** profile path, the runtime **fails** — it does not silently default to `assistant`.
- When the profile name comes from CLI `--profile`, the runtime validates that `~/.chai/profiles/<name>/` exists. Invalid name produces the same class of error.

## Gateway Lock

One gateway process is allowed **per profile**. This is enforced via **per-profile advisory exclusive locks** on `~/.chai/profiles/<name>/gateway.lock`:

- Each profile has its own `gateway.lock` file inside its profile directory.
- The file holds the profile name and PID for human inspection.
- The gateway takes a non-blocking advisory exclusive lock using `fs2` (portable `flock` / `LockFileEx` semantics) at startup. If the lock for that profile is already held, the gateway fails to start.
- A second `chai gateway --profile <name>` invocation checks the same per-profile lock and fails if a gateway is already running for that profile.
- The lock releases when the gateway process exits (including `kill -9` on Unix once the fd closes).
- Multiple gateways can run simultaneously on different profiles, each holding its own independent lock.

This lock prevents:
- Concurrent gateway starts on the same profile racing a check-then-write pattern

Profile switching is always allowed — it only updates the `~/.chai/active` symlink. The per-profile gateway lock prevents starting a second gateway on the same profile, but does not restrict which profile is active.

## Skill Lockfile

Each profile has a **per-profile lockfile** at `profiles/<name>/skills.lock` that records exact content hashes for each skill, enabling reproducible restarts and rollback. Different profiles legitimately pin different revisions (e.g., developer iterates freely while assistant pins stable versions).

### Lockfile Schema

```json
{
  "version": 1,
  "skills": {
    "git": { "hash": "a1b2c3d" },
    "files": { "hash": "f8e9d0b" },
    "notes": { "hash": "9c8d7e6" }
  },
  "generation": 3
}
```

- **Skill identity** — keyed by **directory name** (authoritative). No frontmatter `name` field exists; directory name is the sole identity.
- **Generation** — monotonic integer incremented on each lock update. The lockfile itself is the generation; the integer provides ordering.

### Strictness

Configurable per profile via `skills.lockMode` in `config.json` (see [CONFIGURATION.md](CONFIGURATION.md)):

| Mode | Behavior |
|------|----------|
| `"strict"` (default) | Refuse to start the gateway when the lockfile is missing, any enabled skill has no lock entry (unpinned), or any pinned skill's `active` version does not match its locked hash. The lockfile acts as a complete manifest: every enabled skill must be pinned. Appropriate for assistant profiles. |
| `"warn"` | Log a warning when the `active` symlink target does not match the locked hash, but continue loading. Unpinned skills (no lock entry) load normally. Skip verification entirely when no lockfile is present. Appropriate for developer profiles. |

### Generation Tracking

Each lockfile update increments the generation counter. Previous lockfiles are preserved as `skills.lock.<generation>` (e.g., `skills.lock.1`, `skills.lock.2`) to make each generation addressable for rollback.

### Rollback

Restoring a previous generation's lockfile (generation-level, not per-package) and updating `active` symlinks to match the restored lock entries. This is the same contract as profile switching: stop → rollback → restart.

### Promotion

Developer → assistant promotion: both profiles reference the same `versions/<hash>/` directory — no file copying between profiles. The lock is the only thing that differs. To promote a skill, either:

1. Run `chai skill lock <name>` in the assistant profile — pins that skill to its current `active` hash
2. Or: copy the hash entry from the developer profile's `skills.lock` into the assistant profile's `skills.lock`

## Profile Switching

Switching the active profile updates the `~/.chai/active` symlink. Profile switching is always allowed regardless of whether a gateway is running on any profile — the per-profile gateway lock prevents starting a second gateway on the same profile, but does not restrict which profile is active.

### `chai profile` Subcommands

| Subcommand | Behavior |
|------------|----------|
| `list` | Lists profile directories found under `~/.chai/profiles/` |
| `current` | Shows the persistent profile name from `~/.chai/active`. |
| `switch <name>` | Rewrites `~/.chai/active` to point at `profiles/<name>/`. Always succeeds (no gateway running check). |

### Desktop

The desktop header shows the persistent active profile name. A ComboBox allows switching `~/.chai/active` — the switch is always allowed regardless of whether any gateway is running.

When the gateway is running on a different profile than the active one (detected by scanning per-profile lock files), an amber label indicates which profile the gateway is using. When the desktop spawns the gateway, the active profile is passed via `--profile` so both use the same configuration.

## Initialization

`chai init` creates the profile layout:

1. Creates two default profiles: `assistant` and `developer` with **equivalent scaffolds** (same defaults, same structure)
2. Writes default `config.json` and `agents/orchestrator/AGENT.md` under each profile — only when the files do not already exist
3. Extracts bundled skills to `~/.chai/skills/` — creates version snapshots; sets `active` symlink only for fresh installations (preserves existing user customizations)
4. Creates `sandbox/` under each profile and seeds template files — only when they do not already exist (see [SANDBOX.md](SANDBOX.md)). If a profile directory already exists but its `sandbox/` subdirectory is missing, the sandbox is re-created and seeded.
5. Sets `~/.chai/active → profiles/assistant/` — only when no valid `active` symlink already exists; if the symlink points to a valid profile directory, it is left unchanged

**Re-running `chai init`** is fully non-destructive: existing profile files are never overwritten, bundled skill `active` symlinks are left unchanged when they already point to a valid version, and the profile `active` symlink is preserved if it resolves to a valid profile directory. A deleted `sandbox/` directory is recovered for existing profiles without modifying other files. Only a missing or broken `active` symlink triggers the default (`assistant`).

Default profile names are **mnemonics**, not different runtime policies. Users may rename profiles, add more, or adjust layout after init.

## Relationship to Other Systems

| System | How profiles interact |
|--------|-----------------------|
| **Agents** (see [AGENTS.md](AGENTS.md)) | Agent context directories live under `<profileRoot>/agents/<agentId>/`. Per-agent `enabledSkills` and `contextMode` are in that profile's `config.json`. |
| **Sessions** (see [CONTEXT.md](CONTEXT.md)) | Session files are stored per agent under `<profileRoot>/agents/<agentId>/sessions/`. Each session is one JSON file; binding mappings are in `bindings.json`. Sessions survive gateway restarts. |
| **Sandbox** (see [SANDBOX.md](SANDBOX.md)) | The write sandbox directory is `<profileRoot>/sandbox/`. All agents in a profile share one sandbox. |
| **Skills** (see [SKILL_PACKAGES.md](SKILL_PACKAGES.md)) | Skill packages live in the shared `~/.chai/skills/` store with versioned snapshots. Profiles differ by per-agent enablement and per-profile lockfile pins, not by duplicated package trees. |
| **Orchestration** (see [ORCHESTRATION.md](ORCHESTRATION.md)) | Orchestrator settings, delegation policy, and worker definitions come from the active profile's `config.json`. |
| **Channels** (see [CHANNELS.md](CHANNELS.md)) | Channel stores and session bindings are profile-local. |
| **Providers** (see [PROVIDERS.md](PROVIDERS.md)) | Provider configuration is in the profile's `config.json`. Discovery scope is per the orchestrator's `enabledProviders`. |

## Related Documents

| Document | Purpose |
|----------|---------|
| [adr/RUNTIME_PROFILES.md](../adr/RUNTIME_PROFILES.md) | Architectural decision for the profile model |
| [CONFIGURATION.md](CONFIGURATION.md) | On-disk `config.json` blocks, `skills.lockMode`, and environment overrides |
| [AGENTS.md](AGENTS.md) | Per-agent context and skill configuration within profiles |
| [SKILL_FORMAT.md](SKILL_FORMAT.md) | Skill directory layout, `SKILL.md` content, and frontmatter |
| [SKILL_PACKAGES.md](SKILL_PACKAGES.md) | Skill package versioned layout, startup validation, and CLI commands |
| [SANDBOX.md](SANDBOX.md) | Write sandbox under each profile |
| [CONTEXT.md](CONTEXT.md) | Context on every turn: system message, session history, tool schemas |
