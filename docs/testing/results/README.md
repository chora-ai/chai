# Test Results

This directory stores test run results. Each file covers one playbook, one model, and one date.

## Naming Convention

Files follow the pattern `YYYY-MM-DD-<playbook>-<model>.md`:

- **Date** — when the run was performed.
- **Playbook** — the playbook file number (e.g., `01`, `20`, `21`).
- **Model** — a short model identifier (e.g., `llama3.1-8b`, `qwen3-8b`, `deepseek-r1-7b`).

Examples: `2025-06-07-01-llama3.1-8b.md`, `2025-06-07-20-deepseek-r1-7b.md`.

## When to Create a New File

Create a new file for each distinct combination of playbook, model, and date. Do not overwrite previous results — historical data is valuable for tracking regression and improvement.

## Template

See the result template in the [testing README](../README.md#result-template).
