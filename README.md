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
      "botToken": null,
      "webhookUrl": null,
      "webhookSecret": null
    }
  },
  "agents": {
    "defaultModel": "llama3.2:latest",
    "workspace": null
  },
  "skills": {
    "directory": null,
    "extraDirs": [],
    "disabled": [],
    "contextMode": "full",
    "allowScripts": false
  }
}
```

Use the exact model name from `ollama list` for `defaultModel` (e.g. `llama3.2:latest`, `qwen3:8b`); do not add extra segments like `:latest` unless that tag exists for the model.

For auth when binding beyond loopback, set `"auth": { "mode": "token", "token": "your-secret" }`.

### Configuration Directory (`~/.chai/`)

The configuration directory contains the following:

- **`config.json`** — Main configuration file (see above).
- **`skills`** — Skills directory (or use **`skills.directory`** in config to point elsewhere). After `chai init`, bundled skills are extracted here. Each skill is a subdirectory with **`SKILL.md`**; optionally add **`tools.json`** to define callable tools. Skills without `tools.json` are still loaded for context but have no tools. Add more roots via **`skills.extraDirs`** (same name overwrites).
- **`workspace`** — Directory for agent context (e.g. `AGENTS.md`). Not used for loading skills.

### Environment variables

| Variable | Overrides | Description |
|----------|-----------|-------------|
| `CHAI_CONFIG_PATH` | Config file path | Full path to the configuration file. The default path is `~/.chai/config.json`. |
| `CHAI_GATEWAY_TOKEN` | `gateway.auth.token` | Shared secret for WebSocket connect when auth mode is `token`. |
| `TELEGRAM_BOT_TOKEN` | `channels.telegram.botToken` | Telegram bot token from BotFather. |

## Connections

The gateway supports the following natively:

**WebSocket**

- Clients connect at `ws://<bind>:<port>/ws`, call `connect`, then `agent` (run model) and `send` (deliver message to a channel). Used by the desktop app and for scripting.

**Channels**

- **Telegram** — Can run in **long-poll** mode (gateway pulls updates; good for local use) or **webhook** mode (Telegram POSTs updates to your URL; good for public gateway). Inbound messages trigger an agent turn and the reply is sent back to the chat. To configure for **long-poll** mode, set `channels.telegram.botToken` (or `TELEGRAM_BOT_TOKEN`). To configure for **webhook** mode, set `channels.telegram.webhookUrl` (and optionally `channels.telegram.webhookSecret`).

## Skills

Skills are markdown-based instructions (one per directory with a `SKILL.md` file) that are loaded into the agent’s context. A skill can optionally include a **`tools.json`** in the same directory to declare callable tools (name, parameters, and how they map to a CLI). **Only skills that have a `tools.json` expose tools to the agent;** skills without `tools.json` still provide their SKILL.md text as context but have no callable tools.

**Skill context mode** (`skills.contextMode`): how skill documentation is given to the model.

- **`full`** (default) — All loaded skills’ full SKILL.md content is injected into the system message each turn. Best for few skills and smaller local models (e.g. 7B–9B).
- **`readOnDemand`** — The system message contains only a compact list (name, description). The model uses the **`read_skill`** tool to load a skill’s full SKILL.md when it clearly applies. Keeps the prompt small and scales to many skills; requires the model to call the tool before using a skill.

Skills can be gated by binaries: if a skill lists `metadata.requires.bins`, it is only loaded when all those binaries are on the gateway’s PATH.

To load only one of two skills when both binaries are installed (e.g. only notesmd-cli and not obsidian), set **`skills.disabled`** in config to an array of skill names to skip (e.g. `["obsidian"]`).

**Bundled skills**

- **notesmd-cli** — [NotesMD CLI](https://github.com/yakitrak/notesmd-cli) (binary `notesmd-cli`). Search for file, search content, create note, daily note, read note, and update note in the default vault. Only loaded when `notesmd-cli` is on PATH.
- **obsidian** — The official [Obsidian CLI](https://help.obsidian.md/cli) (early access; binary `obsidian`). Search for file, search content, and create note in the default vault. Only loaded when `obsidian` is on PATH.

**Custom skills**

Add skills to the config directory’s **`skills`** subdirectory (`~/.chai/skills`), or set **`skills.directory`** in config to another path (e.g. a repo’s `skills/` folder), or add paths in **`skills.extraDirs`**. One subdirectory per skill with a **`SKILL.md`** file; add **`tools.json`** in that directory to define the skill’s tools (without it, the skill has no callable tools). Use `name` and `description` in the frontmatter; use `metadata.requires.bins` so the skill loads only when those binaries are on PATH.

## Workspace

The workspace directory includes **frontloaded context** for the agent (e.g. `AGENTS.md`). By default it is `~/.chai/workspace/`, or `agents.workspace` in `config.json` if set.

- **`AGENTS.md`** — Created when you run `chai init` (and only recreated if the file is missing). Edit it to customize your agent. The gateway loads it as **agent-level context** and prepends it to the skills context on every turn. Keep it short and directive (e.g. when to chat normally vs when to call tools).
