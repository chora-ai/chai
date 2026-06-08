# Agents

In Chai, **agents** are configuration entries — not separate services or binaries — that the gateway reads to route each turn and assemble context. This guide explains how agent orchestration works, what delegation looks like, and how to customize agent behavior through on-disk context. For the full list of agent configuration fields, see [Configuration → Agents](03-configuration.md#agents).

## Agent Orchestration

Each entry in the `agents` array has a unique `id`, a `role` (`orchestrator` or `worker`), and optional fields that set the default provider, model, and skills. The gateway uses this to route turns, pass model ids to each provider, decide which APIs to poll for model discovery, and load `AGENT.md` from `<active-profile>/agents/<id>/`.

There is always exactly one **orchestrator** (owns the conversation, handles incoming messages). Workers are optional — they handle subtasks delegated by the orchestrator. With no `agents` key in `config.json`, the gateway runs a single orchestrator with built-in defaults (Ollama, `llama3.2:3b`).

When workers are configured, the orchestrator can delegate subtasks using the built-in `delegate_task` tool. Delegation allowlists and caps are policy on top of agent configuration — see the [Configuration → Agents](03-configuration.md#agents) reference for the full delegation fields.

For multi-agent configuration examples, see [Configuration → Configuring Agents](03-configuration.md#configuring-agents).

## Delegation

When the orchestrator calls `delegate_task`, the gateway:

1. Matches the bracket prefix `[workerId]` at the start of the instruction (if present) to select the target worker.
2. Strips the bracket prefix from the instruction.
3. Injects the task instructions as the worker's user message.
4. Runs a worker turn (model call + tool loop) with the worker's own provider, model, skills, and context.
5. Returns the worker's response to the orchestrator, which can then continue its own turn.

The orchestrator retains control — it decides when to delegate, what to ask, and how to use the result. Workers never delegate further.

Delegation behavior is governed by several orchestrator-only configuration fields:

- `delegateAllowedModels` — restrict which provider+model combinations the orchestrator may delegate to
- `delegateBlockedProviders` — prevent delegation to specific providers
- `maxDelegationsPerTurn` / `maxDelegationsPerSession` / `maxDelegationsPerProvider` — caps on delegation frequency

To target a specific worker, prefix the delegation instruction with the worker's bracket prefix (e.g., `[read-only]`). The system matches `[workerId]` at the start of the instruction, routes to that worker, and strips the prefix before passing the instruction. When no bracket prefix is present, the orchestrator's effective defaults are used (no worker selected).

See the [Configuration → Agents](03-configuration.md#agents) reference for field details and defaults.

## Providers and Models

Each agent references a `defaultProvider` and `defaultModel` that determine which backend handles its turns. The orchestrator's `enabledProviders` field controls which providers are polled for model discovery at startup.

For provider configuration, model id conventions, and the full endpoint type reference, see [Configuration → Configuring Providers](03-configuration.md#configuring-providers). For a decision guide on choosing a provider based on hardware, privacy, and use case, see [Choosing a Provider and Model](10-choosing-a-provider.md).

For deeper reference material, the chai source tree includes provider and model documentation under the `base/` knowledge base:

- **Provider taxonomy and API comparisons** — `base/spec/PROVIDERS.md`
- **Model ids, inventory, and tool-fit notes** — `base/spec/MODELS.md`
- **API alignment roadmap** — `base/spec/API_ALIGNMENT.md` (inside `base/epic/`)
- **Per-backend wire protocol references** — `base/ref/OLLAMA.md`, `base/ref/LM_STUDIO.md`, `base/ref/NVIDIA_NIM.md`, `base/ref/OPENAI.md`
- **Repeatable model test playbooks** — `docs/testing/`

These paths are relative to the chai source tree, not the guides directory. They are intended for contributors and advanced users working inside the repository.

## Agent Context On Disk

Each profile stores per-agent instructions under `agents/<agentId>/`. The file is always `AGENT.md` in that directory. The gateway prepends it to the skills block on each turn, giving each agent its own personality, constraints, and domain knowledge.

`chai init` creates `agents/orchestrator/AGENT.md` for the default orchestrator id. Edit that file to customize the orchestrator's behavior. For workers, create the directory and file manually:

```bash
mkdir -p ~/.chai/active/agents/engineer
# Edit ~/.chai/active/agents/engineer/AGENT.md with your instructions
```

### What Goes in AGENT.md

`AGENT.md` is free-form Markdown. Common patterns:

- **Role definition** — Who the agent is and how it should behave
- **Constraints** — What the agent should not do
- **Domain knowledge** — Facts and conventions the model needs for your use case
- **Workflow instructions** — Step-by-step procedures the agent should follow

The gateway does not parse `AGENT.md` — it sends the raw content as part of the system message. Write it as instructions to the model, not as configuration.

## Try It

For hands-on agent and delegation walkthroughs, see the user journeys:

- [Gateway WebSocket — Agent & Send](../journey/02-gateway-ws-agent.md) — Send a message, observe the agent turn, and test the `send` method
- [Agent: Multi-Agent Configuration](../journey/11-agent-multi.md) — Configure an orchestrator with a worker and verify delegation
