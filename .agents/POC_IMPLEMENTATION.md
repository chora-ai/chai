# POC Implementation — Deliverable

This document is a **record of what was completed** during the proof-of-concept implementation. The POC followed the short-term goals described in `VISION.md`:

1. Running a gateway with the CLI or Desktop application  
2. Support for large language models running locally (via Ollama)  
3. Support for at least one communication channel (Telegram)  
4. Support for at least one skill (managing an Obsidian vault)  
5. A modular architecture that makes it easy to extend the above  

All planned POC scope has been implemented. Code style follows `AGENTS.md`: minimal dependencies, lowercase log and error messages, and separation so CLI and desktop share lib.

**Contents** — [What was completed (overview)](#what-was-completed-overview) · [Differences from OpenClaw](#differences-from-openclaw) · [Next steps beyond the POC](#next-steps-beyond-the-poc) · [Implementation Details](#implementation-details) · [Additional Information](#additional-information) ([Gateway auth and shutdown](#gateway-auth-and-shutdown), [Session and channel routing](#session-and-channel-routing), [Safe execution](#safe-execution), [Telegram channel (long-poll vs webhook)](#telegram-channel-long-poll-vs-webhook), [Pairing](#pairing), [Skills and the LLM](#skills-and-the-llm))

## What was completed (overview)

- **Gateway (CLI + Desktop + lib)** — HTTP and WebSocket on one port; connect handshake with optional auth (token or device); `connect.challenge`, device signing, pairing store, and deviceToken in hello-ok; graceful shutdown with broadcast and channel await; WS methods: `health`, `status`, `send`, `agent`. See [Pairing](#pairing) for device/pairing details.
- **LLM** — Ollama client (list_models, chat, chat_stream), tool-call parsing, model discovery at startup; agent loop uses non-streaming chat with optional streaming and tool execution. See [Skills and the LLM](#skills-and-the-llm) for how skills and context are used.
- **Channels** — One channel: Telegram (long-polling or webhook). Config, registry, inbound → session → agent → reply; `send_message` for agent replies. Webhook: `setWebhook`, `POST /telegram/webhook`, `deleteWebhook` on shutdown.
- **Skills** — Loader (bundled, workspace, extra), gating by `metadata.requires.bins`, one bundled skill: Obsidian (SKILL.md + safe exec via obsidian-cli + tool layer). See [Skills and the LLM](#skills-and-the-llm).
- **Modularity** — lib modules: config, gateway, llm, channels, skills, exec, tools, device. CLI and desktop both use lib; desktop spawns `chai gateway` for Start gateway and uses device identity for connect when fetching status.

For more detail, see [Implementation Details](#implementation-details) and [Additional Information](#additional-information) (gateway auth and shutdown; session and channel routing; safe execution; Telegram channel; pairing; skills and the LLM).

## Differences from OpenClaw

The following is based on the OpenClaw documentation and code. It summarizes how this POC implementation differs from OpenClaw, not a full feature comparison. For continuation work (e.g. adding pending pairing, read-on-demand skills, exec approvals), a more detailed reference extracted from OpenClaw is in [OPENCLAW_REFERENCE.md](.agents/reference/OPENCLAW_REFERENCE.md).

| Area | This POC (Chai) | OpenClaw (from docs) |
|------|-------------------|------------------------|
| **Language & stack** | Rust; single binary (CLI) + desktop (egui/eframe). | TypeScript/Node; CLI, gateway, web UI, macOS app; plugins/extensions. |
| **Scope** | One channel (Telegram), one skill (Obsidian), one LLM (Ollama). | Many channels (Telegram, Discord, Slack, Signal, etc.), many skills, multiple LLM providers; nodes (iOS/Android), plugins. |
| **Gateway protocol** | Connect handshake with `connect.challenge`, optional `params.device`, optional `params.auth.deviceToken`; hello-ok with optional `auth.deviceToken`; methods `health`, `status`, `send`, `agent`. Protocol version 1. | Same connect.challenge/device/deviceToken idea; protocol version 3; roles (operator/node), scopes, caps/commands/permissions for nodes; presence (`system-presence`); idempotency keys; `device.token.rotate`/`device.token.revoke`; TLS and cert pinning. |
| **Pairing** | Device signing + pairing store; **auto-approve** when client provides gateway token (or auth is none). No pending-request UI or CLI. Store: `~/.chai/paired.json`. | Device signing; **pending requests** with approval/reject (CLI: `nodes pending`, `nodes approve`/`reject`; events `node.pair.requested`/`node.pair.resolved`). Optional silent approval (e.g. SSH to gateway host). Pending requests expire (e.g. 5 min). Separate `node.pair.*` API for node pairing. |
| **Skills in the agent** | **Full skill content** (name, description, full SKILL.md text) injected into the system message for every agent turn. Small, fixed set of skills. | **Compact list** in system prompt (name, description, path); model is instructed to use a **`read` tool** to load SKILL.md **on demand** when a skill applies. Keeps base prompt smaller; scales to many skills. |
| **Agent loop** | Single turn: load session, append user message, call Ollama (with skill context and tools), parse tool calls, execute via allowlist, optionally re-call model (max 5 tool iterations); return reply and optionally deliver to channel. | pi-agent-core; streaming lifecycle events; `agent` RPC returns `runId`; `agent.wait` for completion; hooks (e.g. `agent:bootstrap`); queue and concurrency control; workspace bootstrap (AGENTS.md, SOUL.md, etc.); sandboxing. |
| **Channels** | Telegram only; long-poll or webhook. | Many channels; extensions for additional channels; channel-specific config (e.g. topics, allowlists). |
| **Security / ops** | Gateway token or deviceToken; loopback vs non-loopback bind; no sandboxing, no exec-approval flow, no plugin isolation. | Tool policy, exec approvals, sandboxing, channel allowlists; control UI can disable device auth (break-glass); TLS pinning. |

## Next steps beyond the POC

These are natural extensions once the POC is accepted; they are not part of the current deliverable.

- **Gateway / pairing** — Pending pairing approval (operator UI or CLI) instead of auto-approve; device token rotation/revocation; optional TLS and cert pinning; protocol versioning and schema generation.
- **Skills and agent** — Read-on-demand skill loading (compact list + `read` tool) to scale skills and reduce context size; workspace bootstrap files (e.g. AGENTS.md, identity); streaming agent replies and lifecycle events; exec-approval flow and sandboxing.
- **Channels and clients** — Additional channels (e.g. Discord, Slack); CLI use of device identity when connecting to a remote gateway; richer desktop UI (sessions, logs, model selection).
- **Platform** — Packaging and distribution (e.g. installers); optional plugins/extensions model; documentation and operator runbooks.

## Implementation Details

Concrete reference: CLI options, config paths, gateway and channel behavior, and module responsibilities. This section was relocated from the original working document; nothing was removed.

- **CLI** — `chai gateway [--config PATH] [--port PORT]`: config from `--config`, or `CHAI_CONFIG_PATH`, or `~/.chai/config.json` (defaults if missing); `--port` overrides port; `RUST_LOG=info` for logs. Runs gateway in-process via `lib::gateway::run_gateway(config)`.
- **Desktop** — Start/Stop gateway (Start spawns `chai gateway` from same directory as executable or PATH; Stop kills that subprocess). Gateway detection: ~1 s TCP probe to bind:port (800 ms timeout); "Gateway: running" when probe succeeds; "Stop gateway" only when this app started the process. Live details: WebSocket connect then `status`; displays protocol, port, bind, auth, and discovered Ollama models at ~0.5 Hz; uses token from config or `CHAI_GATEWAY_TOKEN` when auth is enabled. Errors (e.g. config load, spawn failure) shown in red.
- **Config** — `Config` (gateway, channels, agents, skills). Load from `~/.chai/config.json` or `CHAI_CONFIG_PATH`. Defaults: port 15151, bind 127.0.0.1. Auth required when binding beyond loopback (startup fails otherwise).
- **Gateway server** — Single port: `GET /` returns health JSON; `GET /ws` upgrades to WebSocket. First frame from client must be `connect`; server sends `connect.challenge` then accepts `connect` and replies with `hello-ok`. Graceful shutdown: on SIGINT/SIGTERM, broadcast `shutdown` event to all WS clients (each subscriber receives the frame and handler exits), then await registered in-process channel tasks, then stop accepting and drain; no timeout.
- **Session, routing, agent** — `SessionStore` (in-memory: create, get_or_create, get, append_message). `SessionBindingStore`: binds (channel_id, conversation_id) ↔ session_id for inbound routing and outbound delivery. Agent: `run_turn` loads session history, prepends system message (skill context), calls Ollama (non-streaming chat; tool_calls parsed and executed via allowlist; up to 5 iterations), appends assistant and tool messages. Channel reply: only the model’s text when non-empty; when reply is empty (e.g. tool-calls-only), nothing is sent to the channel (no placeholder). WS `send`: params `channelId`, `conversationId`, `message` → registry `send_message`. WS `agent`: params `sessionId?`, `message` → get-or-create session, run turn, return `{ reply, sessionId, toolCalls }`; if session is bound to a channel and reply has non-empty text, deliver to that channel via registry.
- **LLM (Ollama)** — `OllamaClient::new(base_url?)`; default base URL `http://127.0.0.1:11434`. `list_models()`, `chat()`, `chat_stream(model, messages, tools?, on_chunk)`; non-streaming and streaming; `chat_stream` parses NDJSON and invokes `on_chunk` for content deltas; tool_calls taken from stream when present. `ChatMessage`/`ChatResponse` with optional `tool_calls` and helpers `content()`, `tool_calls()`.
- **Ollama model discovery** — At startup a task calls `list_models()`; list stored in state and exposed in WS `status` as `ollamaModels` (array of `{ name, size? }`). If Ollama unreachable, list is empty (debug log).
- **Channels** — `ChannelHandle` (id, stop, async `send_message(conversation_id, text)`), `ChannelRegistry` (register, get, ids). `InboundMessage`: channel_id, conversation_id, text over mpsc. Telegram: bot token from config or `TELEGRAM_BOT_TOKEN`; when set, gateway starts the channel. If `channels.telegram.webhookUrl` is set: `setWebhook(url, secret_token?)`, register channel, no getUpdates loop; Telegram POSTs to gateway. If not set: long-poll getUpdates (30 s timeout); stops on `stop()`. Webhook endpoint: `POST /telegram/webhook`; optional `webhookSecret` checked via header `X-Telegram-Bot-Api-Secret-Token`; same `InboundMessage` flow as getUpdates. Shutdown: `deleteWebhook`. `send_message`: Telegram Bot API `sendMessage`. Inbound processor: receive `InboundMessage` → get-or-create session, bind, append user message, run one agent turn, send reply via channel `send_message`. When no Telegram token is configured, the channel is not started.
- **Skills (loader and Obsidian)** — `load_skills(bundled_dir, managed_dir, workspace_dir, extra_dirs)`: all `*/SKILL.md`, YAML frontmatter (name, description), merge by name with precedence extra &lt; bundled &lt; managed &lt; workspace. Gating: `metadata.requires.bins` — skill is loaded only when all listed binaries are on PATH. bundled skills: config directory `skills` subdirectory (populated by `chai init`). Obsidian: bundled `lib/skills/obsidian/SKILL.md`; safe exec (`lib/exec`): allowlisted binary and subcommands only (no shell); obsidian-cli allowlist: search, search-content, create, move, delete, set-default, print-default. Tool layer (`lib/tools/obsidian`): when the Obsidian skill is loaded, the agent gets tools `obsidian_search`, `obsidian_search_content`, `obsidian_create`, `obsidian_move`, `obsidian_delete`; session stores assistant and tool messages for history.
- **Modularity** — Desktop reuses lib: same config, gateway types, Ollama client, channel registry, skill loader, device identity. CLI runs gateway in-process; desktop spawns CLI subprocess for Start gateway.

## Additional Information

### Gateway auth and shutdown

- **Auth**: When the gateway binds to a non-loopback address, startup requires auth to be configured (token or device); otherwise the process exits. This avoids exposing an unauthenticated server on the network. On loopback (e.g. 127.0.0.1), auth can be disabled (`gateway.auth.mode: none`). Connect accepts either the shared gateway token (`params.auth.token`) or a device token from the pairing store (`params.auth.deviceToken`).
- **Shutdown**: On SIGINT or SIGTERM, the server broadcasts a `shutdown` event to all connected WebSocket clients so they can close cleanly, then awaits any registered in-process channel tasks (e.g. the Telegram long-poll loop), then stops accepting new connections and drains in-flight work. There is no timeout; the process exits when the broadcast and channel tasks are done.

### Session and channel routing

- **Session store**: In-memory sessions keyed by session id; each session holds a message history (user, assistant, tool messages) used for the next agent turn.
- **Binding store**: Maps (channel_id, conversation_id) to session_id. When an inbound message arrives (e.g. from Telegram), the gateway looks up or creates a session for that channel and conversation, appends the user message, runs one agent turn, then sends the reply back via the channel’s `send_message`. Outbound delivery (e.g. after an explicit `agent` request from a client) uses the same binding: if the session is bound to a channel and the reply has non-empty text, it is also delivered to that channel.
- **Single-turn flow**: Each inbound message or `agent` request triggers one `run_turn` (load history, call Ollama, execute tool calls in a loop up to 5 times, append results). There is no streaming to the channel in this POC; the full reply is sent when the turn completes. Empty replies (e.g. tool-calls only with no text) are not sent to the channel.

### Safe execution

- **Allowlist**: Tool execution (e.g. for the Obsidian skill) uses `lib/exec`: only an allowlisted binary and allowlisted subcommands can be run. There is no shell; arguments are passed explicitly. For obsidian-cli, the allowlist includes: search, search-content, create, move, delete, set-default, print-default.
- **Rationale**: This limits the impact of malicious or buggy model output: the model can only invoke known commands. It does not provide full sandboxing (e.g. filesystem or network isolation) or an exec-approval flow; those are listed under [Next steps beyond the POC](#next-steps-beyond-the-poc).

### Telegram channel (long-poll vs webhook)

- **When to use which**: If `channels.telegram.webhookUrl` is set in config, the gateway uses **webhook mode**: it calls the Telegram API `setWebhook(url, secret_token?)`, registers the channel, and does **not** start the getUpdates long-poll loop. Telegram then POSTs updates to that URL. If `webhookUrl` is not set, the gateway uses **long-poll mode**: it runs a getUpdates loop (30 s timeout) in-process and stops it on shutdown. Webhook is useful when the gateway is reachable from the internet (e.g. behind a reverse proxy); long-poll is simpler for local or development setups.
- **Same inbound flow**: Both modes produce the same `InboundMessage` (channel_id, conversation_id, text) and feed the same processor: get-or-create session, bind, append user message, run one agent turn, send reply via the channel. The webhook endpoint is `POST /telegram/webhook`; an optional `webhookSecret` is checked via the `X-Telegram-Bot-Api-Secret-Token` header.
- **Shutdown**: On shutdown, if webhook mode was used, the gateway calls `deleteWebhook` so the bot can use getUpdates again after restart (e.g. if you switch back to long-poll). When no Telegram token is configured, the channel is not started.

### Pairing

**What pairing is**

Pairing is how the gateway trusts a **device** (laptop, phone, another machine) the first time it connects, without the user typing a shared token on that device.

- Every client can have a **device identity**: a keypair; `deviceId` is a fingerprint of the public key.
- When a **new** device connects, the gateway can require **proof that the client holds the private key**: the gateway sends a **`connect.challenge`** event (nonce + ts); the client signs a canonical payload (deviceId, client id/mode, role, scopes, signedAt, token, nonce) and sends `connect` with `params.device`: `{ id, publicKey, signature, signedAt, nonce }`.
- The gateway verifies the signature. If the device is not yet in the **pairing store**, the POC **auto-approves** when the client has already provided the gateway token (or auth is none): it issues a **device token**, stores it in `~/.chai/paired.json`, and returns it in `hello-ok.auth.deviceToken`. The client can persist that token and use `params.auth.deviceToken` on later connects so it does not need to sign again.

**Implemented in this POC**

- **Gateway**: On WebSocket open, send `connect.challenge` (nonce UUID v4, ts ms). Connect handler accepts optional `params.device` (verify nonce + Ed25519 signature) and optional `params.auth.deviceToken` (lookup in pairing store). Pairing store: add or look up by device id; issue and persist device token; return `auth: { deviceToken, role, scopes }` in hello-ok when applicable.
- **Desktop**: When fetching gateway status, read first frame (challenge), then send connect. If `~/.chai/device_token` exists, send only `auth.deviceToken`. Otherwise load or create `~/.chai/device.json` (Ed25519 keypair), sign the same canonical payload, send `params.device` and optional `auth.token`; on hello-ok, persist `auth.deviceToken` to `~/.chai/device_token`.

**Possible future work**

- Pending pairing requests with operator approval (CLI/UI) instead of auto-approve.
- Device token rotation/revocation.
- CLI using device identity when connecting to a remote gateway.

### Skills and the LLM

**How this POC uses skills**

- Skills are loaded at gateway startup from bundled, workspace, and config-specified dirs. Each skill is a `SKILL.md` with YAML frontmatter (name, description). Gating: if a skill declares `metadata.requires.bins`, the skill is loaded only when all listed binaries are on PATH.
- For each agent turn, the gateway builds a **system message** that includes the **full content** (name, description, full SKILL.md text) of all loaded skills. The model sees the entire skill set in context and can use the described tools (e.g. obsidian_search, obsidian_create) when relevant.
- The Obsidian skill is bundled; when loaded (gated by `obsidian-cli` on PATH), the agent gets Ollama tools for search/create/move/delete. Tool calls are executed via an allowlisted executor (no shell); the agent loop can run up to 5 tool-iteration steps per turn.

**Contrast with OpenClaw**

- In OpenClaw, the system prompt contains a **compact list** of skills (name, description, path). The model is instructed to use a **`read` tool** to load the skill’s SKILL.md **only when** that skill clearly applies. Full skill text is not injected up front. That keeps the base prompt smaller and scales to many skills.
- This POC does **not** implement read-on-demand: it injects full skill content for a small set of skills. A future step could add the compact list + `read` pattern for better scaling and smaller context.

**Local models (e.g. Llama 3)**

- **Context**: Read-on-demand would help local models with limited context; currently, all loaded skill text is in every turn.
- **Instruction following**: Having the model choose “when to read which skill” and then follow it varies by model size and tool-calling support.
- **Tool use**: The Obsidian tool layer and allowlisted exec work with local models that support tool/function calling (e.g. Llama 3); behavior depends on model capability.
