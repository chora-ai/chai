# Desktop App

The chai desktop application (`chai-desktop`) provides a graphical interface for managing the gateway, chatting with agents, and inspecting configuration — all without touching the command line beyond the initial install.

## Installation

```bash
cargo install --path crates/desktop
```

Then run:

```bash
chai-desktop
```

The desktop app is an egui/eframe native application. It connects to the gateway over WebSocket and also manages the gateway process itself.

## Getting Started

1. **Initialize** — If you haven't run `chai init` yet, the desktop app will prompt you. This creates `~/.chai/` with default profiles and skills (same as the CLI command; see [Configuration](03-configuration.md#initialization)).
2. **Start the gateway** — Click **Start gateway** in the header. The desktop app spawns a `chai gateway` process and watches it.
3. **Chat** — Switch to the **Chat** screen in the sidebar. Type a message and press Enter or click **Send**.

The header shows the current profile, gateway status (stopped / starting / running), and a gateway control button. If the gateway fails to start, the header displays the error.

## Screens

The sidebar provides navigation between screens:

| Screen | Description |
|--------|-------------|
| **Chat** | Send messages to the orchestrator agent, view replies, see tool calls and delegation events. |
| **Skills** | Browse installed skill packages, view SKILL.md and tool descriptor files (tools.json, allowlist.json, execution.json), toggle skills per agent. |
| **Agent** | Inspect the system message for each agent (built at startup from `AGENT.md`, the workers roster, and skills content). The system message is injected as the first message on every turn, separate from the persistent session history. |
| **Tools** | Inspect the resolved tool schemas for each agent. Tool schemas are sent as a separate top-level field on every turn, outside the messages array. |
| **Config** | Read-only view of the active profile's `config.json` with a summary of providers, agents, channels, and delegation rules. Provides the config file path so you can edit it externally. |
| **Gateway** | Runtime snapshot from the gateway: providers, discovered models, agents, channels, skill packages. |
| **Logging** | Gateway log output (stdout/stderr from the spawned process). Useful for diagnosing connection errors or tool failures. |

## Profile Management

The header includes a profile selector dropdown. Switching profiles changes the `~/.chai/active` symlink and updates all UI screens to reflect the new profile's configuration. Profile switching is always allowed regardless of whether a gateway is running. When switching, the desktop resets the new profile's gateway state (sessions, chat messages, orchestrator selection) to prevent stale data from leaking across profiles — only the owned gateway subprocess is preserved. Available profiles are listed from `~/.chai/profiles/`.

## Session Management

### Chat Commands

In the chat input, use these commands:

| Command | Description |
|---------|-------------|
| `/new` | Start a new session (clears conversation history) |
| `/help` | Show available commands |

### Sessions Panel

The right-side sessions panel lists all persisted sessions for the active profile. Each session shows its creation timestamp (e.g. "Jun 10, 12:34") as the primary label, with a short session ID below in dimmer text. Channel-bound sessions display a channel tag (e.g. `(telegram)`).

| Action | How |
|--------|-----|
| **Resume a session** | Click the session in the sidebar. Its full history loads on demand, including tool calls rendered with 🔧 icons, tool names, and collapsible arguments/results — the same rendering as live sessions. |
| **Delete a session** | Click the "×" button on the right side of the session row. The session is removed from the sidebar immediately. |
| **Clear all sessions** | Click **Clear all sessions** at the bottom of the sidebar, then confirm. |
| **Start a new session** | Click **New session** at the top of the sidebar (always visible, regardless of whether a session is active). |

Channel-bound sessions (e.g. from Telegram) are read-only from the desktop — you can view their history but cannot send messages from the desktop chat input, since that would create a new empty session and overwrite the channel session's history.

### Agent Selector

When multiple orchestrators are configured, the right sidebar shows an "Agent" ComboBox above the session list. Selecting a different orchestrator updates the sessions list and the provider/model defaults. The ComboBox is disabled when only one orchestrator is configured, during an active agent turn, or while the gateway is starting up (before status is received). During the loading state, the agent, provider, and model selectors show config-based defaults and are all disabled — they become enabled once the gateway status is available.

### Model and Provider Selection

Below the chat input, dropdown selectors let you override the provider and model for the next message — useful for testing different backends without editing `config.json`.

### Stopping a Turn

While an agent turn is in progress, a **Stop** button appears next to the Send button. Click it to pause the agent after the current tool call or model request completes — the session transcript is preserved and you can send a new message to continue. This is useful when the agent is stuck, heading in the wrong direction, or needs additional guidance you didn't include in your original message. The stop is graceful: the agent finishes whatever it's currently doing, then pauses before starting the next iteration.

## Device Pairing

The desktop pairs with the gateway using a device identity and cryptographic signature. On first connection:

1. The desktop generates a device key pair and saves it to `~/.chai/active/device.json`.
2. It signs a challenge from the gateway and receives a `device_token`.
3. The token is saved to `~/.chai/active/device_token` and used for subsequent connections.

If a token becomes stale (e.g. `paired.json` was deleted from the profile directory), the desktop automatically re-pairs by falling back to the device identity + signature flow.

When the gateway is configured with token auth (`gateway.auth.mode: "token"`), the desktop includes the token in its connect handshake. See [Configuration → Securing the Gateway](03-configuration.md#securing-the-gateway).

## Chat Message Types

The chat screen renders different message types visually:

| Type | Appearance |
|------|-----------|
| **User message** | Right-aligned, strong text |
| **Assistant reply** | Left-aligned, normal text with orchestrator id shown |
| **Tool call** | Collapsible section: tool name, arguments, result |
| **Tool result** | Inline under the tool call, monospace |
| **Delegation start** | Labeled event showing worker assignment |
| **Delegation complete** | Success/event indicator with worker id |
| **Worker reply** | The worker's text response, shown as a blue-bordered message with the worker id as a label |
| **Delegation error** | Red-highlighted error from the worker |
| **Tool loop limit** | Amber warning banner — the orchestrator hit `maxToolLoopsPerTurn`, pending tool calls were paused |
| **Turn stopped** | Amber info banner — the agent turn was stopped by the user, send a message to continue |

## Error Indicators

The desktop app surfaces errors from multiple sources using consistent visual patterns:

| Indicator | Meaning |
|-----------|---------|
| **Red text in header** | Gateway startup failure or unexpected crash (visible from any screen). Hover for the full message if truncated. |
| **Red text on a screen** | A fetch or load operation failed for that screen (e.g. config parse error on the Config screen, skills fetch failure on the Skills screen). |
| **Amber text in header** | Profile mismatch (the gateway is running a different profile than the desktop's active profile) or desktop config load failure (Settings screen shows a notice). |
| **Red-bordered chat message** | An error occurred during a chat turn (e.g. WebSocket RPC failure). |

When the gateway crashes unexpectedly, the desktop extracts the actual error message from the gateway log output (e.g. "sandbox directory not found at...") and displays it in the header and on the Gateway screen. Clicking "Start gateway" clears the previous error. User-initiated stops ("Stop gateway") do not show an error.

## Desktop Settings

The desktop app reads `~/.chai/desktop.json` at startup for appearance, log, and remote profile settings. This file is separate from per-profile `config.json` — it holds machine-local user preferences that don't change when you switch profiles.

When the file is absent, all values use their defaults (dark theme, 14pt font, 1000-line log buffer, no remote profiles). Invalid values are rejected at load time and the desktop falls back to defaults.

```json
{
  "appearance": {
    "theme": "light",
    "fontSize": 16
  },
  "logs": {
    "bufferSize": 5000
  }
}
```

All blocks and fields are optional.

| Field | Default | Description |
|-------|---------|-------------|
| `appearance.theme` | `"dark"` | Color theme: `"dark"` or `"light"`. |
| `appearance.fontSize` | `14` | Base font size in points. |
| `logs.bufferSize` | `1000` | Maximum number of log lines retained in memory per buffer (desktop and gateway). |

Settings are loaded once at startup — changes require restarting the desktop app.

## Remote Profiles

The desktop can connect to a remote gateway instead of spawning a local one. Add a `remote` array to `desktop.json` with entries for each remote gateway:

```json
{
  "remote": [
    {
      "id": "my-remote",
      "url": "wss://gateway.example.com/ws",
      "token": "your-gateway-token"
    }
  ]
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Profile name (appears in the ComboBox). Must be unique and not collide with existing local profile names. |
| `url` | Yes | WebSocket URL. Must start with `ws://` or `wss://`. Supports full paths for reverse proxy setups (e.g., `wss://example.com/chai/ws`). |
| `token` | Yes | Gateway auth token for device pairing. Must match the server's `gateway.auth.token`. |

When a remote profile is selected, the header shows **Connect/Disconnect** instead of Start/Stop. The desktop never spawns a local gateway process for remote profiles.

### Reverse Proxy Setup for TLS

The gateway does not have built-in TLS. To use `wss://`, run a reverse proxy (e.g., Caddy, nginx, Traefik) in front of the gateway with TLS termination. Example Caddy configuration:

```
gateway.example.com {
    tls internal
    reverse_proxy localhost:15151
}
```

Then set the remote entry URL to `wss://gateway.example.com/ws`.

### `CHAI_HOME` Environment Variable

The `CHAI_HOME` environment variable overrides the default `~/.chai` directory. This is useful for testing split deployment on a single machine — the server and client can each use different `CHAI_HOME` values:

```bash
# Server
CHAI_HOME=~/.chai-server chai gateway

# Client
CHAI_HOME=~/.chai-client chai-desktop
```

| Value | Behavior |
|-------|----------|
| Absolute path (existing) | Uses that directory |
| Absolute path (nonexistent) | Accepted (for `chai init`) |
| Relative path | Resolved against current working directory |
| Empty string | Falls back to `~/.chai` |
| Not set | Default `~/.chai` behavior |

## Try It

For a hands-on walkthrough of the desktop app, see the [Desktop — Start/Stop Gateway and Detection](../journey/03-desktop-start-stop-gateway.md) journey.
