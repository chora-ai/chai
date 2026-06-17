---
status: draft
---

# Epic: Split Deployment

**Summary** — Enable a hosted-gateway deployment model where a host runs `chai gateway` on a remote server and a client connects to it using `chai-desktop` on a separate machine. Today, the desktop assumes the gateway is a local subprocess; while an "attach" mode exists, it lacks a dedicated connection address, TLS support, attach-only configuration, and origin validation — making remote deployment fragile, insecure, and undocumented.

**Status** — **Draft (not implemented).** The desktop can attach to an externally owned gateway over the network via TCP probing and the WebSocket challenge-response protocol, but there is no explicit support for split deployment: no remote address configuration, no TLS, no attach-only mode, and no documentation for the scenario.

## Problem Statement

Chai's desktop app is designed around a single-machine model: it spawns `chai gateway` as a local child process, connects to it on loopback, and manages its lifecycle. A developer who wants to host a gateway for a client — running the gateway on a remote server while the client uses the desktop app locally — encounters several obstacles:

- **No way to point the desktop at a remote gateway.** The desktop derives the WebSocket URL from `gateway.bind`, a field that semantically means "address to bind the server to." Setting it to a remote IP on the client machine is a semantic misuse and causes confusion if the client accidentally starts a local gateway.
- **No TLS.** The gateway binds plain HTTP/WebSocket. The desktop hardcodes `ws://` URLs. Auth tokens, device tokens, conversation content, and tool outputs all travel in cleartext over the network.
- **No attach-only mode.** The desktop shows a "Start gateway" button even when configured for a remote gateway. Pressing it would spawn a conflicting local process.
- **No origin validation.** The gateway does not check the `Origin` header on WebSocket upgrades, enabling cross-site WebSocket hijacking on non-loopback deployments.
- **No documentation.** Zero guidance exists for setting up, securing, or operating a split deployment.

The underlying protocol plumbing (device pairing, challenge-response auth, token issuance, log streaming) already works over the network. What's missing is the configuration, security, and UX layer to make this a first-class scenario.

## Goal

A developer can deploy `chai gateway` on a remote server, configure a client's desktop app to connect to it securely, and have the desktop operate in an attach-only mode with no option to spawn a local gateway. The connection is protected by TLS (or a documented reverse-proxy path), the client authenticates via the existing device pairing protocol, and the configuration clearly distinguishes between server-side and client-side concerns.

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

**Client's machine:**
```
~/.chai/
├── active → profiles/assistant/
├── profiles/assistant/
│   ├── config.json           ← connection stub: remote address, auth token
│   ├── device.json           ← Ed25519 keypair (client identity)
│   └── device_token          ← session token from gateway
└── skills/                   ← unused in remote mode (gateway owns skills)
```

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
| No attach-only mode | 🟠 High | Desktop can still spawn a local gateway; no way to disable this |
| No origin validation | 🟡 Medium | Cross-site WebSocket hijacking possible on non-loopback |
| `gateway.lock` is local-only | 🟡 Medium | Desktop can't detect remote gateway via lock file; relies on TCP probe |
| No documentation | 🟡 Medium | Zero guidance for split deployment setup or operation |
| Status shows server paths | 🟢 Low | Gateway status returns server-local absolute paths; confusing but not breaking |

## Scope

### In Scope

- A dedicated `gateway.connectUrl` configuration field (or equivalent mechanism) that separates the client's connection target from the server's bind address.
- `wss://` URL construction in the desktop client for connections to TLS-terminated gateways.
- An "attach-only" mode that prevents the desktop from spawning a local gateway process.
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

## Design

### Remote Gateway Address

The desktop currently derives the WebSocket URL from `gateway.bind` + `gateway.port`, a field that semantically means "address the server binds to." For split deployment, this creates a semantic conflict: the client sets `gateway.bind` to the remote server's IP, but if they accidentally run `chai gateway` locally, it tries to bind to that remote IP on the local machine.

**Approach A: New `gateway.connectUrl` field**

Add a top-level `gateway.connectUrl` field to `config.json` that takes precedence over `gateway.bind:gateway.port` when the desktop constructs the WebSocket URL:

```json
{
  "gateway": {
    "bind": "0.0.0.0",
    "port": 15151,
    "connectUrl": "wss://gateway.example.com:15151"
  }
}
```

- Pros: Clean separation of server-side bind vs. client-side connect; supports `wss://` directly; explicit and unambiguous.
- Cons: Adds a new config field; desktop and server configs diverge further.

**Approach B: Environment variable `CHAI_GATEWAY_URL`**

Add a `CHAI_GATEWAY_URL` environment variable that overrides the WebSocket URL construction:

```bash
export CHAI_GATEWAY_URL=wss://gateway.example.com:15151
```

- Pros: No config schema change; easy to set per-session; useful for CI/testing.
- Cons: Not persisted in config; invisible in the desktop config screen; doesn't solve the semantic bind issue.

**Approach C: Both — `gateway.connectUrl` in config + `CHAI_GATEWAY_URL` as override**

Combine both: add `gateway.connectUrl` to the schema for persisted configuration, and support `CHAI_GATEWAY_URL` as a runtime override (same pattern as `CHAI_PROFILE` overriding the active profile).

- Pros: Persistent config + runtime override; consistent with existing patterns; supports both interactive and scripted use.
- Cons: Two new mechanisms to document and test.

**Recommendation:** Approach C. The `gateway.connectUrl` field provides a clear, persisted way to configure the remote gateway address, and `CHAI_GATEWAY_URL` follows the existing env-var-override pattern for ad-hoc use. When `connectUrl` (or the env var) is set, the desktop uses it for the WebSocket URL and TCP probe instead of deriving from `bind:port`. The `bind` and `port` fields remain server-side only.

### TLS and `wss://` Support

The gateway will not gain built-in TLS termination — this remains the operator's responsibility (reverse proxy with WSS→WS termination). However, the desktop client must support `wss://` URL construction when `connectUrl` specifies it.

**Decision:** When `gateway.connectUrl` starts with `wss://`, the desktop constructs a TLS WebSocket connection. When it starts with `ws://`, it uses plain WebSocket (current behavior). The `gateway.bind:port` path always uses `ws://` (loopback doesn't need TLS).

This requires adding a TLS-enabled WebSocket client to the desktop crate's dependencies (e.g., `tokio-tungstenite` with the `native-tls` or `rustls` feature).

### Attach-Only Mode

When `gateway.connectUrl` (or `CHAI_GATEWAY_URL`) is set, the desktop operates in attach-only mode:

- The "Start gateway" button is hidden or replaced with a "Connect" indicator.
- The desktop does not attempt to spawn `chai gateway` as a subprocess.
- Profile switching is disabled (the gateway's profile is externally managed).
- The desktop still probes for gateway liveness but uses `connectUrl` as the target.

**Decision:** Attach-only mode is implicitly activated when `connectUrl` is present. No separate `gateway.mode` field is needed — the presence of a remote URL is the signal.

### WebSocket Origin Validation

For non-loopback bindings, the gateway should validate the `Origin` header on WebSocket upgrade requests. This prevents cross-site WebSocket hijacking from browser-based attackers.

**Approach A: Reject all non-loopback upgrades without a whitelisted origin**

The gateway maintains an `allowedOrigins` list in `config.json`. If the `Origin` header doesn't match any entry, the upgrade is rejected.

**Approach B: Require `Origin` header on non-loopback, reject only browser-like user agents**

Check for the presence of an `Origin` header (browser WebSocket APIs always send it). If present on a non-loopback connection, validate it against an allowlist. If absent (non-browser client like the desktop app), allow it through.

**Recommendation:** Approach A with a default of `["*"]` when `auth.mode: "token"` is set (token auth already gates access). When `auth.mode: "none"` on non-loopback (which is already refused at startup), this is moot. For stricter deployments, operators can set `allowedOrigins` to specific domains. This is a defense-in-depth measure, not the primary security boundary.

### `gateway.lock` and Remote Gateway Detection

In split deployment, the desktop's local `gateway.lock` doesn't exist (the gateway is on a different machine). The desktop currently uses `gateway.lock` to detect a running gateway and determine its profile.

**Decision:** When `connectUrl` is set, the desktop skips `gateway.lock` detection entirely and relies on the TCP probe against the remote address. The profile is determined from the local `config.json` (since the desktop's config is the connection stub). If the remote gateway's profile name differs, the desktop shows the amber hint (existing behavior for profile mismatches).

### Config Screen Updates

The desktop config screen currently shows "Bind" and "Port" from the local `config.json`. In split deployment, these values are confusing.

**Decision:** When `connectUrl` is set, the config screen shows "Remote gateway" with the connect URL instead of "Bind" / "Port". Auth mode and token are still shown (the client needs them for pairing).

## Requirements

### Functional

- [ ] **`gateway.connectUrl` config field** — A new optional field in `config.json` under the `gateway` block that specifies the WebSocket URL for the desktop to connect to. Takes precedence over `bind:port` for URL construction and TCP probes.
- [ ] **`CHAI_GATEWAY_URL` environment variable** — Runtime override for `gateway.connectUrl`. Follows the same precedence pattern as `CHAI_PROFILE`.
- [ ] **`wss://` support in desktop** — The desktop client can establish TLS WebSocket connections when `connectUrl` specifies `wss://`.
- [ ] **Attach-only mode** — When `connectUrl` or `CHAI_GATEWAY_URL` is set, the desktop hides the "Start gateway" button, does not attempt to spawn a local gateway subprocess, and disables profile switching.
- [ ] **WebSocket origin validation** — The gateway validates the `Origin` header on WebSocket upgrades for non-loopback connections, with a configurable `allowedOrigins` list in `gateway` config.
- [ ] **Config screen awareness** — The desktop config screen shows "Remote gateway: <url>" when `connectUrl` is set, instead of "Bind" / "Port".
- [ ] **TCP probe uses `connectUrl`** — When `connectUrl` is set, the desktop probes the remote gateway address for liveness instead of `bind:port`.
- [ ] **Profile detection skips `gateway.lock`** — When `connectUrl` is set, the desktop does not check for `gateway.lock` and relies on the TCP probe.

### Non-functional

- [ ] **Backward compatibility** — When `connectUrl` is not set, all existing behavior is unchanged. The desktop continues to derive the WebSocket URL from `bind:port` and operate in spawn-or-attach mode.
- [ ] **No new required dependencies for gateway** — TLS support is client-side only (desktop crate). The gateway does not gain TLS dependencies.
- [ ] **Config validation** — `connectUrl` must start with `ws://` or `wss://`. Invalid schemes are rejected at config load time.
- [ ] **Security documentation updated** — `SECURITY.md` updated to reflect origin validation and `wss://` client support, moving those items from "Out of Scope" to implemented or partially implemented.

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | `gateway.connectUrl` + `CHAI_GATEWAY_URL` + attach-only mode | Not started |
| 2 | `wss://` support in desktop client | Not started |
| 3 | WebSocket origin validation | Not started |
| 4 | Documentation, user journey, and config screen updates | Not started |

## Open Questions

- **Should `connectUrl` support HTTP-based gateways behind path-based reverse proxies?** E.g., `wss://example.com/chai/ws` where the reverse proxy routes `/chai/` to the gateway. This would require the desktop to send the full path in the WebSocket upgrade request. The current `ws://bind:port/ws` is always at the root. This could be addressed by allowing `connectUrl` to include a full path, but the gateway's WebSocket handler currently only matches `/ws` at the root.

- **Should the desktop's `config.json` require a minimal subset of fields when `connectUrl` is set?** Currently, the desktop loads the full config (providers, channels, agents) even though only `gateway.*` is relevant for a remote connection. A "thin" config format for remote clients could reduce confusion, but adds a new config variant to maintain.

- **Should `allowedOrigins` default to `["*"]` when token auth is enabled, or should it be explicitly empty (block all browser origins)?** A permissive default reduces friction but weakens defense-in-depth. A restrictive default is safer but may surprise operators who expect browser-based tools to work.

## Follow-ups

### Reverse Proxy Documentation

A step-by-step guide for setting up common reverse proxies (nginx, Caddy, Traefik) with WSS→WS termination in front of the gateway. Includes TLS certificate provisioning (Let's Encrypt), WebSocket proxy configuration, and header forwarding.

### Remote Gateway Status Reporting

The `/status` endpoint currently returns server-local absolute paths (`discoveryRoot`, `contextDirectory`). For remote clients, these paths are meaningless. A future enhancement could omit or normalize these fields when the request comes from a non-loopback connection, or add a `remote` flag to the status response.

### Multi-Client Observability

When multiple desktop clients connect to a single remote gateway, there is no per-client log filtering or session visibility. Each client sees the full gateway log stream and all sessions. Per-client scoping is a broader access-control concern related to session isolation (tracked in [SECURITY.md](../SECURITY.md)).

## Related Epics and Docs

- [SECURITY.md](../SECURITY.md) — Known vulnerabilities and out-of-scope items (TLS, origin validation, session isolation)
- [DESKTOP.md](../spec/DESKTOP.md) — Desktop application spec (spawn vs. attach modes)
- [CONFIGURATION.md](../spec/CONFIGURATION.md) — Configuration schema (gateway block, auth, env overrides)
- [PROFILES.md](../spec/PROFILES.md) — Profile directory structure (device.json, device_token, paired.json)
- [CHANNELS.md](../spec/CHANNELS.md) — Channel behavior (channels live inside the gateway process)
- [GATEWAY_STATUS.md](../spec/GATEWAY_STATUS.md) — Gateway status payload (server-side absolute paths)
- [PERSISTENT_SESSIONS.md](PERSISTENT_SESSIONS.md) — Persistent sessions epic (session management is gateway-side)

**Implementation touchpoints:** `crates/lib/src/config.rs`, `crates/lib/src/profile.rs`, `crates/lib/src/gateway/server.rs`, `crates/desktop/src/app/state/gateway.rs`, `crates/desktop/src/app/screens/config.rs`
