---
status: in-progress
---

# Epic: Split Deployment

**Summary** — Enable a hosted-gateway deployment model where a host runs `chai gateway` on a remote server and a client connects to it using `chai-desktop` on a separate machine. The desktop app assumes the gateway is a local subprocess; this epic adds remote gateway support to the desktop so a client can connect to a remote gateway securely, with no option to spawn a local process.

**Prerequisite** — The `desktop.json` file (appearance and logs blocks) is already implemented. This epic adds a `remote` array to `desktop.json` so the desktop can connect to remote gateways, with the active profile symlink pointing at the selected target (local or remote) so device identity storage is reused.

**Status** — **Phases 1 and 2 implemented.** Phase 1 (remote profile configuration and connection) is complete. Phase 2 (gateway security hardening — origin validation and connection limiting) is complete. Phase 3 (documentation and UX) is not yet started. The desktop can connect to a remote gateway over `ws://` or `wss://` with Connect/Disconnect mode, device identity stored per-profile, and TLS support via reverse proxy. The gateway enforces origin validation and connection limits on non-loopback bindings. The per-profile gateway lock follow-up (previously listed) has been implemented — multiple gateways can now run simultaneously on different profiles, and profile switching is always allowed (see Current State below).

## Problem Statement

Chai's desktop app is designed around a single-machine model: it spawns `chai gateway` as a local child process, connects to it on loopback, and manages its lifecycle. A developer who wants to host a gateway for a client — running the gateway on a remote server while the client uses the desktop app locally — encounters several obstacles:

- **No way to point the desktop at a remote gateway.** The desktop derives the WebSocket URL from `gateway.bind`, a field that semantically means "address to bind the server to." Setting it to a remote IP on the client machine is a semantic misuse and causes confusion if the client accidentally starts a local gateway.
- **No TLS.** The gateway binds plain HTTP/WebSocket. The desktop hardcodes `ws://` URLs. Auth tokens, device tokens, conversation content, and tool outputs all travel in cleartext over the network.
- **No Connect/Disconnect mode.** The desktop shows a "Start gateway" button even when configured for a remote gateway. Pressing it would spawn a conflicting local process.
- **No origin validation.** The gateway does not check the `Origin` header on WebSocket upgrades, enabling cross-site WebSocket hijacking on non-loopback deployments.
- **No connection limit.** The gateway accepts unlimited WebSocket connections. Any number of authenticated clients can connect simultaneously, see the full session list, and interact with any session. For split deployment, this is insecure by default — a compromised token grants full access alongside the legitimate client.
- **No documentation.** Zero guidance exists for setting up, securing, or operating a split deployment.

The underlying protocol plumbing (device pairing, challenge-response auth, token issuance, log streaming) already works over the network. What's missing is the configuration, security, and UX layer to make this a first-class scenario.

## Goal

A developer can deploy `chai gateway` on a remote server, configure a client's desktop app to connect to it securely, and have the desktop operate in a connect-only mode with no option to spawn a local gateway. The connection is protected by TLS (via a documented reverse-proxy path), the client authenticates via the existing device pairing protocol, and the configuration clearly distinguishes between server-side and client-side concerns. Non-loopback gateways default to a single active client connection, providing secure-by-default isolation for split deployments.

## Current State

### Desktop Gateway Modes

The desktop operates in one of two modes (documented in [DESKTOP.md](../spec/DESKTOP.md)):

| Mode | Behavior | Header UI |
|------|----------|-----------|
| **Spawn** | Desktop starts `chai gateway` as a child subprocess. | Start/Stop controls |
| **Attach** | Another process owns the port. | Disabled "Gateway running" button |

The desktop determines which mode it's in via a periodic TCP probe (~1 Hz) to `gateway.bind`:`gateway.port`. If the desktop spawned the process itself, it's in Spawn mode; if the gateway was already listening, it's in Attach mode.

### WebSocket Connection

All desktop-to-gateway communication goes over WebSocket at `ws://<bind>:<port>/ws`. The desktop constructs this URL from its local `config.json` in every connection function:

```rust
let ws_url = format!("ws://{}:{}/ws", bind, port);
```

There is no `wss://` URL construction anywhere in the codebase.

### Device Pairing Protocol

The challenge-response pairing protocol (Ed25519 signatures + device token issuance) is network-ready and works across machines. On first connection, the desktop generates a device identity, signs the server's challenge, and presents the gateway token for auto-approval. The gateway issues a `device_token` that the desktop stores for subsequent connections. Automatic re-pairing handles stale tokens.

### Profile ComboBox

The header shows the persistent active profile (from `~/.chai/active`). A ComboBox allows switching the active profile at any time — profile switching is always allowed regardless of whether any gateway is running. Per-profile gateway locks allow multiple gateways to run simultaneously on different profiles, and the desktop stores per-profile gateway state so switching between running gateways is seamless.

This epic extends the ComboBox to include remote profiles alongside local profiles. For remote profiles, switching requires disconnecting first (the desktop must swap its WebSocket connection and cached state). For local profiles, switching is always allowed — the desktop can switch between profiles with running gateways without stop/restart.

### Currently Implemented `desktop.json` Schema

The `desktop.json` file at the chai home root is already implemented with `appearance` and `logs` blocks (see [DESKTOP.md](../spec/DESKTOP.md)). `desktop.json` is loaded once at startup. This epic adds a `remote` block to the schema.

### `~/.chai` Directory Split

In a split deployment, the two `~/.chai` directories serve different purposes:

**Remote server (developer's machine):**
```
~/.chai/
├── active → profiles/assistant/
├── profiles/assistant/
│   ├── config.json           ← authoritative: providers, channels, agents, auth
│   ├── gateway.lock          ← per-profile advisory lock (PID + profile name)
│   ├── paired.json           ← device trust store (gateway reads this)
│   ├── agents/orchestrator/
│   ├── sandbox/              ← tool execution happens here
│   ├── skills.lock
│   └── .env                  ← provider API keys
└── skills/                   ← skill packages loaded by gateway
```

**Client's machine (remote-only, minimal setup):**
```
~/.chai/
├── active → profiles/assistant-remote/
├── desktop.json              ← desktop settings + remote profile entry
├── profiles/assistant-remote/ ← created at desktop startup; holds device identity
│   ├── device.json           ← Ed25519 keypair (client identity; created on first connect)
│   └── device_token          ← session token from gateway (created on first connect)
└── skills/                   ← unused in remote mode (gateway owns skills)
```

A client machine that only connects to remote gateways does not need a `config.json`, `agents/`, `sandbox/`, `skills.lock`, or `.env` in its profile directory. The profile directory is created at desktop startup (a `mkdir -p` with no files written) so the remote entry appears in the ComboBox. The `device.json` and `device_token` files are created on first connect.

### Gateway Lock vs. Connection Policy

The per-profile `gateway.lock` is a **process-level guard**, not a connection control. It uses an advisory exclusive `flock` (`fs2::FileExt::try_lock_exclusive`) to prevent two gateway processes from running on the same profile. It does not limit, track, or gate WebSocket client connections in any way.

The gateway enforces a connection limit via `ConnectionTracker` in `GatewayState`, which tracks authenticated connections keyed by client identity (device ID). See the Connection Policy design section below for details.

For split deployment, the connection limit defaults to 1 on non-loopback, making single-client the secure default. The connection policy is independent of `gateway.lock` (process guard vs. connection guard — different layers).

### Security Posture

Per [SECURITY.md](../SECURITY.md), the following were previously out of scope and are now implemented:

- **WebSocket origin validation** — Implemented. The gateway validates the `Origin` header on WebSocket upgrades for non-loopback bindings. `allowedOrigins` defaults to empty (reject all browser origins). The desktop app is unaffected (no `Origin` header).
- **Connection limiting** — Implemented. `gateway.maxConnections` caps simultaneously authenticated client identities. Default: 1 on non-loopback, unlimited on loopback. Kick-oldest when limit exceeded.

The following remain out of scope:

- **TLS termination** — "The gateway binds plain HTTP/WebSocket. TLS is the operator's responsibility (e.g., reverse proxy). Binding to non-loopback without TLS exposes the auth token and all data in cleartext."
- **Session isolation across channels** — "No per-client or per-channel session access control; authenticated WebSocket clients can interact with any session."

The gateway does enforce token auth for non-loopback bindings — it refuses to start without `auth.mode: "token"` when bound to a non-loopback address.

### Existing Gaps

| Gap | Severity | Status |
|-----|----------|--------|
| No TLS/WSS | 🔴 Critical | Out of scope (operator's responsibility — reverse proxy) |
| No remote address config | 🟠 High | ✅ Phase 1 — `remote` array in `desktop.json` |
| No Connect/Disconnect mode | 🟠 High | ✅ Phase 1 — remote profiles show Connect/Disconnect |
| No connection limit | 🟠 High | ✅ Phase 2 — `gateway.maxConnections` with kick-oldest |
| No origin validation | 🟡 Medium | ✅ Phase 2 — `gateway.allowedOrigins` on non-loopback |
| `gateway.lock` is per-profile | 🟢 Resolved | Per-profile locks implemented; desktop discovers running profiles via `find_running_gateway_profiles()`. Remote profiles skip lock detection and rely on TCP probe (by design). |
| No documentation | 🟡 Medium | Phase 3 (not yet started) |
| Status shows server paths | 🟢 Low | Gateway status returns server-local absolute paths; confusing but not breaking |

## Scope

### In Scope

- A `remote` array in `desktop.json` that lets the desktop connect to remote gateways. Each entry has an `id` (used as the profile name and ComboBox label), a `url` (the WebSocket connection URL), and a `token` (the gateway auth token for pairing). Local profiles and remote entries appear alongside each other in the ComboBox.
- `wss://` URL construction in the desktop client for connections to TLS-terminated gateways. Full path support in the `url` field for reverse proxy configurations (e.g., `wss://example.com/chai/ws`).
- A Connect/Disconnect mode for remote profiles: when the selected profile is remote, the desktop shows Connect/Disconnect instead of Start/Stop, never spawns a local gateway, and probes the remote URL for liveness.
- WebSocket origin validation on the gateway for non-loopback connections. Default: reject all browser-origin requests on non-loopback. Operators opt in to specific origins via `gateway.allowedOrigins` in `config.json`.
- A connection limit on the gateway for non-loopback deployments. Default: one active client identity. Operators can increase the limit via `gateway.maxConnections` in `config.json`. When the limit is exceeded, the oldest client's connections are disconnected (kick-oldest), which handles reconnection gracefully.
- Documentation and a user journey for the split deployment scenario.
- Updates to `SECURITY.md` to reflect the new capabilities.

### Out of Scope

- **Built-in TLS termination in the gateway** — TLS termination remains the operator's responsibility (reverse proxy). Tracked as a potential future direction in [SECURITY.md](../SECURITY.md).
- **Per-client session isolation** — Authenticated clients within the connection limit can still interact with any session. This is a broader access-control concern tracked separately in [SECURITY.md](../SECURITY.md).
- **Rate limiting** — No limit on message rates or agent turn frequency. Tracked as out of scope in [SECURITY.md](../SECURITY.md).
- **OS-level sandboxing or resource exhaustion controls** — Not related to the split deployment scenario.
- **Remote configuration management** — The client cannot change the server's `config.json` from the desktop. Server configuration is the developer's responsibility.
- **Multi-tenant gateway** — A single gateway serving multiple independent clients with separate configurations. This is a separate concern.
- **`CHAI_GATEWAY_URL` environment variable** — Dropped. Remote gateway configuration is handled through the `remote` array in `desktop.json`.

## Design

### Remote Profile Configuration

The desktop currently derives the WebSocket URL from `gateway.bind` + `gateway.port`, fields that semantically mean "address the server binds to." For split deployment, this creates a semantic conflict: the client would need to set `gateway.bind` to the remote server's IP, but `gateway.bind` is a server-side field in `config.json`.

**Decision: `remote` array in `desktop.json`**

Instead of a single `gateway.connectUrl` field, `desktop.json` gains a `remote` array. Each entry represents a remote gateway the client can connect to:

```json
{
  "appearance": { "theme": "dark", "fontSize": 14 },
  "logs": { "bufferSize": 1000 },
  "remote": [
    {
      "id": "assistant-remote",
      "url": "wss://gateway.example.com/ws",
      "token": "<gateway-token>"
    }
  ]
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | `string` | Yes | Profile name and ComboBox label. Also determines the profile directory under `~/.chai/profiles/` where device identity is stored. Must not collide with existing local profile directory names (enforced at load time; see Collision Handling below). |
| `url` | `string` | Yes | WebSocket connection URL. Must start with `ws://` or `wss://`. Supports full paths for reverse proxy configurations (e.g., `wss://example.com/chai/ws`). The desktop sends the full URL to the WebSocket client library; path mapping to the gateway's `/ws` route is the reverse proxy's responsibility. |
| `token` | `string` | Yes | Gateway auth token for device pairing. Sent in the `auth.token` field of the connection payload. |

This approach unifies local and remote profiles in the ComboBox: the user sees the same dropdown regardless of whether profiles are local or remote. Selecting a local profile means "spawn or attach locally" (Start/Stop). Selecting a remote profile means "connect to a remote gateway" (Connect/Disconnect). The mode is determined by the type of the selected profile, not by a global flag.

**Why an array instead of a single URL:** A developer may host staging and production gateways, or multiple gateways for different clients. The array makes multiple remote gateways first-class. A minimal client setup has a single entry; a `desktop.json` with no `remote` block is unchanged from current behavior.

**Why `id` doubles as the profile name:** Device identity (`device.json`, `device_token`) is stored per-profile under `~/.chai/profiles/<id>/`. Using the `id` as the profile directory name means:

- No collisions between local and remote profiles (the ComboBox lists directory names from `~/.chai/profiles/`, which includes both).
- The ComboBox is populated from a single source: profile directories on disk. Remote entries must have their profile directories created at desktop startup so they appear in the ComboBox — without this, the user would have no way to select a remote entry to connect for the first time.
- Device identity storage is reused from the existing architecture — `ChaiPaths` already resolves `device.json()`, `device_token_path()`, and `paired_json()` from `profile_dir`. The `device.json` and `device_token` files are created on first connect (by `build_connect_params`), but the directory itself must already exist so the ComboBox can list it.

**Startup directory creation:** When `desktop.json` is loaded at startup, the desktop iterates the `remote` array and creates `~/.chai/profiles/<id>/` for each entry that does not already exist. This is a `mkdir -p` operation — no files are written, just the directory. The directory is empty until the user selects the remote profile and clicks Connect, at which point `device.json` is generated and `device_token` is issued by the gateway.

**Collision handling:** If a remote entry `id` collides with an existing local profile directory at load time, the entry is rejected (the desktop logs a warning and skips it). If a local profile directory is created *after* `desktop.json` is loaded (e.g., via `chai init`), the collision is not detected until the next desktop restart. At that point, the same load-time rejection applies. The desktop does not re-scan for collisions at runtime — the local profile on disk takes precedence, and the conflicting remote entry is treated as a configuration error visible in the startup log. This keeps the runtime behavior simple and predictable: profile directories are the source of truth, and `desktop.json` remote entries that conflict are configuration errors.

**Active symlink and profile switching:** The active symlink is updated when selecting a remote profile, same as for local profiles. This is consistent: the symlink always points at whichever profile directory the desktop is using, and `ChaiPaths` resolves device identity from there. Profile switching is always allowed for local profiles — per-profile locks allow the symlink to be freely rewritten regardless of running gateways.

For remote profiles, switching is connection-based: the desktop's WebSocket connection, cached status, session lists, and agent details are all tied to the current remote gateway. Switching to a different remote entry (or a local profile) means abandoning that connection and re-establishing a new one. The desktop enforces disconnect-before-switch for remote profiles to manage this state swap cleanly.

Local profiles no longer have a lock-based switching constraint. Per-profile locks allow multiple gateways to run simultaneously, and the desktop stores per-profile gateway state so switching between local profiles with running gateways is seamless.

### Distinguishing Local and Remote Profiles

The desktop needs to know whether the currently selected profile is local or remote to decide which button label to show (Start/Stop vs Connect/Disconnect) and whether to spawn a local process.

**Decision:** The desktop looks up the selected profile `id` in the `remote` array from `desktop.json`. If the `id` matches a remote entry, the profile is remote. If it does not match, the profile is local. This is a simple lookup against the loaded `desktop.json` `remote` array — no separate marker is needed in the profile directory itself.

### TLS and `wss://` Support

The gateway will not gain built-in TLS termination — this remains the operator's responsibility (reverse proxy with WSS→WS termination). However, the desktop client must support `wss://` URL construction when a remote entry's `url` specifies it.

**Decision:** When a remote entry's `url` starts with `wss://`, the desktop constructs a TLS WebSocket connection. When it starts with `ws://`, it uses plain WebSocket (current behavior for local profiles). Local profiles always use `ws://` (derived from `bind:port`, loopback doesn't need TLS).

This requires adding a TLS-enabled WebSocket client to the desktop crate's dependencies (e.g., `tokio-tungstenite` with the `native-tls` or `rustls` feature).

### Full Path Support in `url`

**Decision:** The `url` field supports full paths (e.g., `wss://example.com/chai/ws`) for reverse proxy configurations. The desktop passes the full URL to the WebSocket client library, which includes the path in the HTTP upgrade request. The reverse proxy is responsible for mapping the path to the gateway's `/ws` route (e.g., nginx `proxy_pass`, Caddy `handle_path`, Traefik `StripPrefix` middleware). No gateway route changes are needed — the gateway continues to serve WebSocket upgrades at `/ws` as it does today.

This is essential for reverse proxy setups, which are the only supported TLS path. Without full path support, operators cannot use common reverse proxy routing patterns.

### Connect/Disconnect Mode

When the selected profile is remote, the desktop operates in connect-only mode:

- The header shows **Connect/Disconnect** controls instead of Start/Stop.
- The desktop does not attempt to spawn `chai gateway` as a subprocess.
- The desktop probes the remote URL for liveness (TCP connect to the host:port extracted from the URL).
- Switching to a different remote profile requires disconnecting first (the desktop must swap its WebSocket connection and cached state — see Active Symlink and Switching above). Local profile switching is always allowed.
- When the user clicks **Connect**, the desktop opens a WebSocket connection to the remote URL and authenticates via the device pairing protocol.
- When the user clicks **Disconnect**, the desktop closes the WebSocket connection.

When the selected profile is local, all existing behavior is unchanged: Start/Stop, subprocess spawning, TCP probe to `bind:port`.

**Decision:** Connect/Disconnect mode is activated when the selected profile is a remote entry. No separate mode field is needed — the profile type determines the behavior.

### WebSocket Origin Validation

For non-loopback bindings, the gateway should validate the `Origin` header on WebSocket upgrade requests. This prevents cross-site WebSocket hijacking from browser-based attackers.

**Approach A: Reject all non-loopback upgrades without a whitelisted origin**

The gateway maintains an `allowedOrigins` list in `config.json`. If the `Origin` header doesn't match any entry, the upgrade is rejected.

**Approach B: Require `Origin` header on non-loopback, reject only browser-like user agents**

Check for the presence of an `Origin` header (browser WebSocket APIs always send it). If present on a non-loopback connection, validate it against an allowlist. If absent (non-browser client like the desktop app), allow it through.

**Decision: Approach A with a secure-by-default empty list.** When `auth.mode: "token"` is set on a non-loopback binding (which is already required), the `allowedOrigins` field defaults to an empty array — meaning all browser-origin requests are rejected. The desktop app does not send an `Origin` header, so it is unaffected. Operators who need browser-based tools can explicitly add origins to `allowedOrigins`. This is a defense-in-depth measure — token auth is the primary security boundary; origin validation prevents cross-site hijacking even if a token is compromised via a browser-side attack.

When `auth.mode: "none"` on non-loopback (which is already refused at startup), this is moot.

### Connection Policy

The gateway previously accepted unlimited WebSocket connections. For split deployment on non-loopback, this was insecure by default — a single compromised token allowed an attacker to connect alongside the legitimate client, observe all sessions, and interact with the gateway.

**Decision: `gateway.maxConnections` with a secure default.**

| Field | Type | Default (loopback) | Default (non-loopback) | Description |
|-------|------|--------------------|------------------------|-------------|
| `gateway.maxConnections` | `integer` | Unchanged (unlimited) | `1` | Maximum number of simultaneously authenticated **client identities** (devices). |

When a new client identity authenticates and the count of distinct clients would exceed `maxConnections`, the gateway disconnects all connections belonging to the longest-running existing client (kick-oldest). This kick-oldest policy handles network interruptions gracefully: if the legitimate desktop reconnects after a brief outage, it displaces any stale connection. If an attacker holds a connection with a stolen token, the legitimate user's reconnection kicks the attacker off.

The desktop handles unexpected disconnections with its existing reconnect logic. When the gateway kicks a connection due to the limit being exceeded, it sends a WebSocket close frame (code 1013, reason "connection limit reached: displaced by newer connection") before disconnecting.

**Why kick-oldest instead of reject-new:** A reject-new policy creates a denial-of-service vector: an attacker with a stolen token can hold a connection and prevent the legitimate user from reconnecting. Kick-oldest ensures the most recent authenticator always gets access, which is the correct tradeoff for a single-operator deployment model.

**Why `maxConnections` instead of per-device limits:** Per-device limits (e.g., one connection per device token) are more granular but more complex. For the initial implementation, a simple global limit with kick-oldest semantics provides the security benefit (single-client default) without requiring device-tracking infrastructure. Per-device limits can be added as a future enhancement if multi-client deployments need finer control.

**Interaction with `gateway.lock`:** The connection policy is independent of `gateway.lock`. The lock prevents multiple gateway *processes* per profile (process-level guard). The connection policy limits WebSocket *client* connections per gateway (connection-level guard). They operate at different layers and do not interact.

#### Identity-Based Tracking (1:N Multi-Connection-Per-Client)

**Decision: Track connections by client identity (device ID) with a 1:N model — one client slot holding N concurrent connections.**

The `ConnectionTracker` in `GatewayState` tracks authenticated connections keyed by **client identity** (device ID), not by individual WebSocket connection. Each client identity maps to a **list of active connections**. The connection limit applies to the number of **distinct clients**, not individual connections:

- **Same-client connections coexist.** When the same client opens an additional WebSocket connection (e.g., the desktop's status fetch alongside its events listener), the new connection is added to that client's existing list without any kick or limit check.
- **Kick-oldest across distinct clients.** If a *new* client connects and the distinct-client limit is exceeded, all connections of the oldest client are kicked (each receives a close frame, each `handle_socket` task breaks).
- **Per-connection unregister.** When a connection disconnects (normally or kicked), `unregister` removes that specific connection from its client's list; if the client's list becomes empty, the client slot is freed entirely.

**Why identity-based tracking:** The desktop opens multiple concurrent WebSocket connections per logical client (session events listener, periodic status/log fetches, on-demand agent turns, etc.) — all sharing the same device identity. A naive 1:1 model (one entry per connection, or one entry per client with same-key displacement) was tried first and produced an **infinite self-displacement churn loop**: every one-shot fetch kicked the long-lived events listener, which reconnected and kicked the next fetch. The 1:N model allows all connections from the same device to coexist without displacing each other, while the distinct-client limit is still enforced across different devices.

#### Session Events Listener Thread Cancellation

**Decision: An `Arc<AtomicBool>` cancel flag on `GatewayState` (`session_events_cancel`) threads cancellation signals to the desktop's background session events listener.**

The desktop's background session events listener thread (`ensure_session_events_listener` in `crates/desktop/src/app/state/chat.rs`) had no cancellation mechanism. Once spawned, it looped forever — reconnecting to the gateway WebSocket every 2–10 seconds regardless of whether the user had disconnected the profile, stopped the local gateway, or switched profiles. This was a pre-existing bug (harmless before Phase 2 because a ghost listener to a dead port was just noise), but the connection limit made it **visible and harmful**: a disconnected desktop's ghost listener thread kept reconnecting, authenticating, and registering with the `ConnectionTracker`, participating in the kick-oldest churn alongside the remaining live instances.

The fix adds a cancel flag stored in `GatewayState`. The flag is created when the listener starts, cloned into the thread, and checked at the top of each retry-loop iteration. All teardown paths set the flag before clearing the receiver:

- `disconnect_remote_profile` (via `clear_session_and_messages`)
- Local gateway stop (via `clear_session_and_messages`)
- Profile switch (cancel old profile's listener before starting new)

`ensure_session_events_listener` itself also calls `stop_session_events_listener()` when `running` is false, making it the single chokepoint for the `!running` path.

### `gateway.lock` and Remote Gateway Detection

In split deployment, the desktop's local `gateway.lock` doesn't exist (the gateway is on a different machine). The desktop detects running local gateways via per-profile lock files at `~/.chai/profiles/<name>/gateway.lock` (using `find_running_gateway_profiles()`).

**Decision:** When the selected profile is remote, the desktop skips lock file detection entirely and relies on the TCP probe against the remote URL. The profile is determined from the selected remote entry `id` (not from lock files or `config.json`). The remote gateway's own profile name (returned in `status`) may differ from the local `id` — this is not a mismatch; the `id` is the client-side label, and the remote gateway's profile name is a server-side detail that is not surfaced as a warning.

When the selected profile is local, the desktop uses per-profile lock file scanning to discover all running local gateway profiles, and uses the TCP probe to determine whether the active profile's gateway is responding.

### Config Screen Visibility

The desktop config screen currently shows `config.json` contents (bind, port, providers, agents, channels). For a remote-only client, there is no `config.json` — server-side configuration is managed server-side.

**Decision:** When the selected profile is remote, the Config screen is hidden from the sidebar. The Gateway screen (which shows `status` from the remote gateway) is the source of truth for the remote gateway's effective configuration. The Config screen reappears when a local profile is selected.

## Requirements

### Functional

- [x] **`remote` array in `desktop.json`** — Add a `remote` array to the `DesktopConfig` struct in `crates/lib/src/config.rs`. Each entry has `id` (string), `url` (string, must start with `ws://` or `wss://`, supports full paths), and `token` (string). Invalid entries are rejected at load time. When `desktop.json` is absent or has no `remote` block, all existing behavior is unchanged.
- [x] **Remote profile directories created at startup** — When `desktop.json` is loaded at startup, the desktop creates `~/.chai/profiles/<id>/` for each remote entry that does not already exist (`mkdir -p`, no files written). This ensures remote entries appear in the ComboBox before the user has ever connected.
- [x] **Remote entry collision detection** — If a remote entry `id` collides with an existing local profile directory at load time, the entry is rejected (logged as a warning, skipped). The desktop does not re-scan for collisions at runtime; disk wins over config.
- [x] **Remote profile in ComboBox** — Remote entry `id`s appear alongside local profile names in the header ComboBox. The ComboBox is populated from `~/.chai/profiles/` directory names. Selecting a remote profile updates the active symlink to point at the remote profile's directory, same as selecting a local profile.
- [x] **Connect/Disconnect mode** — When the selected profile is remote (the `id` matches a `remote` entry in `desktop.json`), the header shows Connect/Disconnect instead of Start/Stop. The desktop does not spawn a local gateway. Clicking Connect opens a WebSocket connection to the remote `url`. Clicking Disconnect closes it. Switching to a different remote profile requires disconnecting first; local profile switching is always allowed.
- [x] **Device identity for remote profiles** — When connecting to a remote profile, the desktop loads/creates `device.json` and `device_token` under `~/.chai/profiles/<remote-id>/` (the directory already exists from startup creation). The `token` from the remote entry is used for the pairing protocol instead of `config.json` `gateway.auth.token`.
- [x] **WebSocket URL from remote entry** — When the selected profile is remote, the desktop uses the `url` from the remote entry for the WebSocket connection and TCP probe instead of deriving from `config.json` `gateway.bind:port`. The full URL (including path) is passed to the WebSocket client library.
- [x] **`wss://` support in desktop** — The desktop client can establish TLS WebSocket connections when a remote entry's `url` specifies `wss://`. Local profiles always use `ws://` (derived from `bind:port`).
- [x] **WebSocket origin validation** — The gateway validates the `Origin` header on WebSocket upgrades for non-loopback connections. An `allowedOrigins` field in the `gateway` config block lists permitted origins. Default is empty (reject all browser origins on non-loopback). The desktop app does not send an `Origin` header and is unaffected. Operators can add specific origins to allow browser-based tools.
- [x] **Connection limit** — The gateway enforces `gateway.maxConnections` (integer) for authenticated WebSocket connections, tracked by client identity (device ID). Default: `1` for non-loopback bindings, unlimited for loopback. When a new client authenticates and the distinct-client limit is exceeded, all connections of the longest-running existing client are disconnected (kick-oldest). The gateway sends a descriptive close frame (code 1013) when disconnecting. The desktop handles unexpected disconnections with its existing reconnect logic.
- [ ] **Config screen hidden for remote profiles** — When the selected profile is remote, the Config screen does not appear in the sidebar. The Gateway screen (showing `status` from the remote gateway) is the sole source of configuration visibility.
- [x] **TCP probe uses remote URL** — When the selected profile is remote, the desktop probes the remote gateway host:port (extracted from the `url`) for liveness instead of `bind:port`.
- [x] **Profile detection skips `gateway.lock` for remote** — When the selected profile is remote, the desktop does not check for `gateway.lock` and relies on the TCP probe. The remote gateway's profile name (from `status`) may differ from the local `id`; this is not surfaced as a mismatch warning.

### Non-functional

- [x] **Backward compatibility** — When `desktop.json` is absent or has no `remote` block, all existing behavior is unchanged. The desktop continues to derive the WebSocket URL from `bind:port`, show Start/Stop controls, and operate in spawn-or-attach mode. The gateway with default `maxConnections` (unlimited for loopback, `1` for non-loopback) does not change behavior for existing loopback deployments.
- [x] **No new required dependencies for gateway** — TLS support is client-side only (desktop crate). The gateway does not gain TLS dependencies.
- [x] **Config validation** — Remote entry `url` must start with `ws://` or `wss://`. `id` must be non-empty and must not collide with existing local profile directory names (enforced at load time). Invalid entries are rejected.
- [x] **Security documentation updated** — `SECURITY.md` updated to reflect origin validation, connection limit, and `wss://` client support, moving those items from "Out of Scope" to implemented or partially implemented.

## Phase 1 Implementation Notes

Phase 1 is implemented. Key implementation details:

### Config Schema

The `DesktopConfig` struct in `crates/lib/src/config.rs` has an optional `remote` field:

```rust
pub struct DesktopConfig {
    pub appearance: AppearanceConfig,
    pub logs: LogsConfig,
    pub remote: Option<Vec<RemoteEntry>>,
}

pub struct RemoteEntry {
    pub id: String,    // profile name / ComboBox label
    pub url: String,   // ws:// or wss:// with full path support
    pub token: String, // gateway auth token for pairing
}
```

Validation at load time: `id` must be non-empty, `url` must start with `ws://` or `wss://`, `token` must be non-empty. Collision detection: if a remote entry `id` matches an existing profile directory with a `config.json` or `gateway.lock`, the entry is logged as a warning and skipped (disk wins).

### `CHAI_HOME` Environment Variable

The `profile::chai_home()` function in `crates/lib/src/profile.rs` checks the `CHAI_HOME` environment variable before falling back to the default `~/.chai`. This enables isolated testing of split deployment on a single machine. The `resolve.rs` `sandbox_raw()` function also respects `CHAI_HOME` (previously it read `$HOME` directly). Seven bundled skill shell scripts were updated from `$HOME/.chai` to `${CHAI_HOME:-$HOME/.chai}`.

### Desktop State

The desktop distinguishes local vs. remote profiles by looking up the selected `id` in the `remote` array from the loaded `desktop.json`. Remote profiles show Connect/Disconnect instead of Start/Stop, never spawn a local gateway, and use the remote entry's `url` and `token` for WebSocket connections. A `remote_disconnected` flag in `GatewayState` prevents the TCP probe from re-detecting the remote gateway after explicit disconnect, ensuring the Connect button remains enabled.

### TLS Dependency

The desktop crate uses `tokio-tungstenite` with the `rustls-tls-webpki-roots` feature for TLS WebSocket connections. Local profiles always use `ws://`.

### TCP Probe

The remote gateway TCP probe uses `ToSocketAddrs::to_socket_addrs()` for DNS resolution so hostnames like `localhost` work alongside numeric IP addresses. The probe extracts host and port from the remote URL (defaulting to port 80 for `ws://` and 443 for `wss://` when no port is specified).

## Phase 2 Implementation Notes

Phase 2 is implemented. Key implementation details:

### Config Schema

The `GatewayConfig` struct in `crates/lib/src/config.rs` gains two fields:

```rust
pub struct GatewayConfig {
    pub port: u16,
    pub bind: String,
    pub auth: GatewayAuthConfig,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    #[serde(default)]
    pub max_connections: Option<usize>,
}
```

- `allowed_origins` defaults to an empty `Vec` (reject all browser origins on non-loopback).
- `max_connections` is `Option<usize>`: `None` = use bind-based default (unlimited for loopback, 1 for non-loopback); `Some(0)` = explicit opt-out (unlimited); `Some(n)` = cap at n.
- `effective_max_connections(bind, config)` resolves the effective limit at runtime.

### ConnectionTracker

The `ConnectionTracker` in `crates/lib/src/gateway/server.rs` is an `Arc`-wrapped struct with internal `Mutex` that tracks authenticated connections keyed by **client identity** (device ID). Each client identity maps to a `Vec` of connection entries, each holding a `oneshot::Sender<()>` kick signal. The tracker:

- `register(client_key, conn_id)`: called after successful auth. If the client already exists, the connection is added to its list (no kick, no limit check). If the client is new and the distinct-client limit is exceeded, all connections of the oldest client are kicked (oneshot signals sent). The new client is then registered.
- `unregister(conn_id)`: called when a socket disconnects. Removes the specific connection from its client's list; if the list becomes empty, the client slot is freed.

The kick signal is received in `handle_socket`'s `select!` loop, which sends a WebSocket close frame (code 1013, reason "connection limit reached: displaced by newer connection") and breaks.

### Origin Validation

The `ws_handler` in `crates/lib/src/gateway/server.rs` extracts `HeaderMap` from the request. On non-loopback bindings, if an `Origin` header is present, it is checked against `allowed_origins`. Non-matching origins receive HTTP 403 ("origin not allowed"). Absent `Origin` header (non-browser client like the desktop app) is allowed. Loopback bindings skip origin validation entirely.

### Session Events Listener Cancellation

The desktop's `GatewayState` gains a `session_events_cancel: Option<Arc<AtomicBool>>` field. The `stop_session_events_listener()` helper on `ChaiApp` sets the flag and drops the receiver. This is called from:

- `clear_session_and_messages` (used by both `disconnect_remote_profile` and local gateway stop)
- Profile-switch teardown path
- `ensure_session_events_listener` when `running` is false

The listener thread checks the flag at the top of each retry-loop iteration and exits cleanly with a log message ("session events listener cancelled, exiting").

### Status Payload

The gateway `status` WebSocket response now includes `gateway.maxConnections` — the effective limit (`null` for unlimited, integer for a cap). This is documented in [GATEWAY_STATUS.md](../spec/GATEWAY_STATUS.md).

## Phases

### Phase 1: Remote Profile Configuration and Connection

Core split deployment support on the client side. After this phase, a desktop can connect to a remote gateway over `ws://` or `wss://`.

| Deliverable | Detail |
|-------------|--------|
| `remote` array in `desktop.json` | Schema, parsing, validation (including full path support in `url`, collision detection at load time) |
| Remote profile directories | `mkdir -p` at startup for each remote entry that does not exist on disk |
| ComboBox integration | Remote `id`s appear alongside local profile names; local/remote distinction via lookup |
| Connect/Disconnect mode | Header shows Connect/Disconnect for remote profiles; no local gateway spawn; disconnect-before-switch for remote |
| Device identity for remote profiles | Load/create `device.json` and `device_token` under `~/.chai/profiles/<remote-id>/`; use remote entry `token` for pairing |
| WebSocket URL from remote entry | Use `url` field instead of `bind:port`; pass full URL (including path) to WebSocket client |
| `wss://` client support | TLS WebSocket connection when `url` specifies `wss://`; add `tokio-tungstenite` with TLS feature to desktop crate |
| TCP probe uses remote URL | Probe remote host:port extracted from `url` instead of `bind:port` |
| Profile detection skips `gateway.lock` for remote | No lock file check; rely on TCP probe |

**Status:** Implemented.

### Phase 2: Gateway Security Hardening

Server-side security measures for non-loopback deployments. After this phase, split deployment is secure by default.

| Deliverable | Detail |
|-------------|--------|
| WebSocket origin validation | `gateway.allowedOrigins` field in `config.json`; default empty (reject all browser origins on non-loopback); desktop unaffected (no `Origin` header) |
| Connection limit | `gateway.maxConnections` field in `config.json`; default `1` for non-loopback, unlimited for loopback; identity-based tracking (1:N multi-connection-per-client); kick-oldest when distinct-client limit exceeded; descriptive close frame (code 1013) |
| Session events listener cancellation | `Arc<AtomicBool>` cancel flag on desktop `GatewayState`; listener thread exits cleanly on disconnect, gateway stop, or profile switch instead of reconnecting indefinitely |

**Status:** Implemented and manually tested. Unit tests: 10 config tests, 10 `ConnectionTracker` tests. Manual tests verified origin validation (reject/allow), connection limit (kick-oldest across distinct clients, no intra-client churn), `maxConnections: 0` (unlimited), `maxConnections: 3` (allows 3 distinct clients), status payload includes `maxConnections`, and session events listener cancellation (disconnect, local gateway stop).

### Phase 3: Documentation and User Experience

Making split deployment a documented, supported scenario.

| Deliverable | Detail |
|-------------|--------|
| Config screen hidden for remote profiles | Sidebar omits Config screen when remote profile is selected |
| User journey documentation | Step-by-step guide for split deployment in `docs/` |
| Reverse proxy setup guide | nginx, Caddy, and Traefik configurations with WSS→WS termination, TLS certificate provisioning, WebSocket proxy configuration, and header forwarding |
| `SECURITY.md` updates | Move origin validation and connection limit from "Out of Scope" to implemented; add `wss://` client support as partially implemented (client-side only); document `maxConnections` and `allowedOrigins` defaults and their security rationale |

**Test checkpoint:** A new user can follow the user journey documentation to set up a split deployment end-to-end, including TLS via a reverse proxy. The Config screen is hidden when a remote profile is selected.

## Follow-ups

### Reverse Proxy Documentation

A step-by-step guide for setting up common reverse proxies (nginx, Caddy, Traefik) with WSS→WS termination in front of the gateway. Includes TLS certificate provisioning (Let's Encrypt), WebSocket proxy configuration, path stripping for non-root deployments, and header forwarding. This is included in Phase 3.

### Remote Gateway Status Reporting

The `/status` endpoint previously returned server-local absolute paths (`discoveryRoot`, `contextDirectory`), which were meaningless for remote clients. These fields have been removed from the status payload. No further normalization is needed for this concern.

### Per-Profile Gateway Lock (Multi-Gateway Switching) — Implemented

Per-profile gateway locks have been implemented. The lock lives at `~/.chai/profiles/<name>/gateway.lock` (one per profile) instead of `~/.chai/gateway.lock` (installation root). Multiple gateways can now run simultaneously on different profiles, each holding its own independent lock. The desktop stores per-profile gateway state (`GatewayState`) in a `HashMap<String, GatewayState>` keyed by profile name, enabling the user to switch between running gateways without stop/restart.

The desktop's profile ComboBox is always enabled — profile switching is always allowed regardless of whether any gateway is running. When a gateway is running on a different profile than the active one, an amber label indicates which profile the gateway is using. The desktop discovers all running profiles via `find_running_gateway_profiles()` (scanning per-profile lock files).

This eliminates the disconnect/stop-before-switch constraint for local profiles described in earlier design sections. The only remaining switching constraint is for remote profiles (connection-based: the desktop must swap its WebSocket connection and cached state). See the design section on active symlink and switching for the updated distinction.

### Multi-Client Deployments

The connection policy (`gateway.maxConnections`) defaults to `1` for non-loopback, making single-client the secure default for split deployment. Operators who need multiple simultaneous clients (e.g., team access to a shared gateway) can increase `maxConnections`. In multi-client deployments, all clients within the connection limit share the same access — there is no per-client session isolation. Per-client scoping is a broader access-control concern tracked in [SECURITY.md](../SECURITY.md) under "Session isolation across channels."

## Related Docs

- [SECURITY.md](../SECURITY.md) — Known vulnerabilities and out-of-scope items (TLS, origin validation, session isolation)
- [DESKTOP.md](../spec/DESKTOP.md) — Desktop application spec (spawn vs. attach modes, `desktop.json` schema)
- [CONFIGURATION.md](../spec/CONFIGURATION.md) — Configuration schema (gateway block, auth, env overrides)
- [PROFILES.md](../spec/PROFILES.md) — Profile directory structure (device.json, device_token, paired.json, active symlink)
- [CHANNELS.md](../spec/CHANNELS.md) — Channel behavior (channels live inside the gateway process)
- [GATEWAY_STATUS.md](../spec/GATEWAY_STATUS.md) — Gateway status payload (server-side absolute paths)
- [SESSIONS.md](../spec/SESSIONS.md) — Session persistence, storage layout, and management (session management is gateway-side)

**Implementation touchpoints:** `crates/lib/src/config.rs` (`DesktopConfig`, `GatewayConfig`), `crates/lib/src/profile.rs` (`ChaiPaths`, `resolve_profile_dir`), `crates/lib/src/device.rs` (`DeviceIdentity`), `crates/lib/src/gateway/server.rs` (origin validation, connection limit), `crates/desktop/src/app.rs` (state, start/stop → connect/disconnect), `crates/desktop/src/app/state/gateway.rs` (WebSocket URL construction, `build_connect_params`), `crates/desktop/src/app/ui/header.rs` (ComboBox, button labels), `crates/desktop/src/app/screens/config.rs` (sidebar visibility)
