# OpenClaw Reference

Reference extracted from OpenClaw documentation and code for continuation work on this project (e.g. pending pairing, read-on-demand skills, exec approvals). Use it to align or extend without depending on the full OpenClaw repo.

## Purpose and how to use

- **Purpose**: Summarize OpenClaw concepts, gateway protocol, pairing, skills, agent loop, exec/sandboxing, config paths, and channels so we can implement missing features or document differences.
- **How to use**: When adding features (pending pairing, read tool for skills, exec-approval flow, etc.), consult this doc and the official docs (URLs below) for the intended behavior; implement in Rust/lib as appropriate for this project.

## Official OpenClaw URLs

- **Website**: https://openclaw.ai/
- **Repository**: https://github.com/openclaw/openclaw
- **Documentation**: https://docs.openclaw.ai/ (Mintlify)
- **Config schema**: https://openclaw.ai/config.json

Relevant doc areas: gateway (security, configuration, remote, tailscale, health, troubleshooting), device pairing, control UI, concepts (agent-workspace, multi-agent), tools (web, skills), security, sandbox, automation/hooks, CLI (webhooks, status, agent, nodes).

## Gateway protocol

- **Connect handshake**: Same idea as this POC — `connect.challenge` (nonce, etc.), client sends `connect` with optional device identity and auth. OpenClaw uses **protocol version 3** (this POC uses 1).
- **Roles**: Operator vs **node** (e.g. mobile apps, other clients). Nodes have scopes, caps, commands, permissions.
- **Presence**: `system-presence` for node status.
- **Idempotency**: Keys for duplicate suppression.
- **Device tokens**: `device.token.rotate` and `device.token.revoke` for rotation/revocation.
- **Security**: TLS and cert pinning options; control UI can disable device auth (break-glass).

## Pairing (device vs node)

- **Device pairing**: First-time connection; device signs challenge; gateway can require approval.
- **Node pairing**: Separate `node.pair.*` API for pairing nodes (e.g. iOS/Android).
- **Pending requests**: Operator sees pending requests; CLI: `nodes pending`, `nodes approve` / `nodes reject`. Events: `node.pair.requested`, `node.pair.resolved`. Pending requests can expire (e.g. 5 min). Optional silent approval (e.g. SSH to gateway host).

## System prompt and skills

- **Compact list**: System prompt gets a **compact list** of skills (name, description, path), not full SKILL.md text.
- **Read-on-demand**: Model is instructed to use a **`read` tool** to load SKILL.md when a skill applies. Keeps base prompt smaller and scales to many skills.

## Context

- Workspace bootstrap files (e.g. AGENTS.md, SOUL.md) and hooks (e.g. `agent:bootstrap`) provide context and identity.

## Agent loop

- **Stack**: pi-agent-core; streaming lifecycle events.
- **RPC**: `agent` returns `runId`; `agent.wait` for completion.
- **Hooks**: e.g. `agent:bootstrap`.
- **Queue and concurrency**: Control for multiple runs.
- **Sandboxing**: Optional sandbox for execution.

## Exec and sandboxing

- Tool policy and **exec approvals** (user approval before running commands).
- **Sandboxing** for isolation. Docs: https://docs.openclaw.ai/sandbox
- Channel allowlists and security hardening (https://docs.openclaw.ai/security).

## Config and state paths

- **Config**: `~/.openclaw/openclaw.json` (or `$OPENCLAW_STATE_DIR/openclaw.json`). Env: `OPENCLAW_HOME`, `CONFIG_PATH`.
- **Gateway token**: Stored in config (`gateway.auth.token`) or env `OPENCLAW_GATEWAY_TOKEN`.

## Channels and extensions

- **Channels**: Telegram, Discord, Slack, Signal, iMessage, web (WhatsApp), etc. Core channel docs: `docs/channels/`; extensions add more (e.g. MSteams, Matrix, Zalo, voice-call).
- **Channel-specific config**: Topics, allowlists, etc.
- **Messaging**: Consider all built-in + extension channels when refactoring shared logic (routing, allowlists, pairing, command gating).

## Summary table

| Area | OpenClaw (reference) |
|------|------------------------|
| **Stack** | TypeScript/Node; CLI, gateway, web UI, macOS app; plugins/extensions. |
| **Gateway** | Protocol 3; roles (operator/node); scopes, caps, permissions; presence; idempotency; device token rotate/revoke; TLS/pinning. |
| **Pairing** | Pending requests; approve/reject (CLI, events); node.pair API; optional silent approval; expiry. |
| **Skills** | Compact list + `read` tool for SKILL.md on demand. |
| **Agent** | pi-agent-core; streaming; runId/agent.wait; hooks; queue; sandboxing. |
| **Channels** | Many (Telegram, Discord, Slack, Signal, etc.); extensions; allowlists. |
| **Security** | Exec approvals, sandboxing, channel allowlists, TLS pinning. |
