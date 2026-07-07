---
status: accepted
---

# Split Deployment Architecture

Remote gateway support enabling a hosted-gateway deployment model where `chai gateway` runs on a server and `chai-desktop` on a separate client machine connects to it over the network.

## Context

Chai's desktop app was built around a single-machine model: it spawns `chai gateway` as a local child process, connects to it on loopback via `ws://`, and manages its lifecycle. A developer who wanted to host a gateway for a client — running the gateway on a remote server while the client uses the desktop app locally — encountered several obstacles:

- **No remote address configuration.** The desktop derived the WebSocket URL from `gateway.bind` + `gateway.port`, fields that semantically mean "address the server binds to." Setting them to a remote IP on the client would be a semantic misuse of server-side config.
- **No TLS.** The gateway bound plain HTTP/WebSocket. The desktop hardcoded `ws://` URLs. All traffic traveled in cleartext over the network.
- **No Connect/Disconnect mode.** The desktop showed a "Start gateway" button even when configured for a remote gateway, which would spawn a conflicting local process.
- **No origin validation.** The gateway did not check the `Origin` header on WebSocket upgrades, enabling cross-site WebSocket hijacking (CSWSH) on non-loopback deployments.
- **No connection limit.** The gateway accepted unlimited WebSocket connections. A compromised token granted full access alongside the legitimate client.

The underlying protocol plumbing (device pairing, challenge-response auth, token issuance, log streaming) already worked over the network. What was missing was the configuration, security, and UX layer.

## Decision

### 1. `remote` Array in `desktop.json`

Remote gateway configuration lives in `desktop.json` (client-side, machine-local), not `config.json` (server-side, per-profile). A `remote` array holds entries with `id` (profile name and ComboBox label), `url` (WebSocket URL with `ws://` or `wss://` and full path support), and `token` (gateway auth token for pairing).

The `id` doubles as the profile directory name under `~/.chai/profiles/`, reusing existing device identity storage (`device.json`, `device_token`) via `ChaiPaths`. Remote profile directories are created at startup (`mkdir -p`, no files written) so entries appear in the ComboBox before first connect.

**Why an array, not a single URL:** A developer may host staging and production gateways, or gateways for different clients. The array makes multiple remote gateways first-class; a minimal setup has one entry.

**Why profile type determines mode (no mode flag):** The desktop looks up the selected profile `id` in the `remote` array. If it matches, the profile is remote (Connect/Disconnect, no local spawn). If not, it is local (Start/Stop, existing behavior). No separate `mode` field is needed — the profile type is the mode.

### 2. TLS via Reverse Proxy, Client-Side `wss://` Support

The gateway does not gain built-in TLS termination. TLS remains the operator's responsibility (reverse proxy with WSS→WS termination). The desktop client supports `wss://` URL construction when a remote entry's `url` specifies it; local profiles always use `ws://` (derived from `bind:port`, loopback doesn't need TLS).

Full path support in the `url` field (e.g., `wss://example.com/chai/ws`) enables reverse proxy routing patterns. The desktop passes the full URL to the WebSocket client library; path mapping to the gateway's `/ws` route is the reverse proxy's responsibility.

### 3. Secure-by-Default Connection Policy

`gateway.maxConnections` caps simultaneously authenticated **client identities** (device IDs), not individual WebSocket connections. The default is 1 on non-loopback (secure-by-default single-client) and unlimited on loopback. `maxConnections: 0` is an explicit opt-out (unlimited).

**Identity-based 1:N tracking:** Each client identity maps to a list of concurrent connections. Same-client connections coexist without displacement (the desktop opens multiple WebSocket connections per logical client — events listener, status fetches, agent turns — all sharing one device identity). A native 1:1 model was tried first and produced an infinite self-displacement churn loop: every one-shot fetch kicked the long-lived events listener, which reconnected and kicked the next fetch.

**Kick-oldest, not reject-new:** When a new client authenticates and the distinct-client limit is exceeded, all connections of the longest-running existing client are disconnected (each receives a WebSocket close frame with code 1013 and reason "connection limit reached: displaced by newer connection"). This prevents a stolen-token attacker from holding a connection and blocking the legitimate user from reconnecting (a DoS vector under reject-new).

### 4. Secure-by-Default Origin Validation

On non-loopback bindings, the gateway validates the `Origin` header on WebSocket upgrade requests against `gateway.allowedOrigins` in `config.json`. The default is an empty array — all browser-origin requests are rejected with HTTP 403. The desktop app does not send an `Origin` header and is unaffected. Operators who need browser-based tools explicitly add origins.

This is a defense-in-depth measure: token auth is the primary security boundary; origin validation prevents CSWSH even if a token is compromised via a browser-side attack.

### 5. Independent Process Guard and Connection Guard

The per-profile `gateway.lock` (advisory exclusive `flock`) is a process-level guard preventing two gateway processes on the same profile. The connection policy (`maxConnections`) is a connection-level guard limiting WebSocket clients per gateway. They operate at different layers and do not interact.

### 6. Remote Profile UX: Show With Message

When a remote profile is selected, the Config and Skills screens display a message directing users to the Gateway screen instead of attempting to load non-existent local `config.json` or skill packages. Both screens remain visible in the sidebar for all profile types. This was chosen over hiding screens from the sidebar because the message is needed regardless of approach (as a fallback for the transition frame), and a consistent sidebar avoids visual jarring and disorienting redirects.

## Alternatives Considered

| Alternative | Why not |
|-------------|---------|
| **`gateway.connectUrl` in `config.json`** | Semantic misuse of a server-side field for client-side configuration. The client's `config.json` may not even exist in a remote-only setup. `desktop.json` correctly separates client-side concerns from server-side config. |
| **Single remote URL instead of an array** | Does not support staging + production or multiple client gateways. The array makes the multi-gateway use case first-class with negligible cost to the minimal single-entry setup. |
| **Built-in TLS termination in the gateway** | Adds TLS dependencies and certificate management to the gateway — scope bloat for a feature that operators already handle via reverse proxies. Client-side `wss://` support is the minimal, sufficient change. |
| **Reject-new connection policy** | Creates a DoS vector: an attacker with a stolen token holds a connection and prevents the legitimate user from reconnecting. Kick-oldest ensures the most recent authenticator always wins. |
| **1:1 per-connection tracking** | Produces infinite self-displacement churn when the desktop opens multiple concurrent connections per logical client. Identity-based 1:N allows same-client connections to coexist while enforcing the distinct-client limit. |
| **Per-device connection limits** | More granular but requires device-tracking infrastructure. A simple global limit with kick-oldest provides the security benefit (single-client default) without the complexity. Can be added as a future enhancement for multi-client deployments. |
| **Hide Config/Skills screens for remote profiles** | The message is needed regardless (as a fallback for the transition frame during profile switch). A consistent sidebar avoids visual jarring and disorienting screen redirects. Using the message as primary content is simpler than using it as a fallback. |
| **`CHAI_GATEWAY_URL` environment variable** | Dropped in favor of the `remote` array in `desktop.json`. Environment variables are inconvenient for multi-gateway setups and invisible to the desktop's ComboBox UI. |

## Consequences

- **Clear server/client separation.** Server-side config (`config.json` — bind, port, auth, providers, channels, agents, skills) and client-side config (`desktop.json` — appearance, logs, remote profiles) serve different audiences and live in different files. Nothing moves out of `config.json`.
- **Secure by default on non-loopback.** Token auth is required, origin validation rejects all browser origins, and connections are limited to one client. Operators must explicitly opt in to relax any of these.
- **TLS is operator-managed.** Binding to non-loopback without TLS exposes the auth token and all data in cleartext. The reverse proxy path is documented but not enforced.
- **No per-client session isolation.** Authenticated clients within the connection limit share the same access — any client can interact with any session. This is tracked as a broader access-control concern, not a split-deployment-specific gap.
- **Session events listener requires explicit cancellation.** The desktop's background session events listener thread had no cancellation mechanism (pre-existing bug). The connection limit made the ghost-listener reconnection loop visible and harmful. An `Arc<AtomicBool>` cancel flag was added to `GatewayState`.

## References

- [spec/GATEWAY.md](../spec/GATEWAY.md) — Gateway server behavioral contract (startup, HTTP, WebSocket lifecycle, connection tracking, origin validation).
- [spec/DESKTOP.md](../spec/DESKTOP.md) — Desktop application spec (spawn/attach/remote modes, `desktop.json` schema).
- [spec/CONFIGURATION.md](../spec/CONFIGURATION.md) — `config.json` gateway block (`allowedOrigins`, `maxConnections`).
- [SECURITY.md](../SECURITY.md) — Security rationale for connection security and origin validation.
- [adr/RUNTIME_PROFILES.md](RUNTIME_PROFILES.md) — Profile model (per-profile gateway locks, device identity).
