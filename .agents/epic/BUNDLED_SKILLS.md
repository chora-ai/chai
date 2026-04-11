---
status: in-progress
---

# Epic: Bundled Skills (Inventory, Generation, and Validation)

**Summary** — Chai ships bundled skill packages under **`~/.chai/skills/`** that give agents structured tool surfaces backed by the allowlist executor. Nineteen skills are inventoried: one complete reference implementation (`notesmd-daily`), sixteen drafted (including read-only variants, local and remote git variants, and the sandbox-aligned `kb` family of five skills), and two blocked pending external dependencies. The skill generation workflow — two meta-skills (`skillgen`, `skillval`) plus a `chai skill` CLI subcommand tree — enables new skills to be authored by capable models and executed by constrained ones. Write-capable skill variants depend on the write sandbox (**[WRITE_SANDBOX.md](WRITE_SANDBOX.md)**).

**Status** — **In progress.** Phases 0, 1, 3, 5, and 6 are complete. Phase 6 delivered read-only skill variants (`git-read`, `devtools-read`), `postProcess` output scripts for RSS (XML → structured table), websearch (JSON → formatted results, HTML → readable text), the `devtools_delete_file` tool, and git clone path defaulting via `resolveCommand`. Phase 2 (empirical validation) has not started. Phase 4 (deployment dependencies) is pending.

## Problem Statement

Chai's value as agent infrastructure depends on the breadth and quality of its skill surface. The AI Assistant vision requires capabilities spanning knowledge base CRUD, code inspection, git operations, web search, feed monitoring, and note relationship discovery. Each skill needs a `tools.json` descriptor (tool definitions, allowlist, execution mapping) and a `SKILL.md` (agent instructions). The generation workflow must be systematized so the skill surface can grow without requiring manual `tools.json` authoring for every new capability.

## Goal

- A complete inventory of bundled skills with clear status and remaining work for each
- A repeatable generation workflow (discover CLI → design tool surface → generate → validate)
- Empirical validation of generated skills against small local models (7B, 13B)
- Write-capable skill variants once the sandbox is available

## Current State

### Skill Inventory

| Skill | Tools | Tier | Status | Dependencies |
|---|---|---|---|---|
| `notesmd-daily` | 2 | minimal | Complete (reference implementation) | notesmd-cli |
| `notesmd` | 7 | moderate | **Drafted** | notesmd-cli |
| `obsidian` | 0 | — | Blocked (no CLI binary) | obsidian CLI |
| `obsidian-daily` | 0 | — | Blocked (no CLI binary) | obsidian CLI |
| `git-read` | 5 | minimal | **Drafted** (read-only variant) | git |
| `git` | 8 | moderate | **Drafted** (local only) | git |
| `git-remote` | 12 | full | **Drafted** (local + network, clone path defaulting) | git |
| `devtools-read` | 3 | minimal | **Drafted** (read-only variant) | cat, ls, grep |
| `devtools` | 5 | full | **Drafted** (read + write + delete) | cat, ls, grep, chai |
| `websearch` | 2 | full | **Drafted** (postProcess: JSON formatting, HTML stripping) | curl, SearXNG instance; scripts: jq |
| `rss` | 2 | moderate | **Drafted** (postProcess: XML → structured table) | curl, cat, feeds config |
| `notelink` | 3 | moderate | **Drafted** (superseded by kb-wikilink) | grep |
| `skillgen` | 6 | full | **Drafted** | chai |
| `skillval` | 3 | moderate | **Drafted** | chai |
| `kb` | 6 | moderate | **Drafted** (sandbox-aligned, CRUD + append + delete) | cat, ls, grep, chai |
| `kb-frontmatter` | 3 | moderate | **Drafted** (sandbox-aligned) | chai |
| `kb-wikilink` | 4 | moderate | **Drafted** (sandbox-aligned, uses postProcess) | grep |
| `kb-wikilink-write` | 1 | moderate | **Drafted** (sandbox-aligned, rename + link updates) | chai |
| `kb-daily` | 3 | minimal | **Drafted** (sandbox-aligned, convention file) | cat, chai |

The Obsidian team offers early CLI access to supporters; the `obsidian` and `obsidian-daily` skills are blocked until that binary is available. The `/usr/bin/obsidian` binary is the Electron desktop app launcher, not a CLI tool.

### Capability Gaps

| Capability | Status | Gap | Impact |
|---|---|---|---|
| Web search | **Skill drafted** (SearXNG backend) | SearXNG instance not yet deployed | Researcher agent has no external information access until SearXNG runs |
| Developer tools | **Write tool added** (`devtools_write_file` via `chai file write`) | `writePath: true` enforces sandbox boundary | Engineer agent can inspect and modify code within sandbox |
| Git operations | **Two variants** (`git` local, `git-remote` full) | Clone validated against sandbox; force-push not exposed | Full contribution workflow: clone, branch, commit, push |
| RSS monitoring | **Functional** (curl backend, feeds configured) | No scheduling trigger | Researcher agent can fetch feeds on demand but can't monitor automatically |
| Note linking | **Complete** (`kb-wikilink` + `kb-wikilink-write`) | None — broken link detection and rename-with-link-updates implemented | Full link discovery and write operations via sandbox-validated tools |
| Write sandbox | **Implemented** ([WRITE_SANDBOX](WRITE_SANDBOX.md)) | Runtime complete; `chai init` and docs pending | Path-argument write tools can use `writePath: true` now |
| Autonomous scheduling | Not started | Gateway is reactive (responds to messages) | No cron-like trigger for "check inbox every morning" |
| Asking boundary | Not started | Not encoded in orchestrator context | Agent can't distinguish when to act vs. escalate |
| MCP integration | Not started | Not yet supported | Can't consume the existing MCP server ecosystem |

## Scope

### In Scope

- Skill inventory maintenance and per-skill design documentation
- Generation workflow systematization (skillgen/skillval skills, CLI subcommands)
- Empirical validation against small models (7B, 13B on Ollama)
- Model-specific skill design (frontmatter fields for capability tier, recommended models, variant relationships)
- Key patterns discovered during generation (compound subcommands, resolveCommand, etc.)

### Out of Scope

- **Write sandbox implementation** — **[WRITE_SANDBOX.md](WRITE_SANDBOX.md)** (complete)
- **Skill package versioning, lockfiles, pins** — **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)** (complete)
- **Per-agent skill configuration** — **[AGENT_ISOLATION.md](AGENT_ISOLATION.md)** (complete)
- **Skill simulation infrastructure** — **[SIMULATIONS.md](SIMULATIONS.md)**
- **Tool approval / asking boundary** — **[TOOL_APPROVAL.md](TOOL_APPROVAL.md)**

## Design

### Per-Skill Design Notes

#### notesmd

Full CRUD operations for `notesmd-cli`: search, read, create, list, delete, frontmatter read, and frontmatter edit. Uses compound subcommands (`frontmatter --print`, `frontmatter --edit`) to encode constant flags into the subcommand string — the executor's `split_whitespace()` expansion handles this naturally, and the allowlist grants each mode separately, making the security boundary more precise.

The `search` subcommand was excluded — it's interactive (fzf-based, opens a note). The agent-friendly equivalent is `search-content`, which takes a positional search term and returns results to stdout.

**Remaining:** None for basic functionality — all 7 tools are drafted and functional. Write operations (`notesmd_create`, `notesmd_delete`) are binary-mediated — `notesmd-cli` resolves note name to vault path internally, bypassing sandbox validation. The `kb` skill family is the planned replacement that routes all writes through sandbox-validated paths (see **KB Skill Family** below).

#### devtools

Read-only variant wrapping standard unix tools: `cat` (read files), `ls` (list directories), `grep` (search content). Uses empty-string subcommands in the allowlist since these binaries have no subcommand structure — `"".split_whitespace()` produces no args in the executor. Uses `flagifboolean` for flag control (`-l`, `-a`, `--recursive`, etc.) since it emits the literal flag string rather than adding a `--` prefix.

Write operations use `chai file write --path <path> --content <content>` as the binary backend. The `devtools_write_file` tool has `writePath: true` on its path parameter, so the executor validates against sandbox boundaries before spawning. The `normalizeNewlines: true` flag on the content parameter converts literal `\n` sequences from JSON into real newlines. Delete operations use `chai file delete --path <path>` — the CLI validates the target is a regular file, refusing directories. The `devtools_delete_file` tool has `writePath: true` for sandbox enforcement. A `run_command` tool is intentionally excluded — it can't be made safe within the allowlist model.

**Remaining:** No immediate gaps. Future additions (file append, mkdir) can follow the same `chai file <subcommand>` pattern.

#### git

Read and local-write operations: status, log, diff, show, branch listing, staging files, and committing. Uses `git` as the binary with subcommands mapping directly to the allowlist. `git_add` takes a positional file path; `git_commit` takes a `-m` flag for the message.

Two skill variants split the trust tiers:

- **`git`** (moderate, 8 tools) — local operations only: status, log, diff, show, branch, add, commit, branch create. No network access. Suitable for smaller models working on local repos.
- **`git-remote`** (full, 12 tools) — superset of `git` plus clone, pull, push, and remote listing. `model_variant_of: git` in frontmatter so the config validator warns if both are enabled (tool name overlap). `git_clone` has `writePath: true` on its path parameter, enforcing sandbox boundaries on clone targets.

Git local write tools operate on CWD (same as read tools). The allowlist gates whether add/commit are available; the agent's CWD determines which repository is affected. No `writePath` annotation is needed for local writes because git resolves write targets internally (`.git/` relative to CWD). CWD restriction is an orchestration/profile concern, not a per-tool sandbox concern.

**Remaining:** No immediate gaps. Force-push prevention could be added as a safety measure (omit `--force` from flag options).

#### websearch

Two tools: `websearch_search` (query SearXNG) and `websearch_fetch` (fetch a URL). Both use `curl` as the binary with compound subcommands encoding constant flags (`-sf --max-time 10`). The search tool uses a `resolveCommand` script to transform a query string into a SearXNG JSON API URL, reading the instance address from `SEARXNG_URL` (defaulting to `http://localhost:8888`).

SearXNG was chosen for alignment with local-first values — all queries stay local, no API keys, no third-party dependencies. The skill structure supports adding alternative backends by swapping the URL-building script.

**Remaining:** SearXNG instance needs to be deployed (container alongside the gateway). Prompt injection via search results is the primary security concern — the SKILL.md includes directives to treat results as untrusted input.

#### rss

Feed monitoring via `curl` (fetch feeds) and `cat` (read feed config). Feeds configured in `<profileRoot>/sandbox/rss-feeds.txt` with `name|url` format. The feeds file lives in the sandbox so the orchestrator can modify it via `devtools_write_file` (sandbox-validated); worker agents with only the `rss` skill can read and fetch but not modify. Resolve scripts use `~/.chai/active/sandbox/rss-feeds.txt` — the active profile symlink ensures the right profile is always used.

The `rss_check_feed` tool accepts either a feed name (resolved to URL via script) or a direct URL. The `rss_list_feeds` tool reads the feeds file using `cat` with a `resolveCommand` script.

**Remaining:** No "new since last check" tracking — the agent gets the full feed XML each time. State tracking (last-seen entry ID per feed) would need a second sandbox file. Integration with the knowledge base (creating inbox notes from feed entries) works through the existing `notesmd` skill.

#### notelink

Knowledge-base-specific operations using `grep`: backlink discovery, tag search, and outgoing link extraction. The `notelink_backlinks` tool uses a `resolveCommand` script to transform a note name into a wikilink grep pattern. The `notelink_outlinks` tool bakes a PCRE lookbehind pattern (`(?<=\[\[)[^\]|]+`) into the compound subcommand to extract clean note names.

The approach provides raw search primitives and relies on SKILL.md instructions to guide multi-step workflows (e.g., "extract outlinks, then verify each exists"). This keeps the tools mechanical and the judgment in the agent.

**Remaining:** `check_frontmatter` and `suggest_links` are not yet implemented. `suggest_links` requires content similarity analysis better handled by the agent's reasoning than by a compiled tool. **Note:** `notelink` is superseded by `kb-wikilink` (with additional broken link detection and tag normalization) — retained during migration period.

#### skillgen and skillval

Two complementary skills for the developer profile. `skillgen` (full tier, 6 tools) handles generation — CLI discovery, reference reading, directory initialization, and writing SKILL.md/tools.json/scripts. `skillval` (moderate tier, 3 tools) handles read-only validation and inspection. The split creates a mechanical security boundary: `skillval`'s allowlist only includes read/validate/list subcommands, safe for delegation to smaller worker agents.

Both skills are backed by the `chai skill` subcommand tree in the CLI. These are binary-mediated writes — `chai` resolves skill name to skills directory path internally. The allowlist enforces compound subcommands (`skill discover`, `skill validate`, etc.).

### KB Skill Family

#### Rationale

The `notesmd`, `notesmd-daily`, `obsidian`, and `obsidian-daily` skills wrap purpose-built CLIs (`notesmd-cli`, future `obsidian` CLI) that resolve vault paths internally. This makes them **binary-mediated writes** — the executor never sees the filesystem path, so the write sandbox cannot validate targets. They also couple the skill surface to Obsidian-specific features (default vault config, `.obsidian/daily-notes.json`), even though the knowledge base is a directory of markdown files that is compatible with Obsidian but not dependent on it.

The `kb` skill family takes a different approach: **sandbox-aligned primitives**. All tools use core binaries (`cat`, `ls`, `grep`, `chai file write`) with `resolveCommand` scripts that transform KB-relative paths into absolute sandbox paths. Write tools use `writePath: true` on the resolved path, so the executor validates every write against the sandbox boundary. The agent thinks in clean relative paths (`01-admin/AI Assistant.md`); the executor enforces spatial boundaries on the resolved absolute path.

This design aligns with three architectural principles:
1. **Uniform sandbox enforcement** — all writes go through `writePath` validation, no binary-mediated exceptions
2. **No external binary dependencies** — standard unix tools + `chai` CLI, no Go/Node/Python CLIs to maintain
3. **Tool-agnostic knowledge base** — compatible with Obsidian but not coupled to it; the same skills work with any markdown-file-based knowledge base

#### Migration Path

The existing `notesmd`/`notesmd-daily` skills remain functional and are not deprecated yet. The `kb` skill family will grow incrementally:

| Skill | Purpose | Status | Replaces |
|---|---|---|---|
| `kb` | Core CRUD: read, write, append, delete, list, search | **Drafted** (6 tools) | `notesmd` (read, create, list, search, delete) |
| `kb-frontmatter` | YAML frontmatter read/edit/delete | **Drafted** (3 tools) | `notesmd` (frontmatter read, frontmatter edit) |
| `kb-wikilink` | Backlink discovery, link validation, broken link detection | **Drafted** (4 tools) | `notelink` (backlinks, outlinks, tags) |
| `kb-wikilink-write` | Rename notes with wikilink updates | **Drafted** (1 tool) | `notesmd` (move) |
| `kb-daily` | Daily note read/write/append with date resolution | **Drafted** (3 tools) | `notesmd-daily` |

Once the `kb` family covers all operations with empirical validation, the `notesmd`/`notesmd-daily`/`obsidian`/`obsidian-daily` skills can be retired. The `notelink` skill may also be absorbed into `kb-wikilink`.

#### kb

Six tools backed by `cat`, `ls`, `grep`, `chai file write`, `chai file append`, and `chai file delete`. A single `resolve-kb-path.sh` script resolves relative paths to absolute sandbox paths. The sandbox root is the KB root (`$HOME/.chai/active/sandbox`).

- `kb_read` — `cat` with resolved path
- `kb_write` — `chai file write` with resolved path (`writePath: true`) and `normalizeNewlines: true`
- `kb_append` — `chai file append` with resolved path (`writePath: true`) and `normalizeNewlines: true`
- `kb_delete` — `chai file delete` with resolved path (`writePath: true`). CLI refuses to delete directories
- `kb_list` — `ls` with resolved path (optional; defaults to KB root)
- `kb_search` — `grep --recursive --line-number` with resolved path (optional; defaults to KB root), optional `files_only`

**Key design choices:**
- Compound subcommand `"--recursive --line-number"` on `kb_search` makes recursive search with line numbers the default — KB search is always across a directory of files
- `kb_write` requires full content (complete overwrite), matching the `devtools_write_file` model. SKILL.md directs the agent to read before writing
- `kb_append` for adding content to existing notes (daily updates, log entries) without reading the full note — reduces model inference for common operations
- `kb_delete` backed by `chai file delete` (not `rm` allowlist) for safety — the CLI validates the target is a regular file, refusing directories

### Script Enhancement Assessment

Scripts offload mechanical work from the model: string formatting, pattern construction, path resolution, output parsing. Every script eliminates a class of errors that even capable models occasionally make, and that smaller models make frequently. Three script roles are now supported:

1. **Input transformation** (`resolveCommand`) — parameter → resolved value before execution. Used by 10+ skills for path resolution, pattern building, URL construction, and tag normalization.
2. **Output transformation** (`postProcess`) — raw stdout → structured text after execution. Used by `kb-wikilink` for broken link detection. Available for RSS XML parsing and websearch output formatting.
3. **Tool backend** (`chai file` subcommands) — complex operations implemented in Rust, invoked as CLI subcommands. Used by `kb`, `kb-frontmatter`, `kb-wikilink-write`, `kb-daily` for frontmatter editing, file deletion, and rename-with-link-updates.

#### Current Script Coverage

| Skill | Scripts | Role |
|---|---|---|
| `notesmd-daily` | `resolve-daily-path.sh` | Path resolution: date → vault daily-note path |
| `rss` | `resolve-feed-url.sh`, `resolve-feeds-path.sh`, `parse-rss.sh` | Name→URL resolution (resolveCommand), default path (resolveCommand), XML → structured table (postProcess) |
| `notelink` | `build-backlink-pattern.sh` | Pattern construction: note name → escaped grep pattern |
| `websearch` | `build-search-url.sh`, `format-search-results.sh`, `strip-html.sh` | URL construction (resolveCommand), JSON → structured results (postProcess), HTML → readable text (postProcess) |
| `kb` | `resolve-kb-path.sh` | Path resolution: relative → absolute sandbox path |
| `kb-wikilink` | `resolve-kb-path.sh`, `build-backlink-pattern.sh`, `normalize-tag.sh`, `check-broken-links.sh` | Path resolution, pattern construction, tag normalization, broken link filtering (postProcess) |
| `kb-wikilink-write` | `resolve-kb-path.sh`, `resolve-kb-root.sh` | Path resolution, constant KB root injection |
| `kb-daily` | `resolve-daily-path.sh` | Date-based path resolution with convention file |
| `kb-frontmatter` | `resolve-kb-path.sh` | Path resolution |
| `git-remote` | `resolve-clone-path.sh` | Relative path → sandbox path (resolveCommand) |

Skills with **no scripts**: `notesmd`, `git`, `git-read`, `devtools`, `devtools-read`, `skillgen`, `skillval`.

#### Per-Skill Script Opportunities

**rss** (done)
- **Output: RSS XML → structured text.** ~~The model receives raw XML and must parse it.~~ Implemented: `parse-rss.sh` postProcess script using awk/sed. Outputs `TITLE | DATE | LINK | SUMMARY` per entry, limits to 20 entries, handles both RSS 2.0 and Atom feeds. Uses mawk-compatible syntax (no gawk extensions).

**websearch** (done)
- **Output: HTML → readable text.** ~~`websearch_fetch` returns raw HTML.~~ Implemented: `strip-html.sh` postProcess script using sed. Extracts `<title>` and `<body>`, strips script/style blocks, decodes common HTML entities, limits to 200 lines.
- **Output: JSON → formatted results.** ~~SearXNG returns JSON.~~ Implemented: `format-search-results.sh` postProcess script using jq. Outputs top 10 results as `TITLE | URL | SNIPPET` lines. Graceful fallback to raw JSON when jq is unavailable.

**notelink** (moderate impact)
- **Input: Tag normalization.** `notelink_by_tag` passes the tag raw. A `resolveCommand` script could normalize format (strip `#` prefix if present, escape regex special characters, build the actual grep pattern). Currently the model must know to pass `#tag` vs `tag` correctly.

**git / git-remote** (partially done)
- **Input: Clone path defaulting.** ~~`git_clone` requires the agent to construct an absolute sandbox path.~~ Implemented: `resolve-clone-path.sh` resolveCommand resolves relative names to sandbox paths. Cross-param URL extraction (deriving repo name from URL) is not possible with the current `resolveCommand` architecture (`$param` only). The model still provides a directory name; the script handles the absolute path.
- **Input: Log format defaulting.** A `resolveCommand` could add `--format="%h %s (%an, %ar)"` to produce a consistent, parseable log format rather than relying on the model to pass `--oneline`. Deferred — low priority, `--oneline` is well-handled by models.

**devtools** (low impact)
- Generic by design. KB-specific improvements belong in the `kb` skill family, not here.

**skillgen / skillval** (low impact)
- Full-tier skills for capable models. The model must generate complex JSON/markdown content where scripts can't substitute for judgment.

#### Architecture Implications

All three script roles are now supported:

1. **Input transformation** (`resolveCommand`) — implemented since Phase 1. Scripts transform parameters before execution.
2. **Output transformation** (`postProcess`) — implemented in Phase 5. The `PostProcessSpec` struct on `ExecutionSpec` pipes stdout through a script; 7 unit tests. First use: `kb_wikilink_broken` filters grep output to only nonexistent link targets.
3. **Tool backend** (`chai file` subcommands) — implemented in Phase 5. Seven subcommands (write, append, delete, frontmatter-read, frontmatter-edit, frontmatter-delete, rename-note) provide typed CLI operations that the allowlist executor invokes. Each subcommand is a thin Rust function with safety guards; the skill's `writePath` provides sandbox enforcement.

**All high-impact script opportunities are now implemented.** RSS (`parse-rss.sh`), websearch (`format-search-results.sh`, `strip-html.sh`), and git-remote (`resolve-clone-path.sh`) output/input scripts are in place. The remaining low-priority opportunity is git log format defaulting (deferred).

**Limitation discovered:** `resolveCommand` only passes `$param` (the current parameter value). Cross-param references (e.g., accessing the URL from the path resolver) are not supported. A `$params.<name>` syntax would enable richer resolver logic but is not needed for any current skill.

### Skill Splitting Strategy

Smaller skills with fewer tools are easier for smaller models. The orchestrator delegates to specialized workers, each with a narrow tool surface. Splitting criteria:

1. **Read/write separation** — read-only skills can be assigned to untrusted or minimal-tier workers
2. **Trust tier boundaries** — network operations, file writes, and destructive operations warrant separate skills with higher tier requirements
3. **Domain coherence** — tools within a skill should serve one conceptual task (inspection, modification, monitoring)
4. **Tool count target** — 2–5 tools per skill for minimal/moderate tiers; up to 8 for full tier

#### Recommended Splits

**git → git-read + git (current) + git-remote (current)** — **Done**

| Variant | Tools | Tier | Agent Role | Status |
|---|---|---|---|---|
| `git-read` | 5: status, log, diff, show, branch | minimal | Code reviewer, inspector | **Drafted** |
| `git` | 8: read + add, commit, branch-create | moderate | Local developer | **Drafted** |
| `git-remote` | 12: all + clone, pull, push, remote | full | Open-source contributor | **Drafted** |

The `git-read` variant enables a pure read-only reviewer agent backed by a 7B model. `model_variant_of: git` links all three. Tool names are shared across variants — config validation should warn if overlapping variants are enabled simultaneously.

**devtools → devtools-read + devtools (current)** — **Done**

| Variant | Tools | Tier | Agent Role | Status |
|---|---|---|---|---|
| `devtools-read` | 3: read_file, list_dir, search_content | minimal | Code inspector, file browser | **Drafted** |
| `devtools` | 5: read + write_file + delete_file | full | File editor with sandbox writes | **Drafted** |

Read-only variant for worker agents that only need to inspect files. Write and delete tools stay in the full-tier skill. `model_variant_of: devtools` on the read-only variant.

**notelink → absorb into kb-wikilink**

Rather than splitting notelink, its three tools migrate into `kb-wikilink` with sandbox path resolution. The standalone `notelink` skill remains available during the migration period but is superseded once `kb-wikilink` is complete.

**kb → kb + kb-frontmatter + kb-wikilink + kb-daily (already planned)**

Already designed for splitting. Each skill handles one domain: CRUD, frontmatter, links, daily notes. See **KB Skill Family** section above and detailed plans below.

**No split recommended:** `rss` (2 tools), `websearch` (2 tools), `skillgen`/`skillval` (already split by read/write).

#### Orchestrator Delegation Patterns

The orchestrator assigns skills to worker agents based on the task:

| Task | Worker Skills | Min Tier |
|---|---|---|
| "Read this note" | `kb` | minimal |
| "Update the frontmatter on these notes" | `kb-frontmatter` | moderate |
| "Check for broken wikilinks" | `kb-wikilink` | moderate |
| "Review the recent commits" | `git-read` | minimal |
| "Clone and set up this repo" | `git-remote` | full |
| "Check today's RSS feeds" | `rss` | moderate |
| "Research this topic online" | `websearch` | full |
| "Create today's daily note" | `kb-daily` | minimal |
| "Inspect this source file" | `devtools-read` | minimal |

### KB Skill Family — Detailed Plans

#### kb-frontmatter (drafted)

YAML frontmatter read, edit, and delete operations. Replaces `notesmd_frontmatter_read` and `notesmd_frontmatter_edit` with sandbox-validated tools backed by `chai file` CLI subcommands.

**Tools (3):**

| Tool | Operation | Backend |
|---|---|---|
| `kb_frontmatter_read` | Extract and display frontmatter from a note | `chai file frontmatter-read` |
| `kb_frontmatter_edit` | Set a frontmatter key to a value (add or update) | `chai file frontmatter-edit` (`writePath: true`) |
| `kb_frontmatter_delete` | Remove a frontmatter key | `chai file frontmatter-delete` (`writePath: true`) |

**CLI implementation (in `crates/cli/src/main.rs`):**
- **frontmatter-read:** Extracts YAML between first `---` pair, outputs without delimiters. Errors if no frontmatter found.
- **frontmatter-edit:** Finds the key line and replaces it, or inserts before closing `---`. Creates a frontmatter block if none exists.
- **frontmatter-delete:** Removes the key line from the frontmatter block. No-op if key not found.

**Improvements over notesmd-cli:**
1. Sandbox-validated writes via `writePath: true` — notesmd-cli resolves paths internally
2. `kb_frontmatter_delete` — notesmd-cli has no frontmatter key deletion
3. Creates frontmatter block if missing — notesmd-cli requires existing frontmatter
4. Path-argument based — executor validates paths, not the binary

#### kb-wikilink (drafted)

Backlink discovery, outgoing link extraction, broken link detection, and tag search. Absorbs `notelink` functionality with sandbox-aligned paths and adds broken link detection via `postProcess`. All paths resolve through `resolve-kb-path.sh` — the agent provides KB-relative paths, scripts resolve to absolute sandbox paths.

**Tools (4):**

| Tool | Operation | Script Mechanism |
|---|---|---|
| `kb_wikilink_backlinks` | Find all notes linking to a given note | `resolveCommand`: `build-backlink-pattern.sh` (note name → grep pattern) + `resolve-kb-path.sh` (optional path) |
| `kb_wikilink_outlinks` | Extract all wikilink targets from a note | `resolveCommand`: `resolve-kb-path.sh` (path). Compound subcommand `-oP (?<=\[\[)[^\]|]+` extracts clean names |
| `kb_wikilink_by_tag` | Find notes containing a tag | `resolveCommand`: `normalize-tag.sh` (strips `#`, escapes regex) + `resolve-kb-path.sh` (optional path) |
| `kb_wikilink_broken` | List broken wikilinks in a note | `resolveCommand`: `resolve-kb-path.sh`. `postProcess`: `check-broken-links.sh` filters grep output to only nonexistent targets |

**Scripts (4):**
- `resolve-kb-path.sh` — same path resolution as the `kb` skill (relative → absolute sandbox path)
- `build-backlink-pattern.sh` — migrated from `notelink`; escapes BRE specials, builds `\[\[<name>` pattern
- `normalize-tag.sh` — **new**; strips `#` prefix, escapes regex specials. Eliminates tag format ambiguity for small models (notelink passed tags raw)
- `check-broken-links.sh` — **new**; `postProcess` script that reads wikilink targets from stdin, checks `<kb_root>/<target>.md` and `<kb_root>/<target>` existence, outputs only broken targets. **Improvement over notelink:** broken link detection is a single tool call instead of a multi-step model workflow (extract outlinks → check each one manually)

**Improvements over notelink:**
1. KB-relative paths instead of absolute paths — cleaner for agents, resolved via scripts
2. Optional search path — defaults to KB root when omitted (notelink required the vault root every time)
3. Tag normalization — `#tag` and `tag` both work (notelink required exact format)
4. One-call broken link detection — `postProcess` does existence checking mechanically (notelink required the model to orchestrate a multi-step workflow)

**Remaining:** None — rename-with-link-updates is now in the separate `kb-wikilink-write` skill (trust tier separation: read-only vs write operations).

#### kb-wikilink-write (drafted)

Rename knowledge base notes with automatic wikilink updates. Separated from `kb-wikilink` (read-only) for trust tier enforcement.

**Tools (1):**

| Tool | Operation | Backend |
|---|---|---|
| `kb_wikilink_rename` | Rename a note and update all wikilinks referencing it | `chai file rename-note` (`writePath: true` on both from/to) |

**CLI implementation (`chai file rename-note`):**
- Validates source exists (regular file), destination doesn't exist, parent directory exists
- Extracts note names from file stems (without `.md` extension)
- Renames the file via `fs::rename`
- Walks all `.md` files under `--root`, replacing `[[old name]]` → `[[new name]]` and `[[old name|` → `[[new name|` (preserving aliases)
- Reports count of files with updated links

**Scripts (2):**
- `resolve-kb-path.sh` — standard KB path resolution (for `--from` and `--to`)
- `resolve-kb-root.sh` — always outputs KB root path, ignoring input (for `--root`)

**Key design choice:** The `--root` parameter is not exposed to the model. It's injected by the executor via `resolve-kb-root.sh` mapped to the `from` param (the script ignores its input). The model only provides `from` and `to` — the link-update scope is always the full KB.

**Improvement over notesmd-cli:** notesmd-cli's move operation is binary-mediated (executor can't validate paths). `kb_wikilink_rename` validates both source and destination against the sandbox via `writePath: true`.

#### kb-daily (drafted)

Daily note operations with configurable date-based path resolution. Replaces `notesmd-daily` without depending on `.obsidian/daily-notes.json`.

**Tools (3):**

| Tool | Operation | Backend |
|---|---|---|
| `kb_daily_read` | Read today's or a specified date's daily note | `cat` with `resolveCommand` date → path |
| `kb_daily_write` | Create or overwrite a daily note | `chai file write` with `resolveCommand` + `writePath` + `normalizeNewlines` |
| `kb_daily_append` | Append content to a daily note | `chai file append` with `resolveCommand` + `writePath` + `normalizeNewlines` |

**Key script:**
- `resolve-daily-path.sh` — reads a convention file in the sandbox (`sandbox/.kb-daily.conf` with `folder=00-daily`) instead of `.obsidian/daily-notes.json`. Falls back to `00-daily/<date>.md` if no config exists. Defaults to today's date when no date parameter is provided.

**Improvements over notesmd-daily:**
1. No Obsidian dependency — convention file instead of `.obsidian/daily-notes.json`
2. Append tool — `kb_daily_append` adds content without reading the full note (notesmd-daily's `update` mode requires read→modify→write for appending)
3. Convention file in sandbox — modifiable by the orchestrator via `devtools_write_file`
4. Date defaults to today — the `resolveCommand` script handles this, not the model

**Capability tier:** minimal. Three tools, deterministic path resolution, no judgment required. Target for 7B models.

#### kb — delete and append tools (implemented)

`kb_delete` is backed by `chai file delete --path <path>`. The CLI validates the target is a regular file and refuses to delete directories — safer than an `rm` allowlist entry. `kb_append` is backed by `chai file append --path <path> --content <content>`, which creates the file if it doesn't exist. Both tools use `writePath: true` for sandbox enforcement.

### Generation Results

Skills were generated using Claude Opus 4 via the developer profile, producing skills through the `chai skill` CLI subcommand tree. Six new skills were generated: `notesmd`, `git`, `devtools`, `websearch`, `rss`, `notelink`. All pass structural validation via `chai skill validate`.

### Key Patterns Discovered During Generation

- **Compound subcommands** encode constant flags into the subcommand string (e.g., `"frontmatter --print"`, `"-sf --max-time 10"`, `"-oP (?<=\\[\\[)[^\\]|]+"`). The executor's `split_whitespace()` expansion handles this, and the allowlist checks the full compound string, making each mode a separate security grant.
- **Empty-string subcommands** handle binaries without subcommand structure (e.g., `"cat": [""]`). The executor's `split_whitespace()` on `""` produces no args.
- **`resolveCommand` scripts** transform parameters at execution time (query→URL, feed name→URL, note name→regex pattern, date→file path). Scripts run via `sh` from the skill's `scripts/` directory with no allowlist entry needed.
- **`flagifboolean`** provides full control over emitted flag strings, avoiding the `--` prefix that `kind: "flag"` adds automatically. Essential for single-dash flags (`-l`, `-a`, `-r`) and git flags (`--cached`, `--all`).

### Model-Specific Skills

Skills should carry frontmatter that makes model requirements and variant relationships explicit:

```yaml
# SKILL.md frontmatter additions
recommended_models:
  - qwen2.5:7b
  - llama3.1:8b
capability_tier: minimal
model_variant_of: notesmd
```

- **`recommended_models`** — models empirically tested against this skill's schema. Populated by simulation results, not guesswork.
- **`capability_tier`** — minimum model capability: `minimal` (pure schema, 7B target), `moderate` (some interpretation, 13B–30B), `full` (judgment-tier, capable cloud or 70B+).
- **`model_variant_of`** — links to a related skill at a different tier. Used for config validation: warn when variant skills are both enabled (creates tool overlap).

Context budget implication: `minimal`-tier skills should use `readOnDemand` context mode so SKILL.md instructions load on demand, not at session start.

### Reference Documents

**Source code:**
- `~/Code/chora-ai/chai/.agents/spec/SKILL_FORMAT.md` — skill directory layout, frontmatter, metadata, context modes
- `~/Code/chora-ai/chai/.agents/spec/TOOLS_SCHEMA.md` — `tools.json` schema: tools array, allowlist, execution mapping, arg kinds, `resolveCommand`, `writePath`
- `~/Code/chora-ai/chai/crates/lib/config/skills/` — all bundled skills (reference + generated)
- `~/Code/chora-ai/chai/crates/cli/src/main.rs` — `chai skill` subcommand implementations

## Requirements

- [x] **Reference implementation** — `notesmd-daily` complete with `tools.json` and SKILL.md
- [x] **Skill inventory** — 17 skills inventoried with status and dependencies
- [x] **Generation workflow** — `skillgen` (6 tools) and `skillval` (3 tools) implemented
- [x] **CLI subcommands** — `chai skill` tree (8 subcommands) implemented
- [x] **Compound subcommand support** — executor `split_whitespace()` change
- [x] **Batch generation** — 6 new skills generated and validated
- [x] **Tool call examples** — added example JSON for every tool in all SKILL.md files to improve small-model accuracy
- [x] **Git write tools** — `git_add` and `git_commit` added to the git skill
- [ ] **Empirical validation** — test skills against 7B and 13B models on Ollama
- [ ] **Capability floor** — document smallest model that reliably generates correct tool calls
- [ ] **Model-specific frontmatter** — implement `recommended_models`, `capability_tier`, `model_variant_of` in SKILL.md parsing (see **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)** for startup validation)
- [ ] **SearXNG deployment** — deploy instance to unblock `websearch` skill
- [x] **Feeds configuration** — `~/.chai/feeds.txt` with arXiv cs.AI and cs.CR; resolve scripts written
- [x] **Devtools write tool** — `devtools_write_file` via `chai file write` with `writePath: true` on path param for sandbox validation
- [x] **Notelink fix workflow** — broken link detection via `kb_wikilink_broken`, rename-with-link-updates via `kb_wikilink_rename`, manual fixes via `kb_write`
- [x] **KB skill** — `kb` skill with 6 tools (read, write, append, delete, list, search) backed by sandbox-validated primitives
- [x] **KB frontmatter skill** — `kb-frontmatter` with 3 tools (read, edit, delete) backed by `chai file frontmatter-*` CLI subcommands
- [x] **KB wikilink skill** — `kb-wikilink` with 4 tools (backlinks, outlinks, by_tag, broken) using sandbox paths and postProcess
- [x] **KB wikilink write skill** — `kb-wikilink-write` with 1 tool (rename) backed by `chai file rename-note` (renames file + updates all wikilinks)
- [x] **KB daily skill** — `kb-daily` with 3 tools (read, write, append) using convention-file-based date path resolution
- [x] **KB delete tool** — `kb_delete` backed by `chai file delete` (validates regular file, refuses directories)
- [x] **chai file subcommands** — 7 subcommands: write, append, delete, frontmatter-read, frontmatter-edit, frontmatter-delete, rename-note
- [x] **Output post-processing** — `postProcess` field on execution specs: pipes stdout through a script, returns transformed output (7 unit tests, used by `kb-wikilink` broken link detection)
- [x] **Script-as-operation (resolved via CLI)** — all complex operations (frontmatter, rename-with-link-updates) resolved via `chai file` subcommands. `sh`-based execution remains an option for future skills but is not blocking any current requirements
- [x] **git-read skill** — read-only git variant (5 tools) for minimal-tier reviewer agents
- [x] **devtools-read skill** — read-only devtools variant (3 tools) for minimal-tier inspector agents
- [x] **devtools delete tool** — `devtools_delete_file` via `chai file delete` with `writePath: true`
- [x] **RSS output script** — `parse-rss.sh` postProcess transforms XML to `TITLE | DATE | LINK | SUMMARY` table (handles RSS 2.0 and Atom)
- [x] **Websearch output scripts** — `format-search-results.sh` (SearXNG JSON → `TITLE | URL | SNIPPET`, requires `jq`); `strip-html.sh` (HTML → readable text via sed)
- [x] **Git clone path defaulting** — `resolve-clone-path.sh` resolveCommand resolves relative names to sandbox; cross-param URL extraction not possible (resolveCommand only passes `$param`)
- [x] **Tag normalization script** — `normalize-tag.sh` in `kb-wikilink` strips `#` prefix and escapes regex specials

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| **0** | **Populate** — complete bundled skill set with `tools.json` and SKILL.md | Done (14 drafted, 2 blocked, 1 complete) |
| **1** | **Generation workflow** — `skillgen`/`skillval` skills, CLI subcommands, batch generation | Done |
| **2** | **Empirical validation** — test against small models, document capability floor, refine schemas | Pending |
| **3** | **Write variants** — git write tools, devtools write/delete tools, kb write/append/delete/rename tools | Done |
| **4** | **Deployment dependencies** — SearXNG instance, feeds configuration | Pending |
| **5** | **Sandbox-aligned skills** — `kb` family (5 skills, 17 tools) replacing `notesmd`/`notelink` with sandbox-validated primitives, `postProcess` executor feature, 7 `chai file` CLI subcommands | Done |
| **6** | **Script enrichment and skill splitting** — read-only skill variants (`git-read`, `devtools-read`), RSS/websearch `postProcess` output scripts, `devtools_delete_file` tool, git clone path defaulting, stale script cleanup | Done |

## Open Questions

- **Obsidian CLI access** — the `obsidian` and `obsidian-daily` skills are blocked until the Obsidian team makes the CLI binary available. Monitor for availability.
- **Model-specific frontmatter parsing** — where should `recommended_models`, `capability_tier`, `model_variant_of` be validated? At skill load time? At profile startup? See **[SKILL_PACKAGES.md](SKILL_PACKAGES.md)** for the startup validation design.
- **Skill variant naming convention** — as variants multiply (`git-read`, `git`, `git-remote`; `devtools-read`, `devtools`), should variant relationships be encoded in naming (prefix/suffix) or purely in frontmatter (`model_variant_of`)? Naming makes relationships visible at the filesystem level; frontmatter keeps names clean but requires inspection to discover relationships.

### Resolved

- **Output post-processing mechanism** — resolved as per-tool `postProcess` field on execution specs. Implemented with `PostProcessSpec` struct, 7 unit tests. Per-tool precision was the right choice; per-skill would have been too coarse (different tools in the same skill need different post-processing).
- **Script-as-operation pattern** — resolved via `chai file` subcommands. All complex operations (frontmatter read/edit/delete, rename-with-link-updates, file delete, file append) are implemented as CLI subcommands. `sh`-based execution remains a future option but is not blocking any current requirements.
- **Cross-param resolution** — `resolveCommand` only passes `$param` (the current parameter value). Cross-param references (e.g., accessing URL from the path resolver in `git_clone`) are not supported. A `$params.<name>` syntax could enable richer resolver logic. Low priority — no current skill requires it; the clone path defaulting works with relative names instead.
- **Script portability** — scripts must target mawk/POSIX, not gawk. The RSS script hit two mawk incompatibilities: `match()` with capture groups (gawk-only) and `close` as a variable name (reserved keyword in mawk). All scripts now use mawk-compatible syntax.

## Related Epics and Docs

| Topic | Where |
|-------|-------|
| Write sandbox (path enforcement) | [WRITE_SANDBOX.md](WRITE_SANDBOX.md) |
| Skill packages (versioning, lockfiles) | [SKILL_PACKAGES.md](SKILL_PACKAGES.md) |
| Agent isolation (per-agent skills) | [AGENT_ISOLATION.md](AGENT_ISOLATION.md) |
| Simulations (model testing) | [SIMULATIONS.md](SIMULATIONS.md) |
| Tool approval (asking boundary) | [TOOL_APPROVAL.md](TOOL_APPROVAL.md) |
| Skill format spec | [spec/SKILL_FORMAT.md](../spec/SKILL_FORMAT.md) |
| Tools schema spec | [spec/TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) |
