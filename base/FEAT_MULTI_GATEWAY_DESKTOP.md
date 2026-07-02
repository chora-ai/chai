---
status: draft
---

# FEAT: Multi-Gateway Desktop (Per-Profile Gateway Management)

## Motivation

The desktop app was designed around a single-gateway assumption. Even with the per-profile lock infrastructure from `FEAT_PER_PROFILE_GATEWAY_LOCK`, the desktop could only manage one gateway at a time — all gateway state (process handle, status, sessions, chat, event listener) was singular on `ChaiApp`. This branch refactors the desktop to support multiple simultaneous gateways, one per profile, allowing the user to start a gateway on one profile, switch profiles, start another gateway, and seamlessly switch between profiles each with their own running gateway and independent session state.

## Architecture

### Core Change: `GatewayState` Struct

All per-gateway state was extracted from `ChaiApp` into a new `GatewayState` struct (defined in `app.rs`). A `HashMap<String, GatewayState>` keyed by profile name lives on `ChaiApp`. Two accessor methods delegate to the active profile's entry:

- `fn gw(&mut self) -> &mut GatewayState` — returns mutable reference, creates entry with `or_default()` if missing
- `fn gw_ref(&self) -> &GatewayState` — returns immutable reference (currently has a compile bug, see below)

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

### Files Modified (all uncommitted on `feat/multi-gateway-desktop`)

#### 1. `crates/lib/src/profile.rs` — ✅ COMPLETE

- Lock path moved from `~/.chai/gateway.lock` to `~/.chai/profiles/<name>/gateway.lock`
- `gateway_is_running(chai_home, profile_name)` — takes profile name parameter
- `find_running_gateway_profiles(chai_home) -> Vec<String>` — new function, scans all profiles
- Removed `read_gateway_lock_profile()` (dead code with per-profile locks)
- `acquire_gateway_lock` error message now includes profile name
- Tests updated for per-profile lock semantics + new `find_running_gateway_profiles` test
- **Known bug**: Windows `set_active_symlink` has a broken `std::fs::symlink_metadata()` call on line 189 — missing the `&link` argument. Should be `std::fs::symlink_metadata(&link).is_ok()`

#### 2. `crates/cli/src/profile.rs` — ✅ COMPLETE

- `gateway_is_running` call updated to pass profile name
- Error message updated

#### 3. `crates/desktop/src/app.rs` — ⚠️ PARTIALLY COMPLETE

**What's done:**
- `GatewayState` struct defined with all per-gateway fields (~50 fields)
- `Default for GatewayState` implemented
- `ChaiApp` struct refactored — singular gateway fields removed, `gateways: HashMap<String, GatewayState>` added
- `cached_gateway_profile` field added alongside `cached_profile_override`
- `running_profiles: Vec<String>` replaced `gateway_lock_profile: Option<String>`
- `effective_profile_override()` — returns `env_profile` only (for UI/config)
- `gateway_profile_override()` — returns `env_profile` > active profile (if running) > first running profile (for WS)
- `refresh_cached_profile_override()` — updates both cached overrides
- `gw()` / `gw_ref()` accessor methods
- `gateway_owned()`, `gateway_running()`, `gateway_probe_completed()`, `gateway_is_owned()`, `gateway_responds()` — delegate to `gw_ref()`
- ~30 forwarding accessor methods added (lines 545–722) to expose GatewayState fields through method calls (e.g., `gateway_status()`, `chat_session_id()`, `session_messages()`, etc.) — these exist so screen files can call methods instead of accessing fields directly
- `switch_profile_to()` — no longer blocks when a gateway is running (per-profile locks make blocking unnecessary)
- `start_gateway()`, `stop_gateway()` — updated to use `gw()` for process handle
- `start_new_session()`, `remove_session_local()`, `clear_session_and_messages()` — updated to use `gw()`
- `start_chat_turn()`, `stop_chat_turn()`, `poll_stop()`, `poll_chat_turn()` — updated to use `gw()` and `cached_gateway_profile` for WS calls
- `poll_sessions_list()`, `poll_sessions_history()`, `poll_sessions_delete()`, `poll_sessions_delete_all()` — updated to use `gw()` and `cached_gateway_profile`
- `reconcile_dashboard_agent_selection()`, `reconcile_model_with_status()` — updated
- `eframe::App::update()` — updated with per-profile gateway stop detection, `running_profiles` refresh, profile dropdown always enabled, mismatch hint for multi-gateway scenarios

**Known compile errors in `app.rs`:**

1. **`gw_ref()` static DEFAULT is not `Sync`** (lines 451–497): `GatewayState` contains `mpsc::Receiver` types which are not `Sync`, so `static DEFAULT: GatewayState` is invalid. **Fix**: Change `gw_ref` to not use a static. Instead, have it return `Option<&GatewayState>` or use a different pattern. The simplest fix is to change all accessor methods that use `gw_ref()` to handle the `None` case:
   ```rust
   pub fn gateway_status(&self) -> Option<&GatewayStatusDetails> {
       let key = self.env_profile.as_deref().unwrap_or(&self.profile_active);
       self.gateways.get(key).and_then(|gw| gw.status.as_ref())
   }
   ```
   For non-Option fields (like `responds`, `chat_stopping`, `sessions_list_fetched`), return a default value:
   ```rust
   pub fn gateway_responds(&self) -> bool {
       let key = self.env_profile.as_deref().unwrap_or(&self.profile_active);
       self.gateways.get(key).map_or(false, |gw| gw.responds)
   }
   ```
   Remove the `static DEFAULT` entirely from `gw_ref()` — either make `gw_ref` return `Option<&GatewayState>` or inline the `self.gateways.get(key)` pattern in each accessor.

2. **`app.rs` line 882** (`new()` method): `desktop_config.appearance.theme.trim().to_lowercase().str()` — the `.str()` call is wrong. **Already fixed** to `.as_str()` in the current working tree (line 882 should now read `.as_str()`).

#### 4. `crates/desktop/src/app/state/gateway.rs` — ⚠️ PARTIALLY COMPLETE

**What's done:**
- `poll_gateway_probe()` — restructured to extract `probe_rx` result first, then write via `gw()` (avoids borrow conflicts)
- `reconcile_model_with_status()` — clones status data to avoid holding borrow
- `poll_status_fetch()` — restructured for borrow safety
- `poll_gateway_logs_fetch()` — restructured
- `poll_agent_detail()` — restructured
- `invalidate_agent_detail_cache()` — uses `gw()`
- All WS thread spawns use `self.cached_gateway_profile.clone()` instead of `self.cached_profile_override.clone()`
- All the free-standing WS functions (`fetch_gateway_status`, `fetch_gateway_logs`, `fetch_agent_detail`, `run_agent_turn`, `send_stop`, `fetch_sessions_list`, `fetch_sessions_history`, `fetch_sessions_delete`, `fetch_sessions_delete_all`) are unchanged (they take `profile_override: Option<&str>` as a parameter)

**Known issue**: The `reconcile_model_with_status()` method references `self.gw_ref().status`, `self.gw_ref().current_provider`, etc. which will have the same `static DEFAULT` `Sync` issue as `app.rs`.

#### 5. `crates/desktop/src/app/state/chat.rs` — ⚠️ PARTIALLY COMPLETE

**What's done:**
- `move_session_to_front()` — uses `gw()`
- `update_session_channel_meta()` — uses `gw()`
- `poll_session_events()` — fully rewritten to use `gw()`/`gw_ref()` with careful borrow management (extracting `ev` from receiver before mutating)
- `ensure_session_events_listener()` — uses `gw_ref()` and `gw()`, `cached_gateway_profile`
- `run_session_events_loop()` free function — unchanged

**Known issue**: Same `gw_ref()` `Sync` issue.

#### 6. `crates/desktop/src/app/state/skills.rs` — ✅ COMPLETE

- `cached_profile_override` changed to `cached_gateway_profile` for the WS fetch thread

#### 7. `crates/desktop/src/app/ui/header.rs` — ✅ COMPLETE

- Dropdown always enabled (`profile_dropdown_enabled` is always `true` from `update()`)
- Mismatch hint is informational only, does not disable dropdown
- Doc comments updated

#### 8. `crates/desktop/src/app/ui/sessions.rs` — ⚠️ PARTIALLY COMPLETE

**What's done:**
- Line 89–90: `app.gw_ref().selected_session_id` / `app.gw().selected_session_id` — updated

**Not done (25 stale field accesses):**
- Lines 118–119: `app.chat_turn_receiver` / `app.chat_stopping` — need `app.chat_turn_receiver()` / `app.chat_stopping()`
- Line 149: `app.session_order` — needs `app.session_order()`
- Line 151: `app.selected_session_id` — needs `app.selected_session_id()`
- Line 153: `app.sessions_delete_receiver` — needs `app.sessions_delete_receiver()`
- Line 157: `app.session_summaries` — needs `app.session_summaries()`
- Line 170: `app.session_messages` — needs `app.session_messages()`
- Line 171: `app.loading_session_id` — needs `app.loading_session_id()`
- Line 172: `app.sessions_history_receiver` — needs `app.sessions_history_receiver()`
- Line 174: `app.loading_session_id = Some(...)` — needs `app.loading_session_id_mut()` or `app.gw().loading_session_id`
- Line 175: `app.cached_profile_override` — needs `app.cached_gateway_profile`
- Line 185: `app.sessions_history_receiver = Some(...)` — needs `app.gw().sessions_history_receiver`
- Line 187: `app.selected_session_id = Some(...)` — needs `app.gw().selected_session_id`
- Line 198: `app.chat_session_id = Some(...)` — needs `app.gw().chat_session_id`
- Line 204: `app.chat_session_id` — needs `app.chat_session_id()`
- Line 205: `app.chat_turn_receiver` / `app.chat_stopping` — needs method calls
- Line 212: `app.cached_profile_override` — needs `app.cached_gateway_profile`
- Line 222: `app.sessions_delete_receiver = Some(...)` — needs `app.gw().sessions_delete_receiver`
- Line 241: `app.session_order` / `app.sessions_list_fetched` — needs method calls
- Lines 247–251: `app.chat_turn_receiver` / `app.chat_stopping` / `app.session_order` / `app.show_clear_all_confirm` — need method calls / mutators
- Lines 256–279: `app.show_clear_all_confirm` (read + writes) / `app.cached_profile_override` / `app.active_orchestrator_id` / `app.sessions_delete_all_receiver` — need method calls / mutators / `app.cached_gateway_profile` / `app.gw()` writes

#### 9. Screen files — ❌ NOT UPDATED (34 stale field accesses each)

The following screen files still use direct field access (`app.gateway_status`, `app.chat_session_id`, etc.) but those fields no longer exist on `ChaiApp`. They need to be updated to use the accessor methods (e.g., `app.gateway_status()`, `app.chat_session_id()`).

**`screens/chat.rs`** (18 stale accesses):
- `app.chat_session_id` → `app.chat_session_id()` (lines 26, 27)
- `app.selected_session_id` → `app.selected_session_id()` (lines 27, 44)
- `app.session_messages` → `app.session_messages()` (line 45)
- `app.chat_messages` → `app.chat_messages()` (line 47)
- `app.loading_session_id` → `app.loading_session_id()` (line 66)
- `app.gateway_status` → `app.gateway_status()` (lines 108, 113, 121, 127, 139)
- `app.active_orchestrator_id` → `app.active_orchestrator_id()` (lines 107, 130)
- `app.default_model` → `app.default_model()` (line 133)
- `app.chat_turn_receiver` → `app.chat_turn_receiver()` (lines 160, 163)
- `app.chat_stopping` → `app.chat_stopping()` (line 171)
- `app.current_model` (read) → `app.current_model()` (line 200)
- `app.current_model` (write) → `app.current_model_mut()` or `*app.current_model_mut() = Some(...)` (line 205)
- `app.current_provider` (write) → `app.current_provider_mut()` (line 232)
- `app.current_model = None` → `*app.current_model_mut() = None` (line 233)

**`screens/gateway.rs`** (2 stale accesses):
- `app.gateway_status` (read) → `app.gateway_status()` (lines 38, 65)

**`screens/tools.rs`** (6 stale accesses):
- `app.gateway_status` → `app.gateway_status()` (lines 21, 26)
- `app.dashboard_agent_id` (write) → `app.dashboard_agent_id_mut()` (line 56)
- `app.agent_detail_cache` → `app.agent_detail_cache()` (lines 65, 82)
- `app.agent_detail_fetch_error` → `app.agent_detail_fetch_error()` (line 88)
- `app.tools_display_buffer` — **no change needed** (still on ChaiApp)

**`screens/agent.rs`** (5 stale accesses):
- `app.gateway_status` → `app.gateway_status()` (line 22)
- `app.dashboard_agent_id` (read) → `app.dashboard_agent_id()` (line 35)
- `app.dashboard_agent_id` (write) → `app.dashboard_agent_id_mut()` (line 58)
- `app.agent_detail_cache` → `app.agent_detail_cache()` (line 73)
- `app.agent_detail_fetch_error` → `app.agent_detail_fetch_error()` (line 82)

**`screens/skills.rs`** (1 stale access):
- `app.gateway_status` → `app.gateway_status()` (line 43)

**`screens/config.rs`** (2 stale accesses):
- `app.default_model` (read) → `app.default_model()` (line 18)
- `app.default_model` (write) → `app.default_model_mut()` (line 20)

## Remaining Work (Ordered)

### 1. Fix `gw_ref()` — Remove `static DEFAULT` (CRITICAL, blocks all compilation)

The `static DEFAULT: GatewayState` in `gw_ref()` (app.rs ~line 452) fails because `GatewayState` contains `mpsc::Receiver` which is not `Sync`. **Fix**: Eliminate `gw_ref()` entirely. Instead, inline the `self.gateways.get(key)` pattern in each accessor method. For `Option` fields, use `and_then`:

```rust
pub fn gateway_status(&self) -> Option<&GatewayStatusDetails> {
    let key = self.env_profile.as_deref().unwrap_or(&self.profile_active);
    self.gateways.get(key).and_then(|gw| gw.status.as_ref())
}
```

For non-`Option` fields, use `map_or`:

```rust
pub fn gateway_responds(&self) -> bool {
    let key = self.env_profile.as_deref().unwrap_or(&self.profile_active);
    self.gateways.get(key).map_or(false, |gw| gw.responds)
}
```

Also update `gateway_probe_completed()`, `gateway_is_owned()`, `chat_stopping()`, `sessions_list_fetched()`, `show_clear_all_confirm()`.

In `state/gateway.rs` and `state/chat.rs`, the `self.gw_ref()` calls that read fields need the same treatment — either inline `self.gateways.get(key)` or restructure the methods to take `&mut self` and use `gw()`.

### 2. Fix `profile.rs` Windows `set_active_symlink` (line 189)

Change `std::fs::symlink_metadata().is_ok()` to `std::fs::symlink_metadata(&link).is_ok()`.

### 3. Update `sessions.rs` (25 stale field accesses)

Replace all direct field accesses with accessor method calls. For writes, use `app.gw().field_name = value` or the `_mut()` accessor methods. For WS thread spawns, change `app.cached_profile_override` to `app.cached_gateway_profile`.

### 4. Update screen files (34 stale field accesses)

Update `screens/chat.rs`, `screens/gateway.rs`, `screens/tools.rs`, `screens/agent.rs`, `screens/skills.rs`, `screens/config.rs` to use accessor methods instead of direct field access.

### 5. Run `cargo_check` and fix remaining errors

After the above changes, run `cargo_check` with `path: "chai"` and `package: "desktop"` to find any remaining compile errors. Common patterns to watch for:
- Borrow conflicts where `gw()` is called multiple times in the same scope
- Missing accessor methods for fields not yet covered
- `cached_profile_override` used where `cached_gateway_profile` is needed (for WS calls)

### 6. Run `cargo_test`

Run `cargo_test` with `path: "chai"` to verify the `profile.rs` lock tests and `find_running_gateway_profiles` test pass.

### 7. Run `cargo_check` for `lib` package

Verify the `lib` package compiles with the per-profile lock changes.

## Key Design Decisions

- **Profile switching is never blocked in the desktop**: The `switch_profile_to()` method no longer checks `gateway_is_running()`. The per-profile lock already prevents starting a second gateway on the same profile, and the desktop should allow the user to return to a profile with a running gateway (e.g. to see an agent's output).
- **Session state is per-profile**: Each `GatewayState` has its own `session_messages`, `session_summaries`, `session_order`, `chat_session_id`, etc. Switching profiles swaps the visible session state.
- **WS threads use `cached_gateway_profile`**: All background WebSocket thread spawns (status fetch, chat turn, session events, logs, agent detail, sessions list/history/delete) use `cached_gateway_profile` to connect to the correct gateway port.
- **UI/config loading uses `cached_profile_override`**: Config loading, skills loading, and provider resolution use `cached_profile_override` to follow the desktop's active profile.
- **Probe loop probes the active profile's gateway**: The TCP probe in `poll_gateway_probe()` uses `cached_gateway_profile` to determine which address to probe. This means only the active profile's gateway is probed for `responds` status — other profiles' gateways are tracked via `running_profiles` (lock file scan).
- **`running_profiles` replaces `gateway_lock_profile`**: Instead of discovering one running gateway profile, the desktop now discovers all running profiles via `find_running_gateway_profiles()`.

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
