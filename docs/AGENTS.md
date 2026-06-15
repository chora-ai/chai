# Agents

This is the `AGENTS.md` file in the root of the `docs` directory.

## Directory

The `docs` directory contains user-facing documentation: guides, journeys, and testing playbooks. See `README.md` for how the three doc sets relate to each other.

Each subdirectory has its own `AGENTS.md` with guidelines and conventions specific to that doc type. This file covers what is shared across all three.

## Guidelines

- Write against **current** behavior. If something feels awkward to document, that is a signal to improve the product, not to work around it in prose.
- Keep content **concise and user-facing**. Internal implementation details, design rationale, and decision notes do not belong in user docs.
- Each doc should be **self-contained enough** to serve its stated purpose. Link to related docs for background and next steps rather than repeating material.
- When multiple docs overlap in topic, each should cover its own angle (conceptual, hands-on, systematic) and link to the others. See the table in `README.md` for the division of responsibility.
- **User documentation is isolated.** Do not link to files outside `docs/` (e.g., `base/spec/`, `base/ref/`, repository root files). If user docs need information that only exists in internal docs, bring the relevant content into the user doc. This ensures user documentation can stand alone.

## Conventions

- Follow the subdirectory-specific guidelines and conventions in each `AGENTS.md`.
- Each subdirectory has a `README.md` with a table of contents. Keep it up to date when files are added or removed.
- Use **relative links** between docs (e.g., `../guides/06-skills.md`, `PROVIDER_SETUP.md`), not absolute paths or full URLs.
