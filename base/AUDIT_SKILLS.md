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
| `logs` | Chai process logs | - | - | TODO |
| `rss` | RSS feed reading | ✅ | ✅ | TODO |
| `skills` | Skill creation and modification | ✅ | ✅ | TODO |
| `skills-design` | Design principles for skill tools | ✅ | ✅ | TODO |
| `skills-read` | Skill inspection (read-only) | ✅ | ✅ | TODO |

## Round 3: Battle-Test Plan

Each skill group is tested in a dedicated session with the relevant skills enabled. Read-only variants are not tested directly but must remain aligned with the base skill's tools and directives.

### Skillset 1: files & files-read

TODO

NOTE: files_replace was added since the last audit and needs to be thoroughly tested

NOTE: files_write_lines was updated since the last audit and needs to be thoroughly tested

QUESTION: is the same extra line logic applied to the opening line? would this reduce inference cost?

---

### Skillset 2: git, git-read, git-remote, logs, rss

TODO

NOTE: logs is a new skill that was added since the last audit

---

### Skillset 3: kb, kb-daily, kb-frontmatter, kb-wikilink

TODO

NOTE: kb_replace was added since the last audit and needs to be thoroughly tested

NOTE: kb_write_lines was updated since the last audit and needs to be thoroughly tested

QUESTION: is the same extra line logic applied to the opening line? would this reduce inference cost?

NOTE: Redundant `resolveCommand` removed from kb-family skills — see below

---

### Change: Removed Redundant `resolveCommand` From kb-Family Skills

**What changed**: Removed `resolveCommand: { script: "resolve-kb-path" }` from all `path` and `kb_root` parameters in `kb`, `kb-read`, `kb-frontmatter`, and `kb-wikilink`. Deleted 5 scripts: `resolve-kb-path.sh` (4 copies) and `resolve-kb-root.sh` (1 copy). Fixed `kb_frontmatter_read` missing `readPath: true` (security gap — resolved absolute path bypassed sandbox validation). Fixed `check-broken-links.sh` to handle absolute `kb_root` values from canonical path substitution (pre-existing bug).

**Why**: `resolve-kb-path.sh` only prepended the sandbox root to relative paths — the same thing `WriteSandbox::validate()` already does natively when it sees a relative path on a `readPath`/`writePath`-annotated parameter (lines 288–292 of `exec.rs`). The `files` skill proves this works without any resolve script. The resolve scripts were a historical artifact from before the sandbox handled relative path resolution.

**What stayed**: `kb-daily/resolve-daily-path.sh` (date → path transformation, reads config files), `kb-wikilink/build-backlink-pattern.sh` (note name → grep pattern), `kb-wikilink/normalize-tag.sh` (tag normalization), `kb-wikilink/sanitize-outlinks.sh` and `kb-wikilink/check-broken-links.sh` (post-processing). These do real value transformation, not just sandbox-root prepending.

**What needs testing**: After rebuilding binaries, verify with the kb skillset enabled:

1. **kb path resolution**: `kb_read`, `kb_list`, `kb_search` with sandbox-relative paths (e.g., `"./notes/entry.md"`) — should resolve within sandbox and return content
2. **kb write operations**: `kb_write`, `kb_write_lines`, `kb_replace`, `kb_append`, `kb_delete`, `kb_delete_dir` with sandbox-relative paths — should write/delete within sandbox
3. **kb-read alignment**: `kb_read`, `kb_read_lines`, `kb_list`, `kb_search` — same behavior as kb equivalents
4. **kb-frontmatter**: `kb_frontmatter_read` (was missing `readPath` — now sandbox-validated), `kb_frontmatter_edit`, `kb_frontmatter_delete` — should all work with sandbox-relative paths
5. **kb-wikilink path params**: `kb_wikilink_backlinks`, `kb_wikilink_outlinks`, `kb_wikilink_by_tag`, `kb_wikilink_broken` with and without optional `path` parameter — when omitted, should default to sandbox root (CWD)
6. **kb-wikilink kb_root**: `kb_wikilink_broken` and `kb_wikilink_rename` with `kb_root` provided and omitted — verify `check-broken-links.sh` handles both relative and canonical-absolute values
7. **kb-wikilink_rename**: `from`/`to` params with sandbox-relative paths, `kb_root` optional — verify `--root` flag receives canonical path
8. **kb-daily**: `kb_daily_read`, `kb_daily_write`, `kb_daily_append` — unchanged, but verify no regression since `resolve-daily-path.sh` was kept
9. **Sandbox enforcement**: Attempt to read/write paths outside the sandbox (e.g., `/etc/passwd`, `../../etc/passwd`) — should be rejected by sandbox validation

### Skillset 4: skills, skills-design, skills-read

TODO

### Cross-cutting

- [ ] Confirm `maxOutputLines` caps are applied to all output-heavy tools across all skills
- [ ] Confirm files and kb skillsets are aligned on directives and `_replace` and `_write_lines` tools are aligned

### Chai Examples

- [ ] Review example skills (`notesmd`, `notesmd-daily`, `websearch`)
- [ ] Make sure examples have valid tools, simplified SKILL.md files, and follow `skills-design`
