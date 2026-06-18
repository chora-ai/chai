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
| **Status** | Runtime snapshot from the gateway: providers, discovered models, agents, channels, skill packages. |
| **Config** | Read-only view of the active profile's `config.json` with a summary of providers, agents, channels, and delegation rules. Provides the config file path so you can edit it externally. |
| **Agent** | Inspect the system message for each agent (built at startup from `AGENT.md`, the workers roster, and skills content). The system message is injected as the first message on every turn, separate from the persistent session history. |
| **Skills** | Browse installed skill packages, view SKILL.md and tools.json, toggle skills per agent. |
| **Tools** | Inspect the resolved tool schemas for each agent. Tool schemas are sent as a separate top-level field on every turn, outside the messages array. |
| **Logs** | Gateway log output (stdout/stderr from the spawned process). Useful for diagnosing connection errors or tool failures. |

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

The right-side sessions panel lists conversation history. Each session shows the first message as a preview. Click a session to resume it. The panel also displays the current session id.

### Model and Provider Selection

Below the chat input, dropdown selectors let you override the provider and model for the next message — useful for testing different backends without editing `config.json`.

### Stopping a Turn

While an agent turn is in progress, a **Stop** button appears next to the Send button. Click it to pause the agent after the current tool call or model request completes — the session transcript is preserved and you can send a new message to continue. This is useful when the agent is stuck, heading in the wrong direction, or needs additional guidance you didn't include in your original message. The stop is graceful: the agent finishes whatever it's currently doing, then pauses before starting the next iteration.

## Device Pairing

The desktop pairs with the gateway using a device identity and cryptographic signature. On first connection:

1. The desktop generates a device key pair and saves it to `~/.chai/active/device.json`.
2. It signs a challenge from the gateway and receives a `device_token`.
3. The token is saved to `~/.chai/active/device_token.json` and used for subsequent connections.

If a token becomes stale (e.g. `paired.json` was deleted on the gateway side), the desktop automatically re-pairs by falling back to the device identity + signature flow.

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

## Try It

For a hands-on walkthrough of the desktop app, see the [Desktop — Start/Stop Gateway and Detection](../journey/03-desktop-start-stop-gateway.md) journey.
