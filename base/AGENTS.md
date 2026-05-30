# Agents

This is the `AGENTS.md` file in the root of the `base` directory.

## Directory

The `base` directory is the root of the chai knowledge base and also contains ad-hoc working notes for bugs and improvements being tracked. **Always read [README.md](README.md)** — the entry point for this directory's structured documentation.

## Conventions

- **Issue tracking**: Bugs and feature requests are tracked in files prefixed with `BUG_` and `FEAT_` respectively (e.g. `BUG_GREP_EXIT_1.md`, `FEAT_DOCUMENT_SIDE_READ.md`). Summaries are maintained in this file under "Active Work"; full details are in the individual files.

## Working Notes

The `BUG_*`/`FEAT_*` files in the root of the `base` directory are a **lighter-weight tracking layer**. They're for small bugs and improvements being worked on through the agent before they're ready for the formal structured documentation. The relationship is:

- **Working notes** (`BUG_*`/`FEAT_*`) = active tracking, ad-hoc. For agent-driven discovery and quick iteration.
- **Structured docs** (`adr/`, `epic/`, etc.) = canonical, versioned, shared. Formal frontmatter and structure. For design decisions and project-wide reference.

When a working note matures (e.g. a bug fix is verified, or a feature grows into a design decision), its substance should **graduate into structured documentation**:
- A fix that changes architecture → new ADR (e.g., `successExitCodes` in `tools.json` → `adr/`)
- A feature that grows in scope → new epic (e.g., tool approval workflow → `epic/`)
- A spec that needs updating → update existing spec (e.g., `spec/TOOLS_SCHEMA.md`)
- Reference material → `ref/`

**For now**, keep working notes and structured documentation separate. Small fixes and improvements stay in `BUG_*`/`FEAT_*` files until they're resolved or mature enough to promote.

## Active Work

### Improvement: Better Skill Instructions for `devtools_search_content`

**Status**: Open

Clarify no-match behavior, regex flavor, and recursive defaults. See [FEAT_SEARCH_CONTENT_INSTRUCTIONS.md](FEAT_SEARCH_CONTENT_INSTRUCTIONS.md).

### Improvement: Document Side-Read Behavior in Skill Instructions

**Status**: Open

The `sideRead` behavior that appends `AGENTS.md` to directory listings is undocumented. See [FEAT_DOCUMENT_SIDE_READ.md](FEAT_DOCUMENT_SIDE_READ.md).

### Improvement: Clarify Purpose of `devtools-read` Skill Variant

**Status**: Open

Two devtools skill directories exist (`devtools/` and `devtools-read/`) with unclear relationship. See [FEAT_CLARIFY_DEVTOOLS_READ_SKILL.md](FEAT_CLARIFY_DEVTOOLS_READ_SKILL.md).

### Feature: Audit and Break Up Large Files

**Status**: Open

Several source files exceed practical size limits for the write tool and mix concerns that could be separated. `generic.rs` was partially refactored; further splits proposed. See [FEAT_AUDIT_LARGE_FILES.md](FEAT_AUDIT_LARGE_FILES.md).

## Resolved

- [FEAT_LINE_LEVEL_WRITES.md](FEAT_LINE_LEVEL_WRITES.md) — **Verified**. Added `devtools_read_lines` (read line ranges with line numbers) and `devtools_write_lines` (replace/delete line ranges without rewriting entire files). Implemented via `chai file read-lines` and `chai file patch` CLI subcommands with `patch_string()` core logic. All operations tested: single-line read, range read, single-line replace, range expansion/contraction, and deletion via empty content.
- [BUG_GREP_NO_EXTENDED_REGEX.md](BUG_GREP_NO_EXTENDED_REGEX.md) — **Verified**. `grep` was invoked without `-E`, so `|`, `+`, `?` were treated as literals; added `-E` as grep subcommand in both `devtools` and `devtools-read` tools.json, updated tool description to say "extended regex supported", and added missing `successExitCodes: [0, 1]` to `devtools-read`. All ERE features (`|`, `+`, `()`, `[]`, no-match exit 1) tested and working.
- [BUG_WRITE_SILENT_FAIL.md](BUG_WRITE_SILENT_FAIL.md) — **Verified**. Both fixes implemented: (1) `extract_stdin_content` now validates required stdin params and returns an error instead of silently falling through; (2) all `child.stdin.take()` sites now use `ok_or_else` with explicit block-scope drop (guaranteeing EOF) instead of `if let Some` (which silently skipped). `run_post_process` extracted to `post_process.rs` with a `pipe_stdin` helper; `generic.rs` refactored to directory module `generic/mod.rs`.
- [BUG_KB_DAILY_NO_DATE.md](BUG_KB_DAILY_NO_DATE.md) — `kb_daily_write`/`kb_daily_append` failed when date omitted (optional flag params skipped resolveCommand)
- [BUG_KB_PATH_DOUBLING.md](BUG_KB_PATH_DOUBLING.md) — resolve scripts doubled sandbox root in file paths (scripts not idempotent)
- [BUG_SKILLGEN_MULTILINE_FLAG.md](BUG_SKILLGEN_MULTILINE_FLAG.md) — skillgen write tools failed with multiline content (`kind: "flag"` broke on newlines)
- [BUG_WRITE_TOOL_ESCAPES.md](BUG_WRITE_TOOL_TOOL_ESCAPES.md) — `normalizeNewlines` double-decode corrupted `\n`/`\t` escape sequences in written content
- [BUG_LOADING_AGENTS.md](BUG_LOADING_AGENTS.md) — side-read loaded AGENTS.md from wrong directory (used raw args instead of canonical paths)
- [BUG_GREP_EXIT_1.md](BUG_GREP_EXIT_1.md) — grep exit code 1 on no matches surfaced as tool error

## Chai Architecture Notes

> **Note**: These are chai-internal gotchas learned from bug fixes. As they stabilize, they should graduate into formal specs or ADRs under the appropriate subdirectory (`adr/`, `spec/`, etc.).

- **`subcommand` field passes fixed flags before args**: The `subcommand` in `ExecutionSpec` is split by whitespace and prepended before the `args` list. Setting `subcommand: "-E"` for `grep` produces `grep -E [flags] pattern path`. The allowlist must include the subcommand value (e.g. `"grep": ["", "-E"]`).
- **Resolve scripts must be idempotent**: `resolveCommand` scripts are invoked twice for `writePath`/`readPath` params — first in `validate_write_paths()` (result canonicalized and substituted into args), then again in `build_argv()` on the already-resolved value. Scripts that prepend a root path must check whether the input is already absolute and return it unchanged. The pattern is: `case "$path" in /*) echo "$path"; exit 0 ;; esac`.
- **Optional params with `resolveCommand` invoke the resolver when omitted**: When an optional param (`optional: true`) has a `resolveCommand` and the caller omits it, the generic executor runs the resolver with an empty string. If the resolver produces a non-empty value, the flag/positional is added to argv; if it returns empty, the param is skipped. This applies to both `kind: "flag"` and `kind: "positional"` args.
- `successExitCodes` on `ExecutionSpec` allows per-tool configuration of exit codes treated as success (e.g. `[0, 1]` for grep where exit 1 = no matches, not an error).
- ~~`normalizeNewlines`~~ on `ArgMapping` is **deprecated** — it caused a double-decode bug. Do not use on any new or existing skill.
- `kind: "stdin"` on `ArgMapping` is required for any parameter that contains multiline content — `kind: "flag"` causes `clap` to break on newlines.
- **Required stdin params must be validated**: `extract_stdin_content` returns `Result<Option<String>, String>` and errors on missing/null required `kind: "stdin"` params. Previously it returned `Option<String>` and silently returned `None` on missing content, causing the executor to fall through to the no-stdin code path — which ran the child binary without piped content and produced silent failures (empty/missing files with exit 0).
- **Stdin pipe must be explicitly scoped**: All `child.stdin.take()` sites must use `ok_or_else` (not `if let Some`) and wrap the pipe write in a block scope that drops the pipe before calling `wait_with_output()`. This guarantees (1) the child sees EOF on stdin before we wait, and (2) pipe unavailability surfaces as an error rather than being silently skipped. The `if let Some` pattern masked a potential failure mode.
- `substitute_canonical_paths` replaces raw param values with canonical absolute paths for `build_argv`. After the fix, `apply_side_read` also uses these canonical paths.
- The `sideRead` feature appends a file (like `AGENTS.md`) to tool results using the `pathParam` value from args as `<path>/<filename>`.
- `oncePerSession` deduplication uses the path string as a key — switching to canonical paths improves dedup correctness.
