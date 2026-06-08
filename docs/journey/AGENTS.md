# Agents

This is the `AGENTS.md` file in the root of the `journey` directory.

## Directory

The `journey` directory is for user journeys.

## Writing Guidelines

Each file is a single journey: prerequisites, steps, and expected outcomes. Keep steps minimal but enough to complete the flow.

## Filename Guidelines

Filenames follow `NN-keyterm-rest.md`: a two-digit number, then a hyphen, then a **key term** aligned with the codebase, then an optional short description.

- **Number** — Order for doing the journeys (e.g. `01` before `02`).
- **Key term** — Matches a main concept in the source (e.g. `gateway`, `desktop`, `channel`, `skill`, `provider`, `agent`, `profile`). Use this so readers can find the right journey and so new journeys are named consistently.

Examples: `00-setup-init.md`, `01-gateway-cli-health-and-ws.md`, `03-desktop-start-stop-gateway.md`, `04-channel-telegram.md`, `05-skill-files.md`, `06-skill-kb.md`, `07-skill-skills.md`, `08-channel-matrix.md`, `09-channel-signal.md`, `10-provider-ollama-lmstudio.md`, `11-agent-multi.md`, `12-gateway-auth.md`, `13-profile-manage.md`. When adding a journey, pick the key term that best fits (gateway, desktop, channel, skill, provider, agent, profile, or another top-level concept), then add a short, hyphenated rest.
