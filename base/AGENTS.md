# Agents

This is the `AGENTS.md` file in the root of the `base` directory.

The `base/` directory is the root of the chai knowledge base and also contains ad-hoc working notes for bugs and improvements being tracked. **Always read [README.md](README.md)** — the entry point for this directory's structured documentation.

## Working Notes

The `BUG_*`/`FEAT_*` files in the root of the `base/` directory are a **lighter-weight tracking layer**. They're for small bugs and improvements being worked on through the agent before they're ready for the formal structured documentation. The relationship is:

- **Working notes** (`BUG_*`/`FEAT_*`) = active tracking, ad-hoc. For agent-driven discovery and quick iteration.
- **Structured docs** (`adr/`, `epic/`, etc.) = canonical, versioned, shared. Formal frontmatter and structure. For design decisions and project-wide reference.

When a working note matures (e.g. a bug fix is verified, or a feature grows into a design decision), its substance should **graduate into structured documentation**:
- A fix that changes architecture → new ADR (e.g., `successExitCodes` in `tools.json` → `adr/`)
- A feature that grows in scope → new epic (e.g., tool approval workflow → `epic/`)
- A spec that needs updating → update existing spec (e.g., `spec/TOOLS_SCHEMA.md`)
- Reference material → `ref/`

**For now**, keep working notes and structured documentation separate. Small fixes and improvements stay in `BUG_*`/`FEAT_*` files until they're resolved or mature enough to promote.

## Conventions

- **Issue tracking**: Bugs and feature requests are tracked in files prefixed with `BUG_` and `FEAT_` respectively (e.g. `BUG_GREP_EXIT_1.md`, `FEAT_DOCUMENT_SIDE_READ.md`). Summaries are maintained in this file under "Active Work"; full details are in the individual files.

## Active Work

### Bug: devtools_write_file Silently Fails on Some Content (Retry Succeeds)

**Status**: Open

`devtools_write_file` occasionally fails silently — no error is returned but the file is not created. Retrying with identical content then succeeds. Observed during verification of the `normalizeNewlines` fix. Failures seem to correlate with content containing `\\` followed by certain characters, but the pattern is not fully consistent — same content can fail on one attempt and succeed on another, suggesting a transient or race condition. Possible causes include: `chai file write` exiting 0 on failure, stdin pipe write interruption, or filesystem timing. See [BUG_WRITE_SILENT_FAIL.md](BUG_WRITE_SILENT_FAIL.md) for detailed evidence table and investigation steps.

### Bug: Pipe `|` in Search Pattern Doesn't Work as Alternation

**Status**: Open

`grep` is invoked without `-E`, so `|` is treated as a literal character rather than alternation. The tool description says "basic regex supported" which is misleading. See [BUG_GREP_NO_EXTENDED_REGEX.md](BUG_GREP_NO_EXTENDED_REGEX.md).

### Improvement: Better Skill Instructions for `devtools_search_content`

**Status**: Open

Clarify no-match behavior, regex flavor, and recursive defaults. See [FEAT_SEARCH_CONTENT_INSTRUCTIONS.md](FEAT_SEARCH_CONTENT_INSTRUCTIONS.md).

### Improvement: Document Side-Read Behavior in Skill Instructions

**Status**: Open

The `sideRead` behavior that appends `AGENTS.md` to directory listings is undocumented. See [FEAT_DOCUMENT_SIDE_READ.md](FEAT_DOCUMENT_SIDE_READ.md).

### Improvement: Clarify Purpose of `devtools-read` Skill Variant

**Status**: Open

Two devtools skill directories exist (`devtools/` and `devtools-read/`) with unclear relationship. See [FEAT_CLARIFY_DEVTOOLS_READ_SKILL.md](FEAT_CLARIFY_DEVTOOLS_READ_SKILL.md).

## Resolved

- [BUG_KB_DAILY_NO_DATE.md](BUG_KB_DAILY_NO_DATE.md) — `kb_daily_write`/`kb_daily_append` failed when date omitted (optional flag params skipped resolveCommand)
- [BUG_KB_PATH_DOUBLING.md](BUG_KB_PATH_DOUBLING.md) — resolve scripts doubled sandbox root in file paths (scripts not idempotent)
- [BUG_SKILLGEN_MULTILINE_FLAG.md](BUG_SKILLGEN_MULTILINE_FLAG.md) — skillgen write tools failed with multiline content (`kind: "flag"` broke on newlines)
- [BUG_WRITE_TOOL_ESCAPES.md](BUG_WRITE_TOOL_ESCAPES.md) — `normalizeNewlines` double-decode corrupted `\n`/`\t` escape sequences in written content
- [BUG_LOADING_AGENTS.md](BUG_LOADING_AGENTS.md) — side-read loaded AGENTS.md from wrong directory (used raw args instead of canonical paths)
- [BUG_GREP_EXIT_1.md](BUG_GREP_EXIT_1.md) — grep exit code 1 on no matches surfaced as tool error

## Chai Architecture Notes

> **Note**: These are chai-internal gotchas learned from bug fixes. As they stabilize, they should graduate into formal specs or ADRs under the appropriate subdirectory (`adr/`, `spec/`, etc.).

- **Resolve scripts must be idempotent**: `resolveCommand` scripts are invoked twice for `writePath`/`readPath` params — first in `validate_write_paths()` (result canonicalized and substituted into args), then again in `build_argv()` on the already-resolved value. Scripts that prepend a root path must check whether the input is already absolute and return it unchanged. The pattern is: `case "$path" in /*) echo "$path"; exit 0 ;; esac`.
- **Optional params with `resolveCommand` invoke the resolver when omitted**: When an optional param (`optional: true`) has a `resolveCommand` and the caller omits it, the generic executor runs the resolver with an empty string. If the resolver produces a non-empty value, the flag/positional is added to argv; if it returns empty, the param is skipped. This applies to both `kind: "flag"` and `kind: "positional"` args.
- `successExitCodes` on `ExecutionSpec` allows per-tool configuration of exit codes treated as success (e.g. `[0, 1]` for grep where exit 1 = no matches, not an error).
- ~~`normalizeNewlines`~~ on `ArgMapping` is **deprecated** — it caused a double-decode bug. Do not use on any new or existing skill.
- `kind: "stdin"` on `ArgMapping` is required for any parameter that contains multiline content — `kind: "flag"` causes `clap` to break on newlines.
- `substitute_canonical_paths` replaces raw param values with canonical absolute paths for `build_argv`. After the fix, `apply_side_read` also uses these canonical paths.
- The `sideRead` feature appends a file (like `AGENTS.md`) to tool results using the `pathParam` value from args as `<path>/<filename>`.
- `oncePerSession` deduplication uses the path string as a key — switching to canonical paths improves dedup correctness.
