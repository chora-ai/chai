# Agents

In Chai, **agents** hold **configuration** for the assistant the gateway runs: they name the **orchestrator** that owns the conversation, optionally define **workers** for delegated subtasks, and set **defaults** for which **provider** and **model** to use, how **model discovery** is scoped, and per-role **skills** (`skillsEnabled`, `contextMode`). An agent is not a separate service or binary — the `agents` block is configuration the gateway reads to route each turn and assemble context.

Skills supply instructions and optional tools; top-level `providers` supply URLs and API keys; the `agents` block ties those inputs to one orchestrator and any workers you define. Delegation allowlists, caps, and routes are policy on top of that configuration (see [Configuration](03-configuration.md) for the full agents field reference).

## Agent Orchestration

Each entry in `agents` has a unique `id`, a `role` (`orchestrator` or `worker`), and the optional fields described in [Configuration → Agents](03-configuration.md#agents). The gateway uses this to route turns to the right backend, pass model ids to each provider, decide which APIs to poll for model discovery, and load `AGENT.md` from `<active-profile>/agents/<id>/`. With workers configured, the orchestrator can delegate subtasks using the built-in `delegate_task` tool.

`chai init` creates `agents/orchestrator/AGENT.md` for the default orchestrator id. Edit that file (or add `agents/<workerId>/AGENT.md` for workers) to customize on-disk agent context; see [Agent Context On Disk](#agent-context-on-disk) below.

**Multi-agent example** — orchestrator with two workers:

```json
"agents": [
  {
    "id": "assistant",
    "role": "orchestrator",
    "defaultProvider": "ollama",
    "defaultModel": "llama3.2:3b",
    "enabledProviders": ["ollama", "lms"]
  },
  {
    "id": "engineer",
    "role": "worker",
    "defaultProvider": "lms",
    "defaultModel": "ibm/granite-4-micro",
    "enabledProviders": ["lms"]
  },
  {
    "id": "researcher",
    "role": "worker",
    "defaultProvider": "lms",
    "defaultModel": "ibm/granite-4-micro",
    "enabledProviders": ["lms"]
  }
]
```

## Providers and Models

The gateway integrates six model backends. For an overview of each provider, configuration, and model id conventions, see [Configuration](03-configuration.md).

For deeper reference material, the chai source tree includes provider and model documentation under the `base/` knowledge base:

- **Provider taxonomy and API comparisons** — `base/spec/PROVIDERS.md`
- **Model ids, inventory, and tool-fit notes** — `base/spec/MODELS.md`
- **API alignment roadmap** — `base/epic/API_ALIGNMENT.md`
- **Per-backend wire protocol references** — `base/ref/OLLAMA.md`, `base/ref/LM_STUDIO.md`, `base/ref/VLLM.md`, `base/ref/HUGGINGFACE.md`, `base/ref/NVIDIA_NIM.md`, `base/ref/OPENAI.md`
- **Repeatable model test playbooks** — `docs/testing/`

These paths are relative to the chai source tree, not the guides directory. They are intended for contributors and advanced users working inside the repository.

For systematic model and provider testing, see the [Testing Playbooks](../testing/README.md).

## Agent Context On Disk

Each profile stores per-agent instructions under `agents/<agentId>/` (the agent context directory for that `id`). The file is always `AGENT.md` in that directory. `chai init` creates `agents/orchestrator/AGENT.md` for the default orchestrator id.

- **`AGENT.md`** — Agent-level context for that role; the gateway prepends it to the skills block on each turn.

To customize an agent's behavior, edit its `AGENT.md`. For workers, create the directory and file manually:

```bash
mkdir -p ~/.chai/active/agents/engineer
# Edit ~/.chai/active/agents/engineer/AGENT.md with your instructions
```

See [Configuration](03-configuration.md) for the full list of agent configuration fields.
