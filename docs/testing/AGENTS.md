# Agents

This is the `AGENTS.md` file in the root of the `testing` directory.

## Directory

The `testing` directory contains repeatable playbooks for systematically evaluating model behavior with chai. Playbooks use shared message sequences defined in `README.md` and record results in `results/`.

## Guidelines

- Write against **current** behavior. If something feels awkward to document, that is a signal to improve the product, not to work around it in prose.
- Playbooks are **repeatable procedures**, not explorations. Everything a tester needs to run the playbook (model list, config snippet, sequence reference) must be in the playbook file or linked from it. Do not assume the tester has read other playbooks.
- Provider-specific setup belongs in `PROVIDER_SETUP.md`, not in individual playbook files. Playbooks reference the relevant section: `[Provider setup](PROVIDER_SETUP.md#provider-name)`.
- Message sequences live in `README.md` and are referenced by letter (Sequence A, B, C, D). Do not duplicate the full sequence in a playbook — reference it and summarize the key points.
- Results are **append-only**. Use the result template from `README.md` and create a new dated file for each run. Never overwrite previous results.

## Conventions

### Filename

- **Tool-use playbooks (01–19):** `NN-category-provider-family.md` — number, deployment category (`local` or `third-party`), provider id, model family. Example: `01-local-ollama-llama.md`, `07-third-party-nim-llama.md`.
- **Special-purpose playbooks (20+):** `NN-category-description.md` — number, category (`conversation`, `multi-tool`, `error-handling`, etc.), short description. Example: `20-conversation-no-tools.md`, `21-multi-tool.md`.

### Playbook Structure

Each playbook file follows this structure in order:

1. **Title** — `# <Category> <Provider> <Family>` for tool-use playbooks, or `# <Category> (<Description>)` for special-purpose playbooks.
2. **Scope** — One-paragraph summary of what the playbook covers.
3. **Setup** — Reference to `PROVIDER_SETUP.md` and a minimal config snippet showing the provider and agent settings needed.
4. **Models** — Grouped under three headings: "The following models support tools:", "The following models support tools but not trained on tool use:", and "The following models do not support tools:". Each entry is a code-span model id, a dash, and a source link in parentheses. Models without tool support include a note directing the reader to `20-conversation-no-tools.md`.
5. **Procedure** — Reference to the shared sequence in `README.md` by letter, with a brief summary of the key steps.
6. **See Also** — Links to: `PROVIDER_SETUP.md`, the configuration guide, and the model selection guide.

Special-purpose playbooks may omit the Models section when it is not applicable, but must include Scope, Setup, Procedure (or Message Sequence), and See Also.

### Model List Format

- **Supports tools:** Code-span model id, dash, source link. Example: `` `llama3.1:8b` - [source (Ollama)](https://ollama.com/library/llama3.1:8b) ``
- **Supports tools but not trained on tool use:** Same format, with an added note about unreliable tool results.
- **Does not support tools:** Same format, followed by a blockquote note: `> **Note:** Models without tool support should be tested with [20-conversation-no-tools.md](20-conversation-no-tools.md) instead of the shared tool-use message sequence.`
- **Excluded or fabricated models:** Use parenthetical notes (e.g., "(excluded from list)").

### Results

- Stored in `results/` with filenames following `YYYY-MM-DD-<playbook>-<model>.md` (e.g., `2025-06-07-01-llama3.1-8b.md`).
- Use the result template from `README.md`. Do not modify the template columns.
- Do not overwrite previous result files — create a new dated file for each run.
- Pass/fail is determined by the rubric in `README.md`. Record specific failures in the Notes column.

### Last Verified

Model lists may be preceded by a comment: `<!-- Last verified: YYYY-MM-DD -->`. This indicates the model ids were confirmed available on that date. Update this comment when re-verifying model availability.
