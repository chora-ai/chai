# Getting Started

This guide walks you through installing chai, starting the gateway, and sending your first message. By the end, you will have a running system with a local model responding to you.

## Prerequisites

- **Rust toolchain** — Install via [rustup](https://rustup.rs/) if you don't have it.
- **Ollama** — The default provider. Install from [ollama.com](https://ollama.com) and pull a model:
  ```bash
  ollama pull llama3.2:3b
  ```
  Ollama must be running before you start the gateway (`ollama serve` or the system tray app).

## Install the CLI

```bash
cargo install --path crates/cli
```

To include the optional Matrix channel adapter:

```bash
cargo install --path crates/cli --features matrix
```

Verify the installation:

```bash
chai version
```

## Initialize

Run `chai init` to create the chai configuration directory:

```bash
chai init
```

This creates `~/.chai/` with:

- Two default profiles: `assistant` and `developer`
- An `active` symlink → `profiles/assistant/`
- A shared `skills/` tree (bundled skills extracted from the application)
- A `sandbox/` directory per profile for write-capable tools

Each profile gets its own `config.json`, agent context directories, and state. The active profile is `assistant` by default.

`chai init` is safe to re-run on an existing configuration — it never overwrites files that already exist and preserves user customizations to bundled skills. See [Configuration](03-configuration.md) for the full breakdown of re-run behavior.

## Your First Chat

### Start the Gateway

```bash
chai gateway
```

The gateway starts an HTTP/WebSocket server on `127.0.0.1:15151` (the defaults). You will see log output confirming the startup, provider discovery, and skill loading.

### Chat via the CLI

In a separate terminal, start an interactive chat:

```bash
chai chat
```

This connects to the running gateway using the active profile. Type a message and press Enter. The orchestrator agent will respond using the configured provider and model (Ollama + `llama3.2:3b` by default).

### Chat via the Desktop App

If you have the desktop app installed:

```bash
chai-desktop
```

The desktop app connects to the gateway, lets you start and stop it, and provides a chat interface. See the desktop app's built-in help for controls.

## Session Management

Each conversation is a **session** with its own message history. In the CLI or desktop chat, type `/new` to start a fresh session. This resets the history while keeping the same agent and tools.

## What Happened Behind the Scenes

When you sent your first message, the gateway:

1. Loaded the orchestrator's system context — the agent instructions from `~/.chai/active/agents/orchestrator/AGENT.md` plus any enabled skill content.
2. Sent the system message and your message to the provider (Ollama).
3. Received the model's response and streamed it back.
4. Stored the exchange in the session history for context on the next turn.

## Next Steps

Now that you have a working system, customize it:

- **Switch models** — Edit `config.json` to change `defaultProvider` or `defaultModel`. See [Configuration](03-configuration.md).
- **Choose a provider** — Compare local vs. cloud options and find the right model for your hardware. See [Choosing a Provider and Model](10-choosing-a-provider.md).
- **Add a channel** — Connect Telegram, Matrix, or Signal so you can chat outside the CLI. See [Connections](04-connections.md).
- **Configure agents** — Add workers for delegated subtasks. See [Agents](05-agents.md).
- **Enable skills** — Give your agent tools for file operations, notes, and more. See [Skills](06-skills.md).
- **Set up the write sandbox** — Control where skill tools can write. See [Write Sandbox](07-sandbox.md).
- **Try the desktop app** — Use the GUI to manage the gateway and chat. See [Desktop App](09-desktop.md).
- **CLI reference** — All commands and flags. See [CLI Reference](08-cli-reference.md).

For hands-on walkthroughs of each feature, try the [User Journeys](../journey/README.md). To compare models and providers systematically, see the [Testing Playbooks](../testing/README.md). If something isn't working, see [Troubleshooting](11-troubleshooting.md).
