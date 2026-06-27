# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

#### CLI

- `chai resolve` subcommand — sandbox-aware path resolution for tool `resolveCommand` entries, replacing shell scripts with type-safe, testable Rust validation. Five variants: `repo-path` (validates git repository root), `cargo-path` (validates cargo workspace manifest), `clone-path` (validates clone target paths), `file-path` (validates generic file paths), and `sandbox` (generic sandbox boundary check). Handles symlinked directories and non-existent paths via ancestor-walk canonicalization. `repo-path` and `cargo-path` unconditionally validate the working directory is inside the sandbox regardless of whether project discovery succeeds, preventing path-traversal attacks when the discovery command fails. Rejection messages display the user-provided path
- `chai sessions list` — list sessions for the active profile (or a specified profile via `--profile`) directly from disk; displays session id, timestamps, message count, and channel binding; sorted by most recently updated; no gateway connection required
- `chai sessions delete <ID>` — delete a session by id directly from disk; removes the session and its binding; no gateway connection required
- `chai sessions clear` — delete all sessions directly from disk; reports the count of deleted sessions; no gateway connection required

#### Desktop

- Session sidebar loads persisted sessions on gateway connect via `sessions.list` — sidebar is populated with timestamps (e.g. "Jun 10, 12:34") and short session IDs instead of raw UUIDs
- Session history loads on demand when selecting a persisted session via `sessions.history` — chat area shows "Loading session history…" while the fetch is in flight
- Per-session "×" delete button in the session sidebar — calls `sessions.delete` on the gateway; right-aligned via RTL layout so labels cannot push the button off screen
- "Clear all sessions" button with confirmation dialog at the bottom of the session sidebar — calls `sessions.delete_all`; confirmation dialog stacks the warning label above the buttons to fit the narrow 220px panel
- "New session" button is always visible in the sessions panel, regardless of whether a session is active
- Channel-bound sessions display a channel tag (e.g. `(telegram)`) in the sidebar and are read-only from the desktop — clicking a channel session loads its history for viewing but disables the chat input, preventing accidental session overwrites

#### Runtime and Configuration

- `sessions.list` WebSocket method: returns summary metadata (id, timestamps, message count, channel binding) for all sessions, sorted by most recently updated
- `sessions.history` WebSocket method: returns full message history for a given session id, with optional `limit` and `offset` pagination
- `sessions.delete` WebSocket method: deletes a session from memory and disk, removes associated bindings, broadcasts a `session.deleted` event
- `sessions.delete_all` WebSocket method: deletes all sessions for the active profile from memory and disk, broadcasts a `sessions.cleared` event

### Changed

#### Skills

- Git, git-read, git-remote, and cargo skills now use `chai resolve` subcommands (`repo-path`, `cargo-path`, `clone-path`) via `resolveCommand.binary`/`resolveCommand.subcommand` instead of shell scripts for sandbox boundary validation — the same `is_inside_sandbox` logic is now type-safe, testable Rust code instead of copy-pasted shell
- 25 hint scripts across 8 skills migrated from `postProcess` to inline `hintConditions` in `tools.json`: files (`hint-not-found`, `hint-overwrite`, `hint-search-results`), files-read (`hint-not-found`, `hint-search-results`), git (`hint-not-repo`, `hint-commit-status`, `hint-merge`, `hint-rebase`, `hint-cherry-pick`, `hint-diff-ref-main`, `hint-reset`), git-read (`hint-not-repo`, `hint-diff-ref-main`), git-remote (`hint-pull-errors`, `hint-push-errors`), notes (`hint-not-found`, `hint-overwrite`, `hint-search-results`), notes-read (`hint-not-found`, `hint-search-results`), notes-daily (`hint-not-found`, `hint-daily-overwrite`), skills (`hint-init-next-steps`, `hint-validate-errors`), skills-read (`hint-validate-errors`). Simple exit-code and substring-match hints are now declarative; `postProcess` is reserved for complex hints requiring output transformation or external commands
- Diagnostic hints ADR updated: three-tier hint architecture (1. `hintConditions` for simple conditions, 2. `postProcess` scripts for complex logic, 3. binary-level for internal state)
- `TOOLS_SCHEMA.md` spec updated with `hintConditions` field documentation

### Removed

#### Skills

- Sandbox resolve shell scripts removed (replaced by `chai resolve` subcommand): `resolve-repo-path.sh` (git, git-read, git-remote), `resolve-clone-path.sh` (git-remote), `resolve-cargo-path.sh` (cargo)
- Hint scripts removed (replaced by inline `hintConditions`): `hint-not-found.sh` (files, files-read, notes, notes-read, notes-daily), `hint-overwrite.sh` (files, notes), `hint-search-results.sh` (files, files-read, notes, notes-read), `hint-not-repo.sh` (git, git-read), `hint-commit-status.sh` (git), `hint-merge.sh` (git), `hint-rebase.sh` (git), `hint-cherry-pick.sh` (git), `hint-diff-ref-main.sh` (git, git-read), `hint-reset.sh` (git), `hint-pull-errors.sh` (git-remote), `hint-push-errors.sh` (git-remote), `hint-daily-overwrite.sh` (notes-daily), `hint-init-next-steps.sh` (skills), `hint-validate-errors.sh` (skills, skills-read)
- Empty `git-read/scripts/` directory removed

### Fixed

#### Security and Sandboxing

- Git, git-read, and git-remote skills now validate that git's resolved repository root (`.git` directory) is inside the sandbox before allowing commands to run — previously, when the `repo` parameter pointed to a sandbox subdirectory without its own `.git`, git traversed upward and could read or modify repository state outside the sandbox
- Cargo skill now validates that the resolved workspace manifest (`Cargo.toml`) is inside the sandbox before allowing commands to run — previously, when the `path` parameter pointed to a sandbox subdirectory without its own `Cargo.toml`, cargo traversed upward and could compile or test a workspace outside the sandbox
- `git_clone` now validates that absolute clone target paths are inside the sandbox — previously, absolute paths passed through unchanged, allowing clones outside the sandbox boundary
- `notes_daily` `scope` parameter now rejects values containing `..` path traversal — previously, the scope was used to construct a path without validation, allowing access to directories outside the sandbox
- Resolve-command errors are now propagated (tool call rejected) instead of silently falling back to the unresolved parameter value — previously, resolve-command validation failures were silently swallowed, allowing tool calls to proceed with unvalidated paths

#### Skills

- Diagnostic hints now follow consistent formatting conventions: `verify_original` hints start at column 0 (no leading indentation), multiple hints are separated by blank lines, and `hint-reset.sh` always produces a blank line before the hint even when git output is empty
- Hint script pass-through calls now use `printf '%s\n'` instead of `printf '%s'` to restore the trailing newline stripped by command substitution, ensuring blank-line separators between tool output and hints render correctly
- Truncation notices now frame continuation as optional (e.g., "To continue reading, use X; omit end_line to read the rest") instead of imperative ("Use X to read the remaining lines")
- Git hint scripts use `printf '%s'` instead of `echo` for output pass-through, preventing POSIX `echo` from interpreting escape sequences
- `notes_daily_append` hint now correctly acknowledges that the file was created by the append operation, instead of implying the operation failed
- `cargo_check` and `cargo_test` now show compiler warnings — previously, stderr was discarded on exit code 0, so warnings emitted to stderr (e.g., unused variable) were invisible to the agent and the tools reported "no warnings" even when warnings existed
- `cargo_check` compilation errors and `cargo_test` test failures now produce filtered output — previously, exit code 101 bypassed the postProcess script, returning hundreds of lines of unfiltered output (progress lines, passing test lines) that consumed context window without providing actionable information; now only diagnostics (errors, warnings with multi-line context) and summaries (test result lines, crate-level summaries) are shown

#### Skill Authoring

- `hintConditions` field on execution specs: declarative inline hint conditions that the executor evaluates after `postProcess` and before truncation. Four condition types: `match` (substring in output), `exitCode` (integer or `"nonzero"`), `notEmpty` (non-empty output), `whenArg` (parameter-value match). Multiple conditions on the same entry use AND logic. Multiple entries all produce hints when matched. The `hint` field supports `{param_name}` template variables for dynamic text. Replaces simple `postProcess` hint scripts (those that only inspect output and append a static hint) with one-liner declarations in `tools.json`, reserving `postProcess` for hints that require output transformation, external commands, or multi-step logic
- Skills-design SKILL.md now documents upward traversal by external commands (git, cargo) and the requirement for resolve scripts to validate the resolved project root is inside the sandbox, including symlinked directories
- Skills-design SKILL.md now documents the pre/post-resolution validation gap for parameters referenced via `$name` in `resolveCommand.args` (not in the execution `args` array and not validated by the sandbox)
- Skills-design SKILL.md security audit checklist now includes two new checks: (5) resolve scripts constructing paths from parameters not in the `args` array must reject dangerous values, and (6) tools using `workingDir` with upward-traversing commands must validate the project root

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
