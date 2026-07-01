---
status: draft
---

# Epic: Split Deployment

**Summary** — Enable a hosted-gateway deployment model where a host runs `chai gateway` on a remote server and a client connects to it using `chai-desktop` on a separate machine. The desktop app assumes the gateway is a local subprocess; this epic adds remote gateway support to the desktop so a client can connect to a remote gateway securely, with no option to spawn a local process.

**Prerequisite** — The `desktop.json` file (appearance and logs blocks) is already implemented. This epic adds a `remote` array to `desktop.json` so the desktop can connect to remote gateways, with the active profile symlink pointing at the selected target (local or remote) so device identity storage is reused.

**Status** — **Draft (not implemented).** The desktop can attach to an externally owned gateway over the network via TCP probing and the WebSocket challenge-response protocol, but there is no explicit support for split deployment: no remote address configuration, no TLS, no Connect/Disconnect mode, and no documentation for the scenario.

## Problem Statement

Chai's desktop app is designed around a single-machine model: it spawns `chai gateway` as a local child process, connects to it on loopback, and manages its lifecycle. A developer who wants to host a gateway for a client — running the gateway on a remote server while the client uses the desktop app locally — encounters several obstacles:

- **No way to point the desktop at a remote gateway.** The desktop derives the WebSocket URL from `gateway.bind`, a field that semantically means "address to bind the server to." Setting it to a remote IP on the client machine is a semantic misuse and causes confusion if the client accidentally starts a local gateway.
- **No TLS.** The gateway binds plain HTTP/WebSocket. The desktop hardcodes `ws://` URLs. Auth tokens, device tokens, conversation content, and tool outputs all travel in cleartext over the network.
- **No Connect/Disconnect mode.** The desktop shows a "Start gateway" button even when configured for a remote gateway. Pressing it would spawn a conflicting local process.
- **No origin validation.** The gateway does not check the `Origin` header on WebSocket upgrades, enabling cross-site WebSocket hijacking on non-loopback deployments.
- **No documentation.** Zero guidance exists for setting up, securing, or operating a split deployment.

The underlying protocol plumbing (device pairing, challenge-response auth, token issuance, log streaming) already works over the network. What's missing is the configuration, security, and UX layer to make this a first-class scenario.

## Goal

A developer can deploy `chai gateway` on a remote server, configure a client's desktop app to connect to it securely, and have the desktop operate in a connect-only mode with no option to spawn a local gateway. The connection is protected by TLS (or a documented reverse-proxy path), the client authenticates via the existing device pairing protocol, and the configuration clearly distinguishes between server-side and client-side concerns.

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

The header shows the persistent active profile (from `~/.chai/active`). A ComboBox rewrites the symlink when the gateway is **not** running. Profile switching is disabled while the gateway is up (enforced by the advisory lock on `~/.chai/gateway.lock` — see [PROFILES.md](../spec/PROFILES.md)).

This epic extends the ComboBox to include remote profiles alongside local profiles. The switching constraint applies to both, but for different reasons: local profiles are blocked by the advisory lock (a server-side gateway is running), while remote profiles are blocked by the need to swap the active WebSocket connection and cached state. In both cases the UX is the same — disconnect/stop before switching.

### Currently Implemented `desktop.json` Schema

The `desktop.json` file at the chai home root is already implemented with `appearance` and `logs` blocks (see [DESKTOP.md](../spec/DESKTOP.md)). `desktop.json` is loaded once at startup. This epic adds a `remote` block to the schema.

### `~/.chai` Directory Split

In a split deployment, the two `~/.chai` directories serve different purposes:

**Remote server (developer's machine):**
```
~/.chai/
├── active → profiles/assistant/
├── gateway.lock              ← PID + profile name
├── profiles/assistant/
│   ├── config.json           ← authoritative: providers, channels, agents, auth
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

### Security Posture

Per [SECURITY.md](../SECURITY.md), the following are explicitly out of scope:

- **TLS termination** — "The gateway binds plain HTTP/WebSocket. TLS is the operator's responsibility (e.g., reverse proxy). Binding to non-loopback without TLS exposes the auth token and all data in cleartext."
- **WebSocket origin validation** — "The gateway does not check the `Origin` header on WebSocket upgrades. On loopback this is mitigated by same-origin policy; on non-loopback deployments, cross-site WebSocket hijacking is possible without additional network controls."
- **Session isolation across channels** — "No per-client or per-channel session access control; authenticated WebSocket clients can interact with any session."

The gateway does enforce token auth for non-loopback bindings — it refuses to start without `auth.mode: "token"` when bound to a non-loopback address.

### Existing Gaps

| Gap | Severity | Description |
|-----|----------|-------------|
| No TLS/WSS | 🔴 Critical | All data (tokens, messages, tool outputs) sent in cleartext over the network |
| No remote address config | 🟠 High | `gateway.bind` repurposed as connect-to address; semantically wrong and confusing |
| No Connect/Disconnect mode | 🟠 High | Desktop can still spawn a local gateway; no way to disable this |
| No origin validation | 🟡 Medium | Cross-site WebSocket hijacking possible on non-loopback |
| `gateway.lock` is local-only | 🟡 Medium | Desktop can't detect remote gateway via lock file; relies on TCP probe |
| No documentation | 🟡 Medium | Zero guidance for split deployment setup or operation |
| Status shows server paths | 🟢 Low | Gateway status returns server-local absolute paths; confusing but not breaking |

## Scope

### In Scope

- A `remote` array in `desktop.json` that lets the desktop connect to remote gateways. Each entry has an `id` (used as the profile name and ComboBox label), a `url` (the WebSocket connection URL), and a `token` (the gateway auth token for pairing). Local profiles and remote entries appear alongside each other in the ComboBox.
- `wss://` URL construction in the desktop client for connections to TLS-terminated gateways.
- A Connect/Disconnect mode for remote profiles: when the selected profile is remote, the desktop shows Connect/Disconnect instead of Start/Stop, never spawns a local gateway, and probes the remote URL for liveness.
- WebSocket origin validation on the gateway for non-loopback connections.
- Documentation and a user journey for the split deployment scenario.
- Updates to `SECURITY.md` to reflect the new capabilities.

### Out of Scope

- **Built-in TLS termination in the gateway** — TLS termination remains the operator's responsibility (reverse proxy). Tracked as a potential future direction in [SECURITY.md](../SECURITY.md).
- **Per-client session isolation** — Authenticated clients can still interact with any session. This is a broader access-control concern tracked separately in [SECURITY.md](../SECURITY.md).
- **Rate limiting** — No limit on concurrent connections or message rates. Tracked as out of scope in [SECURITY.md](../SECURITY.md).
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
| `id` | `string` | Yes | Profile name and ComboBox label. Also determines the profile directory under `~/.chai/profiles/` where device identity is stored. Must not collide with existing local profile directory names. |
| `url` | `string` | Yes | WebSocket connection URL. Must start with `ws://` or `wss://`. May include a path (e.g., `wss://example.com/chai/ws`). |
| `token` | `string` | Yes | Gateway auth token for device pairing. Sent in the `auth.token` field of the connection payload. |

This approach unifies local and remote profiles in the ComboBox: the user sees the same dropdown regardless of whether profiles are local or remote. Selecting a local profile means "spawn or attach locally" (Start/Stop). Selecting a remote profile means "connect to a remote gateway" (Connect/Disconnect). The mode is determined by the type of the selected profile, not by a global flag.

**Why an array instead of a single URL:** A developer may host staging and production gateways, or multiple gateways for different clients. The array makes multiple remote gateways first-class. A minimal client setup has a single entry; a `desktop.json` with no `remote` block is unchanged from current behavior.

**Why `id` doubles as the profile name:** Device identity (`device.json`, `device_token`) is stored per-profile under `~/.chai/profiles/<id>/`. Using the `id` as the profile directory name means:

- No collisions between local and remote profiles (the ComboBox lists directory names from `~/.chai/profiles/`, which includes both).
- The ComboBox is populated from a single source: profile directories on disk. Remote entries must have their profile directories created at desktop startup so they appear in the ComboBox — without this, the user would have no way to select a remote entry to connect for the first time.
- Device identity storage is reused from the existing architecture — `ChaiPaths` already resolves `device.json()`, `device_token_path()`, and `paired_json()` from `profile_dir`. The `device.json` and `device_token` files are created on first connect (by `build_connect_params`), but the directory itself must already exist so the ComboBox can list it.

**Startup directory creation:** When `desktop.json` is loaded at startup, the desktop iterates the `remote` array and creates `~/.chai/profiles/<id>/` for each entry that does not already exist. This is a `mkdir -p` operation — no files are written, just the directory. The directory is empty until the user selects the remote profile and clicks Connect, at which point `device.json` is generated and `device_token` is issued by the gateway.

**Active symlink and profile switching:** The active symlink is updated when selecting a remote profile, same as for local profiles. This is consistent: the symlink always points at whichever profile directory the desktop is using, and `ChaiPaths` resolves device identity from there. Switching profiles requires disconnecting/stopping the gateway first.

There are two distinct reasons for this constraint, and they apply differently to local and remote profiles:

- **Local profiles** — The advisory lock on `~/.chai/gateway.lock` prevents `switch_active_profile()` from rewriting the symlink while a gateway holds the lock. This is a hard server-side constraint: a second `chai gateway` invocation on a different profile is refused until the first stops.
- **Remote profiles** — There is no local lock involved. The constraint is connection-based: the desktop's WebSocket connection, cached status, session lists, and agent details are all tied to the current remote gateway. Switching to a different remote entry (or a local profile) means abandoning that connection and re-establishing a new one. The desktop enforces disconnect-before-switch to manage this state swap cleanly.

Under the current root-level lock, both constraints produce the same UX (stop/disconnect before switching). A future per-profile lock direction (see Follow-ups) would relax the local lock constraint but the remote connection-based constraint would remain — the desktop would still need to swap its WebSocket connection when switching between remote gateways.

### Distinguishing Local and Remote Profiles

The desktop needs to know whether the currently selected profile is local or remote to decide which button label to show (Start/Stop vs Connect/Disconnect) and whether to spawn a local process.

**Decision:** The desktop looks up the selected profile `id` in the `remote` array from `desktop.json`. If the `id` matches a remote entry, the profile is remote. If it does not match, the profile is local. This is a simple lookup against the loaded `desktop.json` `remote` array — no separate marker is needed in the profile directory itself.

### TLS and `wss://` Support

The gateway will not gain built-in TLS termination — this remains the operator's responsibility (reverse proxy with WSS→WS termination). However, the desktop client must support `wss://` URL construction when a remote entry's `url` specifies it.

**Decision:** When a remote entry's `url` starts with `wss://`, the desktop constructs a TLS WebSocket connection. When it starts with `ws://`, it uses plain WebSocket (current behavior for local profiles). Local profiles always use `ws://` (derived from `bind:port`, loopback doesn't need TLS).

This requires adding a TLS-enabled WebSocket client to the desktop crate's dependencies (e.g., `tokio-tungstenite` with the `native-tls` or `rustls` feature).

### Connect/Disconnect Mode

When the selected profile is remote, the desktop operates in connect-only mode:

- The header shows **Connect/Disconnect** controls instead of Start/Stop.
- The desktop does not attempt to spawn `chai gateway` as a subprocess.
- The desktop probes the remote URL for liveness (TCP connect to the host:port extracted from the URL).
- Profile switching is disabled while connected (the desktop must disconnect before selecting a different profile — see Active Symlink and Switching below).
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

**Recommendation:** Approach A with a default of `["*"]` when `auth.mode: "token"` is set (token auth already gates access). When `auth.mode: "none"` on non-loopback (which is already refused at startup), this is moot. For stricter deployments, operators can set `allowedOrigins` to specific domains. This is a defense-in-depth measure, not the primary security boundary.

### `gateway.lock` and Remote Gateway Detection

In split deployment, the desktop's local `gateway.lock` doesn't exist (the gateway is on a different machine). The desktop currently uses `gateway.lock` to detect a running gateway and determine its profile.

**Decision:** When the selected profile is remote, the desktop skips `gateway.lock` detection entirely and relies on the TCP probe against the remote URL. The profile is determined from the selected remote entry `id` (not from `gateway.lock` or `config.json`). The remote gateway's own profile name (returned in `status`) may differ from the local `id` — this is not a mismatch; the `id` is the client-side label, and the remote gateway's profile name is a server-side detail that is not surfaced as a warning.

**Future direction — per-profile `gateway.lock`:** The current lock lives at `~/.chai/gateway.lock` (installation root) and enforces one gateway per Chai installation. A future direction moves the lock to `~/.chai/profiles/<name>/gateway.lock`, allowing multiple gateways to run on different profiles simultaneously. This would let the desktop switch between running local gateways without stop/restart. This epic does not implement per-profile locks, but its design is compatible with them: remote profiles already skip lock detection, startup directory creation does not create lock files, and device identity resolution from `ChaiPaths` works regardless of lock placement. See Follow-ups for details.

### Config Screen Visibility

The desktop config screen currently shows `config.json` contents (bind, port, providers, agents, channels). For a remote-only client, there is no `config.json` — server-side configuration is managed server-side.

**Decision:** When the selected profile is remote, the Config screen is hidden from the sidebar. The Gateway screen (which shows `status` from the remote gateway) is the source of truth for the remote gateway's effective configuration. The Config screen reappears when a local profile is selected.

## Requirements

### Functional

- [ ] **`remote` array in `desktop.json`** — Add a `remote` array to the `DesktopConfig` struct in `crates/lib/src/config.rs`. Each entry has `id` (string), `url` (string, must start with `ws://` or `wss://`), and `token` (string). Invalid entries are rejected at load time. When `desktop.json` is absent or has no `remote` block, all existing behavior is unchanged.
- [ ] **Remote profile directories created at startup** — When `desktop.json` is loaded at startup, the desktop creates `~/.chai/profiles/<id>/` for each remote entry that does not already exist (`mkdir -p`, no files written). This ensures remote entries appear in the ComboBox before the user has ever connected.
- [ ] **Remote profile in ComboBox** — Remote entry `id`s appear alongside local profile names in the header ComboBox. The ComboBox is populated from `~/.chai/profiles/` directory names. Selecting a remote profile updates the active symlink to point at the remote profile's directory, same as selecting a local profile.
- [ ] **Connect/Disconnect mode** — When the selected profile is remote (the `id` matches a `remote` entry in `desktop.json`), the header shows Connect/Disconnect instead of Start/Stop. The desktop does not spawn a local gateway. Clicking Connect opens a WebSocket connection to the remote `url`. Clicking Disconnect closes it. Profile switching is disabled while connected (same lock rule as local).
- [ ] **Device identity for remote profiles** — When connecting to a remote profile, the desktop loads/creates `device.json` and `device_token` under `~/.chai/profiles/<remote-id>/` (the directory already exists from startup creation). The `token` from the remote entry is used for the pairing protocol instead of `config.json` `gateway.auth.token`.
- [ ] **WebSocket URL from remote entry** — When the selected profile is remote, the desktop uses the `url` from the remote entry for the WebSocket connection and TCP probe instead of deriving from `config.json` `gateway.bind:port`.
- [ ] **`wss://` support in desktop** — The desktop client can establish TLS WebSocket connections when a remote entry's `url` specifies `wss://`. Local profiles always use `ws://` (derived from `bind:port`).
- [ ] **WebSocket origin validation** — The gateway validates the `Origin` header on WebSocket upgrades for non-loopback connections. Uses Approach A: an `allowedOrigins` list in `gateway` config rejects upgrades whose `Origin` doesn't match. Defaults to `["*"]` when `auth.mode: "token"` is set (token auth already gates access; this is defense-in-depth). When `auth.mode: "none"` on non-loopback (already refused at startup), this is moot. Operators can restrict to specific domains for stricter deployments.
- [ ] **Config screen hidden for remote profiles** — When the selected profile is remote, the Config screen does not appear in the sidebar. The Gateway screen (showing `status` from the remote gateway) is the sole source of configuration visibility.
- [ ] **TCP probe uses remote URL** — When the selected profile is remote, the desktop probes the remote gateway host:port (extracted from the `url`) for liveness instead of `bind:port`.
- [ ] **Profile detection skips `gateway.lock` for remote** — When the selected profile is remote, the desktop does not check for `gateway.lock` and relies on the TCP probe. The remote gateway's profile name (from `status`) may differ from the local `id`; this is not surfaced as a mismatch warning.

### Non-functional

- [x] **Backward compatibility** — When `desktop.json` is absent or has no `remote` block, all existing behavior is unchanged. The desktop continues to derive the WebSocket URL from `bind:port`, show Start/Stop controls, and operate in spawn-or-attach mode.
- [ ] **No new required dependencies for gateway** — TLS support is client-side only (desktop crate). The gateway does not gain TLS dependencies.
- [x] **Config validation** — Remote entry `url` must start with `ws://` or `wss://`. `id` must be non-empty and must not collide with existing local profile directory names (enforced at load time). Invalid entries are rejected.
- [ ] **Security documentation updated** — `SECURITY.md` updated to reflect origin validation and `wss://` client support, moving those items from "Out of Scope" to implemented or partially implemented.

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | `remote` array in `desktop.json` + startup directory creation + ComboBox integration + Connect/Disconnect mode + device identity for remote profiles | Not started |
| 2 | `wss://` support in desktop client | Not started |
| 3 | WebSocket origin validation | Not started |
| 4 | Documentation, user journey, config screen visibility, and `SECURITY.md` updates | Not started |

## Open Questions

- **Should `url` support full paths for reverse proxies?** E.g., `wss://example.com/chai/ws` where the reverse proxy routes `/chai/` to the gateway. The desktop would send the full path in the WebSocket upgrade request. The current gateway WebSocket handler matches `/ws` at the root — a path-based reverse proxy would need to strip the prefix or the gateway would need to handle non-root paths.

- **Should `allowedOrigins` default to `["*"]` when token auth is enabled, or should it be explicitly empty (block all browser origins)?** A permissive default reduces friction but weakens defense-in-depth. A restrictive default is safer but may surprise operators who expect browser-based tools to work.

- **What happens when a local profile and a remote entry share the same `id`?** This is rejected at load time (the `id` must not collide with existing profile directories). But if a local profile is created *after* `desktop.json` is loaded (e.g., via `chai init`), the collision is not detected until the next desktop restart. Should the desktop re-scan for collisions at runtime?

## Follow-ups

### Reverse Proxy Documentation

A step-by-step guide for setting up common reverse proxies (nginx, Caddy, Traefik) with WSS→WS termination in front of the gateway. Includes TLS certificate provisioning (Let's Encrypt), WebSocket proxy configuration, and header forwarding.

### Remote Gateway Status Reporting

The `/status` endpoint previously returned server-local absolute paths (`discoveryRoot`, `contextDirectory`), which were meaningless for remote clients. These fields have been removed from the status payload. No further normalization is needed for this concern.

### Per-Profile Gateway Lock (Multi-Gateway Switching)

The current `gateway.lock` lives at `~/.chai/gateway.lock` (the installation root) and enforces a single gateway per Chai installation. A future direction is to move the lock to `~/.chai/profiles/<name>/gateway.lock`, allowing multiple gateways to run simultaneously on different profiles. This would enable the desktop to switch between running gateways (local or remote) by changing the active symlink and swapping the WebSocket connection — no stop/restart needed for local gateways.

This epic does not implement per-profile locks, but it avoids design decisions that would block them:

- Remote profiles already skip `gateway.lock` detection entirely (TCP probe only).
- Startup directory creation does not create lock files — the profile directory is empty until first connect.
- The ComboBox is populated from profile directories, not from a central registry — adding per-profile locks would not change this.
- Device identity is resolved from the profile directory via `ChaiPaths`, which works the same whether the lock is at the root or per-profile.

The one assumption this epic *does* carry forward is that switching the active symlink requires disconnecting/stopping first. This is correct today because the root-level lock prevents symlink rewriting while any gateway runs. Under per-profile locks, this prevention goes away — the symlink can be freely rewritten, and the desktop's constraint becomes purely about swapping its WebSocket connection and cached state. See the design section on active symlink and switching for the distinction.

### Multi-Client Observability

When multiple desktop clients connect to a single remote gateway, there is no per-client log filtering or session visibility. Each client sees the full gateway log stream and all sessions. Per-client scoping is a broader access-control concern related to session isolation (tracked in [SECURITY.md](../SECURITY.md)).

## Related Docs

- [SECURITY.md](../SECURITY.md) — Known vulnerabilities and out-of-scope items (TLS, origin validation, session isolation)
- [DESKTOP.md](../spec/DESKTOP.md) — Desktop application spec (spawn vs. attach modes, `desktop.json` schema)
- [CONFIGURATION.md](../spec/CONFIGURATION.md) — Configuration schema (gateway block, auth, env overrides)
- [PROFILES.md](../spec/PROFILES.md) — Profile directory structure (device.json, device_token, paired.json, active symlink)
- [CHANNELS.md](../spec/CHANNELS.md) — Channel behavior (channels live inside the gateway process)
- [GATEWAY_STATUS.md](../spec/GATEWAY_STATUS.md) — Gateway status payload (server-side absolute paths)
- [SESSIONS.md](../spec/SESSIONS.md) — Session persistence, storage layout, and management (session management is gateway-side)

**Implementation touchpoints:** `crates/lib/src/config.rs` (`DesktopConfig`, `GatewayConfig`), `crates/lib/src/profile.rs` (`ChaiPaths`, `resolve_profile_dir`), `crates/lib/src/device.rs` (`DeviceIdentity`), `crates/lib/src/gateway/server.rs` (origin validation), `crates/desktop/src/app.rs` (state, start/stop → connect/disconnect), `crates/desktop/src/app/state/gateway.rs` (WebSocket URL construction, `build_connect_params`), `crates/desktop/src/app/ui/header.rs` (ComboBox, button labels), `crates/desktop/src/app/screens/config.rs` (sidebar visibility)
