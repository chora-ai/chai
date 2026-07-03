---
status: accepted
---

# Runtime Profiles

Named runtime profiles under `~/.chai/profiles/` with one active profile at a time, a symlink-based persistent default, and per-profile gateway locks allowing multiple simultaneous gateways.

## Context

Before runtime profiles, Chai used a single flat `~/.chai/config.json` with all state (config, pairing, device identity, channel stores) in one undifferentiated directory. Users who wanted different trust or capability boundaries for different tasks — for example, a personal assistant profile with sensitive context and local models versus a separate profile for skill experimentation with frontier models — had to manually swap config files or maintain separate machines. There was no first-class way to isolate pairing credentials, agent context, and channel history between independent operational contexts.

## Decision

Chai uses a NixOS-like switching model with **named runtime profiles**:

- Each profile is a directory under `~/.chai/profiles/<name>/` containing `config.json`, `agents/<id>/` (on-disk agent context), `paired.json`, device identity, channel stores, and `.env`. Skill packages live in a **shared store** at `~/.chai/skills/` — profiles do not duplicate package trees; they differ by per-agent `enabledSkills` lists and `contextMode` in that profile's `config.json`.
- The **persistent active profile** is a symlink at `~/.chai/active` pointing to `profiles/<name>/`. The symlink is the canonical default — the only override is CLI `--profile` (ephemeral, per-process). When the symlink is missing, broken, or invalid, the runtime fails rather than silently defaulting.
- **Profile switching is always allowed.** `chai profile switch` rewrites the symlink regardless of whether a gateway is running on any profile. With per-profile locks, the gateway's profile identity is inherent (it's the profile whose lock file is held) — switching the active profile changes nothing about an already-running gateway. The per-profile lock prevents starting a second gateway on the same profile; that is the only restriction needed.
- `chai init` creates two default profiles (`assistant` and `developer`) with equivalent scaffolds and sets `active → profiles/assistant/`. The names are mnemonics, not different runtime policies.

## Alternatives Considered

| Alternative | Why not |
|-------------|---------|
| **Flat `~/.chai/config.json`** (prior state) | No path isolation between operational contexts. Users must manually swap configs to separate trust boundaries. |
| **Environment-variable-only switching** (no persistent symlink) | Inconvenient for day-to-day use. Every terminal and desktop launch needs the variable set. The symlink gives a persistent default with optional per-command overrides via `--profile`. `CHAI_PROFILE` was originally supported as an intermediate override between `--profile` and the symlink, but was removed because it created an immutable override that could not be changed at runtime in the desktop, preventing seamless profile switching. |
| **Hot-reload / no-restart switching** | Significantly more complex: the gateway would need to tear down and rebuild all state (sessions, channels, provider clients, skill tools) atomically. Per-profile locks already allow multiple gateways to run simultaneously, which is the practical use case for no-restart switching — the user starts a gateway on a different profile and switches the desktop connection. Full hot-reload within a single gateway remains out of scope. |
| **Per-profile skill trees** (each profile has its own `skills/` directory) | Duplicates packages that are typically shared. The shared store with per-agent enablement lists is cleaner and supports future pin/lockfile semantics across profiles. |
| **OS-level sandboxing** between profiles (containers, VMs, seccomp) | Profiles are runtime and path isolation first. Stronger isolation is a possible follow-on, not a requirement for this design. |

## Consequences

- **Data isolation is first-class.** Pairing credentials, device identity, channel history, and agent context never cross profile boundaries.
- **Multiple simultaneous gateways.** Per-profile locks allow multiple gateways to run on different profiles at the same time. Profile switching is always allowed — the active profile symlink is independent of running gateways.
- **Shared skill store.** Skill definitions are visible to all profiles; enablement is per-agent per-profile. Sensitive instructions belong in profile-local `AGENT.md` files, not in skill packages.

## References

- [spec/PROFILES.md](../spec/PROFILES.md) — Behavioral contract for the profile system.
- [spec/CONFIGURATION.md](../spec/CONFIGURATION.md) — On-disk `config.json` blocks and environment overrides.
- [spec/AGENTS.md](../spec/AGENTS.md) — Per-agent context directories and skill configuration within profiles.
- [adr/AGENT_ISOLATION.md](AGENT_ISOLATION.md) — Per-agent context and skill decisions.
