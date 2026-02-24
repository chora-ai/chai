# chai

An experimental project for creating, managing, and orchestrating autonomous agents.

## Overview

- **`crates/cli`** — A command-line interface for creating, managing, and orchestrating autonomous agents
- **`crates/desktop`** — A graphical user interface for creating, managing, and orchestrating autonomous agents
- **`crates/lib`** — A shared library providing core functionality for agent creation, management, and orchestration

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

### Configuration file (`config.json`)

The main configuration is loaded from a JSON file. The default path for the configuration file is `~/.chai/config.json`. The default path can be overridden with `CHAI_CONFIG_PATH`. If the configuration file is missing, default values are used.

**Minimal** — empty object uses all defaults:

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
    "extraDirs": []
  }
}
```

For auth when binding beyond loopback, set `"auth": { "mode": "token", "token": "your-secret" }`.

### Configuration directory (`~/.chai/`)

Created by **`chai init`** (or on first use). It contains:

- **`config.json`** — Main configuration file (see above). Created with `{}` by init if missing.
- **`workspace`** — Directory for workspace skills. If the configuration does not set `agents.workspace`, this directory is used. Add one subdirectory per skill, each containing a `SKILL.md` file. When a skill name appears in more than one place (bundled, managed, extra, or workspace), the workspace version is used.
- **`skills`** — Default (bundled) skills, copied by `chai init` from the app. The gateway and desktop app load skills from here. If you never run init, this directory does not exist and only `skills.extraDirs` and the workspace directory are used.

### Environment variables

| Variable | Overrides | Description |
|----------|-----------|-------------|
| `CHAI_CONFIG_PATH` | Config file path | Full path to the config file. Default: `~/.chai/config.json`. |
| `CHAI_GATEWAY_TOKEN` | `gateway.auth.token` | Shared secret for WebSocket connect when auth mode is `token`. |
| `TELEGRAM_BOT_TOKEN` | `channels.telegram.botToken` | Telegram bot token from BotFather. |

When both an environment variable and a configuration value are set, the environment variable overrides the configuration value.
