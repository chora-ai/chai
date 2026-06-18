# FEAT: Desktop Configuration File

Add a `desktop.json` file at the chai home root (`~/.chai/desktop.json`) for desktop-application settings and gateway connection configuration. This separates client-side concerns from `config.json`, which remains a server-side operator document.

## Problem

All chai configuration lives in per-profile `config.json` files. This file is a server-side operator document: it expresses what the gateway should do (bind address, auth, channels, providers, agents). The desktop app reads `config.json` for its own needs — `gateway.bind`, `gateway.port`, `gateway.auth` — but these fields belong to the gateway, not the desktop.

Two concrete problems arise:

1. **Desktop application settings have no home.** Settings like log buffer size, theme, and font size are currently hardcoded in the desktop crate. They are machine-local user preferences, not per-profile gateway concerns, yet there is no file for them.

2. **Split deployment will put client-side connection details in a server-side config.** The split deployment epic proposes adding `gateway.connectUrl` to `config.json`. This field is only meaningful to the desktop client — the gateway ignores it. Adding it to `config.json` creates a semantic conflict: the same file means different things depending on who reads it.

## Design

### File Location

`~/.chai/desktop.json` — the chai home root, not per-profile.

This is the first config file at the home root level. The placement is intentional: desktop settings are machine-local and user-specific, not tied to any profile. A user who switches profiles does not change their desktop preferences or their remote gateway connection target.

### Schema

```json
{
  "gateway": {
    "connectUrl": "wss://gateway.example.com:15151"
  },
  "appearance": {
    "theme": "dark",
    "fontSize": 14
  },
  "logs": {
    "bufferSize": 2000
  }
}
```

All blocks and fields are optional. The file may be absent entirely — the desktop falls back to current behavior when it does not exist.

#### `gateway` Block

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `connectUrl` | `string` | `null` | WebSocket URL for the desktop to connect to. When set, the desktop operates in attach-only mode (no local gateway spawn). Must start with `ws://` or `wss://`. Takes precedence over `config.json`'s `gateway.bind:port` for URL construction and TCP probes. |

This field is the persistent source for the remote gateway address. The split deployment epic builds on top of it.

**Auth secrets do not go here.** The gateway token follows the existing precedence (env → `.env` → `config.json`), consistent with the secrets management model in [SECURITY.md](SECURITY.md). `desktop.json` is for addressing, not secrets.

#### `appearance` Block

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `theme` | `string` | `"dark"` | Color theme: `"dark"` or `"light"`. |
| `fontSize` | `number` | `14` | Base font size in points. |

#### `logs` Block

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `bufferSize` | `number` | `2000` | Maximum number of log lines retained in memory. |

### Loading and Precedence

The desktop loads `desktop.json` at startup. When the file is absent, all values use their defaults — no change from current behavior.

For the gateway connection URL, the full precedence chain is:

```
CHAI_GATEWAY_URL (env)  →  desktop.json gateway.connectUrl  →  config.json gateway.bind:port (fallback)
```

This mirrors the existing `CHAI_PROFILE` → `~/.chai/active` pattern.

### Interaction With Profile Switching

`desktop.json` is profile-independent. Its presence does not affect profile switching in local mode:

| Condition | Desktop behavior | Profile switching |
|-----------|-----------------|-------------------|
| `desktop.json` absent, or `connectUrl` unset | Current spawn/attach mode. Reads `config.json` for `bind:port`. Start/Stop button. | Fully enabled (when gateway stopped). ComboBox rewrites `~/.chai/active`. |
| `desktop.json` has `connectUrl` | Attach-only mode. Connects to remote URL. No Start/Stop — "Connect" indicator. | Disabled. The gateway's profile is externally managed. |

A user running the desktop on the same machine as the gateway (the common case today) would have a `desktop.json` with only appearance and log settings, no `connectUrl`. Profile switching works identically to today.

### Config Screen Updates

When `gateway.connectUrl` is set, the desktop config screen shows "Remote gateway: \<url\>" instead of "Bind" / "Port". Auth mode and token are still shown (the client needs them for pairing).

This requirement is deferred to the split deployment epic, which owns the full config screen redesign for remote mode.

### Relationship to `config.json`

`desktop.json` and `config.json` are complementary, not competing:

| File | Owner | Who reads it | Contains |
|------|-------|-------------|----------|
| `config.json` (per-profile) | Operator / developer | `chai gateway` | Bind, port, auth, channels, providers, agents, skills |
| `desktop.json` (home root) | Client / end user | `chai-desktop` | connectUrl, appearance, logs |

Nothing moves out of `config.json`. The desktop still reads `config.json` for display purposes (providers, agents, channels) and for the `gateway.bind:port` fallback. `desktop.json` is purely additive: it provides a home for settings that currently have none.

## Implementation Requirements

### Desktop Crate

1. Add a `DesktopConfig` struct with serde deserialization matching the schema above. All fields optional with defaults.
2. Add a `load_desktop_config()` function that reads `~/.chai/desktop.json`. Returns default values when the file is absent.
3. Replace hardcoded values:
   - Log buffer size (`2000`) → read from `desktop.json` `logs.bufferSize`.
   - Theme and font size → read from `desktop.json` `appearance.*`.
4. Update gateway connection logic: when `desktop.json` has `gateway.connectUrl`, use it for the WebSocket URL and TCP probe instead of deriving from `config.json` `gateway.bind:port`.
5. When `gateway.connectUrl` is set, activate attach-only mode: hide the "Start gateway" button, replace with a "Connect" indicator, and disable profile switching.

### Gateway Crate

No changes. The gateway does not read `desktop.json`.

### CLI Crate

No changes. `desktop.json` is a desktop-application file. `chai gateway` and `chai chat` are unaffected.

### Environment Variable

Support `CHAI_GATEWAY_URL` as a runtime override for `desktop.json`'s `gateway.connectUrl`. This follows the same pattern as `CHAI_PROFILE` overriding the active profile symlink. Implementation of this env var can be deferred to the split deployment epic if desired — the `desktop.json` loading is the prerequisite.

### Validation

- `gateway.connectUrl` must start with `ws://` or `wss://`. Invalid schemes are rejected at load time.
- `logs.bufferSize` must be a positive integer. Zero or negative values are rejected.
- `appearance.fontSize` must be a positive integer.
- `appearance.theme` must be `"dark"` or `"light"`.

### Spec Updates

- [DESKTOP.md](spec/DESKTOP.md) — Document `desktop.json` loading, settings, and the gateway connection precedence chain.
- [PROFILES.md](spec/PROFILES.md) — Add `desktop.json` to the shared resources table (alongside `active` symlink, skill packages, and gateway lock).
- [CONFIGURATION.md](spec/CONFIGURATION.md) — Add a cross-reference noting that the desktop reads `desktop.json` for connection and appearance settings; no `config.json` schema changes.

## Scope

This feature covers:

- The `desktop.json` file format, loading, and validation.
- Migrating hardcoded desktop settings to the new config.
- The `gateway.connectUrl` field and its effect on desktop gateway lifecycle (attach-only mode).
- `CHAI_GATEWAY_URL` env var override (may be deferred to the split deployment epic).

This feature does **not** cover:

- **`wss://` TLS connection support** — the desktop client constructing TLS WebSocket connections. Tracked in the split deployment epic.
- **WebSocket origin validation** — gateway-side `Origin` header checking. Tracked in the split deployment epic.
- **Config screen redesign for remote mode** — showing "Remote gateway" instead of "Bind"/"Port". Tracked in the split deployment epic.
- **Reverse proxy documentation** — operator guides for TLS termination. Tracked in the split deployment epic.

## Prerequisite For

[epic/SPLIT_DEPLOYMENT.md](epic/SPLIT_DEPLOYMENT.md) — The split deployment epic depends on `desktop.json` as the persistent source for `gateway.connectUrl`. The epic's Approach C (`gateway.connectUrl` in `config.json` + `CHAI_GATEWAY_URL`) is superseded: `connectUrl` moves from `config.json` to `desktop.json`, keeping the env var override. This eliminates the semantic conflict of putting a client-side field in a server-side config file and resolves the epic's open question about a "thin" config format for remote clients.

## Related Documents

- [DESKTOP.md](spec/DESKTOP.md) — Desktop application spec (spawn vs. attach modes, screens, data sources)
- [CONFIGURATION.md](spec/CONFIGURATION.md) — On-disk `config.json` blocks and environment overrides
- [PROFILES.md](spec/PROFILES.md) — Profile directory structure and shared resources
- [SECURITY.md](SECURITY.md) — Secrets management and gateway auth
- [epic/SPLIT_DEPLOYMENT.md](epic/SPLIT_DEPLOYMENT.md) — Split deployment epic (remote gateway, TLS, origin validation)
