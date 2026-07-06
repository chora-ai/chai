# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

#### Desktop

- Per-profile gateway state — each profile has its own `GatewayState` (sessions, chat, status, process handle) stored in a `HashMap<String, GatewayState>` on `ChaiApp`, enabling multiple simultaneous gateways and seamless profile switching with preserved state
- `running_profiles` discovery — the desktop scans all per-profile lock files via `find_running_gateway_profiles()` to discover which profiles have running gateways (replaces the old single `gateway_lock_profile`)
- Pre-flight port conflict check in `start_gateway()` — before spawning the gateway child process, the desktop attempts a `TcpListener::bind` on the configured port and produces a clear error identifying which running profile holds the port
- Gateway error suppression of profile-mismatch hint — when a `gateway_error` is set, the amber profile-mismatch hint is suppressed to avoid stacking redundant messages
- Gateway error clearing on profile switch — `gateway_error` is specific to the profile that was active when the start was attempted and is cleared on profile switch
- Remote profile support — connect the desktop to a remote gateway via WebSocket instead of spawning a local gateway process. Remote entries in `desktop.json` define the profile id, WebSocket URL, and gateway auth token. Remote profiles appear in the profile ComboBox alongside local profiles and show Connect/Disconnect buttons instead of Start/Stop. Device identity is stored under the remote profile directory. Both `ws://` and `wss://` (TLS) URLs are supported with full path support for reverse proxy deployments.
- `remote` array in `desktop.json` — each entry has `id` (profile name), `url` (WebSocket URL with `ws://` or `wss://`), and `token` (gateway auth token). Invalid entries are rejected at load time with warnings; entries colliding with existing local profile directories are skipped (disk wins).
- Remote profile directories created automatically at startup and on config reload — ensures remote profiles appear in the ComboBox before the user has connected.
- Remote gateway TCP probe — probes the remote URL's host:port instead of the local `gateway.bind:port`.
- Remote profile disconnect-before-switch — switching away from a connected remote profile auto-disconnects first.
- Remote entries shown in Settings dashboard — the "Remote Profiles" section lists each entry's id and URL.

#### Skills

- `ref` parameter on `git_log` — view commit history for a specific branch, tag, or ref range (e.g., `main`, `HEAD~5..HEAD`); works in both `git` and `git-read` skill variants
- `continue` and `abort` boolean parameters on `git_rebase` — continue or abort an in-progress rebase after conflict resolution (replaces separate `git_rebase_continue` and `git_rebase_abort` tools)
- `continue` and `abort` boolean parameters on `git_cherry_pick` — continue or abort an in-progress cherry-pick after conflict resolution (replaces separate `git_cherry_pick_continue` and `git_cherry_pick_abort` tools)

### Changed

#### CLI

- `chai profile switch` always succeeds — no longer checks `gateway_is_running()` before switching (switching only updates the `~/.chai/active` symlink)
- `chai profile current` no longer displays `CHAI_PROFILE` (the environment variable has been removed)

#### Desktop

- Profile ComboBox is always enabled — profile switching is always allowed regardless of whether any gateway is running (previously disabled while a gateway was running)
- All WebSocket operations use the active profile directly — removed `cached_gateway_profile`, `gateway_profile()`, `refresh_cached_gateway_profile()`, and the entire override system (`env_profile`, `cached_profile_override`, `effective_profile_override()`, `gw_key()`)
- `start_gateway()` always passes `--profile` to the child process
- Session id resolution reads `"id"` instead of `"sessionId"` from `sessions.list` responses (matching the gateway's response format)

#### Runtime and Configuration

- Per-profile gateway locks — lock files moved from `~/.chai/gateway.lock` (shared across all profiles) to `~/.chai/profiles/<name>/gateway.lock` (one per profile), allowing multiple gateways to run simultaneously on different profiles
- Lock acquisition moved to the beginning of `run_gateway()` — same-profile conflicts now produce immediate errors instead of being buried after startup logs
- `CHAI_PROFILE` environment variable removed — profile resolution is now 2-tier: CLI `--profile` (per-command) → `~/.chai/active` (persistent default)
- `read_gateway_lock_profile()` removed as dead code — with per-profile locks, the profile is implicit from the lock file's path

#### Skills

- `files_read` + `files_read_lines` consolidated into `files_read` with optional `start_line` and `end_line` parameters — the agent uses a single tool for both full-file and line-range reads (applied to `files`, `files-read`, `notes`, `notes-read` skill variants)
- `files_write` + `files_write_lines` consolidated into `files_write` with two modes: whole-file write (with optional `overwrite` guard) and surgical edit (with `start_line` + `original_content`); routing between modes uses `paramCondition` (applied to `files` and `notes` skill variants)
- `overwrite` parameter on `files_write` and `notes_write` — whole-file writes to existing files are rejected without `overwrite: true`; new-file writes succeed regardless; hint conditions inform the agent when a new file was created with `overwrite: true` set
- `paramCondition` field on execution specs — parameter-based routing between multiple execution specs with the same tool name; supports `present` (parameter must be provided) and `absent` (parameter must be omitted) constraints with AND logic; partial-match hints when paired parameters are incomplete (e.g., `start_line` without `original_content`)
- Schema-enforced validation — the executor validates tool call parameters against the tool schema before execution; undeclared parameters and type mismatches are rejected; startup alignment check warns when schema parameters lack execution handlers
- `--overwrite` flag on `chai file write` — rejects writes to existing files without the flag; new-file writes succeed regardless
- `git_rebase` and `git_cherry_pick` consolidated — `continue`/`abort` operations are now boolean parameters on the parent tool instead of separate tool names; routing uses `paramCondition` (same pattern as `files_write` surgical edit vs whole-file write); both `continue` and `abort` provided simultaneously is rejected as ambiguous
- `git_reset` changed to unstage-only — `ref` parameter removed; new required `paths` parameter (with `split: true`) mirrors `git_add` for targeted unstaging (e.g., `paths: "."` to unstage all, `paths: "file.rs"` to unstage a specific file); the tool can no longer move the branch pointer or lose commits
- `git_add` `paths` description clarified — documents space-separated multi-file support and `"."` for staging all

### Fixed

#### Skills

- Truncation `{next_start}` derives from line-number prefix — when output lines use the `{number}\t{content}` format (e.g., `files_read` with `start_line`), `{next_start}` is now the last kept line number + 1 instead of `kept + 1`, fixing incorrect pagination hints that caused infinite re-read loops when reading from a line offset

### Breaking Changes

- `CHAI_PROFILE` environment variable removed — existing setups using `CHAI_PROFILE` must switch to `--profile` (per-command) or `~/.chai/active` (persistent default)
- `~/.chai/gateway.lock` moved to `~/.chai/profiles/<name>/gateway.lock` — scripts or tooling checking the old lock path must be updated
- Tool removals — existing tool calls using removed tool names will fail:
  - `files_read_lines` → use `files_read` with `start_line` and `end_line` parameters
  - `files_write_lines` → use `files_write` with `start_line` and `original_content` parameters (surgical edit mode)
  - `notes_read_lines` → use `notes_read` with `start_line` and `end_line` parameters
  - `notes_write_lines` → use `notes_write` with `start_line` and `original_content` parameters (surgical edit mode)
- Tool behavior changes — existing tool calls may produce different results:
  - `files_write` and `notes_write` now reject overwrites of existing files without `overwrite: true`; previously, whole-file writes silently overwrote existing files
  - `git_reset` no longer accepts a `ref` parameter and cannot move the branch pointer; existing calls using `ref` must switch to the `paths` parameter for unstaging (e.g., `git_reset({paths: "."})` to unstage all)
- Tool removals — existing tool calls using removed tool names will fail:
  - `git_rebase_continue` → use `git_rebase` with `continue: true`
  - `git_rebase_abort` → use `git_rebase` with `abort: true`
  - `git_cherry_pick_continue` → use `git_cherry_pick` with `continue: true`
  - `git_cherry_pick_abort` → use `git_cherry_pick` with `abort: true`

## [0.4.0] - 2026-06-30

### Added

#### CLI

- `--agent <id>` flag on `chai chat` — selects which orchestrator to use for the chat session; passes `orchestratorId` in the agent RPC
- `--agent <id>` flag on `chai session list` — scopes session listing to a specific orchestrator's session store
- `--agent <id>` flag on `chai session clear` — scopes session clearing to a specific orchestrator's session store; without `--agent`, clears the default orchestrator's sessions
- `chai skill write-allowlist-json` and `chai skill write-execution-json` — new CLI subcommands for writing the companion files independently
- `chai skill read --file allowlist_json` and `--file execution_json` — new file type values for reading companion files
- `chai skill dry-run` — preview what a tool call would execute without running the command; shows argv mapping, sandbox validation, deny pattern checks, stdin content, temp files, and post-processing pipeline; optional `--simulated-output` flag previews postProcess, hintConditions, and truncation on provided output

#### Desktop

- Orchestrator selector ComboBox in the chat sidebar — "Agent" section heading above "Sessions"; switching updates the session list and provider/model defaults; disabled when only one orchestrator is configured or during an active agent turn
- Config screen shows all orchestrators with per-orchestrator `enabledWorkers` display — `None` shows "(none)", empty array shows "(all)", non-empty shows comma-separated worker ids
- Gateway screen shows `enabledWorkers` per orchestrator with "(none)"/"(all)" display — the row is always visible (previously `None` was hidden)
- Skills screen correctly identifies all orchestrator agents (not just the default) for green/blue color coding
- Agent and Tools screens correctly label all orchestrator agents with `— orchestrator` suffix (not `— worker`) using a HashSet of orchestrator IDs
- "Clear all sessions" scopes deletion to the active orchestrator — passes `orchestratorId` to `sessions.delete_all`
- Session event filtering by orchestrator — desktop ignores `session.deleted` and `sessions.cleared` events from non-active orchestrators
- Provider/model ComboBoxes cascade on orchestrator switch — selecting a different orchestrator updates the Provider and Model ComboBoxes to reflect the new orchestrator's defaults

#### Skills

- `skills_write_allowlist_json` and `skills_write_execution_json` — new skill-authoring tools for writing companion files
- `skills_dry_run` — preview tool for skill authoring and auditing; available in both `skills` and `skills-read` skills

#### Runtime and Configuration

- Multiple orchestrator entries in the `agents` array — validation relaxes from "exactly one orchestrator" to "at least one"; each orchestrator has its own provider, model, skills, and delegation policy
- `enabledWorkers` field on orchestrator entries — optional array of worker ids this orchestrator can delegate to; absent or `null` means no workers enabled (`delegate_task` not offered); empty array means all profile workers; non-empty means only listed workers; unknown worker ids produce a validation error; rejected on worker entries at parse time
- Per-orchestrator session stores — each orchestrator gets its own `SessionStore` at `<profile_dir>/agents/<orchestrator_id>/sessions/`; held in `GatewayState.session_stores: HashMap<String, Arc<SessionStore>>`; sessions from one orchestrator are isolated from another
- `agent` RPC `orchestratorId` parameter — optional; when omitted, the default (first) orchestrator is used; when provided, the gateway resolves the matching `OrchestratorRuntime` and `SessionStore`; unknown orchestrator IDs return an error
- `sessions.list` RPC `orchestratorId` parameter — optional; when omitted, the default orchestrator's session store is queried; enables per-orchestrator session listing
- `sessions.delete_all` RPC `orchestratorId` parameter — optional; when provided, only clears sessions for that orchestrator's session store; when omitted, clears all (backward compatible); `sessions.cleared` event includes `orchestratorId`

### Changed

#### CLI

- CLI subcommand rename: `chai sessions` → `chai session` — singular naming convention for resource-type namespaces (consistent with `chai profile`, `chai skill`, `chai file`)
- CLI subcommand description updates — tool-backend commands (`file`, `git`, `logs`, `skill`) now use "Tool backend for ..." descriptions; resource-management commands describe the resource
- `chai skill init` — creates three-file format templates: `tools.json` as `[]`, `allowlist.json` as `{}`, `execution.json` as `[]`
- `chai skill list` — shows population status for `allowlist.json` and `execution.json` alongside `tools.json`
- `chai skill validate` — validates each file independently (JSON syntax, schema conformance) and checks cross-file consistency (tool names in `execution.json` must match `tools.json`; binary/subcommand pairs in `execution.json` must be in `allowlist.json`)

#### Skills

- All 15 bundled skills with tool descriptors migrated from the legacy single-file `tools.json` format to the three-file format (`tools.json`, `allowlist.json`, `execution.json`)
- `files_write_lines` and `notes_write_lines` `original_content` parameter renamed to `expected_content` — the new name communicates the verification-guard semantics ("the content I expect at this line range") instead of the ambiguous search-target model; CLI flags `--original-content` → `--expected-content`, `--original-content-file` → `--expected-content-file`
- `files_search` and `notes_search` `files_with_matches` parameter removed — it returned no line numbers, breaking the search → read-lines feedback loop; one less parameter, one less decision for the agent
- `files_replace` and `notes_replace` `max_replacements` parameter replaced with `dry_run` boolean — preview the replacement diff without modifying the file; the agent sees what would change and can adjust the pattern before applying; CLI flag `--dry-run`
- `files_replace` and `notes_replace` tool descriptions simplified — removed verbose regex instructions (`$$` for literal `$`, empty string for deletion, multiline newline instructions); these are standard regex knowledge or discoverable through dry_run

#### Skill Authoring

- Three-file tool descriptor format — the monolithic `tools.json` (root object with `tools`, `allowlist`, `execution` keys) is split into three independent files: `tools.json` (root array of tool definitions), `allowlist.json` (root object of security grants), and `execution.json` (root array of execution specs). Each file has a single responsibility: communication, security, and implementation respectively

### Fixed

#### Desktop

- "Clear all sessions" button and per-session "×" delete button for the active session are disabled while the agent is running — previously, these could be clicked during an active turn, deleting the session out from under the running agent

### Breaking Changes

- CLI subcommand renames — existing scripts using old subcommand will fail:
  - `chai sessions` → `chai session`
- CLI flag renames — existing scripts using old flag names will fail:
  - `file patch`: `--original-content` → `--expected-content`, `--original-content-file` → `--expected-content-file`
  - `file replace`: `--max-replacements` removed; use `--dry-run` to preview changes instead
- Tool parameter removals — existing tool calls using removed parameters will fail:
  - `files_search` and `notes_search`: `files_with_matches` parameter removed
  - `files_replace` and `notes_replace`: `max_replacements` parameter removed; use `dry_run` to preview changes instead
- Tool parameter renames — existing tool calls using old parameter names will fail:
  - `files_write_lines` and `notes_write_lines`: `original_content` → `expected_content`

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
