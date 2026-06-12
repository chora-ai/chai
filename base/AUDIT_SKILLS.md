# AUDIT: Bundled Skills Review

## Purpose

Cross-skill audit of all bundled skills in `chai/crates/lib/config/skills/`, guided by the design principles in `skills-design/SKILL.md`.

## Bundled Skills

| Skill | Purpose | Round 1 | Round 2 | Round 3 |
|-------|---------|---------|---------|---------|
| `files` | Read, write, search, delete files and directories | ✅ | ✅ | TODO |
| `files-read` | Read-only subset of `files` | ✅ | ✅ | TODO |
| `git` | Git operations (write) | ✅ | ✅ | TODO |
| `git-read` | Git operations (read-only) | ✅ | ✅ | TODO |
| `git-remote` | Git remote operations (clone, pull, push) | ✅ | ✅ | TODO |
| `kb` | Knowledge base management | ✅ | ✅ | TODO |
| `kb-read` | Read-only subset of `kb` | ✅ | ✅ | TODO |
| `kb-daily` | Daily note creation | ✅ | ✅ | TODO |
| `kb-frontmatter` | Frontmatter manipulation | ✅ | ✅ | TODO |
| `kb-wikilink` | Wikilink resolution and rename | ✅ | ✅ | TODO |
| `rss` | RSS feed reading | ✅ | ✅ | TODO |
| `skills` | Skill creation and modification | ✅ | ✅ | TODO |
| `skills-design` | Design principles for skill tools | ✅ | ✅ | TODO |
| `skills-read` | Skill inspection (read-only) | ✅ | ✅ | TODO |

## Round 3: Battle-Test Plan

Each skill group is tested in a dedicated session with the relevant skills enabled. Read-only variants are not tested directly but must remain aligned with the base skill's tools and directives.

### Skillset 1: files & files-read

TODO

---

### Skillset 2: git, git-read, git-remote, rss

TODO

---

### Skillset 3: kb, kb-daily, kb-frontmatter, kb-wikilink

TODO

---

### Skillset 4: skills, skills-design, skills-read

TODO
