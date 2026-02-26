# User Journeys — Manual Testing

This directory contains step-by-step journeys for manually testing Chai functionality. Use them to verify behavior after changes or before release.

## Writing Guidelines

Each file is a single journey: prerequisites, steps, and expected outcomes. Keep steps minimal but enough to complete the flow.

## File Structure

Filenames follow `NN-keyterm-rest.md`: a two-digit number, then a hyphen, then a **key term** aligned with the codebase, then an optional short description.

- **Number** — Order for doing the journeys (e.g. `01` before `02`). Run in sequence when possible.
- **Key term** — Matches a main concept in the source (e.g. `gateway`, `desktop`, `channel`, `skill`). Use this so readers can find the right journey and so new journeys are named consistently.

Examples: `01-gateway-cli-health-and-ws.md`, `04-desktop-start-stop-gateway.md`, `05-channel-telegram.md`, `06-skill-obsidian.md`, `07-skill-notesmd-cli.md`. When adding a journey, pick the key term that best fits (gateway, desktop, channel, skill, or another top-level concept), then add a short, hyphenated rest.
