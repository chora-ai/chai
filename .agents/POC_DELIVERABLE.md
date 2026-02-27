# Proof-of-Concept — Deliverable

This document is a **record of what was completed** during the proof-of-concept implementation.

The POC followed the short-term goals described in [VISION.md](../VISION.md):

1. Running a gateway with the CLI or Desktop application  
2. Support for large language models running locally (via Ollama)  
3. Support for at least one communication channel (Telegram)  
4. Support for at least one skill (managing an Obsidian vault)  
5. A modular architecture that makes it easy to extend the above  

## What was completed

- **Gateway (CLI + Desktop + lib)** — HTTP and WebSocket on one port; connect handshake with optional auth (token or device); `connect.challenge`, device signing, pairing store, and deviceToken in hello-ok; graceful shutdown with broadcast and channel await; WS methods: `health`, `status`, `send`, `agent`. See [Pairing](POC_IMPLEMENTATION.md#pairing) for device/pairing details.
- **LLM** — Ollama client (list_models, chat, chat_stream), tool-call parsing, model discovery at startup; agent loop uses non-streaming chat with optional streaming and tool execution. See [Skills and the LLM](POC_IMPLEMENTATION.md#skills-and-the-llm) for how skills and context are used.
- **Channels** — One channel: Telegram (long-polling or webhook). Config, registry, inbound → session → agent → reply; `send_message` for agent replies. Webhook: `setWebhook`, `POST /telegram/webhook`, `deleteWebhook` on shutdown.
- **Skills** — Loader from config dir `skills` (~/.chai/skills) and config `skills.extraDirs`; gating by `metadata.requires.bins`; two bundled skills: `notesmd-cli` and `obsidian` (each with `SKILL.md` and `tools.json`; optional `scripts/` when `skills.allowScripts`). See [Skills and the LLM](POC_IMPLEMENTATION.md#skills-and-the-llm).
- **Modularity** — lib modules: config, gateway, llm, channels, skills, exec, tools, device. CLI and desktop both use lib; desktop spawns `chai gateway` for Start gateway and uses device identity for connect when fetching status.

For more details about the implementation, see [POC_IMPLEMENTATION.md](POC_IMPLEMENTATION.md).

## Differences from OpenClaw

The following is based on the OpenClaw documentation and code. It summarizes how this POC implementation differs from OpenClaw, not a full feature comparison. For continuation work (e.g. adding pending pairing, read-on-demand skills, exec approvals), a more detailed reference extracted from OpenClaw is in [OPENCLAW_REFERENCE.md](ref/OPENCLAW_REFERENCE.md).

| Area | This POC (Chai) | OpenClaw (from docs) |
|------|-------------------|------------------------|
| **Language & stack** | Rust; single binary (CLI) + desktop (egui/eframe). | TypeScript/Node; CLI, gateway, web UI, macOS app; plugins/extensions. |
| **Scope** | One channel (Telegram), two bundled skills (notesmd-cli, Obsidian), one LLM (Ollama). | Many channels (Telegram, Discord, Slack, Signal, etc.), many skills, multiple LLM providers; nodes (iOS/Android), plugins. |
| **Gateway protocol** | Connect handshake with `connect.challenge`, optional `params.device`, optional `params.auth.deviceToken`; hello-ok with optional `auth.deviceToken`; methods `health`, `status`, `send`, `agent`. Protocol version 1. | Same connect.challenge/device/deviceToken idea; protocol version 3; roles (operator/node), scopes, caps/commands/permissions for nodes; presence (`system-presence`); idempotency keys; `device.token.rotate`/`device.token.revoke`; TLS and cert pinning. |
| **Pairing** | Device signing + pairing store; **auto-approve** when client provides gateway token (or auth is none). No pending-request UI or CLI. Store: `~/.chai/paired.json`. | Device signing; **pending requests** with approval/reject (CLI: `nodes pending`, `nodes approve`/`reject`; events `node.pair.requested`/`node.pair.resolved`). Optional silent approval (e.g. SSH to gateway host). Pending requests expire (e.g. 5 min). Separate `node.pair.*` API for node pairing. |
| **Skills in the agent** | **Full or compact**: `skills.contextMode` **`full`** (default; full SKILL.md per skill in system message) or **`readOnDemand`** (compact list + **`read_skill`** tool to load SKILL.md on demand). Tools from skills’ `tools.json`; optional scripts for param resolution when `skills.allowScripts`. | **Compact list** in system prompt (name, description, path); model is instructed to use a **`read` tool** to load SKILL.md **on demand** when a skill applies. Keeps base prompt smaller; scales to many skills. |
| **Agent loop** | Single turn: load session, append user message, call Ollama (with skill context and tools), parse tool calls, execute via allowlist, optionally re-call model (max 5 tool iterations); return reply and optionally deliver to channel. | pi-agent-core; streaming lifecycle events; `agent` RPC returns `runId`; `agent.wait` for completion; hooks (e.g. `agent:bootstrap`); queue and concurrency control; workspace bootstrap (AGENTS.md, SOUL.md, etc.); sandboxing. |
| **Channels** | Telegram only; long-poll or webhook. | Many channels; extensions for additional channels; channel-specific config (e.g. topics, allowlists). |
| **Security / ops** | Gateway token or deviceToken; loopback vs non-loopback bind; no sandboxing, no exec-approval flow, no plugin isolation. | Tool policy, exec approvals, sandboxing, channel allowlists; control UI can disable device auth (break-glass); TLS pinning. |

## Next steps beyond the POC

These are natural extensions once the POC is accepted; they are not part of the current deliverable.

- **Gateway / pairing** — Pending pairing approval (operator UI or CLI) instead of auto-approve; device token rotation/revocation; optional TLS and cert pinning; protocol versioning and schema generation.
- **Skills and agent** — Further skill scaling; workspace bootstrap files (e.g. AGENTS.md, identity); streaming agent replies and lifecycle events; exec-approval flow and sandboxing.
- **Channels and clients** — Additional channels (e.g. Discord, Slack); CLI use of device identity when connecting to a remote gateway; richer desktop UI (sessions, logs, model selection).
- **Platform** — Packaging and distribution (e.g. installers); optional plugins/extensions model; documentation and operator runbooks.
