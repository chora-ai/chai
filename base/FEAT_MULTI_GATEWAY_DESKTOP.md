---
status: ready-for-testing
---

# FEAT: Multi-Gateway Desktop (Per-Profile Gateway Management)

## Motivation

The desktop app was designed around a single-gateway assumption. Even with the per-profile lock infrastructure from `FEAT_PER_PROFILE_GATEWAY_LOCK`, the desktop could only manage one gateway at a time — all gateway state (process handle, status, sessions, chat, event listener) was singular on `ChaiApp`. This branch refactors the desktop to support multiple simultaneous gateways, one per profile, allowing the user to start a gateway on one profile, switch profiles, start another gateway, and seamlessly switch between profiles each with their own running gateway and independent session state.

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

1. **`crates/lib/src/profile.rs`** — Per-profile locks. `gw_ref()` returns `Option`. Windows `symlink_metadata` bug fixed.
2. **`crates/cli/src/profile.rs`** — Updated `gateway_is_running` call to pass profile name.
3. **`crates/desktop/src/app.rs`** — `GatewayState` struct, `HashMap<String, GatewayState>`, accessor methods returning `Option`/defaults, all methods updated for borrow-safe `gw()`/`gw_ref()` usage.
4. **`crates/desktop/src/app/state/gateway.rs`** — All `gw_ref()` calls updated to handle `Option`, borrow conflicts resolved.
5. **`crates/desktop/src/app/state/chat.rs`** — All `gw_ref()` calls updated to handle `Option`, borrow conflicts resolved.
6. **`crates/desktop/src/app/state/skills.rs`** — Uses `cached_gateway_profile`.
7. **`crates/desktop/src/app/ui/header.rs`** — Dropdown always enabled, mismatch hint informational.
8. **`crates/desktop/src/app/ui/sessions.rs`** — All 25 stale field accesses replaced with accessor methods.
9. **Screen files** (`chat.rs`, `gateway.rs`, `tools.rs`, `agent.rs`, `skills.rs`, `config.rs`) — All stale field accesses replaced with accessor methods.

### Remaining Warnings (non-blocking)

- `drop(gw)` calls on `&mut`/`&` references in `app.rs` and `state/chat.rs` — these are no-ops (dropping a reference does nothing). The borrow checker handles the lifetimes correctly via NLL.
- Some unused accessor methods (`gateway_is_owned`, `gateway_responds`, `chat_messages_mut`, `session_messages_mut`, `session_summaries_mut`, `sessions_delete_all_receiver`) — these are public API methods that may be used by future code or are kept for API completeness.

## Key Design Decisions

- **Profile switching is never blocked in the desktop**: The `switch_profile_to()` method no longer checks `gateway_is_running()`. The per-profile lock already prevents starting a second gateway on the same profile, and the desktop should allow the user to return to a profile with a running gateway (e.g. to see an agent's output).
- **Session state is per-profile**: Each `GatewayState` has its own `session_messages`, `session_summaries`, `session_order`, `chat_session_id`, etc. Switching profiles swaps the visible session state.
- **WS threads use `cached_gateway_profile`**: All background WebSocket thread spawns (status fetch, chat turn, session events, logs, agent detail, sessions list/history/delete) use `cached_gateway_profile` to connect to the correct gateway port.
- **UI/config loading uses `cached_profile_override`**: Config loading, skills loading, and provider resolution use `cached_profile_override` to follow the desktop's active profile.
- **Probe loop probes the active profile's gateway**: The TCP probe in `poll_gateway_probe()` uses `cached_gateway_profile` to determine which address to probe. This means only the active profile's gateway is probed for `responds` status — other profiles' gateways are tracked via `running_profiles` (lock file scan).
- **`running_profiles` replaces `gateway_lock_profile`**: Instead of discovering one running gateway profile, the desktop now discovers all running profiles via `find_running_gateway_profiles()`.
- **`gw_ref()` returns `Option<&GatewayState>`**: The original design used a `static DEFAULT` fallback, but `GatewayState` contains `mpsc::Receiver` which is not `Sync`, making a `static` impossible. All callers now handle `None` (returning `None` for `Option` fields, `false` for `bool` fields, empty collections for map fields).

## Manual Testing Instructions

### What Was Implemented

The gateway lock was moved from `~/.chai/gateway.lock` (shared across all profiles) to `~/.chai/profiles/<name>/gateway.lock` (per-profile). Each profile now has its own independent advisory lock, allowing multiple gateways to run simultaneously on different profiles. The desktop was refactored to store per-profile gateway state (sessions, chat, status, process handle) in a `GatewayState` map, enabling the user to start a gateway on one profile, switch profiles, start another gateway, and switch back to see the agent still running.

### Step-by-Step Test Cases

1. **Same-profile double start is blocked (CLI):**
   - Start: `chai gateway --profile assistant`
   - In another terminal: `chai gateway --profile assistant`
   - Expected: Second start fails immediately with `gateway failed: acquire gateway lock: a chai gateway is already running for profile "assistant"`
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

- [ ] Same-profile concurrent gateway start is blocked (CLI)
- [ ] Different-profile concurrent gateway start succeeds (CLI, different ports)
- [ ] `chai profile switch <name>` is blocked only when target profile has running gateway
- [ ] `chai profile switch <name>` succeeds when other profiles have running gateways
- [ ] Lock files are created at `~/.chai/profiles/<name>/gateway.lock`
- [ ] Lock files are removed on clean gateway shutdown
- [ ] Desktop profile dropdown is always enabled (never greyed out by a running gateway)
- [ ] Desktop Start gateway button starts a gateway for the active profile
- [ ] Desktop Stop gateway button stops the active profile's gateway
- [ ] Desktop can start gateways on two different profiles simultaneously
- [ ] Desktop profile switch preserves per-profile session state (sessions, chat)
- [ ] Desktop profile switch preserves WebSocket connections to running gateways
- [ ] Agent turn started on one profile continues running when switching to another profile and back
- [ ] Desktop amber mismatch hint shows when gateways are running on other profiles
- [ ] Stale lock cleanup works correctly for per-profile lock files
