# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

#### Skills

- `cargo` skill with `cargo_check` and `cargo_test` tools: verify that code changes compile and pass tests during a session, with optional `package` parameter to scope to a specific workspace member. Uses OR-group bins (`cargo` or `nix`) so the skill works on both standard Rust installs and NixOS environments. PostProcess hint scripts produce concise summaries (clean check confirmation, test result lines). `cargo build` is intentionally excluded — building the binary is the user's responsibility
- `git` skill tools for branch integration: `git_merge` (squash merge by default), `git_rebase`, `git_rebase_continue`, `git_rebase_abort`, `git_cherry_pick`, `git_cherry_pick_continue`, `git_cherry_pick_abort`, and `git_reset` (mixed reset, default `HEAD~1`)
- `git_diff_lines` and `git_show_lines` line-range companion tools: read a range of lines from truncated `git diff` or `git show` output with `{line_number}\t{content}` format. When `end_line` is omitted, reads from `start_line` to the end of the output. CLI subcommands: `chai git diff-lines` and `chai git show-lines`
- `git_log` `skip` parameter: skip N commits before starting output, enabling native pagination through commit history
- `git_branch_delete` `force` parameter: when `true`, runs `git branch -D` instead of `git branch -d`, enabling deletion of branches that were squash-merged (where Git does not consider the branch fully merged). Protected branches (`main`, `release/*`) are always blocked by the deny pattern regardless of the `force` setting
- `absentDefault` support for positional args (e.g., `git_reset` default ref `HEAD~1`), with `postProcess` arg substitution
- `split` positional args for whitespace-separated multi-value parameters (e.g., `git_add` files, `git_cherry_pick` commits)
- `tools.json` schema additions: `kind: "literal"` and `kind: "tempfile"` arg kinds, `value` field for literal args, `split` field for positional args, `subcommandOverride` field on `flagIfBoolean` args to switch the execution spec's subcommand when the boolean is true, `truncationHint` field on execution specs for per-tool truncation notice templates
- `binaryWrapper` field on execution specs: wrap binary invocations through a command prefix (e.g. `nix develop --command`) for environments where tools are not directly on PATH
- OR-group semantics for `metadata.requires.bins`: a list of lists `[["cargo"], ["nix"]]` loads the skill when any one group has all its binaries on PATH (backward-compatible; flat lists still require all binaries)
- `condition` field on execution specs: loader selects specs based on which bin group matched, keeping the executor unaware of bin group logic

### Fixed

#### Desktop

- Gateway errors (config parse failure, missing binary, spawn failure) are now visible on the Gateway screen and in the header — previously, the error label was inside a `!running` guard that hid it when the gateway failed to start
- Gateway crash errors are surfaced with the actual error message extracted from the log buffer (e.g. "sandbox directory not found at...") — previously, crashes were completely silent
- Crash vs. user-initiated stop is now distinguished: clicking "Stop gateway" shows no error, while an unexpected exit surfaces the error from the log buffer
- `CHAI_BIN` set in `.env` to a non-existent path now prevents the gateway from starting with a clear error message — previously, the gateway started with a broken tool executor that failed silently on every tool call
- Skills fetch failures show a red error message on the Skills screen instead of an indefinite "Loading skills..." spinner
- Agent detail fetch failures show a red error message on the Agent and Tools screens instead of an indefinite "Loading agent detail..." placeholder
- Desktop config load failure (`desktop.json`) shows an amber notice on the Settings screen — previously, failures fell back to defaults silently
- Config screen parse errors now show the actual error message with file path and detail (e.g. "failed to load config: parsing config from /path/to/config.json: expected ...") — previously, a vague weak-text "could not load profile" message was shown
- Long error messages in the header are truncated to 80 characters with a hover tooltip showing the full text
- Screen subtitles are always shown alongside errors on Config and Skills screens, consistent with the Gateway screen pattern
- Removed vestigial `chat_error` field that was never set to a non-None value

#### Skills

- `format_flag()` now strips leading dashes before prefixing, so both bare flag names (`"p"`) and pre-dashed values (`"-p"`) produce the correct CLI flag (previously `"-p"` was mangled to `"---p"`)
- Write sandbox now excludes `.git/` directories from all write targets, preventing bypass of `git` skill branch protection and allowlist restrictions (attack vectors: branch rewrite, branch deletion, force switch, hook injection, config manipulation, object injection)
- Runtime path-like value check now rejects unannotated `positional` and `flag` parameters that target a `.git/` directory
- `files_replace` and `files_write_lines` diff output now uses post-edit line numbers: removed lines show original-file numbers, added and context-after lines show new-file numbers (previously, context-after lines showed stale original numbers, and multi-match replacements could produce misordered diffs due to LCS ambiguity with repeated lines)
- `files_write_lines` `original_content` validation now tolerates blank-line boundary differences: a new Stage 5 in the `verify_original` cascade strips leading and trailing blank lines from both actual and expected content before comparing, allowing edits to succeed when the LLM includes or excludes blank lines at the range boundary differently from the file (interior blank lines are not tolerated)
- `files_write_lines` `original_content` mismatch errors now include a line-diff hint identifying the first line that differs, in addition to the existing byte-offset hint
- `files_read_lines` and `notes_read_lines` output now uses tab as the line-number separator instead of pipe (`|`), eliminating visual ambiguity when file content contains `|` characters (e.g., markdown tables, pipe-delimited data); `files_write_lines` and `files_replace` diff output also updated to use tab separator for consistency
- `files_read_lines` and `notes_read_lines` default `end_line` changed from single-line (`start_line`) to read-to-end: when `end_line` is omitted, reads from `start_line` to the end of the file instead of returning only one line. This makes line-range tools the natural pagination path after truncation
- `files_read_lines` and `notes_read_lines` no longer have `maxOutputLines` truncation — the agent controls output size via the explicit range, so truncation of agent-chosen ranges is unnecessary
- Truncated tool output now provides tool-specific pagination instructions via `truncationHint` templates: `git_diff` → `git_diff_lines`, `git_show` → `git_show_lines`, `files_read` → `files_read_lines`, `notes_read` → `notes_read_lines`, `git_log` → `skip`/`oneline`. Previously, all truncated output used a generic "Narrow your query" suggestion that was misleading when no pagination path existed
- `files_replace` automatically collapses runs of two or more consecutive blank (or whitespace-only) lines down to a single blank line before writing the file, preventing double-blank-line artifacts from deletion operations

### Changed

- Tool name renames for consistency (drop redundant noun suffixes, adopt verb-based naming): `files_read_file` → `files_read`, `files_write_file` → `files_write`, `files_delete_file` → `files_delete`, `files_search_content` → `files_search`, `files_list_dir` → `files_list`, `notes_wikilink_backlinks` → `notes_wikilink_find_backlinks`, `notes_wikilink_outlinks` → `notes_wikilink_find_outlinks`, `notes_wikilink_by_tag` → `notes_wikilink_find_by_tag`, `notes_wikilink_broken` → `notes_wikilink_find_broken`
- Parameter name renames for consistency: git `path` (repo root) → `repo`, git `file_path` → `path`, git `name`/`branch` → `branch_name`, git `files` → `paths`, notes `root` → `scope`, search `files_only` → `files_with_matches`, search `case_insensitive` → `ignore_case`, search/replace `line_numbers` → `line_number`, git-remote `directory` → `path`, git `count`/`skip` type changed from string to integer

## [0.1.0] - 2026-06-20

### Added

#### Runtime and Configuration

- Gateway with WebSocket API for agent turns, status, and streaming events
- Named runtime profiles with per-profile `config.json`, sandbox, agents, skills lockfile, device identity, and `.env` secrets
- Profile switching via CLI (`chai profile switch`) and desktop (disabled while gateway running)
- Gateway lock — only one gateway process per installation
- Token-based gateway authentication with loopback-only enforcement for unauthenticated binds
- Ed25519 device pairing protocol for WebSocket clients
- Configuration-driven architecture: `gateway`, `channels`, `providers`, `sandbox`, `agents`, `skills` blocks in `config.json`

#### Agents and Orchestration

- Orchestrator–worker delegation model with bracket-prefix targeting (`[workerId]`)
- Per-agent context directories (`AGENT.md`), skill configuration, system context, and tool lists
- Two skill context modes: `full` (inlined SKILL.md bodies) and `readOnDemand` (compact list with `read_skill` tool)
- Delegation caps: `maxDelegationsPerTurn`, `maxDelegationsPerSession`, `maxDelegationsPerWorker`
- Tool loop limit (`maxToolLoopsPerTurn`) with interrupted-state events
- User-initiated turn stop via WebSocket

#### Messaging Channels

- Telegram channel (long-poll and webhook modes; always on)
- Matrix adapter (experimental, `--features matrix`) — E2EE, room allowlist, SAS device verification via gateway HTTP routes
- Signal adapter (experimental, `--features signal`) — BYO signal-cli daemon, SSE inbound, JSON-RPC send

#### LLM Providers

- Ollama endpoint type (native `/api/chat` and `/api/tags`; local default)
- OpenAI-compatible endpoint type (`/v1/chat/completions` and `/v1/models`) — covers LM Studio, NearAI, NVIDIA NIM, and any compatible server
- Model discovery modes: `auto` (default), `lmstudio` (native LM Studio API with auto-retry on model unload), `static` (user-curated list)
- API key resolution from shell env, profile `.env`, or `config.json` (in precedence order)
- Startup warnings for privacy-sensitive provider configurations (e.g., NVIDIA NIM as default)

#### Skills

- 15 bundled skills: `files`, `files-read`, `git`, `git-read`, `git-remote`, `logs`, `notes`, `notes-daily`, `notes-frontmatter`, `notes-read`, `notes-wikilink`, `rss`, `skills`, `skills-design`, `skills-read`
- Declarative `tools.json` schema: typed tool definitions, binary allowlist, execution mapping (positional, flag, stdin, workingdir args)
- Skill scripts (`resolveCommand.script`, `postProcess.script`) for param resolution and output processing
- Sandbox annotations on tool parameters: `writePath`, `readPath`, `unsafePath`
- Deny patterns (`denyPattern`, `denyResolveCommand`, `denyAlwaysResolve`) for semantic constraints (e.g., branch protection)
- Content-addressed skill packages with immutable snapshots and atomic rollback via symlink
- Per-profile `skills.lock` with content hash pinning, monotonic generation counter, and strict/warn modes
- Capability tiers (`minimal`, `moderate`, `full`) for model–skill matching
- Skill variants (e.g., `files-read` as read-only variant of `files`) with `variant_of` frontmatter

#### Security and Sandboxing

- Per-profile write sandbox with symlink-as-authorization model
- Three-layer defense: runtime path-like value check, CWD confinement, sandbox path validation
- Binary allowlist with no shell execution (direct `execvp`)
- Skill lockfile integrity verification at gateway startup
- Agent isolation — workers receive only their own context, tools, and skills

#### CLI

- `chai init` — creates `assistant` and `developer` profiles with default config and bundled skills
- `chai gateway` — starts the gateway with optional `--profile` and `--port`
- `chai chat` — interactive CLI chat with `/new`, `/help`, `/exit`
- `chai profile` — list, current, switch
- `chai skill` — list, read, validate, init, delete, write, lock, generations, rollback, discover
- `chai file` — read-lines, write, append, patch, replace, delete, frontmatter operations, rename
- `chai logs` — recent, search

#### Desktop Application

- Native egui/eframe GUI (Chat, Skills, Agent, Tools, Config, Gateway, Logging, Settings screens)
- Gateway lifecycle: spawn mode (start/stop subprocess) or attach mode (connect to existing)
- Profile switching, session panel, model/provider dropdowns
- Streaming chat with real-time tool call/result display and worker delegation rendering
- Turn stop button and tool loop limit banner
- Desktop settings (`desktop.json`): theme, font size, log buffer size

#### Build and Distribution

- Nix flake with CLI and desktop build targets for Linux (x86_64, ARM64) and macOS (ARM64)
- Pre-built release binaries for Linux (x86_64)
- Release build script (`scripts/build-release.sh`) producing tarballs and SHA-256 checksums
- `chai init` idempotent re-runs and sandbox directory recovery

#### Documentation

- User guides (11 topics): introduction, getting started, configuration, connections, agents, skills, sandbox, CLI reference, desktop, choosing a provider, troubleshooting
- Hands-on journeys (14 walkthroughs) covering setup, channels, skills, providers, multi-agent, auth, and profiles
- Testing playbooks with provider/model coverage and standardized message sequences

### Changed

- License changed from LGPL-3.0 to GPL-3.0
