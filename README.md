# chai

An experimental project for creating, managing, and orchestrating agents.

## Overview

- **`crates/cli`** — A command-line interface for creating, managing, and orchestrating agents
- **`crates/desktop`** — A graphical user interface for creating, managing, and orchestrating agents
- **`crates/lib`** — A shared library for creating, managing, and orchestrating agents

## Commands

```bash
# Build everything
cargo build

# Run the command-line interface
cargo run -p cli -- --help
cargo run -p cli -- version
cargo run -p cli -- init
cargo run -p cli -- gateway

# Run the desktop application
cargo run -p desktop

# Test everything
cargo test
```

## Command-Line Interface

Install the CLI locally:

```bash
cargo install --path crates/cli
```

Run the installed CLI:

```bash
chai --help
chai version
chai init
chai gateway
```

## Desktop Application

Install the app locally:

```bash
cargo install --path crates/desktop
```

Run the installed app:

```bash
chai-desktop
```

## Configuration

The command-line interface and desktop application use the same configuration.

### Initialization

After installing, run **`chai init`** to create the configuration directory (`~/.chai/`).

### Configuration File (`config.json`)

The main configuration is loaded from a JSON file. The default path is `~/.chai/config.json`. The 
default path can be overridden with `CHAI_CONFIG_PATH`. An empty configuration file is created at initialization.

**Minimal example** — empty object uses all defaults:

```json
{}
```

**Full example** — all top-level keys (and their default values):

```json
{
  "gateway": {
    "port": 15151,
    "bind": "127.0.0.1",
    "auth": { "mode": "none" }
  },
  "channels": {
    "telegram": {
      "botToken": "YOUR_BOT_TOKEN",
      "webhookUrl": null,
      "webhookSecret": null
    }
  },
  "agents": {
    "defaultModel": "ollama/llama3.2:latest",
    "workspace": null
  },
  "skills": {
    "extraDirs": [],
    "disabled": []
  }
}
```

For auth when binding beyond loopback, set `"auth": { "mode": "token", "token": "your-secret" }`.

### Configuration Directory (`~/.chai/`)

The configuration directory contains the following:

- **`config.json`** — Main configuration file (see above).
- **`bundled`** — Bundled skills (use `workspace` for adding custom skills).
- **`workspace`** — Directory for workspace skills. If the configuration does not set `agents.workspace`, this directory is used. Add a subdirectory for each skill containing a `SKILL.md` file. When a skill name appears in more than one place (bundled, workspace, or extra), the workspace version is used.

### Environment variables

| Variable | Overrides | Description |
|----------|-----------|-------------|
| `CHAI_CONFIG_PATH` | Config file path | Full path to the configuration file. The default path is `~/.chai/config.json`. |
| `CHAI_GATEWAY_TOKEN` | `gateway.auth.token` | Shared secret for WebSocket connect when auth mode is `token`. |
| `TELEGRAM_BOT_TOKEN` | `channels.telegram.botToken` | Telegram bot token from BotFather. |

## Connections

The gateway supports these connection types natively:

- **WebSocket** — Clients connect to the gateway at `ws://<bind>:<port>/ws`, send `connect`, then can call `agent` (run the model) and `send` (deliver a message to a channel). Used by the desktop app and for scripting.
- **Telegram** — The bot can run in **long-poll** mode (gateway pulls updates; good for local use) or **webhook** mode (Telegram POSTs updates to your URL; for a public gateway). Inbound messages trigger an agent turn and the reply is sent back to the chat. Configure `channels.telegram.botToken` (or `TELEGRAM_BOT_TOKEN`) and optionally `channels.telegram.webhookUrl` and `channels.telegram.webhookSecret`.

## Skills

Skills are markdown-based instructions (one per directory with a `SKILL.md` file) that are loaded into the agent’s context. Skills can be gated on binaries: if a skill lists `metadata.requires.bins`, it is only loaded when all those binaries are on the gateway’s PATH. To load only one of two skills when both binaries are installed (e.g. only notesmd-cli and not obsidian), set **`skills.disabled`** in config to an array of skill names to skip (e.g. `["obsidian"]`).

**Natively supported skills (bundled):**

- **obsidian** — Official Obsidian CLI (early access; binary `obsidian`). Search, search-content, create, move, delete in the default vault. Only loaded when `obsidian` is on PATH.
- **notesmd-cli** — [yakitrak/notesmd-cli](https://github.com/yakitrak/notesmd-cli). Same operations; works without Obsidian running. Only loaded when `notesmd-cli` is on PATH.

Skill format, frontmatter, and adding custom skills are described in [`crates/lib/config/bundled/README.md`](crates/lib/config/bundled/README.md).

## Agent Context (AGENTS.md)

In addition to skills, the gateway can load **agent-level context** from an `AGENTS.md` file in the workspace. This text is prepended to the skills context on every agent turn.

- **Workspace directory:** By default `~/.chai/workspace/`, or `agents.workspace` in `config.json` if set.
- **File:** Place an `AGENTS.md` at the root of that workspace. Keep it short and directive (e.g. when to chat normally vs when to call tools).

Example `AGENTS.md` snippet:

```markdown
If someone says hello, say hello back. You are more than your skills. Use obsidian only when the user clearly asks to search, read, create, or work with notes or vaults.
```
