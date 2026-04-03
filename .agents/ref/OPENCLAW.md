---
status: current
---

# OpenClaw Reference

Reference extracted from OpenClaw documentation and code for continuation work on this project (e.g. pending pairing, read-on-demand skills, exec approvals). Use it to align or extend without depending on the full OpenClaw repo.

## Purpose and How to Use

- **Purpose:** Summarize OpenClaw concepts, gateway protocol, pairing, skills, agent loop, exec/sandboxing, config paths, and channels so we can implement missing features or document differences.
- **How to use:** When adding features (pending pairing, read tool for skills, exec-approval flow, etc.), consult this doc and the official documentation (below) for the comparable behavior; implement in Rust/lib as appropriate for this project.

## Official OpenClaw Documentation

- **Website:** https://openclaw.ai/
- **Repository:** https://github.com/openclaw/openclaw
- **Documentation:** https://docs.openclaw.ai/
- **Config schema:** https://openclaw.ai/config.json

Relevant doc areas: gateway (security, configuration, remote, tailscale, health, troubleshooting), device pairing, control UI, concepts (agent-workspace, multi-agent), tools (web, skills), security, sandbox, automation/hooks, CLI (webhooks, status, agent, nodes).

## Chai vs OpenClaw (Feature Summary)

Minimal scan. **Yes** = supported, **Partial** = subset or different shape, **No** = not present, **N/A** = not applicable. See [Chai vs OpenClaw (Detailed)](#chai-vs-openclaw-detailed) for prose-level rows.

| Feature | Chai | OpenClaw |
|---------|------|----------|
| WebSocket gateway + `agent` turn | Yes | Yes |
| Protocol version | v1 | v3 |
| Operator / **node** roles, node pairing API | No | Yes |
| Pending device/node approval (CLI/UI) | No | Yes |
| Many messaging channels (Discord, Slack, …) | No (Telegram) | Yes |
| Skills: compact list + **read** tool on demand | Partial (`read_skill`) | Yes (`read`) |
| Skills: full SKILL.md in system context | Yes (`contextMode: full`) | No (compact + read) |
| Plugin / extension model | No | Yes |
| `agent` → `runId` + `agent.wait` | No | Yes |
| Tool exec **approval** + gateway sandbox options | No | Yes |
| TLS / cert pinning on gateway | Partial | Yes |

## Overview

The following subsections summarize OpenClaw’s gateway protocol, pairing, system prompt and skills, context, agent loop, exec and sandboxing, config and state paths, and channels. Use them to align or extend this project’s implementation. Quick **Chai vs OpenClaw** scan: [Feature Summary](#chai-vs-openclaw-feature-summary); narrative rows: [Detailed](#chai-vs-openclaw-detailed).

### Gateway Protocol

- **Connect handshake**: Same idea as the current implementation — `connect.challenge` (nonce, etc.), client sends `connect` with optional device identity and auth. OpenClaw uses **protocol version 3** (Chai uses **1** pre-release).
- **Roles**: Operator vs **node** (e.g. mobile apps, other clients). Nodes have scopes, caps, commands, permissions.
- **Presence**: `system-presence` for node status.
- **Idempotency**: Keys for duplicate suppression.
- **Device tokens**: `device.token.rotate` and `device.token.revoke` for rotation/revocation.
- **Security**: TLS and cert pinning options; control UI can disable device auth (break-glass).

### Pairing (Device vs Node)

- **Device pairing**: First-time connection; device signs challenge; gateway can require approval.
- **Node pairing**: Separate `node.pair.*` API for pairing nodes (e.g. iOS/Android).
- **Pending requests**: Operator sees pending requests; CLI: `nodes pending`, `nodes approve` / `nodes reject`. Events: `node.pair.requested`, `node.pair.resolved`. Pending requests can expire (e.g. 5 min). Optional silent approval (e.g. SSH to gateway host).

### System Prompt and Skills

- **Compact list**: System prompt gets a **compact list** of skills (name, description, path), not full SKILL.md text.
- **Read-on-demand**: Model is instructed to use a **`read` tool** to load SKILL.md when a skill applies. Keeps base prompt smaller and scales to many skills.

### Context

- Workspace bootstrap files (e.g. AGENTS.md, SOUL.md) and hooks (e.g. `agent:bootstrap`) provide context and identity.

### Agent Loop

- **Stack**: pi-agent-core; streaming lifecycle events.
- **RPC**: `agent` returns `runId`; `agent.wait` for completion.
- **Hooks**: e.g. `agent:bootstrap`.
- **Queue and concurrency**: Control for multiple runs.
- **Sandboxing**: Optional sandbox for execution.

### Exec and Sandboxing

- Tool policy and **exec approvals** (user approval before running commands).
- **Sandboxing** for isolation. Docs: https://docs.openclaw.ai/sandbox
- Channel allowlists and security hardening (https://docs.openclaw.ai/security).

### Config and State Paths

- **Config**: `~/.openclaw/openclaw.json` (or `$OPENCLAW_STATE_DIR/openclaw.json`). Env: `OPENCLAW_HOME`, `CONFIG_PATH`.
- **Gateway token**: Stored in config (`gateway.auth.token`) or env `OPENCLAW_GATEWAY_TOKEN`.

### Channels and Extensions

- **Channels**: Telegram, Discord, Slack, Signal, iMessage, web (WhatsApp), etc. Core channel docs: `docs/channels/`; extensions add more (e.g. MSteams, Matrix, Zalo, voice-call).
- **Channel-specific config**: Topics, allowlists, etc.
- **Messaging**: Consider all built-in + extension channels when refactoring shared logic (routing, allowlists, pairing, command gating).

## Chai vs OpenClaw (Detailed)

Finer-grained differences between **Chai** and **OpenClaw** only (supplements [CLAW_ECOSYSTEM.md](CLAW_ECOSYSTEM.md)).

| Area | Chai (this repo) | OpenClaw (from docs) |
|------|------------------|------------------------|
| **Language & stack** | Rust; CLI + desktop (egui/eframe). | TypeScript/Node; CLI, gateway, web UI, macOS app; plugins/extensions. |
| **Scope** | Telegram channel; bundled skills (notesmd, notesmd-daily, obsidian, obsidian-daily); multiple LLM **backends** (Ollama, LM Studio, vLLM, NIM, …). | Many channels; many skills; multiple **model providers**; nodes (iOS/Android); plugins. |
| **Gateway protocol** | Connect handshake with `connect.challenge`, optional `params.device`, optional `params.auth.deviceToken`; hello-ok with optional `auth.deviceToken`; methods `health`, `status`, `send`, `agent`. Protocol version **1** (pre-release); nested `status` payload (`clock`, `gateway`, `channels`, `providers`, `agents`, `skillPackages`; `entries` per agent id under `agents`). | Same connect.challenge/device/deviceToken idea; protocol version **3**; roles (operator/node), scopes, caps/commands/permissions for nodes; presence (`system-presence`); idempotency keys; `device.token.rotate`/`device.token.revoke`; TLS and cert pinning. |
| **Pairing** | Device signing + pairing store; **auto-approve** when client provides gateway token (or auth is none). No pending-request UI or CLI. Store: `~/.chai/paired.json`. | Device signing; **pending requests** with approval/reject (CLI: `nodes pending`, `nodes approve`/`reject`; events `node.pair.requested`/`node.pair.resolved`). Optional silent approval (e.g. SSH to gateway host). Pending requests expire (e.g. 5 min). Separate `node.pair.*` API for node pairing. |
| **Skills in the agent** | **Full or compact**: `skills.contextMode` **`full`** (default; full SKILL.md per skill in system message) or **`readOnDemand`** (compact list + **`read_skill`** tool). Tools from skills’ `tools.json`; optional scripts via `resolveCommand.script`. | **Compact list** in system prompt (name, description, path); model uses a **`read` tool** to load SKILL.md **on demand**. |
| **Agent loop** | Session load, append message, call configured **Provider**, tool iterations (capped), reply to channel when applicable. | pi-agent-core; streaming lifecycle events; `agent` returns `runId`; `agent.wait`; hooks (e.g. `agent:bootstrap`); queue/concurrency; workspace bootstrap; sandboxing options. |
| **Security / ops** | Gateway token or deviceToken; loopback vs non-loopback bind; no WASM sandbox or exec-approval flow in-repo. | Tool policy, exec approvals, sandboxing, channel allowlists; control UI can disable device auth (break-glass); TLS pinning. |
