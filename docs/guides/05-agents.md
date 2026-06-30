# Agents

In Chai, **agents** are configuration entries — not separate services or binaries — that the gateway reads to route each turn and assemble context. This guide explains how agent orchestration works, what delegation looks like, and how to customize agent behavior through on-disk context. For the full list of agent configuration fields, see [Configuration → Agents](03-configuration.md#agents).

## Agent Orchestration

Each entry in the `agents` array has a unique `id`, a `role` (`orchestrator` or `worker`), and optional fields that set the default provider, model, and skills. The gateway uses this to route turns, pass model ids to each provider, decide which APIs to poll for model discovery, and load `AGENT.md` from `<active-profile>/agents/<id>/`.

There is always at least one **orchestrator** (owns the conversation, handles incoming messages). Multiple orchestrators are supported — each can have its own provider, model, skills, and worker visibility. Workers are optional — they handle subtasks delegated by the orchestrator. With no `agents` key in `config.json`, the gateway runs a single orchestrator with built-in defaults (Ollama, `llama3.2:3b`).

When workers are configured, the orchestrator can delegate subtasks using the built-in `delegate_task` tool. Delegation caps are policy on top of agent configuration — see the [Configuration → Agents](03-configuration.md#agents) reference for the full delegation fields.

For multi-agent configuration examples, see [Configuration → Configuring Agents](03-configuration.md#configuring-agents).

## Multiple Orchestrators

When you need different agent behaviors for different tasks — for example, a developer orchestrator that writes code and a reviewer orchestrator that audits it — you can configure multiple orchestrators in the same profile. Each orchestrator gets its own `AGENT.md`, provider/model defaults, skill set, and worker visibility, while sharing the same sandbox, providers, and worker definitions.

### Multiple Orchestrators vs. Profile Switching

| Aspect | Profile Switch | Orchestrator Switch |
|--------|---------------|-------------------|
| Scope | Everything: config, workers, sandbox, providers, channels, skills | Agent context, skills, worker visibility, provider/model defaults |
| Workers | Different per profile | Shared (with optional `enabledWorkers` filter) |
| Sandbox | Isolated per profile | Shared |
| Sessions | All sessions lost | Per-orchestrator session stores; switching shows the selected orchestrator's sessions |
| Gateway restart | Required | Not required |

Use multiple orchestrators when you want to change the agent's role and context without changing the entire environment. Use profile switching when you need separate sandboxes, different providers, or different channel configurations.

### Orchestrator Selector

When multiple orchestrators are configured, you select which one to use:

- **Desktop** — An "Agent" ComboBox appears in the sessions sidebar above the session list. Switching updates the sessions list and provider/model defaults.
- **CLI** — Pass `--agent <id>` to `chai chat`, `chai session list`, or `chai session clear` to select a specific orchestrator.
- **WebSocket** — Pass `orchestratorId` in the `agent` RPC or `sessions.list` RPC.

When only one orchestrator is configured, the selector is visible but disabled in the desktop; the CLI `--agent` flag defaults to the single orchestrator.

### Per-Orchestrator Worker Visibility

Each orchestrator can restrict which workers it delegates to using `enabledWorkers`:

- **Absent or `null`** — No workers enabled; `delegate_task` is not offered.
- **Empty array** (`[]`) — All profile workers are available.
- **Non-empty array** — Only the listed workers are visible and delegatable.

This lets a reviewer orchestrator delegate read-only tasks to a `reader` worker while preventing it from delegating to an `engineer` worker that could modify files.

### Sessions

Each orchestrator has its own session store at `<profile_dir>/agents/<orchestrator_id>/sessions/`. Switching orchestrators switches which sessions are visible. Sessions from one orchestrator are completely separate from another — switching back shows the original sessions.

For a hands-on walkthrough, see [Agent: Multi-Agent Configuration](../journey/11-agent-multi.md).

## Delegation

When the orchestrator calls `delegate_task`, the gateway:

1. Matches the bracket prefix `[workerId]` at the start of the instruction (if present) to select the target worker.
2. Strips the bracket prefix from the instruction.
3. Injects the task instructions as the worker's user message.
4. Runs a worker turn (model call + tool loop) with the worker's own provider, model, skills, and context.
5. Returns the worker's response to the orchestrator, which can then continue its own turn.

The orchestrator retains control — it decides when to delegate, what to ask, and how to use the result. Workers never delegate further.

Delegation behavior is governed by orchestrator-only configuration fields:

- `maxDelegationsPerTurn` / `maxDelegationsPerSession` / `maxDelegationsPerWorker` — caps on delegation frequency

To target a specific worker, prefix the delegation instruction with the worker's bracket prefix (e.g., `[read-only]`). The system matches `[workerId]` at the start of the instruction, routes to that worker, and strips the prefix before passing the instruction. When no bracket prefix is present, the orchestrator's effective defaults are used (no worker selected).

See the [Configuration → Agents](03-configuration.md#agents) reference for field details and defaults.

## Providers and Models

Each agent references a `defaultProvider` and `defaultModel` that determine which backend handles its turns. The orchestrator's `enabledProviders` field controls which providers are polled for model discovery at startup.

For provider configuration, model id conventions, and the full endpoint type reference, see [Configuration → Configuring Providers](03-configuration.md#configuring-providers). For a decision guide on choosing a provider based on hardware, privacy, and use case, see [Choosing a Provider and Model](10-choosing-a-provider.md). For repeatable model test playbooks, see [User Testing](../testing/README.md).

## Agent Context On Disk

Each profile stores per-agent instructions under `agents/<agentId>/`. The file is always `AGENT.md` in that directory. The gateway builds a system message from the `AGENT.md` content (plus the workers roster and skills) at startup and injects it as the first message on every turn — it is never persisted in the session history.

`chai init` creates `agents/orchestrator/AGENT.md` for the default orchestrator id. Edit that file to customize the orchestrator's behavior. For additional orchestrators or workers, create the directory and file manually:

```bash
mkdir -p ~/.chai/active/agents/reviewer
# Edit ~/.chai/active/agents/reviewer/AGENT.md with reviewer-specific instructions

mkdir -p ~/.chai/active/agents/engineer
# Edit ~/.chai/active/agents/engineer/AGENT.md with engineer-specific instructions
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
