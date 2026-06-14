# Conversation (No Tools)

## Scope

Models that do not support tool calling. This playbook uses Sequence B (pure conversational message sequence) — no tool calls are expected at any point.

Use this playbook for models listed as "does not support tools" in the provider-specific playbooks (03, 06, 09), or for any model where tool calling is unreliable or unsupported.

## Setup

Follow the provider setup instructions in [PROVIDER_SETUP.md](PROVIDER_SETUP.md) for your chosen provider, then set `agents.defaultModel` to a no-tool model.

When no-tool models are the only models available for a provider, disable skills that require tools. Set `skills.contextMode: "full"` or `"readOnDemand"` as usual — the agent will still load the skill context, but models should not attempt tool calls.

## Message Sequence

Follow **Sequence B: Conversation Only** in [README.md](README.md): message sequence, expectations, and run procedure.

In summary:

1. Greeting and identity — no tool expected.
2. Reasoning task — no tool expected; correct answer required.
3. Instruction following — no tool expected; format compliance required.
4. Consistency check — no tool expected; correct recall of prior context required.
5. Closing — no tool expected; friendly sign-off.

## See Also

- [Provider setup](PROVIDER_SETUP.md) · [Configuration → Providers](../guides/03-configuration.md#configuring-providers) · [Provider spec](../../base/spec/PROVIDERS.md) · [Model spec](../../base/spec/MODELS.md)
