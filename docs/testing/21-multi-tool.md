# Multi-Tool Use

## Scope

Models that support tool calling, tested with multiple skills enabled to verify correct tool selection across turns. This playbook uses Sequence C from the README, which exercises both the daily note and files skills.

Use this playbook alongside the single-skill playbooks (01–10) to verify that a model can distinguish between tools and invoke the right one for each request.

## Setup

Follow the provider setup instructions in [PROVIDER_SETUP.md](PROVIDER_SETUP.md) for your chosen provider, then configure the agent with **both** the daily note and files skills enabled.

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

Follow **Sequence C: Multi-Tool Use** in [README.md](README.md): message sequence, expectations, and run procedure.

In summary:

1. Greeting — no tool expected.
2. Create daily note — daily note tool expected.
3. Read README.md — files tool expected.
4. Append to daily note — daily note tool expected.
5. Read daily note — daily note tool expected.

## Run Procedure

1. Set the playbook model in `agents.defaultModel`.
2. Ensure both `kb-daily` and `files` are in `skillsEnabled`.
3. Run all five messages in `skills.contextMode: "full"` for three runs.
4. Repeat in `skills.contextMode: "readOnDemand"` for three runs.
5. Record tool usage, tool selection accuracy, response summary, pass/fail, and any errors.

Key observation: whether the model consistently picks the correct tool category (daily note vs. files) for messages 3 and 4.

## See Also

- [Provider setup](PROVIDER_SETUP.md) · [Configuration → Providers](../guides/03-configuration.md#configuring-a-provider) · [Provider spec](../../base/spec/PROVIDERS.md) · [Model spec](../../base/spec/MODELS.md) · [Agents spec (skillsEnabled)](../../base/spec/AGENTS.md)
