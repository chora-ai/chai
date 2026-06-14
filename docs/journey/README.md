# User Journeys

Step-by-step walkthroughs for setting up chai, verifying its behavior, and testing individual features. Each journey is a self-contained file with prerequisites, steps, and expected outcomes.

## Who These Are For

- **New users** walking through chai for the first time — start with journey 00 and work forward.
- **Developers** verifying behavior after code changes — jump to the journey that covers the feature you changed.
- **QA** running a smoke test before a release — follow the recommended path below, then hit channel and skill journeys.

## Recommended Path

1. **00 — Setup** — install, init, first chat. Everything else assumes you have completed this.
2. **01 — Gateway health & WebSocket** — verify the HTTP and WebSocket protocols.
3. **02 — Agent & send over WebSocket** — send a message, get a reply, test the `send` method.
4. **03 — Desktop** — (optional) use the desktop app to manage the gateway.
5. **Pick a channel** — connect Telegram (04), Matrix (08), or Signal (09) for end-to-end messaging.
6. **Pick a skill** — test files (05), knowledge base (06), or skills management (07).
7. **Go deeper** — providers (10), multi-agent (11), gateway auth (12), profile management (13).

## Journeys

| # | Journey | Description |
|---|---------|-------------|
| 00 | [Setup: init, configure, verify](00-setup-init.md) | Install the CLI, run `chai init`, configure Ollama, start the gateway, and send your first message. The on-ramp for all other journeys. |
| 01 | [Gateway (CLI): health & WebSocket](01-gateway-cli-health-and-ws.md) | Start the gateway and verify HTTP health, WebSocket connect/handshake, and the `health`/`status` methods. |
| 02 | [Gateway WebSocket: agent & send](02-gateway-ws-agent.md) | Run one agent turn over WebSocket, test session continuity, and call the `send` method for channel delivery. |
| 03 | [Desktop: start/stop gateway](03-desktop-start-stop-gateway.md) | Use the desktop app to start and stop the gateway, and verify external gateway detection. |
| 04 | [Channel: Telegram](04-channel-telegram.md) | Connect Telegram (long-poll or webhook), send a message, and verify the bot replies. |
| 05 | [Skill: Files](05-skill-files.md) | Test the files skill: read, write, patch, search, and delete files in the write sandbox. |
| 06 | [Skill: Knowledge Base](06-skill-kb.md) | Test the kb skill: create, read, search, append, list, and delete notes. Covers kb-daily, kb-frontmatter, and kb-wikilink extensions. |
| 07 | [Skill: Skills](07-skill-skills.md) | Test the skills skill: list, read, validate, discover, init, write, and delete skill packages. |
| 08 | [Channel: Matrix (Experimental)](08-channel-matrix.md) | Connect Matrix, verify message round-trip, and optionally test E2EE device verification. Requires `--features matrix`. |
| 09 | [Channel: Signal (Experimental)](09-channel-signal.md) | Connect Signal via signal-cli, send a message, and verify the agent replies. Requires `--features signal`. |
| 10 | [Provider: local and cloud](10-provider-ollama-lmstudio.md) | Switch the default model, add LM Studio, NearAI, or NVIDIA NIM as providers, verify model discovery. |
| 11 | [Agent: multi-agent configuration](11-agent-multi.md) | Configure an orchestrator with a worker, trigger delegation, verify the worker's response. |
| 12 | [Gateway: auth](12-gateway-auth.md) | Enable token auth, verify connect with and without the token, test protected HTTP routes. |
| 13 | [Profile: manage and switch](13-profile-manage.md) | Create a second profile, switch profiles, verify the gateway uses the active config, clean up. |

## Guides vs. Journeys

- **Journeys** (this directory) are **hands-on** — step-by-step procedures with commands to run and results to check.
- **Guides** ([`../guides/`](../guides/README.md)) are **conceptual** — background on how features work, configuration reference, and design rationale.

Each journey links to the relevant guide under "Background" at the top. The guides also link back to journeys for hands-on procedures.

## Testing Playbooks

For systematic model and provider comparison (not feature walkthroughs), see the [Testing Playbooks](../testing/README.md).
