# Proof-of-Concept - Changelog

This document is the **changelog** of the proof-of-concept implementation.

## Features

- **Gateway (CLI + lib)** — Added `chai gateway` with HTTP and WebSocket on one port; connect handshake with optional auth (token or device); `connect.challenge`, device signing, pairing store, deviceToken in hello-ok; graceful shutdown with broadcast and channel await; WS methods `health`, `status`, `send`, `agent`.
- **Desktop** — Added Start/Stop gateway (spawns `chai gateway`); TCP probe for gateway detection; WebSocket `status` for live details; device identity for connect when fetching status.
- **Config** — Added `Config` (gateway, channels, agents, skills); load from `~/.chai/config.json` or `CHAI_CONFIG_PATH`; auth required when binding beyond loopback.
- **Session and routing** — Added in-memory `SessionStore` and `SessionBindingStore`; single-turn agent flow; WS `send` and `agent` with optional channel delivery.
- **LLM** — Added Ollama client (`list_models`, `chat`, `chat_stream`), tool-call parsing, model discovery at startup; agent loop with non-streaming chat and tool execution (up to 5 iterations).
- **Channels** — Added Telegram channel (long-poll or webhook); `setWebhook`, `POST /telegram/webhook`, `deleteWebhook` on shutdown; inbound → session → agent → reply; `send_message` for agent replies.
- **Skills** — Added loader (config dir skills + extraDirs), gating by `metadata.requires.bins`; bundled skills `notesmd-cli` and `obsidian` with SKILL.md and tools.json; generic executor from descriptors; optional read-on-demand (`skills.contextMode`: compact list + `read_skill` tool); optional scripts (`skills.allowScripts`, `scripts/` for param resolution).
- **Pairing** — Added device signing, pairing store (`~/.chai/paired.json`), deviceToken in hello-ok; desktop device identity and `~/.chai/device_token` persistence; auto-approve when gateway token provided or auth none.
- **Safe execution** — Allowlisted binary and subcommand execution in `lib/exec` (no shell); allowlist and execution mapping from each skill’s `tools.json`.
