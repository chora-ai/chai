# Agents

This is the `AGENTS.md` file in the root of the `guides` directory.

## Directory

The `guides` directory contains conceptual and reference material — what things are, how to configure them, and how they work. Each guide covers one topic (e.g., skills, configuration, troubleshooting) and links to the relevant journey for hands-on practice.

## Guidelines

- Write against **current** behavior. If something feels awkward to document, that is a signal to improve the product, not to work around it in prose.
- Guides are **conceptual and reference**, not step-by-step procedures. For hands-on walkthroughs, link to the relevant journey instead of embedding steps. Exception: short inline examples (a single command, a config snippet) are fine when they clarify a concept.
- When a feature has both a guide and a journey, the guide explains *how it works* and the journey shows *how to do it*. Do not duplicate procedural content in the guide.
- Structure guides for **random access**. A reader should be able to jump to a section by heading and get a complete answer without reading the whole page. Cross-link sections within the guide when concepts depend on each other.
- Use **code blocks** for configuration examples, CLI commands, and directory layouts. Annotate non-obvious fields with inline comments. Prefer realistic examples over minimal stubs.
- When documenting configuration fields, include the **field name**, **type**, **default value**, and a brief note. Group related fields in tables rather than burying them in prose.

## Conventions

- **Filenames:** `NN-kebab-case.md` — a two-digit number for reading order, then a hyphen, then a short kebab-case topic name (e.g., `06-skills.md`, `10-choosing-a-provider.md`).
- **Opening:** Each guide starts with a one-paragraph summary of what it covers, followed by section headings. Do not repeat the guide title as a heading.
- **Links to journeys:** At the end of a guide (or within relevant sections), link to the corresponding journey with a "Try It" or "Next Steps" section. Use relative paths: `../journey/05-skill-files.md`.
- **Links to testing:** When a guide mentions provider or model details that have systematic testing, link to the testing playbooks: `../testing/README.md`.
- **Summary tables:** For guides with many concepts or fields, include a summary table at the end that maps questions to answers or key names to descriptions.
