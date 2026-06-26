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
| **Skills** | Browse installed skill packages, view SKILL.md and tools.json, toggle skills per agent. |
| **Agent** | Inspect the system message for each agent (built at startup from `AGENT.md`, the workers roster, and skills content). The system message is injected as the first message on every turn, separate from the persistent session history. |
| **Tools** | Inspect the resolved tool schemas for each agent. Tool schemas are sent as a separate top-level field on every turn, outside the messages array. |
| **Config** | Read-only view of the active profile's `config.json` with a summary of providers, agents, channels, and delegation rules. Provides the config file path so you can edit it externally. |
| **Gateway** | Runtime snapshot from the gateway: providers, discovered models, agents, channels, skill packages. |
| **Logging** | Gateway log output (stdout/stderr from the spawned process). Useful for diagnosing connection errors or tool failures. |

## Profile Management

The header includes a profile selector dropdown. Switching profiles stops the gateway (if running), changes the `~/.chai/active` symlink, and restarts with the new configuration. Available profiles are listed from `~/.chai/profiles/`.

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
| **Resume a session** | Click the session in the sidebar. Its full history loads on demand. |
| **Delete a session** | Click the "×" button on the right side of the session row. The session is removed from the sidebar immediately. |
| **Clear all sessions** | Click **Clear all sessions** at the bottom of the sidebar, then confirm. |
| **Start a new session** | Click **New session** at the top of the sidebar (always visible, regardless of whether a session is active). |

Channel-bound sessions (e.g. from Telegram) are read-only from the desktop — you can view their history but cannot send messages from the desktop chat input, since that would create a new empty session and overwrite the channel session's history.

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

The desktop app reads `~/.chai/desktop.json` at startup for appearance and log settings. This file is separate from per-profile `config.json` — it holds machine-local user preferences that don't change when you switch profiles.

When the file is absent, all values use their defaults (dark theme, 14pt font, 1000-line log buffer). Invalid values are rejected at load time and the desktop falls back to defaults.

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

## Try It

For a hands-on walkthrough of the desktop app, see the [Desktop — Start/Stop Gateway and Detection](../journey/03-desktop-start-stop-gateway.md) journey.
