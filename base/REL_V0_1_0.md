# REL: v0.1.0 Release Requirements

Track requirements and open questions for the first official release of Chai (v0.1.0). This is a working document; items will be checked off as they are completed. For the release process itself (how releases are tagged, where notes live, changelog conventions), see [REL_PROCESS.md](REL_PROCESS.md).

## v0.1.0 Scope

v0.1.0 is the first tagged release. It establishes the baseline: a working multi-agent management system with messaging channels, skill-based tooling, and profile-based configuration. The bar is "usable and well-documented for early adopters," not "feature-complete."

Per project conventions (see sandbox `AGENTS.md`), backwards compatibility is **not** a concern before v0.1.0. This release is a clean slate — no migration shims, no deprecated fields, no compat layers.

### Epics in Scope

| Epic | Status | Notes |
|------|--------|-------|
| [MSG_CHANNELS.md](epic/MSG_CHANNELS.md) | In-progress (mostly shipped) | Telegram shipped. Matrix shipped (optional feature). Signal shipped (BYO signal-cli). Hardening tasks remain. |

### Epics Explicitly Out of Scope

| Epic | Reason |
|------|--------|
| [DESKTOP_FILES.md](epic/DESKTOP_FILES.md) | In progress but not a release blocker |
| [PARALLEL_WORKFLOWS.md](epic/PARALLEL_WORKFLOWS.md) | Draft; not scheduled |
| [PERSISTENT_SESSIONS.md](epic/PERSISTENT_SESSIONS.md) | Draft; not scheduled |
| [TOOL_APPROVAL.md](epic/TOOL_APPROVAL.md) | Draft; not scheduled |

## Requirements

### Skills Audit and Refinement

- [ ] Complete cross-skill audit ([AUDIT_SKILLS.md](AUDIT_SKILLS.md)) — all bundled skills reviewed
- [ ] Confirm `maxOutputLines` caps are applied to all output-heavy tools across all skills

### Sandbox Security 🔴

- [x] Add `readPath: true` to all path parameters that read files but currently lack the annotation — see [AUDIT_SKILLS.md](AUDIT_SKILLS.md) Issue 4 for the full list (`rss_list_feeds`, `kb_read`, `kb_daily_read`, `kb_wikilink`, `kb_search`)
- [x] Implement secure-by-default sandbox path validation (runtime heuristic + CWD confinement + sandbox validation) — see [SECURITY.md](SECURITY.md)
- [ ] Verify that sandbox-relative paths still work after adding `readPath` (resolve scripts prepend sandbox root for relative paths; canonical path then validates against sandbox)
- [ ] Verify that absolute paths within the sandbox (e.g. canonical paths from the executor) still work after adding `readPath`
- [ ] Test that arbitrary filesystem paths outside the sandbox are rejected for all affected tools
- [ ] Update `skills-design/SKILL.md` with sandbox security requirements for skill authors

### Branch Protection 🔴

- [x] Implement branch protection mechanism for `main` and `release/*` branches
- [x] Protect `git_commit` from committing on protected branches (resolve current branch from working directory, reject if protected) — `denyPattern` + `denyAlwaysResolve` + `denyResolveCommand` on `path` arg
- [x] Protect `git_push` from pushing to protected branches (including when `branch` parameter is omitted — resolve current branch) — `denyPattern` + `denyResolveCommand` on `branch` arg
- [x] Protect `git_branch_delete` from deleting protected branches — `denyPattern` on `name` arg
- [x] Test that commits on `main` and `release/*` are rejected
- [x] Test that pushes to `main` and `release/*` are rejected
- [x] Test that branch deletions on `main` and `release/*` are rejected
- [x] Test that commits and pushes on non-protected branches work normally
- [x] Update `skills-design/SKILL.md` with branch protection requirements for skill authors

### Messaging Channels

- [ ] Signal hardening — reconnect tuning, richer `receive` payloads (attachments, edits) ([MSG_CHANNELS.md](epic/MSG_CHANNELS.md))
- [ ] Matrix hardening — rate limits / backoff, sync reconnect tuning ([MSG_CHANNELS.md](epic/MSG_CHANNELS.md))
- [ ] Config and docs polish — channel-agnostic quickstart (not Telegram-first only) ([MSG_CHANNELS.md](epic/MSG_CHANNELS.md))
- [ ] Operational hardening — structured logging, secrets rotation notes ([MSG_CHANNELS.md](epic/MSG_CHANNELS.md))
- [ ] Test Signal integration via `crates/spike` (signal-probe) and gateway end-to-end
- [ ] Test Matrix integration via `crates/spike` (matrix-probe) and gateway end-to-end

### Code Quality and Structure

- [ ] Audit naming conventions — consistent naming across crates, modules, types, and config fields
- [ ] Audit file structure — no oversized files, logical module organization
- [ ] Remove any backwards-compatible code or compatibility notes (clean slate for v0.1.0)
- [ ] Verify all `log::info!` / `log::warn!` / `log::error!` messages follow lowercase-first convention (per `chai/AGENTS.md`)

### Desktop App

- [ ] Add stop button to desktop chat input area — see [FEAT_STOP_BUTTON.md](FEAT_STOP_BUTTON.md)
- [ ] Stop button pauses agent turn after current iteration completes, preserving session context
- [ ] Gateway `stop` WebSocket method signals the agent to break out of the tool loop
- [ ] Send button disabled during active turn, enabled when idle or paused
- [ ] Test that stopped turns produce valid session transcripts and can be continued

### Release Build and Distribution

- [ ] Create CI workflow for release builds with binary assets (Linux, macOS, Windows)
- [ ] Decide on release asset naming and structure (see [REL_PROCESS.md](REL_PROCESS.md) design questions)
- [ ] Decide whether to ship experimental feature binaries as release assets or require manual builds (see [REL_PROCESS.md](REL_PROCESS.md) design question 6)
- [ ] Verify `cargo install --path crates/cli` and `cargo install --path crates/desktop` work cleanly from the release tag
- [ ] Test `--features matrix` builds produce working binaries
- [ ] If Signal becomes an optional feature (`--features signal`), test that build path as well

### Documentation

- [ ] Review and update `chai/README.md` — must be accurate for v0.1.0 (concise, up-to-date, no decision notes per `chai/AGENTS.md`)
- [ ] Review and update `chai/docs/` guides for accuracy against shipped features
- [ ] Ensure channel-agnostic documentation (not Telegram-first only)
- [ ] Verify all cross-references in `base/` documents are accurate

### chai-examples Alignment

The `chai-examples` repository contains example profiles and skills that users reference alongside chai. Before v0.1.0, these examples must be reviewed and updated to align with the release. The examples repository should be tagged with the same version number as chai so users can identify which examples work with which release (see [REL_PROCESS.md](REL_PROCESS.md) design question 7).

- [ ] Review example profiles (`assistant`, `developer`, `skillsmith`) against v0.1.0 config schema and agent model
- [ ] Review example skills (`notesmd`, `notesmd-daily`, `obsidian`, `obsidian-daily`, `websearch`) against v0.1.0 skill format and tools schema
- [ ] Update `chai-examples/README.md` if installation instructions or skill tables have changed
- [ ] Verify bundled skill replacements table in `chai-examples/README.md` is still accurate
- [ ] Tag `chai-examples` with `v0.1.0` aligned to the chai release tag

### License and Legal

- [ ] Decide on license for v0.1.0 — current license is LGPL-3.0; the question of switching to GPL has been raised (see open questions below)
- [ ] If Signal is promoted from adapter to full integration, confirm licensing implications (see [SIGNAL_CLI_INTEGRATION.md](adr/SIGNAL_CLI_INTEGRATION.md))

## Open Questions

### License

The question of switching from LGPL-3.0 to GPL has been raised. This is a significant decision with implications for how others can use Chai as a library. The current LGPL-3.0 license allows Chai to be used as a library in non-GPL applications; GPL would not. This question is separate from the Signal integration question (Signal uses BYO signal-cli, which avoids GPL redistribution obligations — see [SIGNAL_CLI_INTEGRATION.md](adr/SIGNAL_CLI_INTEGRATION.md)).

**Decision needed:** Confirm LGPL-3.0 or switch to GPL before v0.1.0. If switching, update `LICENSE`, all `Cargo.toml` files, and `VISION.md`.

### Signal as Full Adapter

The question has been raised whether Signal should be treated as a full "adapter" with complete CLI support, similar to how Matrix is handled via `crates/adapters/matrix`. Currently Signal lives in `crates/lib` (not a separate adapter crate) and uses BYO signal-cli only. Promoting it to an adapter crate would:

- Mirror the Matrix architecture (separate crate, optional feature)
- Keep the default installation small (no signal-cli dependency)
- Potentially conflict with GPL concerns if signal-cli were ever bundled (which is explicitly out of scope per the ADR)

**Decision needed:** Keep Signal in `lib` or promote to `crates/adapters/signal` before v0.1.0.

### Matrix Adapter Default

Matrix is currently opt-in via `--features matrix`. Should it remain opt-in for v0.1.0, or should the default build include it? Including it by default makes the first-release experience simpler (one fewer flag to remember) but increases binary size and dependency count. The adapter crate approach (separate crate, optional feature) is already the right architecture.

**Decision needed:** Default feature flags for v0.1.0 release builds.

### v0.1.0 Content

What constitutes the minimum viable release? The requirements above represent a comprehensive list. Should any items be deferred to v0.1.1 or later? Specifically:

- Are Signal and Matrix hardening tasks blockers, or can they ship as "experimental" with known limitations?
- Is the channel-agnostic quickstart a blocker, or can the first release remain Telegram-first in docs?

**Decision needed:** Define the hard cutoff for v0.1.0 scope.

## ADR Needed

Before v0.1.0, an ADR should record the decision on the Matrix adapter package structure (why Matrix lives in a separate crate with an optional feature). This is partially captured in the existing architecture but not formally recorded as an ADR.

- [ ] Create ADR for Matrix adapter package design (`adr/MATRIX_ADAPTER.md` or similar)
