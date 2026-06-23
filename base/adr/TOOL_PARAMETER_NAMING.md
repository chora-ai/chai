---
status: accepted
---

# Tool and Parameter Naming Conventions

## Context

The bundled skills evolved organically, resulting in inconsistent naming patterns across tool names and parameter names. These inconsistencies make the API surface harder for LLMs to learn and increase the risk of tool-selection errors:

- **Redundant noun suffixes**: The `files` skill used `{skill}_{verb}_{noun}` (e.g., `files_read_file`, `files_list_dir`) while the `notes` skill used `{skill}_{verb}` (e.g., `notes_read`, `notes_list`). The noun suffixes carried zero discriminating information when the verb was unambiguous within the skill.
- **Ad-hoc wikilink naming**: The `notes-wikilink` sub-skill mixed four different naming styles (noun, prepositional phrase, adjective, verb) while other notes sub-skills were consistently verb-based.
- **Parameter semantic overload**: The `path` parameter meant different things across skills — a repo root in git tools, a file target in files tools, and a directory scope in notes tools. The `root` parameter was used inconsistently for directory scope in some skills while `path` or `directory` served the same role in others.
- **Unqualified identifiers**: Branch references used bare `name` or `branch` instead of the qualified `branch_name`.
- **Flag-name misalignment**: Search tools used `files_only` and `case_insensitive` while the underlying grep flags are `--files-with-matches` and `--ignore-case`.
- **Chai CLI flag misalignment**: Chai subcommand flags used names that conflicted with the parameter conventions (e.g., `--path` for the repository root instead of `--repo`, `--file-path` instead of `--path`, `--root` instead of `--scope`). Since we control both the tool schema and the CLI, the CLI flags should align to the conventions.
- **Incorrect types**: `count` and `skip` in `git_log` were typed as strings despite being numeric.

## Decision

Adopt the following naming conventions for all bundled skill tools and parameters.

### Tool Naming

Pattern: `{skill}_{verb}` with noun suffix only for disambiguation. The `{skill}_` prefix is always the skill directory name. For sub-skills that introduce new tools (as opposed to permission-restricted subsets), the sub-skill name becomes a middle segment: `{skill}_{subskill}_{verb}`.

Rules:

- **No redundancy.** The skill prefix already establishes the domain; a noun suffix is added only when two tools share a verb and need disambiguation.
- **Disambiguation rule.** When two tools share a verb, the primary operation keeps the short name. Example: `files_delete` (file) vs. `files_delete_dir` (directory) — the primary delete operation keeps the short name.
- **Verb-based sub-skill tools.** Sub-skill tools follow `{skill}_{subskill}_{verb}`. Example: `notes_wikilink_find_backlinks` (not `notes_wikilink_backlinks`).

### Parameter Naming

| Rule | Convention | Example |
|------|-----------|---------|
| Target path | `path` — the file, note, or directory the tool operates on directly | `files_read` → `path`, `git_diff_lines` → `path` (file within repo) |
| Repository root | `repo` — disambiguates from target path in git tools | `git_status` → `repo` |
| Directory scope | `scope` — when a tool needs a directory to narrow its search or operation | `notes_daily_read` → `scope`, `notes_wikilink_find_by_tag` → `scope` |
| Qualified identifiers | `{domain}_name` — always use a qualified form | `git_branch_create` → `branch_name` |
| Multi-value | Plural for multi-value, singular for single-value | `git_add` → `paths` (accepts multiple), `git_checkout` → `branch_name` (singular) |
| Numeric types | `integer` for numeric parameters | `git_log` → `count: integer`, `skip: integer` |
| External binary flags | Parameter names align to the binary's existing flag names | `--files-with-matches` → `files_with_matches`, `--ignore-case` → `ignore_case` |
| Chai binary flags | CLI flags align to the ADR conventions | `--repo` (not `--path` for repo root), `--path` (not `--file-path` for file target), `--scope` (not `--root` for search directory) |

### Flag-Name Alignment

Parameter names should match the flags or argument names of the binaries they use, reducing the amount of inference required for skill authors who know the underlying commands. The direction of alignment depends on whether we control the binary:

- **External binaries** (git, grep, etc.) — parameter names align *to* the binary's flag names. We don't control the binary, so our parameters must match its interface. Example: `--ignore-case` → parameter `ignore_case`.
- **Chai subcommands** — CLI flags align *to* the ADR conventions. We control both the tool schema and the CLI binary, so the CLI flag names should use `repo`, `path`, `scope`, etc. per the semantic rules above. Example: the `git diff-lines` subcommand uses `--repo` (not `--path`) for the repository root and `--path` (not `--file-path`) for the file target.

## Alternatives Considered

| Alternative | Why Not Chosen |
|------------|----------------|
| `{skill}_{verb}_{noun}` always (files-style) | Noun suffixes on unique verbs carry zero discriminating information and waste tokens for LLMs |
| `{skill}_{noun}_{verb}` (REST-style) | Inconsistent with the git skill which already follows `{skill}_{verb}`; inverts the natural action-first mental model |
| `directory` for repo root | Ambiguous — could mean the directory to operate on rather than the repository root |
| `root` for directory scope | Overloaded meaning (could be repository root, filesystem root, or project root); `scope` more precisely conveys narrowing |
| Universal flag alignment (chai flags match external binaries) | Chai subcommands are not external binaries — they are our own interface. Aligning chai flags to the same conventions as the parameter schema eliminates the ambiguity of a one-directional rule and ensures the CLI is self-consistent |
| Keep existing inconsistencies | Increases LLM error rate; no principled way to decide naming for new tools |

## Consequences

- **Consistent API surface.** All bundled skills follow the same naming patterns, making tool names predictable for LLMs and skill authors.
- **Noun suffix as signal.** When a noun suffix appears, it means the verb needed disambiguation — the exception proves the rule.
- **`path` is unambiguous.** In every tool, `path` always means the direct target. Git tools use `repo` for the repository root and `path` for the file within it.
- **`scope` is unambiguous.** When a tool needs a directory to narrow its operation, `scope` consistently names that parameter.
- **Flag alignment aids discovery.** Parameter names that match CLI flag names reduce cognitive load for skill authors who know the underlying commands.
- **Directional clarity.** The flag-alignment rule is unambiguous: align to external binaries, align from chai subcommands to conventions.
- **Breaking change for custom integrations.** These renames are technically breaking for any external prompts or integrations that reference old tool or parameter names. Since bundled skills are replaced on `chai init`, the impact is limited to custom configurations.

## References

- [spec/TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) — Declarative tool schema and execution mapping.
- [spec/SKILL_FORMAT.md](../spec/SKILL_FORMAT.md) — Skill directory layout and frontmatter.
