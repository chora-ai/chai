# Agents

This is the `AGENTS.md` file in the root of the `testing` directory.

## Directory

The `testing` directory contains user testing playbooks for systematically evaluating model behavior with chai.

## Guidelines

- **New playbooks** should follow the existing naming convention and include: scope, setup (reference [PROVIDER_SETUP.md](PROVIDER_SETUP.md)), model list (with tool-support classification and source links), procedure reference, and a "See Also" section.
- **Model lists** must classify each model as: supports tools, supports tools but not trained on tool use, or does not support tools. Models without tool support should reference [20-conversation-no-tools.md](20-conversation-no-tools.md) instead of the shared tool-use message sequence.
- **Provider-specific setup** belongs in [PROVIDER_SETUP.md](PROVIDER_SETUP.md), not in individual playbook files. Playbooks should reference the provider setup section.
- **Results** are stored in `results/` with filenames following the pattern `YYYY-MM-DD-<playbook>-<model>.md`. Do not overwrite previous results.

## Conventions

- **Filename format:** `NN-category-provider-family.md` for tool-use playbooks (e.g. `01-local-ollama-llama.md`), or `NN-category-description.md` for special-purpose playbooks (e.g. `20-conversation-no-tools.md`, `21-multi-tool.md`, `22-error-handling.md`). Numbering: 01–19 for provider×family tool-use playbooks, 20+ for special-purpose playbooks.
- **Sequence references:** Each playbook specifies which sequence from the README it uses (Sequence A, B, C, or D). Do not duplicate the message sequence in the playbook file.
- **Model list format:** Group models under "supports tools," "supports tools but not trained on tool use," and "does not support tools." Each entry includes the model id as a code span, a dash, and a source link in parentheses. Fabricated or excluded models use parenthetical notes (e.g. "(excluded from list)").
- **See Also section:** Every playbook links to [PROVIDER_SETUP.md](PROVIDER_SETUP.md), the configuration guide, the provider spec, and the model spec.
- **No-tool models:** Playbooks whose models all lack tool support must direct the reader to [20-conversation-no-tools.md](20-conversation-no-tools.md) rather than the shared tool-use procedure.
- **Pass/fail:** Use the rubric in the [README](README.md#passfail-rubric) to determine pass/fail. Record specific failures in the Notes column.
