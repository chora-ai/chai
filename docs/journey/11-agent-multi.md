# Journey: Agent — multi-agent configuration

**Goal:** Configure an orchestrator with a worker agent, verify delegation works by sending a message that triggers `delegate_task`, and confirm the worker's response comes back through the orchestrator.

**Background:** [Agents](../guides/05-agents.md) · [Configuration → Agents](../guides/03-configuration.md#configuring-agents)

By default, chai runs a single orchestrator agent. This journey adds a worker that handles delegated subtasks with its own provider, model, and skills. Delegation is the orchestrator's built-in `delegate_task` tool — the orchestrator decides when and what to delegate.

## Prerequisites

- **Setup complete** — You have installed chai, run `chai init`, and verified the gateway works with defaults (see [00-setup-init.md](00-setup-init.md)).
- **Ollama** running with at least one model. For best results, use a model large enough to reliably call tools (7B+). A larger orchestrator model (e.g. `llama3.1:70b`) delegates more reliably, but even `llama3.2:3b` can delegate with clear instructions.
- **Optional:** A second provider (e.g. LM Studio) if you want the worker to use a different backend. The journey uses Ollama for both agents by default.

## Steps

1. **Edit config.json**
   - Open `~/.chai/profiles/assistant/config.json` and configure two agents:
   ```json
   {
     "providers": [
       { "id": "ollama", "endpointType": "ollama" }
     ],
     "agents": [
       {
         "id": "assistant",
         "role": "orchestrator",
         "defaultProvider": "ollama",
         "defaultModel": "llama3.2:3b",
         "enabledProviders": ["ollama"],
         "enabledSkills": ["files"]
       },
       {
         "id": "engineer",
         "role": "worker",
         "defaultProvider": "ollama",
         "defaultModel": "llama3.2:3b"
       }
     ]
   }
   ```
   - The orchestrator (`assistant`) has the `files` skill enabled so it can do file work itself while also being able to delegate.
   - The worker (`engineer`) has no skills — it receives delegated tasks and returns text responses.

2. **Create worker context (optional)**
   - Create the worker's agent context directory:
   ```bash
   mkdir -p ~/.chai/profiles/assistant/agents/engineer
   ```
   - Create `~/.chai/profiles/assistant/agents/engineer/AGENT.md` with instructions, e.g.:
   ```markdown
   # Engineer

   You are a focused coding assistant. When given a task, provide concise, correct answers.
   ```
   - This is optional — the worker will work without an `AGENT.md`, but giving it instructions improves delegation quality.

3. **Start the gateway**
   ```bash
   chai gateway
   ```
   - **Expect:** Log lines showing both agents are configured: `agent assistant (orchestrator)` and `agent engineer (worker)`. Provider discovery shows the Ollama models.

4. **Chat with the orchestrator**
   ```bash
   chai chat
   ```
   - Send: "Delegate the following task to the engineer worker: explain what a Fibonacci sequence is in two sentences."
   - **Expect:** The orchestrator calls `delegate_task` with the instruction prefixed by `[engineer]`. The system matches the bracket prefix, routes to the worker, strips the prefix, and runs the worker turn. The orchestrator relays the answer back to you.
   - In the gateway logs with `RUST_LOG=info`, look for lines about delegation starting and completing.

5. **Verify delegation in the desktop app (optional)**
   - If you have the desktop app running, the chat screen shows delegation events:
     - **Delegation start** — Labeled event showing `engineer` as the worker.
     - **Delegation complete** — Success indicator with the worker's response.
   - This gives a visual confirmation that delegation happened.

6. **Test non-delegated interaction**
   - Send: "List the files in the current directory."
   - **Expect:** The orchestrator handles this directly using the `files` skill — no delegation occurs. The reply includes the file listing.

7. **Stop the gateway** with Ctrl+C.

## Delegation Configuration Details

The `delegate_task` tool behavior is controlled by orchestrator-only configuration fields:

- **`maxDelegationsPerTurn`** / **`maxDelegationsPerSession`** — Caps on how often delegation occurs.

To target a specific worker, prefix the delegation instruction with the worker's bracket prefix (e.g., `[engineer]`). The system matches `[workerId]` at the start of the instruction, routes to that worker, and strips the prefix before passing the instruction.

See [Configuration → Agents](../guides/03-configuration.md#agents) for field details.

## If Something Fails

- **Gateway won't start after config change** — Validate the JSON: `cat ~/.chai/profiles/assistant/config.json | python3 -m json.tool`. Common issues: missing commas, unclosed brackets, or an `agents` array with no `orchestrator` role.
- **Orchestrator doesn't delegate** — Small models (3B) may not reliably call `delegate_task`. Try a larger model for the orchestrator, or phrase the request more explicitly: "Use the delegate_task tool to send this task to the worker with id 'engineer': ..."
- **`delegate_task` returns an error** — The worker id must match exactly (case-sensitive). Check the `id` field in the worker's `agents` entry matches the bracket prefix the orchestrator is using.
- **Worker turn fails** — The worker's `defaultProvider` must be valid and the model must be available. If the worker uses a different provider than the orchestrator, ensure that provider is configured in `providers` and is in the orchestrator's `enabledProviders`.
- **No delegation events in logs** — Delegation may not have occurred. With `RUST_LOG=info`, look for `delegate_task` in the logs. If absent, the orchestrator answered directly (which is normal for questions it can handle).
- **Orchestrator delegates every message** — The model may be over-eager to delegate. Adjust `AGENT.md` instructions or use `maxDelegationsPerTurn` to limit delegation frequency.

## Summary

| Step | Action | Expected Outcome |
|------|--------|-------------------|
| 1 | Add worker agent to config | Two agents configured |
| 2 | Create engineer AGENT.md (optional) | Worker context directory created |
| 3 | `chai gateway` | Both agents loaded |
| 4 | "Delegate a task to engineer" | Delegation → worker turn → reply |
| 5 | Desktop app (optional) | Delegation events visible |
| 6 | Non-delegated request | Orchestrator handles directly |
| 7 | Ctrl+C | Gateway stops |

**Next:** [12 — Gateway: auth](12-gateway-auth.md) · [13 — Profile: manage](13-profile-manage.md)
