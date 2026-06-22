# Documentation

This directory includes user guides and other user documentation.

- **[User Guides](guides/README.md)** — Conceptual guides for getting started and advanced configurations.
- **[User Journeys](journey/README.md)** — Step-by-step hands-on walkthroughs to learn and manually test the stack.
- **[User Testing](testing/README.md)** — Repeatable playbooks to compare models and providers with a live gateway.

## How the Docs Relate

The three doc sets serve different purposes and link to each other:

| What | Purpose | When to use |
|------|---------|-------------|
| **Guides** | Conceptual and reference material — what things are, how to configure them, how they work. | Reading up on a feature before trying it; looking up a config field. |
| **Journeys** | Hands-on procedures — prerequisites, steps, expected outcomes. | Walking through a feature for the first time; verifying behavior after changes. |
| **Testing** | Repeatable model/provider playbooks — controlled message sequences and result templates. | Comparing models on tool use; validating a provider setup; regression testing before a release. |

Each guide links to the relevant journey for hands-on practice. Each journey links back to the guide for background. The testing playbooks link to guides and journeys for setup instructions.
