# Journey: Agent — Multi-Agent Configuration

**Goal:** Configure an orchestrator with a worker agent, verify delegation works, then add a second orchestrator and verify per-orchestrator session isolation and worker visibility.

**Background:** [Agents](../guides/05-agents.md) · [Configuration → Agents](../guides/03-configuration.md#configuring-agents)

By default, chai runs a single orchestrator agent. The `agents` array supports multiple orchestrators — each with its own provider, model, skills, and worker visibility (`enabledWorkers`). This journey first adds a worker that handles delegated subtasks, then adds a second orchestrator to demonstrate how multiple orchestrators share the same sandbox while maintaining separate sessions and worker access.

## Prerequisites

- **Setup complete** — You have installed chai, run `chai init`, and verified the gateway works with defaults (see [00-setup-init.md](00-setup-init.md)).
- **Ollama** running with at least one model. For best results, use a model large enough to reliably call tools (7B+). A larger orchestrator model (e.g. `llama3.1:70b`) delegates more reliably, but even `llama3.2:3b` can delegate with clear instructions.
- **Optional:** A second provider (e.g. LM Studio) if you want the worker to use a different backend. The journey uses Ollama for both agents by default.

## Part 1: Orchestrator With a Worker

### Steps

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
         "enabledSkills": ["files"],
         "enabledWorkers": ["engineer"]
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
   - The orchestrator (`assistant`) has the `files` skill enabled and `enabledWorkers: ["engineer"]` so it can delegate to the engineer worker.
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

## Part 2: Multiple Orchestrators

### Steps

8. **Add a second orchestrator to config.json**
   - Update `~/.chai/profiles/assistant/config.json` to add a reviewer orchestrator:
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
         "enabledSkills": ["files"],
         "enabledWorkers": ["engineer"]
       },
       {
         "id": "reviewer",
         "role": "orchestrator",
         "defaultProvider": "ollama",
         "defaultModel": "llama3.2:3b",
         "enabledProviders": ["ollama"],
         "enabledSkills": ["files", "git-read"]
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
   - The reviewer orchestrator has `enabledSkills: ["files", "git-read"]` and no `enabledWorkers` — it cannot delegate. The assistant orchestrator retains its `enabledWorkers: ["engineer"]`.

9. **Create reviewer context**
   ```bash
   mkdir -p ~/.chai/profiles/assistant/agents/reviewer
   ```
   - Create `~/.chai/profiles/assistant/agents/reviewer/AGENT.md` with reviewer-specific instructions:
   ```markdown
   # Reviewer

   You are a code reviewer. Analyze code for correctness, style, and potential issues. You do not modify files.
   ```

10. **Start the gateway**
    ```bash
    chai gateway
    ```
    - **Expect:** Log lines showing three agents: `assistant (orchestrator)`, `reviewer (orchestrator)`, and `engineer (worker)`.

11. **Chat with the assistant orchestrator (default)**
    ```bash
    chai chat
    ```
    - Send: "Delegate a task to the engineer: what is 2+2?"
    - **Expect:** Delegation to the engineer worker succeeds (same as Part 1).

12. **Chat with the reviewer orchestrator**
    - Exit the current chat (`/exit`), then:
    ```bash
    chai chat --agent reviewer
    ```
    - Send: "What files are in the current directory?"
    - **Expect:** The reviewer handles this directly using its `files` skill. No delegation occurs because the reviewer has no `enabledWorkers`.

13. **Verify per-orchestrator sessions in the desktop (optional)**
    - If the desktop app is running, the sessions sidebar now has an "Agent" ComboBox above the session list.
    - Switch between `assistant` and `reviewer` — the session list changes to show only that orchestrator's sessions.
    - The provider/model dropdowns update to reflect each orchestrator's defaults.

14. **Verify session isolation**
    - Start a chat session with the assistant (`chai chat`), send a message, then exit.
    - Start a chat session with the reviewer (`chai chat --agent reviewer`), send a message, then exit.
    - List sessions per orchestrator:
    ```bash
    chai session list --agent assistant
    chai session list --agent reviewer
    ```
    - **Expect:** Each command shows only that orchestrator's sessions. The sessions are isolated.

15. **Stop the gateway** with Ctrl+C.

## Delegation Configuration Details

The `delegate_task` tool behavior is controlled by orchestrator-only configuration fields:

- **`maxDelegationsPerTurn`** / **`maxDelegationsPerSession`** — Caps on how often delegation occurs.
- **`enabledWorkers`** — Which workers this orchestrator can delegate to. Absent or `null` means no workers (`delegate_task` not offered); empty array means all workers; non-empty means only listed workers.

To target a specific worker, prefix the delegation instruction with the worker's bracket prefix (e.g., `[engineer]`). The system matches `[workerId]` at the start of the instruction, routes to that worker, and strips the prefix before passing the instruction.

See [Configuration → Agents](../guides/03-configuration.md#agents) for field details.

## If Something Fails

- **Gateway won't start after config change** — Validate the JSON: `cat ~/.chai/profiles/assistant/config.json | python3 -m json.tool`. Common issues: missing commas, unclosed brackets, or an `agents` array with no `orchestrator` role.
- **Orchestrator doesn't delegate** — Small models (3B) may not reliably call `delegate_task`. Try a larger model for the orchestrator, or phrase the request more explicitly: "Use the delegate_task tool to send this task to the worker with id 'engineer': ..."
- **`delegate_task` returns an error** — The worker id must match exactly (case-sensitive). Check the `id` field in the worker's `agents` entry matches the bracket prefix the orchestrator is using.
- **Worker turn fails** — The worker's `defaultProvider` must be valid and the model must be available. If the worker uses a different provider than the orchestrator, ensure that provider is configured in `providers` and is in the orchestrator's `enabledProviders`.
- **No delegation events in logs** — Delegation may not have occurred. With `RUST_LOG=info`, look for `delegate_task` in the logs. If absent, the orchestrator answered directly (which is normal for questions it can handle).
- **Orchestrator delegates every message** — The model may be over-eager to delegate. Adjust `AGENT.md` instructions or use `maxDelegationsPerTurn` to limit delegation frequency.
- **Reviewer orchestrator tries to delegate** — If `enabledWorkers` is omitted, `delegate_task` is not offered to the model. If the reviewer has `enabledWorkers` set but you didn't intend delegation, remove the field or set it to `null`.
- **`--agent` flag not recognized** — Ensure you're using a version that supports the `--agent` flag (v0.4.0+). Run `chai version` to check.
- **Desktop doesn't show the Agent ComboBox** — The ComboBox appears only when multiple orchestrators are configured. Verify your `config.json` has at least two entries with `role: "orchestrator"`.

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
| 8 | Add reviewer orchestrator | Three agents configured |
| 9 | Create reviewer AGENT.md | Reviewer context directory created |
| 10 | `chai gateway` | Three agents loaded |
| 11 | `chai chat` (default) | Assistant delegates to engineer |
| 12 | `chai chat --agent reviewer` | Reviewer handles directly (no delegation) |
| 13 | Desktop Agent ComboBox (optional) | Session list updates per orchestrator |
| 14 | `chai session list --agent ...` | Per-orchestrator sessions are isolated |
| 15 | Ctrl+C | Gateway stops |

**Next:** [12 — Gateway: auth](12-gateway-auth.md) · [13 — Profile: manage](13-profile-manage.md)
