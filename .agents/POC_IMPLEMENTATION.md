# Proof-of-Concept — Implementation

This document is the **detailed technical reference** for the proof-of-concept implementation.

## Implementation Details

The following subsections are a reference for developers: CLI and desktop behavior, config, gateway server, session/routing/agent, LLM, Ollama model discovery, channels, skills, modularity, and how the crates fit together.

### CLI

- **Command**: `chai gateway [--config PATH] [--port PORT]`.
- **Config**: From `--config`, or `CHAI_CONFIG_PATH`, or `~/.chai/config.json` (defaults if missing). `--port` overrides port.
- **Logging**: `RUST_LOG=info` for logs.
- **Execution**: Runs gateway in-process via `lib::gateway::run_gateway(config, config_path)`.

### Desktop

- **Start/Stop**: Start spawns `chai gateway` from same directory as executable or PATH; Stop kills that subprocess.
- **Gateway detection**: ~1 s TCP probe to bind:port (800 ms timeout). "Gateway: running" when probe succeeds; "Stop gateway" only when this app started the process.
- **Live details**: WebSocket connect then `status`; displays protocol, port, bind, auth, and discovered Ollama models at ~0.5 Hz. Uses token from config or `CHAI_GATEWAY_TOKEN` when auth is enabled.
- **Errors**: Config load or spawn failure shown in red.

### Config

- **Structure**: `Config` (gateway, channels, agents, skills). Load from `~/.chai/config.json` or `CHAI_CONFIG_PATH`.
- **Skills**: `skills.directory` (optional override for primary skill root), `skills.extraDirs`, **`skills.enabled`** (list of skill names to load; default empty), **`skills.contextMode`** (`"full"` | `"readOnDemand"`; default `full`), **`skills.allowScripts`** (when true, skills may run scripts from their `scripts/` dir for param resolution; default false).
- **Defaults**: Port 15151, bind 127.0.0.1.
- **Auth**: Required when binding beyond loopback (startup fails otherwise).

### Gateway server

- **Endpoints**: Single port. `GET /` returns health JSON; `GET /ws` upgrades to WebSocket.
- **Connect**: First frame from client must be `connect`; server sends `connect.challenge` then accepts `connect` and replies with `hello-ok`.
- **Graceful shutdown**: On SIGINT/SIGTERM, broadcast `shutdown` event to all WS clients (each subscriber receives the frame and handler exits), then await registered in-process channel tasks, then stop accepting and drain; no timeout.

### Session, routing, and agent

- **SessionStore**: In-memory; create, get_or_create, get, append_message.
- **SessionBindingStore**: Binds (channel_id, conversation_id) ↔ session_id for inbound routing and outbound delivery.
- **Agent `run_turn`**: Loads session history, prepends system message (skill context), calls Ollama (non-streaming chat; tool_calls parsed and executed via allowlist; up to 5 iterations), appends assistant and tool messages.
- **Channel reply**: Only the model’s text when non-empty; when reply is empty (e.g. tool-calls-only), nothing is sent to the channel (no placeholder).
- **WS `send`**: Params `channelId`, `conversationId`, `message` → registry `send_message`.
- **WS `agent`**: Params `sessionId?`, `message` → get-or-create session, run turn, return `{ reply, sessionId, toolCalls }`; if session is bound to a channel and reply has non-empty text, deliver to that channel via registry.

### LLM (Ollama)

- **Client**: `OllamaClient::new(base_url?)`; default base URL `http://127.0.0.1:11434`.
- **API**: `list_models()`, `chat()`, `chat_stream(model, messages, tools?, on_chunk)`; non-streaming and streaming. `chat_stream` parses NDJSON and invokes `on_chunk` for content deltas; tool_calls taken from stream when present.
- **Types**: `ChatMessage`/`ChatResponse` with optional `tool_calls` and helpers `content()`, `tool_calls()`.

### Ollama model discovery

- At startup a task calls `list_models()`; list stored in state and exposed in WS `status` as `ollamaModels` (array of `{ name, size? }`).
- If Ollama unreachable, list is empty (debug log).

### Channels

- **Types**: `ChannelHandle` (id, stop, async `send_message(conversation_id, text)`), `ChannelRegistry` (register, get, ids). `InboundMessage`: channel_id, conversation_id, text over mpsc.
- **Telegram**: Bot token from config or `TELEGRAM_BOT_TOKEN`; when set, gateway starts the channel.
  - If `channels.telegram.webhookUrl` is set: `setWebhook(url, secret_token?)`, register channel, no getUpdates loop; Telegram POSTs to gateway.
  - If not set: long-poll getUpdates (30 s timeout); stops on `stop()`.
- **Webhook endpoint**: `POST /telegram/webhook`; optional `webhookSecret` checked via header `X-Telegram-Bot-Api-Secret-Token`; same `InboundMessage` flow as getUpdates.
- **Shutdown**: `deleteWebhook`. `send_message`: Telegram Bot API `sendMessage`.
- **Inbound processor**: Receive `InboundMessage` → get-or-create session, bind, append user message, run one agent turn, send reply via channel `send_message`.
- When no Telegram token is configured, the channel is not started.

### Skills (loader and bundled skills)

- **Loader**: `load_skills(skills_dir, extra_dirs)`: discovers `*/SKILL.md` under the primary skill root and each extra dir; parses YAML frontmatter (name, description); if a skill dir contains `tools.json`, parses it and attaches `SkillEntry.tool_descriptor`. Merge by name: primary root first, then extra (extra overwrites by name).
- **Skill root**: Primary root = config dir’s `skills` subdirectory, or **`skills.directory`** in config (relative to config file parent). **`skills.extraDirs`** add more roots. Only skills listed in **`skills.enabled`** are loaded (default: none).
- **Gating**: `metadata.requires.bins` — skill is loaded only when all listed binaries are on PATH.
- **Tools**: Tool list and executor come only from skills that have a `tools.json` descriptor. Generic executor builds argv from execution spec and runs via descriptor allowlist; when **`skills.allowScripts`** is true, param resolution can use scripts from a skill’s `scripts/` dir (see [TOOLS_SCHEMA.md](spec/TOOLS_SCHEMA.md)). When **`skills.contextMode`** is **`readOnDemand`**, gateway prepends a **`read_skill`** tool and a wrapper executor that returns a skill’s SKILL.md content.
- **Bundled skills**: The skills that ship with the app (notesmd-cli, obsidian) live in `crates/lib/config/skills/` with SKILL.md and tools.json; `chai init` extracts them to the user’s skill root.
- **Safe exec** (`lib/exec`): Allowlisted binary and subcommands only (no shell). Allowlist is defined per skill in `tools.json`. Session stores assistant and tool messages for history.

### Modularity

- Desktop reuses lib: same config, gateway types, Ollama client, channel registry, skill loader, device identity.
- CLI runs gateway in-process; desktop spawns CLI subprocess for Start gateway.


## Key Concepts Explained

The following subsections describe how the gateway handles authentication and shutdown, how messages are routed to sessions and back to channels, how tool execution is constrained, how the Telegram channel works, how device pairing works, and how skills are loaded and used by the LLM.

### Gateway auth and shutdown

When the gateway is bound beyond loopback, authentication is required; on loopback it can be disabled. Shutdown is graceful so that clients and in-process channel tasks can finish cleanly.

- **Auth**: When the gateway binds to a non-loopback address, startup requires auth to be configured (token or device); otherwise the process exits. This avoids exposing an unauthenticated server on the network. On loopback (e.g. 127.0.0.1), auth can be disabled (`gateway.auth.mode: none`). Connect accepts either the shared gateway token (`params.auth.token`) or a device token from the pairing store (`params.auth.deviceToken`).
- **Shutdown**: On SIGINT or SIGTERM, the server broadcasts a `shutdown` event to all connected WebSocket clients so they can close cleanly, then awaits any registered in-process channel tasks (e.g. the Telegram long-poll loop), then stops accepting new connections and drains in-flight work. There is no timeout; the process exits when the broadcast and channel tasks are done.

### Session and channel routing

Inbound messages from a channel (e.g. Telegram) are mapped to a session by channel and conversation id; one agent turn is run and the reply is sent back via the same channel. The following bullets spell out the stores and the single-turn flow.

- **Session store**: In-memory sessions keyed by session id; each session holds a message history (user, assistant, tool messages) used for the next agent turn.
- **Binding store**: Maps (channel_id, conversation_id) to session_id. When an inbound message arrives (e.g. from Telegram), the gateway looks up or creates a session for that channel and conversation, appends the user message, runs one agent turn, then sends the reply back via the channel’s `send_message`. Outbound delivery (e.g. after an explicit `agent` request from a client) uses the same binding: if the session is bound to a channel and the reply has non-empty text, it is also delivered to that channel.
- **Single-turn flow**: Each inbound message or `agent` request triggers one `run_turn` (load history, call Ollama, execute tool calls in a loop up to 5 times, append results). There is no streaming to the channel in the current implementation; the full reply is sent when the turn completes. Empty replies (e.g. tool-calls only with no text) are not sent to the channel.

### Safe execution

Tool execution for skills is restricted to an allowlist of binaries and subcommands defined in each skill’s **`tools.json`**; there is no shell and no full sandboxing in the current implementation.

- **Allowlist**: Each skill’s `tools.json` declares which (binary, subcommand) pairs it may run. The generic executor builds argv from the descriptor’s execution mapping and calls `lib/exec::Allowlist::run()`. There is no shell; arguments are passed explicitly. No skill-specific code lives in the lib—bundled skills (notesmd-cli, obsidian) ship with their own `tools.json` under `crates/lib/config/skills/`.
- **Rationale**: This limits the impact of malicious or buggy model output: the model can only invoke commands declared in the skill descriptor. It does not provide full sandboxing (e.g. filesystem or network isolation) or an exec-approval flow; those are listed under [What Comes Next](POC_DELIVERABLE.md#what-comes-next).

### Telegram channel (long-poll vs webhook)

The Telegram channel can run in either long-poll mode (getUpdates in-process) or webhook mode (Telegram POSTs to the gateway). Both produce the same inbound flow; the choice depends on whether the gateway is reachable from the internet.

- **When to use which**: If `channels.telegram.webhookUrl` is set in config, the gateway uses **webhook mode**: it calls the Telegram API `setWebhook(url, secret_token?)`, registers the channel, and does **not** start the getUpdates long-poll loop. Telegram then POSTs updates to that URL. If `webhookUrl` is not set, the gateway uses **long-poll mode**: it runs a getUpdates loop (30 s timeout) in-process and stops it on shutdown. Webhook is useful when the gateway is reachable from the internet (e.g. behind a reverse proxy); long-poll is simpler for local or development setups.
- **Same inbound flow**: Both modes produce the same `InboundMessage` (channel_id, conversation_id, text) and feed the same processor: get-or-create session, bind, append user message, run one agent turn, send reply via the channel. The webhook endpoint is `POST /telegram/webhook`; an optional `webhookSecret` is checked via the `X-Telegram-Bot-Api-Secret-Token` header.
- **Shutdown**: On shutdown, if webhook mode was used, the gateway calls `deleteWebhook` so the bot can use getUpdates again after restart (e.g. if you switch back to long-poll). When no Telegram token is configured, the channel is not started.

### Pairing

Pairing is how a new device gains the gateway’s trust (via device signing and an optional device token) so that the user does not have to type the gateway token on every device.

**What pairing is**

Pairing is how the gateway trusts a **device** (laptop, phone, another machine) the first time it connects, without the user typing a shared token on that device.

- Every client can have a **device identity**: a keypair; `deviceId` is a fingerprint of the public key.
- When a **new** device connects, the gateway can require **proof that the client holds the private key**: the gateway sends a **`connect.challenge`** event (nonce + ts); the client signs a canonical payload (deviceId, client id/mode, role, scopes, signedAt, token, nonce) and sends `connect` with `params.device`: `{ id, publicKey, signature, signedAt, nonce }`.
- The gateway verifies the signature. If the device is not yet in the **pairing store**, the current implementation **auto-approves** when the client has already provided the gateway token (or auth is none): it issues a **device token**, stores it in `~/.chai/paired.json`, and returns it in `hello-ok.auth.deviceToken`. The client can persist that token and use `params.auth.deviceToken` on later connects so it does not need to sign again.

**Currently implemented**

- **Gateway**: On WebSocket open, send `connect.challenge` (nonce UUID v4, ts ms). Connect handler accepts optional `params.device` (verify nonce + Ed25519 signature) and optional `params.auth.deviceToken` (lookup in pairing store). Pairing store: add or look up by device id; issue and persist device token; return `auth: { deviceToken, role, scopes }` in hello-ok when applicable.
- **Desktop**: When fetching gateway status, read first frame (challenge), then send connect. If `~/.chai/device_token` exists, send only `auth.deviceToken`. Otherwise load or create `~/.chai/device.json` (Ed25519 keypair), sign the same canonical payload, send `params.device` and optional `auth.token`; on hello-ok, persist `auth.deviceToken` to `~/.chai/device_token`.

**Possible future work**

- Pending pairing requests with operator approval (CLI/UI) instead of auto-approve.
- Device token rotation/revocation.
- CLI using device identity when connecting to a remote gateway.

### Skills and the LLM

Skills are loaded at gateway startup from the primary skill root (default `~/.chai/skills`, or **`skills.directory`** in config) and any **skills.extraDirs**. Each skill is a directory with **`SKILL.md`** (YAML frontmatter: name, description) and optionally **`tools.json`** (see [TOOLS_SCHEMA.md](spec/TOOLS_SCHEMA.md)). Only skills with a valid `tools.json` contribute callable tools; skills without it are still loaded for context.

**Skill context mode** (`skills.contextMode` in config):

- **`full`** (default): The system message includes the **full content** (name, description, full SKILL.md body) of every loaded skill. The model sees the entire skill set and can use the tools (e.g. notesmd_cli_search, obsidian_create) when relevant. Best for few skills and smaller local models.
- **`readOnDemand`**: The system message contains only a **compact list** (name + description per skill) and instructions to use the **`read_skill(skill_name)`** tool to load a skill’s full SKILL.md when it clearly applies. The gateway registers `read_skill` and a wrapper executor returns that skill’s content in-process. Keeps the prompt small and scales to many skills; aligns with OpenClaw’s pattern.

**Tool execution**: Tool list and executor are built only from skills’ `tools.json` descriptors. A single **generic executor** builds argv from each tool’s execution spec (positional, flag, flagifboolean) and runs via the descriptor’s allowlist (`lib/exec`). When **`skills.allowScripts`** is true, tools can use `resolveCommand.script` to run scripts from the skill’s **`scripts/`** directory for param resolution (no allowlist entry). No hardcoded skill code in the lib. Bundled skills (notesmd-cli, obsidian) live under `crates/lib/config/skills/` with their own SKILL.md and tools.json. Gating: if a skill declares `metadata.requires.bins`, it is loaded only when all listed binaries are on PATH. The agent loop runs up to 5 tool-iteration steps per turn.
