---
status: accepted
---

# Tool Consolidation

## Context

Bundled skills expose tools to the LLM via declarative schemas. Several tool surfaces had unnecessary fragmentation (separate tool names for operations that differ only by a single flag) and missing parameters that agents frequently needed. The `paramCondition` routing mechanism — which selects between multiple execution specs for the same tool name based on which parameters the agent provides — provides a clean way to consolidate separate tool names into a single tool with multiple modes.

### File Tool Mismatches

The `files_read` and `files_write` tool schemas (in `tools.json`) exposed fewer parameters than their execution specs (in `execution.json`) actually accepted. The runtime accepted parameters the LLM had never been told about, creating a schema/runtime mismatch.

| Tool | LLM Schema (tools.json) | Execution Spec (execution.json) | Mismatch |
|------|------------------------|-------------------------------|----------|
| `files_read` | `path` only | `path` + `start_line` (optional, `absentDefault: 1`) | `start_line` is absent from schema but wired in execution |
| `notes_read` | `path` only | `path` + `start_line` (optional, `absentDefault: 1`) | Same |

When the agent sent `files_read({path: "foo.rs", start_line: 50})`, the system accepted it because the provider layer passed the raw JSON object through with no schema validation and the argv builder iterated over `execution.json` args and found `start_line: 50` in the tool call JSON.

Additionally, the read and write tool surfaces were split across separate tool names (`files_read`/`files_read_lines` and `files_write`/`files_write_lines`) despite sharing the same conceptual domain. This created unnecessary surface area for the LLM to navigate.

The system also silently accepted parameters absent from the LLM-facing schema but present in the execution spec, with no validation at the execution boundary.

### Git Tool Pain Points

1. **`git_log` lacks a `ref` parameter.** Agents frequently want to view commit history for a specific branch or ref (e.g., `git log main`, `git log HEAD~5..HEAD`) but `git_log` only supports `count`, `skip`, `oneline`, `path`, and `repo`. The agent must fall back to `git_show` or shell out, neither of which is ideal.

2. **`git_reset` has a dangerous `ref` parameter with an unsafe default.** The `ref` parameter's `absentDefault` is `"HEAD~1"`, which means calling `git_reset` with no arguments silently resets the branch to `HEAD~1` — a destructive operation that can lose commits. Agents should revert changes by editing files and committing as a new commit (keeping an audit trail), and use `git_reset` only to correct staged files — not to restore a branch to a previous state.

3. **Four separate continue/abort tools are unnecessary surface area.** `git_rebase_continue`, `git_rebase_abort`, `git_cherry_pick_continue`, and `git_cherry_pick_abort` are each single-purpose tools that add exactly one literal flag to their parent command. They account for 4 of 20 tools in the git skill (20% of the tool surface) while being used infrequently.

4. **Three branch tools could potentially consolidate.** `git_branch` (list), `git_branch_create` (create + switch), and `git_branch_delete` (delete) are three separate tools for branch operations. Whether consolidation is appropriate depends on whether the routing is clean and the parameter space doesn't become confusing.

## Decision

### Decision 1: Consolidate File Tool Definitions

Merge `files_read` + `files_read_lines` → `files_read`. Merge `files_write` + `files_write_lines` → `files_write`, adding an `overwrite` parameter as an intentional-action guard for whole-file writes.

The consolidated `files_write` has two modes determined by parameter presence:

| Mode | Parameters | Behavior |
|------|-----------|----------|
| **Whole-file write** | `path`, `content` [, `overwrite`] | Create new file, or overwrite existing file only if `overwrite: true` |
| **Surgical edit** | `path`, `content`, `start_line`, `original_content` | Replace `original_content` at `start_line` with `content` (verification via `original_content` match) |

**Whole-file write behavior:**

| Condition | Result | Hint |
|-----------|--------|------|
| File does not exist, no `overwrite` | Succeed | None |
| File does not exist, `overwrite: true` | Succeed | New file was created (no overwrite necessary, verify this was intended) |
| File exists, no `overwrite` | Fail | `overwrite` must be set to `true` to overwrite existing files |
| File exists, `overwrite: true` | Succeed | None (overwrite was set as intended) |

**Surgical edit behavior:** `overwrite` is decoupled from surgical edits. The `original_content` verification mechanism already provides the safety guard for surgical edits — `overwrite` only applies to whole-file writes. If the agent passes `overwrite` with surgical edit parameters, it is silently ignored.

Mode routing between multiple execution specs with the same tool name is handled by `paramCondition` — a field on `ExecutionSpec` that declares which parameters must be present or absent for that spec to be selected (see [TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md)).

The same consolidation was applied to `notes_read`/`notes_read_lines` and `notes_write`/`notes_write_lines` across both full and read-only skill variants (`files`, `files-read`, `notes`, `notes-read`).

#### Evolution: Three-Tool Model

The consolidated two-mode `files_write` was later split into two separate tools, superseding the `paramCondition` routing for the writing/editing surface:

- `files_write` — whole-file create/overwrite only (`path`, `content`, `overwrite`)
- `files_edit` — surgical in-place edit (`path`, `old_content`, `new_content`, optional `start_line`)

The two-mode `files_write` was overloaded: the `content` parameter served double duty (whole-file content vs replacement content), forcing the surgical edit parameter to be named `original_content` to differentiate itself. Splitting into `files_edit` gives the surgical edit tool its own parameter namespace (`old_content` / `new_content`) with no naming tension, and eliminates the `paramCondition` routing on `files_write`.

`files_edit` also gained a search mode: when `start_line` is omitted, the binary searches for `old_content` using the same five-stage verification cascade and requires exactly one match. This frees the agent from knowing the exact line number when the content is unique.

The `paramCondition` mechanism itself is retained for git tools (`git_rebase`, `git_cherry_pick`), where `continue`/`abort` boolean parameters route to different CLI invocations.

### Decision 2: Schema-Enforced Validation

The tool schema is the contract. The executor validates tool call parameters against the schema before execution — undeclared parameters and type mismatches are rejected immediately with a clear error.

| Violation | Example | Behavior |
|-----------|---------|----------|
| Undeclared parameter | Agent sends `start_line` to a schema that only declares `path` | Reject — parameter not in schema |
| Type mismatch | Agent sends `start_line: "50"` when schema declares `start_line: integer` | Reject — type does not match schema |
| Valid call | Agent sends parameters that match the schema | Execute — schema is the contract |

A startup validation (`check_schema_execution_alignment`) warns when a tool's schema declares a parameter that has no corresponding execution handler, preventing the reverse drift case.

### Decision 3: Consolidate Rebase and Cherry-Pick Tools

Merge `git_rebase_continue` and `git_rebase_abort` into `git_rebase` using `continue` and `abort` boolean parameters with `paramCondition` routing. Merge `git_cherry_pick_continue` and `git_cherry_pick_abort` into `git_cherry_pick` the same way.

Each consolidated tool has three execution specs:

| Spec | `paramCondition` | Behavior |
|------|-----------------|----------|
| Continue | `{ "present": ["continue"] }` | `git rebase --continue` / `git cherry-pick --continue` |
| Abort | `{ "present": ["abort"] }` | `git rebase --abort` / `git cherry-pick --abort` |
| Default | none | Normal rebase onto `onto` / cherry-pick `commits` |

**Mutual exclusivity:** Setting both `continue: true` and `abort: true` is rejected by the executor — multiple `paramCondition` entries match, which is an ambiguous-call error. The "multiple match" error message includes the parameter names that caused each condition to match (e.g., `present: [continue]; present: [abort]`), so the agent can correct the call.

**`binaryWrapper` for non-interactive rebase continue:** `git rebase --continue` opens an editor by default to amend the commit message. In the non-interactive chai subprocess context, this causes `nano` (or any interactive editor) to fail. The continue spec uses `binaryWrapper: ["env", "GIT_EDITOR=true"]` to set `GIT_EDITOR` to the no-op `true` command for that invocation only, preventing the editor from opening and allowing git to proceed with the original commit message. This is a proven pattern — the cargo skill already uses `binaryWrapper: ["nix", "develop", "--command"]` for NixOS environments.

**Deferred consolidations:**

- **Branch tools:** The current 3-tool split (`git_branch`, `git_branch_create`, `git_branch_delete`) is clear and well-named. A 4-mode consolidated tool would have the most complex `paramCondition` routing in the codebase, and the parameter semantics (some parameters only apply to some modes) could confuse the LLM more than the current separate tools. This can be revisited after the simpler consolidations are proven.
- **`git_diff_lines` / `git_show_lines`:** These tools use the `chai` binary (not `git` directly). Consolidating them into `git_diff` and `git_show` would require routing between two different binaries, which the current `paramCondition` mechanism doesn't support. Revisit after the current consolidations are complete.

### Decision 4: `git_log` `ref` Parameter

Add a `ref` parameter to `git_log` for specifying a commit, branch, or ref range (e.g., `main`, `HEAD~5..HEAD`). The parameter is optional — omitting it preserves the current behavior of showing the current branch's history.

When both `ref` and `path` are provided, the executor inserts `--` between them automatically (via `disambiguateAfterSkippedPositionals` on the `path` arg) to disambiguate the path from the ref.

### Decision 5: `git_reset` Unstage-Only Tool

Remove `ref` entirely and add a `paths` parameter, making `git_reset` a pure unstage tool that mirrors `git_add`:

- `git_reset({paths: "file.rs"})` → `git reset -- file.rs` (unstage specific file)
- `git_reset({paths: "."})` → `git reset -- .` (unstage all staged changes)
- `paths` is required (mirrors `git_add`) — the agent must be intentional about what to unstage
- `paths` with `split: true` handles multiple files (e.g., `git_reset({paths: "a.rs b.rs"})`)

**Rationale:** An agent should never be able to reset `main` or release branches to a previous state, and feature branches should keep an audit trail. The agent reverts changes by editing files and committing as a new commit. `git_reset` is then used exclusively to correct staged commits, not to restore a branch to a previous state. This eliminates the dangerous path entirely while providing the same flexibility as `git_add` for targeted unstaging. `paths` is required (not optional) for the same reason `git_add` requires it — a bare call without paths would unstage everything by default, which is a broad operation the agent should opt into explicitly with `"."`.

The `denyPattern` on `repo` (`^(main|release/.+)$`) was removed since branch-level resets are no longer possible — unstaging files is safe on any branch.

## Alternatives Considered

### File Tool Consolidation

| Alternative | Why Not Chosen |
|-------------|----------------|
| **Add missing parameters to schemas without consolidating tools** | Fixes the mismatch but leaves the split tool surface (`files_read`/`files_read_lines`) in place — more tools for the LLM to navigate, no overwrite guard for whole-file writes |
| **Keep separate tool names, add overwrite to `files_write` only** | Partially addresses the overwrite safety gap but does not reduce tool surface or eliminate the schema/runtime mismatch for read tools |
| **Schema validation only, no tool consolidation** | Prevents future mismatches but does not fix existing ones — `files_read` would still lack `start_line` in the schema unless the parameter was added without consolidation |
| **Consolidate tools, no schema validation** | Fixes the surface area but leaves the execution boundary unguarded — new mismatches could silently reappear |

### Git Tool Consolidation

| Alternative | Why Not Chosen |
|-------------|----------------|
| **Keep separate continue/abort tool names** | 4 extra tools for thin wrappers that add one literal flag — 20% of the git skill's tool surface for infrequently used operations |
| **Consolidate branch tools as well** | The 3-mode branch tool would have the most complex `paramCondition` routing; parameter semantics differ per mode and could confuse the LLM |
| **Add `--no-edit` as a literal arg for rebase continue** | `--no-edit` is a `git commit` flag, not a `git rebase` flag — `git rebase --continue --no-edit` produces an error |
| **Keep `git_reset` `ref` parameter with safer defaults** | Any default ref is destructive (loses commits); agents should revert by editing and committing, not by moving the branch pointer |

## Consequences

**Positive:**

- **Smaller tool surface.** The LLM navigates fewer tool names — 4 fewer git tools (20 → 16), and 4 fewer file tool names (replaced by consolidated `files_read` / `files_write` / `files_edit` / `files_replace`).
- **`paramCondition` routing is a proven, reusable pattern.** Multi-mode tools (same name, different execution) are established for git skills (`git_rebase`, `git_cherry_pick`). The pattern is generalizable to future tool designs. The file editing surface originally used `paramCondition` for `files_write` but was later split into `files_write` + `files_edit` — see "Evolution: Three-Tool Model" above.
- **Schema is the contract.** The execution boundary validates against the schema, preventing silent acceptance of undeclared parameters.
- **Overwrite guard.** Whole-file writes to existing files are blocked without explicit `overwrite: true`, closing a safety gap.
- **Partial match and multiple match hints.** When the agent provides incomplete or conflicting parameters, the error message identifies which parameters are involved — the agent learns what to correct.
- **`git_log` with `ref`** enables agents to view commit history for specific branches and ranges without workarounds.
- **`git_reset` unstage-only** eliminates the ability to move the branch pointer or lose commits. Agents keep an audit trail by reverting through edits and new commits.
- **`binaryWrapper` for non-interactive git.** The `GIT_EDITOR=true` wrapper prevents interactive editors from blocking the subprocess, solving a problem that existed in the old separate `git_rebase_continue` tool as well.

**Negative:**

- **More parameters per tool.** Consolidated tools have more parameters than the separate tools they replaced. This is more schema surface for the LLM to understand, though the modes are clearly separated by parameter presence.
- **Schema/execution alignment must be maintained.** Every schema parameter must have a corresponding execution handler. The startup warning catches the reverse drift case, but it is a build-time concern, not a runtime enforcement.
- **`git_rebase --continue` requires `binaryWrapper`.** The non-interactive editor workaround is git-specific knowledge that must be maintained in the execution spec. If git's behavior changes (e.g., adding a native `--no-edit` flag to `rebase --continue`), the wrapper can be removed.

**Updated by the Three-Tool Model:**

The following negative consequences from the original two-mode `files_write` consolidation were resolved by splitting into `files_write` + `files_edit`:

- ~~**`overwrite` is silently ignored in surgical edit mode.**~~ — Resolved. `files_edit` has no `overwrite` parameter; schema validation rejects it if provided. `files_write` is always whole-file, so `overwrite` always applies.
- ~~**The `content` parameter serves double duty**~~ — Resolved. `files_write` has `content` (whole-file), `files_edit` has `old_content` / `new_content`. No naming tension.

A new positive consequence of the split:

- **Granular file editing tools.** `files_write` (whole-file) and `files_edit` (surgical) each have a dedicated parameter namespace with no ambiguity. The agent selects the tool by purpose, not by parameter combination.

## References

- [TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) — `paramCondition` field, `truncationHint` `{next_start}` derivation, `binaryWrapper` field, `kind: "literal"` args
- [TOOL_PARAMETER_NAMING.md](TOOL_PARAMETER_NAMING.md) — Tool and parameter naming conventions
- [DIAGNOSTIC_HINTS.md](DIAGNOSTIC_HINTS.md) — Hint conditions including `whenArg` for overwrite guard hints
