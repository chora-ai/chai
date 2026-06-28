# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

#### Runtime and Configuration

- Multiple orchestrator entries in the `agents` array — validation relaxes from "exactly one orchestrator" to "at least one"; each orchestrator has its own provider, model, skills, and delegation policy
- `enabledWorkers` field on orchestrator entries — optional array of worker ids this orchestrator can delegate to; absent or `null` means no workers enabled (`delegate_task` not offered); empty array means all profile workers; non-empty means only listed workers; unknown worker ids produce a validation error; rejected on worker entries at parse time
- `OrchestratorConfig` type with accessor methods — `AgentsConfig` now holds `Vec<OrchestratorConfig>` with `default_orchestrator()`, `orchestrator(id)`, and `orchestrator_ids()` instead of flat top-level fields; on-disk `config.json` format is unchanged
- Per-orchestrator `OrchestratorRuntime` — gateway builds a separate runtime (system context, skills, tools, executor, context mode) for each orchestrator at startup; `GatewayState` replaces flat `system_context`/`skills`/`tools_list`/`tool_executor` fields with `orchestrator_runtimes: HashMap<String, OrchestratorRuntime>`
- Per-orchestrator session stores — each orchestrator gets its own `SessionStore` at `<profile_dir>/agents/<orchestrator_id>/sessions/`; held in `GatewayState.session_stores: HashMap<String, Arc<SessionStore>>`; sessions from one orchestrator are isolated from another
- `agent` RPC `orchestratorId` parameter — optional; when omitted, the default (first) orchestrator is used; when provided, the gateway resolves the matching `OrchestratorRuntime` and `SessionStore`; unknown orchestrator IDs return an error
- `sessions.list` RPC `orchestratorId` parameter — optional; when omitted, the default orchestrator's session store is queried; enables per-orchestrator session listing
- `sessions.delete_all` RPC `orchestratorId` parameter — optional; when provided, only clears sessions for that orchestrator's session store; when omitted, clears all (backward compatible); `sessions.cleared` event includes `orchestratorId`
- `sessions.history` and `sessions.delete` search across all session stores — a session can be retrieved or deleted regardless of which orchestrator created it
- `enabledWorkers` system prompt filtering — `build_workers_context()` only includes workers in the orchestrator's `enabledWorkers` (when set) in the `## Workers` roster; the model never sees excluded workers
- `enabledWorkers` delegation enforcement — `resolve_delegate_target()` rejects delegation to a worker not in the orchestrator's `enabledWorkers`, mirroring the `enabledProviders` check
- `enabledProviders` per-orchestrator enforcement — delegation to a worker is rejected when the worker's provider is not in the requesting orchestrator's `enabledProviders`; enforced in `resolve_delegate_target()` against the calling orchestrator's config
- `agentDetail` RPC per-orchestrator resolution — the handler checks `orchestrator_runtimes` map first, then falls back to `worker_delegate_runtimes`
- Channel-bound messages always use the default orchestrator — `process_inbound_message` has no `orchestratorId` parameter
- Gateway broadcast events (`session.message`, `session.deleted`, delegation events) include `orchestratorId` in their payloads — enables clients to filter events by active orchestrator

#### Desktop

- Orchestrator selector ComboBox in the chat sidebar — "Agent" section heading above "Sessions"; switching updates the session list and provider/model defaults; disabled when only one orchestrator is configured or during an active agent turn
- Config screen shows all orchestrators with per-orchestrator `enabledWorkers` display — `None` shows "(none)", empty array shows "(all)", non-empty shows comma-separated worker ids
- Gateway screen shows `enabledWorkers` per orchestrator with "(none)"/"(all)" display — the row is always visible (previously `None` was hidden)
- Skills screen correctly identifies all orchestrator agents (not just the default) for green/blue color coding
- Agent and Tools screens correctly label all orchestrator agents with `— orchestrator` suffix (not `— worker`) using a HashSet of orchestrator IDs
- "Clear all sessions" scopes deletion to the active orchestrator — passes `orchestratorId` to `sessions.delete_all`
- Session event filtering by orchestrator — desktop ignores `session.deleted` and `sessions.cleared` events from non-active orchestrators
- Provider/model ComboBoxes cascade on orchestrator switch — selecting a different orchestrator updates the Provider and Model ComboBoxes to reflect the new orchestrator's defaults

#### CLI

- `--agent <id>` flag on `chai chat` — selects which orchestrator to use for the chat session; passes `orchestratorId` in the agent RPC
- `--agent <id>` flag on `chai sessions list` — scopes session listing to a specific orchestrator's session store
- `--agent <id>` flag on `chai sessions clear` — scopes session clearing to a specific orchestrator's session store; without `--agent`, clears the default orchestrator's sessions

## [0.3.0] - 2026-06-27

### Added

#### CLI

- `chai resolve` subcommand with five variants: `repo-path`, `cargo-path`, `clone-path`, `file-path`, and `sandbox` — sandbox-aware path resolution that validates paths before they reach tools
- `chai sessions list` — list sessions for the active profile (or a specified profile via `--profile`) directly from disk; displays session id, timestamps, message count, and channel binding; sorted by most recently updated; no gateway connection required
- `chai sessions delete <ID>` — delete a session by id directly from disk; removes the session and its binding; no gateway connection required
- `chai sessions clear` — delete all sessions directly from disk; reports the count of deleted sessions; no gateway connection required

#### Desktop

- Session sidebar loads persisted sessions on gateway connect — sidebar is populated with timestamps and short session IDs
- Session history loads on demand when selecting a persisted session — chat area shows "Loading session history…" while the fetch is in flight
- Per-session "×" delete button in the session sidebar
- "Clear all sessions" button with confirmation dialog at the bottom of the session sidebar
- "New session" button is always visible in the sessions panel, regardless of whether a session is active
- Channel-bound sessions display a channel tag in the sidebar and are read-only from the desktop — clicking a channel session loads its history for viewing but disables the chat input

#### Runtime and Configuration

- `sessions.list` WebSocket method: returns summary metadata (id, timestamps, message count, channel binding) for all sessions, sorted by most recently updated
- `sessions.history` WebSocket method: returns full message history for a given session id, with optional `limit` and `offset` pagination
- `sessions.delete` WebSocket method: deletes a session from memory and disk, removes associated bindings, broadcasts a `session.deleted` event
- `sessions.delete_all` WebSocket method: deletes all sessions for the active profile from memory and disk, broadcasts a `sessions.cleared` event

#### Skill Authoring

- `hintConditions` field on execution specs — declarative inline hint conditions with four types: `match` (substring in output), `exitCode` (integer or `"nonzero"`), `notEmpty` (non-empty output), and `whenArg` (parameter-value match). The `hint` field supports `{param_name}` template variables for dynamic text

### Changed

#### Skills

- `git_reset` denyPattern expanded from `"^release/.+$"` to `"^(main|release/.+)$"` — resets on `main` are now blocked in addition to `release/*`
- Delete-confirmation hintConditions added to `files_delete`, `files_delete_dir`, `notes_delete`, and `notes_delete_dir` — every deletion now produces a verification hint
- `git_branch_delete` hintCondition added for "not fully merged" error — suggests `force: true` when a branch was squash-merged
- `files_read` and `notes_read` now include line numbers in `{line_number}\t{content}` format — previously, these tools used `cat` which produced raw output without line numbers, requiring an extra `files_read_lines` or `files_search` call before editing
- `files_search`, `notes_search`, `files_replace`, and `notes_replace` now always show line numbers — the `line_number` parameter has been removed from all four tools (it defaulted to `true` and was never needed as `false`)
- Sandbox README files removed from bundled profile templates — `chai init` no longer creates `sandbox/README.md`

### Fixed

#### Security and Sandboxing

- Git, git-read, and git-remote skills now validate that git's resolved repository root (`.git` directory) is inside the sandbox before allowing commands to run — previously, when the `repo` parameter pointed to a sandbox subdirectory without its own `.git`, git traversed upward and could read or modify repository state outside the sandbox
- Cargo skill now validates that the resolved workspace manifest (`Cargo.toml`) is inside the sandbox before allowing commands to run — previously, when the `path` parameter pointed to a sandbox subdirectory without its own `Cargo.toml`, cargo traversed upward and could compile or test a workspace outside the sandbox
- `git_clone` now validates that absolute clone target paths are inside the sandbox — previously, absolute paths passed through unchanged, allowing clones outside the sandbox boundary
- `notes_daily` `scope` parameter now rejects values containing `..` path traversal — previously, the scope was used to construct a path without validation, allowing access to directories outside the sandbox
- Resolve-command errors are now propagated (tool call rejected) instead of silently falling back to the unresolved parameter value — previously, resolve-command validation failures were silently swallowed, allowing tool calls to proceed with unvalidated paths

#### Skills

- Truncation notices now frame continuation as optional (e.g., "To continue reading, use X; omit end_line to read the rest") instead of imperative ("Use X to read the remaining lines")
- `notes_daily_append` hint now correctly acknowledges that the file was created by the append operation, instead of implying the operation failed
- `cargo_check` and `cargo_test` now show compiler warnings — previously, stderr was discarded on exit code 0, so warnings were invisible to the agent
- `cargo_check` compilation errors and `cargo_test` test failures now produce filtered output — previously, exit code 101 bypassed the postProcess script, returning hundreds of lines of unfiltered output that consumed context window without providing actionable information; now only diagnostics and summaries are shown
- `files_write_lines` `original_content` mismatch errors now include file line numbers (e.g., `file line N`) alongside content-relative line numbers, and the length-mismatch hint clarifies `original_content` vs file range
- `log::warn!` diagnostic messages no longer appear in `files_write_lines` and `files_replace` tool output — previously, fuzzy-match warnings leaked into agent tool results via stderr, creating confusing `WARN` lines in successful operations

## [0.2.0] - 2026-06-24

### Added

#### Skills

- `cargo` skill with `cargo_check` and `cargo_test` tools: verify that code changes compile and pass tests during a session, with optional `package` parameter to scope to a specific workspace member. Uses OR-group bins (`cargo` or `nix`) so the skill works on both standard Rust installs and NixOS environments. PostProcess hint scripts produce concise summaries (clean check confirmation, test result lines). `cargo build` is intentionally excluded — building the binary is the user's responsibility
- `git` skill tools for branch integration: `git_merge` (squash merge by default), `git_rebase`, `git_rebase_continue`, `git_rebase_abort`, `git_cherry_pick`, `git_cherry_pick_continue`, `git_cherry_pick_abort`, and `git_reset` (mixed reset, default `HEAD~1`)
- `git_diff_lines` and `git_show_lines` line-range companion tools: read a range of lines from truncated `git diff` or `git show` output with `{line_number}\t{content}` format. When `end_line` is omitted, reads from `start_line` to the end of the output. CLI subcommands: `chai git diff-lines` and `chai git show-lines`
- `git_log` `skip` parameter: skip N commits before starting output, enabling native pagination through commit history
- `git_branch_delete` `force` parameter: when `true`, runs `git branch -D` instead of `git branch -d`, enabling deletion of branches that were squash-merged (where Git does not consider the branch fully merged). Protected branches (`main`, `release/*`) are always blocked by the deny pattern regardless of the `force` setting

#### Skill Authoring

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
- Worker tool call events from successive delegations are no longer silently dropped — previously, overlapping tool call indices between delegations triggered the desktop's duplicate detection, causing worker tool call results to disappear from the chat view

#### Skills

- CLI flags with leading dashes (e.g. `-p`) are no longer mangled into invalid forms (e.g. `---p`) when passed to underlying commands
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
- CLI subcommand rename for consistency: `chai file rename-note` → `chai file rename`
- CLI flag renames for consistency with ADR parameter naming: `git diff-lines --path` → `--repo` (repo root), `git diff-lines --file-path` → `--path` (file within repo), `git show-lines --path` → `--repo` (repo root), `file replace --line-numbers` → `--line-number`, `file rename --root` → `--scope`

### Breaking Changes

- CLI subcommand `chai file rename-note` renamed to `chai file rename` — scripts using `rename-note` will fail
- CLI flag renames — existing scripts using old flag names will fail:
  - `git diff-lines`: `--path` (repo root) → `--repo`, `--file-path` → `--path`
  - `git show-lines`: `--path` (repo root) → `--repo`
  - `file replace`: `--line-numbers` → `--line-number`
  - `file rename`: `--root` → `--scope`

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
