# Error Handling

## Scope

Models that support tool calling, tested with scenarios that trigger tool errors — reading a nonexistent file, writing outside the sandbox. This playbook uses Sequence D from the README, which verifies that a model acknowledges errors and recovers gracefully.

Use this playbook alongside the single-skill playbooks (01–10) to verify error handling behavior.

## Setup

Follow the provider setup instructions in [PROVIDER_SETUP.md](PROVIDER_SETUP.md) for your chosen provider, then configure the agent with both the daily note and files skills enabled (files is needed for the read/write operations that trigger errors).

Example configuration:

```json
{
  "agents": [{
    "id": "orchestrator",
    "role": "orchestrator",
    "defaultProvider": "<provider>",
    "defaultModel": "<model>",
    "skillsEnabled": ["kb-daily", "files"]
  }]
}
```

## Message Sequence

Follow **Sequence D: Error Handling** in [README.md](README.md): message sequence, expectations, and run procedure.

In summary:

1. Greeting — no tool expected.
2. Read nonexistent file (`does_not_exist.md`) — tool error expected; model should acknowledge.
3. Create daily note — should succeed despite prior error; model should not be derailed.
4. Write outside sandbox (`/etc/test.txt`) — sandbox/permission error expected; model should explain.
5. Closing — no tool expected; model should not be in an error state.

## Run Procedure

1. Set the playbook model in `agents.defaultModel`.
2. Ensure both `kb-daily` and `files` are in `skillsEnabled`.
3. Run all five messages in `skills.contextMode: "full"` for three runs.
4. Repeat in `skills.contextMode: "readOnDemand"` for three runs.
5. Record error acknowledgment, recovery behavior, response summary, pass/fail, and any errors.

Key observations:
- Whether the model acknowledges errors versus hallucinating success.
- Whether error recovery on message 3 works cleanly (no lingering error state).
- Whether the model explains sandbox restrictions on message 4 without attempting the operation.

## See Also

- [Provider setup](PROVIDER_SETUP.md) · [Configuration → Providers](../guides/03-configuration.md#configuring-providers) · [Provider spec](../../base/spec/PROVIDERS.md) · [Model spec](../../base/spec/MODELS.md) · [Sandbox spec](../../base/spec/SANDBOX.md)
