---
status: stable
---

# Gateway Server

This spec describes the gateway's **server behavior**: HTTP routes, startup validation, WebSocket connection lifecycle, authentication, connection tracking, and origin validation. For the `status` WebSocket response **payload**, see [GATEWAY_STATUS.md](GATEWAY_STATUS.md). For `config.json` field definitions, see [CONFIGURATION.md](CONFIGURATION.md). For the architectural decisions behind split deployment (remote gateways, connection policy, origin validation), see [adr/SPLIT_DEPLOYMENT.md](../adr/SPLIT_DEPLOYMENT.md).

## Startup Validation

### Loopback Enforcement

The gateway refuses to start when binding to a non-loopback address without token auth. A non-loopback address is any `bind` value that is not `127.0.0.1`, `::1`, or `localhost`.

| Condition | Behavior |
|-----------|----------|
| Loopback bind + `auth.mode: "none"` | Allowed (default — local-only deployment) |
| Loopback bind + `auth.mode: "token"` | Allowed |
| Non-loopback bind + `auth.mode: "none"` | **Refuses to start** — hard error before any further setup |
| Non-loopback bind + `auth.mode: "token"` + resolvable token | Allowed |
| Non-loopback bind + `auth.mode: "token"` + no resolvable token | **Refuses to start** |

The gateway token is resolved from `CHAI_GATEWAY_TOKEN` env or `gateway.auth.token` in `config.json` (see [CONFIGURATION.md](CONFIGURATION.md)).

### Connection Limit Resolution

At startup, the effective connection limit is resolved from `gateway.maxConnections` and the bind address:

| `maxConnections` config value | Loopback | Non-loopback |
|-------------------------------|----------|--------------|
| Omitted (`None`) | Unlimited | 1 (secure-by-default single-client) |
| `0` (explicit opt-out) | Unlimited | Unlimited |
| `n` (positive integer) | `n` | `n` |

### Gateway Lock

One gateway process is allowed **per profile**. The lock is an advisory exclusive `flock` on `~/.chai/profiles/<name>/gateway.lock`, acquired at the start of `run_gateway()`. If the lock for that profile is already held, the gateway fails to start immediately. Multiple gateways can run simultaneously on different profiles — each holds its own lock. See [PROFILES.md](PROFILES.md).

## Protocol Version

The wire protocol version is **`1`**. The gateway reports it in:

- `GET /` health response (`protocol` field)
- WebSocket `health` method response (`protocol` field)
- `connect` `hello-ok` response (`protocol` field, after negotiation)

### Protocol Negotiation

During the WebSocket `connect` handshake, the agreed protocol is `min(client.maxProtocol, server PROTOCOL_VERSION)`. If the client omits `maxProtocol`, the server's version is used.

## HTTP Routes

The gateway serves HTTP and WebSocket on a single TCP port (`gateway.port`, default 15151). All routes are served by a single `axum` router.

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/` | Health check (JSON) |
| `GET` | `/ws` | WebSocket upgrade |
| `GET` | `/logs` | Recent gateway log lines (plain text) |
| `POST` | `/telegram/webhook` | Telegram webhook callback |

Matrix verification routes (`/matrix/verification/*`) are available only when the `matrix` Cargo feature is enabled. See [ref/MATRIX.md](../ref/MATRIX.md).

### `GET /` Health Check

Returns JSON:

```json
{
  "status": "running",
  "protocol": 1,
  "port": 15151
}
```

## WebSocket Lifecycle

### Upgrade

The `GET /ws` handler upgrades the HTTP connection to WebSocket. On non-loopback bindings, origin validation is applied before the upgrade (see [Origin Validation](#origin-validation) below).

### Connection Challenge

Immediately after the WebSocket upgrade, before the client sends any request, the gateway sends a `connect.challenge` event:

```json
{
  "type": "event",
  "event": "connect.challenge",
  "payload": {
    "nonce": "<uuid>",
    "ts": 1700000000000
  }
}
```

The `nonce` is a fresh UUID. The client must include this nonce in its signing payload if using device-based authentication.

### `connect` — Authentication

The client's first request must be `method: "connect"`. Three authentication paths are supported:

#### Path A: Device Token (Previously Paired)

The client sends `auth.deviceToken` from a prior pairing. The gateway looks it up in `paired.json`. If found, authentication succeeds and the device ID, role, and scopes are loaded from the pairing entry. If not found, the gateway returns an error (`"invalid device token"`).

#### Path B: Device Signing (New Pairing)

The client sends a `device` object with its Ed25519 public key, signature, signed-at timestamp, and the challenge nonce. The gateway:

1. Verifies the nonce matches the challenge.
2. Constructs the canonical signing payload: `deviceId\nclientId\nclientMode\nrole\nscopes\nsignedAt\ntoken\nnonce` (newline-separated).
3. Verifies the signature using `ed25519_dalek::verify_strict`.

If the device is already paired (by device ID), its existing token, role, and scopes are returned. If the device is **new**, the gateway requires a valid gateway token (`auth.token` must match the configured token). If the gateway token matches, the device is auto-approved: a new random device token (UUID) is issued, the pairing entry is persisted to `paired.json`, and the token is returned to the client. If the gateway token doesn't match, the connection is rejected with `"pairing required: provide gateway token to approve this device"`.

See [SECURITY.md](../SECURITY.md) for the device pairing protocol threat model.

#### Path C: Gateway Token (No Device)

When no `device` object is provided but the gateway is configured with token auth:

- If `auth.token` is empty → error `"unauthorized: gateway token missing"`
- If `auth.token` doesn't match → error `"unauthorized: gateway token mismatch"`
- If it matches → auth succeeds with no device pairing

### `hello-ok` Response

After successful authentication, the gateway sends:

```json
{
  "type": "res",
  "id": "<request_id>",
  "ok": true,
  "payload": {
    "type": "hello-ok",
    "protocol": 1,
    "policy": { "tickIntervalMs": 15000 },
    "auth": null
  }
}
```

The `auth` field is `null` for gateway-token-only auth (Path C). For device-based auth (Paths A and B), it contains `deviceToken`, `role`, and `scopes`.

### Post-Auth Registration

After `hello-ok` is sent, the connection is registered with the `ConnectionTracker` (see [Connection Tracking](#connection-tracking) below).

### WebSocket Methods

After authentication, the client can call the following methods via the `req`/`res` envelope:

| Method | Purpose |
|--------|---------|
| `status` | Runtime snapshot (see [GATEWAY_STATUS.md](GATEWAY_STATUS.md)) |
| `health` | Lightweight health check (`status`, `protocol` — no `port`) |
| `agent` | Start an agent turn (streamed events) |
| `stop` | Stop an in-progress agent turn |
| `send` | Send a message to a channel-bound session |
| `agentDetail` | On-demand per-agent heavy data |
| `sessions.list` | List sessions for an orchestrator |
| `sessions.history` | Fetch full session history |
| `sessions.delete` | Delete a session |
| `sessions.delete_all` | Delete all sessions for an orchestrator |
| `logs` | Fetch recent log lines |

Unknown methods return `"unknown method: {method}"`.

### Close Behavior

| Trigger | Behavior |
|---------|----------|
| Displaced by connection limit | Close frame (code 1013, reason `"connection limit reached: displaced by newer connection"`) |
| Gateway shutdown | Shutdown event sent as text frame, then socket closes |
| Client disconnect / socket error | Loop ends; no close frame sent |
| Broadcast channel closed | Loop ends; no close frame sent |

Authentication failures are **not** sent as close frames — they are `WsResponse::err` text frames (`{type:"res", ok:false, error:"..."}`), and the connection stays open so the client can retry `connect`.

## Connection Tracking

The `ConnectionTracker` tracks authenticated WebSocket connections keyed by **client identity** (device ID or token-derived key), not by individual connection. Each client identity maps to a list of concurrent connections.

### Client Key Derivation

| Auth path | Client key |
|-----------|-----------|
| Device-based (Paths A and B) | Device ID |
| Gateway token only (Path C) | `token:<token>` |
| Anonymous (no auth) | `anonymous` |

### 1:N Multi-Connection Model

A single desktop client opens multiple concurrent WebSocket connections (session events listener, status fetches, agent turns, etc.) — all sharing the same device identity. The 1:N model allows all connections from the same client to coexist without displacing each other.

The connection limit applies to the number of **distinct clients**, not individual connections.

### `register`

Called after successful authentication. If `maxConnections` is unlimited (`None`), registration is a no-op. Otherwise:

1. If the client key matches an **existing client** → the connection is appended to that client's list. No limit check — the slot already exists.
2. If the client key is **new** → while the number of distinct clients would exceed `maxConnections`, the oldest client (first in the ordered list) is removed and **all of its connections** are kicked (each receives a `oneshot` signal that triggers a close frame). Then the new client entry is appended.

### `unregister`

Called when a socket disconnects (normally or kicked). Removes the specific connection from its client's list. If the client's list becomes empty, the client slot is freed entirely.

### Kick-Oldest Policy

When a new client authenticates and the distinct-client limit is exceeded, all connections of the longest-running existing client are disconnected. Each receives a WebSocket close frame (code 1013, reason `"connection limit reached: displaced by newer connection"`).

This policy prevents a stolen-token attacker from holding a connection and blocking the legitimate user from reconnecting (a DoS vector under a reject-new policy). The most recent authenticator always wins.

See [adr/SPLIT_DEPLOYMENT.md](../adr/SPLIT_DEPLOYMENT.md) for the rationale behind identity-based tracking and kick-oldest.

## Origin Validation

On non-loopback bindings, the gateway validates the `Origin` header on WebSocket upgrade requests against `gateway.allowedOrigins` in `config.json`.

| Condition | Behavior |
|-----------|----------|
| Loopback bind | Origin validation skipped entirely |
| Non-loopback + no `Origin` header | Allowed (non-browser client like the desktop app) |
| Non-loopback + `Origin` matches `allowedOrigins` | Allowed |
| Non-loopback + `Origin` does not match `allowedOrigins` | HTTP 403 (`"origin not allowed"`) |

The default `allowedOrigins` is an empty array — all browser-origin requests are rejected on non-loopback. Operators who need browser-based tools explicitly add origins.

Origin validation is a defense-in-depth measure: token auth is the primary security boundary; origin validation prevents cross-site WebSocket hijacking (CSWSH) even if a token is compromised via a browser-side attack. See [SECURITY.md](../SECURITY.md).

## Independence from `gateway.lock`

The connection policy (`maxConnections`) and the process guard (`gateway.lock`) operate at different layers and are independent:

| Guard | Layer | Purpose |
|-------|-------|---------|
| `gateway.lock` | Process | Prevents two gateway processes on the same profile |
| `maxConnections` | Connection | Limits authenticated WebSocket clients per gateway |

They do not interact. A gateway holding its profile lock enforces connection limits independently of the lock state.

## Related Documents

| Document | Purpose |
|----------|---------|
| [GATEWAY_STATUS.md](GATEWAY_STATUS.md) | `status` WebSocket response payload |
| [CONFIGURATION.md](CONFIGURATION.md) | `config.json` gateway block (`bind`, `port`, `auth`, `allowedOrigins`, `maxConnections`) |
| [DESKTOP.md](DESKTOP.md) | Desktop gateway modes (spawn, attach, remote) and `desktop.json` |
| [PROFILES.md](PROFILES.md) | Per-profile gateway lock and profile directory structure |
| [SECURITY.md](../SECURITY.md) | Gateway authentication, connection security, device pairing threat model |
| [adr/SPLIT_DEPLOYMENT.md](../adr/SPLIT_DEPLOYMENT.md) | Architectural decisions for split deployment, connection policy, and origin validation |
| [adr/RUNTIME_PROFILES.md](../adr/RUNTIME_PROFILES.md) | Per-profile gateway lock design |
| [CHANNELS.md](CHANNELS.md) | Channel behavior and session bindings within the gateway |
| [ORCHESTRATION.md](ORCHESTRATION.md) | Agent turn lifecycle and delegation |
| [CONTEXT.md](CONTEXT.md) | Per-session context: system message, history, tool schemas |
| [SESSIONS.md](SESSIONS.md) | Session persistence and management |
