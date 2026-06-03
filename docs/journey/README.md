# User Journeys

This directory contains step-by-step user journeys for understanding the system and manually testing it. Run through them to test behavior after significant changes or before a release.

For conceptual background on any topic, see the [User Guides](../guides/README.md). For systematic model and provider comparison, see the [Testing Playbooks](../testing/README.md).

- [01 — Gateway (CLI): health and WebSocket connect](01-gateway-cli-health-and-ws.md) — Start the gateway, verify HTTP health and WebSocket handshake. Background: [Configuration](../guides/03-configuration.md)
- [02 — Gateway WebSocket: agent](02-gateway-ws-agent.md) — Run one agent turn over WebSocket with Ollama. Background: [Agents](../guides/05-agents.md)
- [03 — Gateway WebSocket: send](03-gateway-ws-send.md) — Test the `send` method for channel delivery. Background: [Connections](../guides/04-connections.md)
- [04 — Desktop: start/stop gateway](04-desktop-start-stop-gateway.md) — Use the desktop app to manage the gateway. Background: [Getting Started](../guides/02-getting-started.md)
- [05 — Channel: Telegram](05-channel-telegram.md) — Connect Telegram and verify message round-trip. Background: [Connections → Telegram](../guides/04-connections.md#telegram) · [Configuration → Channels](../guides/03-configuration.md#configuring-channels)
- [06 — Skill: NotesMD](06-skill-notesmd.md) — Test the notesmd skill with search, create, and daily note tools. Background: [Skills](../guides/06-skills.md)
- [07 — Skill: Obsidian](07-skill-obsidian.md) — Test the official Obsidian CLI skill. Background: [Skills](../guides/06-skills.md)
- [08 — Channel: Matrix](08-channel-matrix.md) — Connect Matrix, verify message round-trip and optional E2EE verification. Background: [Connections → Matrix](../guides/04-connections.md#matrix) · [Configuration → Channels](../guides/03-configuration.md#configuring-channels)
- [09 — Channel: Signal](09-channel-signal.md) — Connect Signal via signal-cli and verify message round-trip. Background: [Connections → Signal](../guides/04-connections.md#signal) · [Configuration → Channels](../guides/03-configuration.md#configuring-channels)
