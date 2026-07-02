---
status: ready-for-testing
---

# FEAT: Multi-Gateway Desktop (Per-Profile Gateway Management)

## Motivation

The desktop app was designed around a single-gateway assumption. Even with the per-profile lock infrastructure, the desktop could only manage one gateway at a time — all gateway state (process handle, status, sessions, chat, event listener) was singular on `ChaiApp`. This branch refactors the desktop to support multiple simultaneous gateways, one per profile, allowing the user to start a gateway on one profile, switch profiles, start another gateway, and seamlessly switch between profiles each with their own running gateway and independent session state.

This branch supersedes `FEAT_PER_PROFILE_GATEWAY_LOCK` — the per-profile lock infrastructure and all bug fixes from that branch have been incorporated here.

## Architecture

### Core Change: `GatewayState` Struct

All per-gateway state was extracted from `ChaiApp` into a new `GatewayState` struct (defined in `app.rs`). A `HashMap<String, GatewayState>` keyed by profile name lives on `ChaiApp`. Three accessor methods delegate to the active profile's entry:

- `fn gw_key(&self) -> &str` — returns the key used to look up the active profile's GatewayState (`env_profile` or `profile_active`)
- `fn gw(&mut self) -> &mut GatewayState` — returns mutable reference, creates entry with `or_default()` if missing
- `fn gw_ref(&self) -> Option<&GatewayState>` — returns immutable reference to the active profile's GatewayState, or `None` if no entry exists yet

The active profile is determined by `env_profile` (if `CHAI_PROFILE` is set) or `profile_active` (from `~/.chai/active`).

### Profile Override Split

Two cached profile overrides replace the old single `cached_profile_override`:

- `cached_profile_override: Option<String>` — for UI/config loading, follows the desktop's active profile (`env_profile` or `profile_active`)
- `cached_gateway_profile: Option<String>` — for WebSocket background thread spawns, follows the running gateway's profile (prefers active profile if it has a running gateway, falls back to first running profile)

The `gateway_profile_override()` method resolves which profile to connect WS threads to:
1. `CHAI_PROFILE` env var (if set)
2. Active profile (if it has a running gateway)
3. First running profile (if any)
4. `None` (no gateway running)

### Running Profiles Discovery

Replaced the old singular `gateway_lock_profile: Option<String>` (which used `read_gateway_lock_profile` to read one lock file) with `running_profiles: Vec<String>` (which uses `find_running_gateway_profiles` to scan all profile directories for held locks). This is refreshed on probe cadence (~1 Hz).

## Implementation Status

### All Files — ✅ COMPLETE

All compile errors fixed. `cargo_check` and `cargo_test` pass for both `lib` and `desktop` packages.

#### Summary of Changes

1. **`crates/lib/src/profile.rs`** — Per-profile locks (`gateway_lock_path` takes `profile_name`, `gateway_is_running` takes `profile_name`, `find_running_gateway_profiles` returns `Vec<String>`, removed `read_gateway_lock_profile` dead code). Windows `symlink_metadata` bug fixed.
2. **`crates/lib/src/gateway/server.rs`** — Lock acquisition moved to the very beginning of `run_gateway()` (before any startup work) so same-profile conflicts produce immediate errors.
3. **`crates/cli/src/profile.rs`** — Updated `gateway_is_running` call to pass profile name. Switch blocks only when target profile's gateway is running.
4. **`crates/desktop/src/app.rs`** — `GatewayState` struct, `HashMap<String, GatewayState>`, accessor methods returning `Option`/defaults, all methods updated for borrow-safe `gw()`/`gw_ref()` usage. Profile override split (`effective_profile_override` for UI, `gateway_profile_override` for WS). `switch_profile_to()` no longer checks `gateway_is_running()`.
5. **`crates/desktop/src/app/state/gateway.rs`** — All `gw_ref()` calls updated to handle `Option`, borrow conflicts resolved.
6. **`crates/desktop/src/app/state/chat.rs`** — All `gw_ref()` calls updated to handle `Option`, borrow conflicts resolved.
7. **`crates/desktop/src/app/state/skills.rs`** — Uses `cached_gateway_profile`.
8. **`crates/desktop/src/app/ui/header.rs`** — Dropdown always enabled, mismatch hint informational.
9. **`crates/desktop/src/app/ui/sessions.rs`** — All 25 stale field accesses replaced with accessor methods.
10. **Screen files** (`chat.rs`, `gateway.rs`, `tools.rs`, `agent.rs`, `skills.rs`, `config.rs`) — All stale field accesses replaced with accessor methods.
11. **`base/adr/RUNTIME_PROFILES.md`** — Updated decision text for per-profile locks and multiple simultaneous gateways.
12. **`base/spec/PROFILES.md`** — Gateway lock moved to per-profile resources, directory structure updated, Gateway Lock and Profile Switching sections rewritten for per-profile semantics. Fixed duplicated "This lock prevents" lines.

### Remaining Warnings (non-blocking)

- `drop(gw)` calls on `&mut`/`&` references in `app.rs` and `state/chat.rs` — these are no-ops (dropping a reference does nothing). The borrow checker handles the lifetimes correctly via NLL.
- Some unused accessor methods (`gateway_is_owned`, `gateway_responds`, `chat_messages_mut`, `session_messages_mut`, `session_summaries_mut`, `sessions_delete_all_receiver`) — these are public API methods that may be used by future code or are kept for API completeness.

## Bug History (Migrated From FEAT_PER_PROFILE_GATEWAY_LOCK)

The following bugs were discovered and fixed during manual testing of the per-profile lock infrastructure on `FEAT_PER_PROFILE_GATEWAY_LOCK`. All fixes are incorporated in this branch:

1. **Lock acquisition at end of `run_gateway()`**: Lock was acquired after ~600 lines of startup work (provider discovery, skill loading, channel setup). A same-profile conflict emitted `log::error!` but it was buried in startup logs. **Fix**: Moved `acquire_gateway_lock()` to the very beginning of `run_gateway()`, right after `init::require_initialized()`. Now a same-profile conflict produces an immediate error with no preceding startup logs.

2. **CLI keeps profile switch block for target profile**: The CLI (`chai profile switch`) blocks switching to a profile with a running gateway — switching `~/.chai/active` while a gateway runs on the target profile creates confusing inconsistency for stateless CLI commands. The desktop is different: it's a long-lived UI where the user may legitimately want to switch back to the profile with the running gateway.

3. **`read_gateway_lock_profile()` removed as dead code**: With per-profile locks, the profile is implicit from the lock file's path. No code reads the profile name from the lock file content; it's written for debugging convenience only.

4. **Desktop dropdown disabled by profile mismatch**: The old `profile_mismatch` logic disabled the dropdown whenever `CHAI_PROFILE` was set or `gateway_lock_profile != profile_active`. **Fix**: Changed `header.rs` to use `profile_dropdown_enabled` directly (always `true` with per-profile locks). The amber mismatch hint is informational only.

5. **ComboBox reverted to gateway profile after switch**: The header's ComboBox displayed `effective_profile` (which included `gateway_lock_profile` in its resolution chain) instead of `profile_active`. After a switch, `gateway_lock_profile` remained set, causing the ComboBox to revert. **Fix**: Changed the header call to pass `effective_profile` (which now resolves to `env_profile` or `profile_active` only — no gateway lock fallback). The gateway profile is tracked separately via `cached_gateway_profile`.

6. **UI not updating after profile switch**: `effective_profile_override()` included `gateway_lock_profile` in its resolution chain, so all config loading (UI screens, skills, providers) still resolved to the gateway's profile after a switch. **Fix**: Split `effective_profile_override()` into two methods: `effective_profile_override()` returns `env_profile` only (for UI/config), `gateway_profile_override()` returns `env_profile > active profile (if running) > first running profile` (for WS connections). Added `cached_gateway_profile` alongside `cached_profile_override`, and updated all WS thread spawns to use it.

## Key Design Decisions

- **Profile switching is never blocked in the desktop**: The `switch_profile_to()` method no longer checks `gateway_is_running()`. The per-profile lock already prevents starting a second gateway on the same profile, and the desktop should allow the user to return to a profile with a running gateway (e.g. to see an agent's output).
- **Session state is per-profile**: Each `GatewayState` has its own `session_messages`, `session_summaries`, `session_order`, `chat_session_id`, etc. Switching profiles swaps the visible session state.
- **WS threads use `cached_gateway_profile`**: All background WebSocket thread spawns (status fetch, chat turn, session events, logs, agent detail, sessions list/history/delete) use `cached_gateway_profile` to connect to the correct gateway port.
- **UI/config loading uses `cached_profile_override`**: Config loading, skills loading, and provider resolution use `cached_profile_override` to follow the desktop's active profile.
- **Probe loop probes the active profile's gateway**: The TCP probe in `poll_gateway_probe()` uses `cached_gateway_profile` to determine which address to probe. This means only the active profile's gateway is probed for `responds` status — other profiles' gateways are tracked via `running_profiles` (lock file scan).
- **`running_profiles` replaces `gateway_lock_profile`**: Instead of discovering one running gateway profile, the desktop now discovers all running profiles via `find_running_gateway_profiles()`.
- **`gw_ref()` returns `Option<&GatewayState>`**: The original design used a `static DEFAULT` fallback, but `GatewayState` contains `mpsc::Receiver` which is not `Sync`, making a `static` impossible. All callers now handle `None` (returning `None` for `Option` fields, `false` for `bool` fields, empty collections for map fields).
- **Lock acquired before any startup work**: `acquire_gateway_lock()` is called at the very beginning of `run_gateway()` (right after `require_initialized()`), before provider discovery, skill loading, and channel setup. This ensures same-profile conflicts produce immediate, visible errors rather than being buried after startup logs.

## Manual Testing Instructions

### What Was Implemented

The gateway lock was moved from `~/.chai/gateway.lock` (shared across all profiles) to `~/.chai/profiles/<name>/gateway.lock` (per-profile). Each profile now has its own independent advisory lock, allowing multiple gateways to run simultaneously on different profiles. The desktop was refactored to store per-profile gateway state (sessions, chat, status, process handle) in a `GatewayState` map, enabling the user to start a gateway on one profile, switch profiles, start another gateway, and switch back to see the agent still running.

The lock is acquired before any startup work (provider discovery, skill loading, channel setup) so that a same-profile conflict produces an immediate, visible error instead of being buried after startup logs.

The desktop profile dropdown is always interactive (never disabled by a profile mismatch). When a gateway runs on a different profile than the desktop's active profile, an amber hint is shown but the user can still switch. The profile override is split: UI/config loading follows the desktop's active profile (`effective_profile_override`), while WebSocket connections follow the running gateway's profile (`gateway_profile_override`).

### Step-by-Step Test Cases

1. **Same-profile double start is blocked immediately (CLI):**
   - Start: `chai gateway --profile assistant`
   - In another terminal: `chai gateway --profile assistant`
   - Expected: Second start fails immediately with `gateway failed: acquire gateway lock: a chai gateway is already running for profile "assistant"` and exit code 1. No startup logs (model discovery, channel config) should appear before the error.
   - Stop the first gateway (Ctrl+C)

2. **Different-profile concurrent start succeeds (CLI):**
   - Start: `chai gateway --profile assistant --port 15151`
   - In another terminal: `chai gateway --profile developer --port 15152`
   - Expected: Both gateways start successfully on their respective ports
   - Verify lock files exist: `ls ~/.chai/profiles/assistant/gateway.lock` and `ls ~/.chai/profiles/developer/gateway.lock`
   - Stop both gateways

3. **Profile switch blocked when target gateway running (CLI):**
   - Start: `chai gateway --profile assistant`
   - In another terminal: `chai profile switch assistant`
   - Expected: Fails with `gateway is running for profile "assistant"; stop it before switching`
   - Stop the gateway

4. **Profile switch allowed when other profile's gateway running (CLI):**
   - Start: `chai gateway --profile assistant`
   - In another terminal: `chai profile switch developer`
   - Expected: Succeeds, prints `active profile is now developer`
   - Stop the gateway

5. **Lock file cleanup on gateway exit:**
   - Start: `chai gateway --profile assistant`
   - Verify: `cat ~/.chai/profiles/assistant/gateway.lock` shows profile name and PID
   - Stop the gateway (Ctrl+C)
   - Expected: `~/.chai/profiles/assistant/gateway.lock` is removed

6. **Desktop multi-gateway management:**
   - Start the desktop app
   - Start a gateway on profile `assistant` (click Start gateway)
   - Switch to profile `developer` via the dropdown (should always be enabled)
   - Expected: All UI screens (config, skills, providers) reflect `developer`'s data
   - Start a gateway on profile `developer` (click Start gateway — should work since `developer` has no running gateway)
   - Expected: Both gateways are running simultaneously
   - Switch back to `assistant`
   - Expected: The gateway is still running, sessions/chat from `assistant` are preserved, agent (if running) is still active
   - Switch back to `developer`
   - Expected: `developer`'s gateway state is preserved

7. **Agent continues running across profile switches:**
   - Start a gateway on profile `assistant`
   - Send a message that triggers a long agent turn (e.g. a complex task)
   - Switch to profile `developer` while the agent is running
   - Switch back to `assistant`
   - Expected: The agent turn is still in progress or has completed — chat messages show the ongoing/completed turn

### Edge Cases

- **Stale lock file after kill -9**: Kill a gateway process with `kill -9`. The OS releases the advisory lock but the file may remain. Restarting the gateway on that profile should succeed (the stale lock is not held).
- **Multiple desktop instances**: If two desktop instances are running, both should detect running gateways independently via `find_running_gateway_profiles()`.

### Verification Checklist

- [ ] Same-profile concurrent gateway start is blocked immediately (no startup logs before error)
- [ ] Different-profile concurrent gateway start succeeds (CLI, different ports)
- [ ] `chai profile switch <name>` is blocked only when target profile has running gateway
- [ ] `chai profile switch <name>` succeeds when other profiles have running gateways
- [ ] Lock files are created at `~/.chai/profiles/<name>/gateway.lock`
- [ ] Lock files are removed on clean gateway shutdown
- [ ] Desktop profile dropdown is always enabled (never greyed out by a running gateway)
- [ ] Desktop profile ComboBox reflects the active profile (not the gateway's profile)
- [ ] Desktop profile switch updates all UI screens (config, skills, providers) to the new profile
- [ ] Desktop profile switch preserves WebSocket connection to the running gateway (chat, sessions work)
- [ ] Desktop amber mismatch hint shows when gateway is on a different profile, disappears when profiles match
- [ ] Desktop Start gateway button starts a gateway for the active profile
- [ ] Desktop Stop gateway button stops the active profile's gateway
- [ ] Desktop can start gateways on two different profiles simultaneously
- [ ] Desktop profile switch preserves per-profile session state (sessions, chat)
- [ ] Desktop profile switch preserves WebSocket connections to running gateways
- [ ] Agent turn started on one profile continues running when switching to another profile and back
- [ ] Desktop amber mismatch hint shows when gateways are running on other profiles
- [ ] Stale lock cleanup works correctly for per-profile lock files
