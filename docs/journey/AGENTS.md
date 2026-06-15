# Agents

This is the `AGENTS.md` file in the root of the `journey` directory.

## Directory

The `journey` directory contains step-by-step hands-on walkthroughs — prerequisites, steps, and expected outcomes. Each journey is a self-contained file that a reader can follow from start to finish.

## Guidelines

- Write against **current** behavior. If something feels awkward to document, that is a signal to improve the product, not to work around it in prose.
- Journeys are **hands-on procedures**, not conceptual explanations. Every step should include a concrete action (a command to run, a message to send, a file to check) and a clear expected outcome. Do not explain *why* something works — link to the guide for that.
- Keep steps **minimal but sufficient**. Include enough detail to complete the flow without ambiguity, but do not over-explain or pad with background that belongs in the guide.
- Each journey must be **self-contained**: a reader who has completed the setup journey (00) should be able to follow any other journey without needing external context beyond what is listed in the prerequisites.
- Include an **"If Something Fails"** section with common failure modes and recovery steps. These are distinct from the troubleshooting guide — they cover journey-specific issues (wrong command, missing prerequisite) rather than general product issues.

## Conventions

### Filename

`NN-keyterm-rest.md`: a two-digit number, then a hyphen, then a **key term** aligned with the codebase, then an optional short hyphenated description.

- **Number** — Order for doing the journeys (e.g., `00` before `01`).
- **Key term** — Matches a main concept in the source. Use one of: `gateway`, `desktop`, `channel`, `skill`, `provider`, `agent`, `profile`, `setup`. When adding a journey, pick the key term that best fits the primary feature being exercised.

Examples: `00-setup-init.md`, `01-gateway-cli-health-and-ws.md`, `04-channel-telegram.md`, `05-skill-files.md`, `10-provider-ollama-lmstudio.md`.

### Structure

Each journey file follows this structure in order:

1. **Title** — `# Journey: <Feature> — <Scope>`
2. **Goal** — One sentence starting with "Goal:" that states what the reader will confirm or achieve.
3. **Background** — One line linking to the relevant guide(s): `**Background:** [Guide Name](../guides/NN-topic.md)`
4. **Prerequisites** — Bulleted list of what must be true before starting. Always include "Setup complete" linking to journey 00 when this is not the setup journey itself.
5. **Steps** — Numbered list. Each step has a title, a concrete action, and an expected outcome prefixed with "**Expect:**".
6. **If Something Fails** — Bulleted list of common failure modes with recovery steps.
7. **Summary** — A table with columns: Step, Action, Expected Outcome.
8. **Next** — Links to the next journey(s) in sequence.

### Links

- **Background links** always point to guides using relative paths: `../guides/NN-topic.md`.
- **Prerequisite links** point to earlier journeys: `00-setup-init.md`.
- **Next links** at the bottom point to related journeys.
