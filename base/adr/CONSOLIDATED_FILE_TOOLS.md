---
status: accepted
---

# Consolidated File Tool Schemas

## Context

The `files_read` and `files_write` tool schemas (in `tools.json`) exposed fewer parameters than their execution specs (in `execution.json`) actually accepted. The runtime accepted parameters the LLM had never been told about, creating a schema/runtime mismatch.

### Specific Mismatches

| Tool | LLM Schema (tools.json) | Execution Spec (execution.json) | Mismatch |
|------|------------------------|-------------------------------|----------|
| `files_read` | `path` only | `path` + `start_line` (optional, `absentDefault: 1`) | `start_line` is absent from schema but wired in execution |
| `notes_read` | `path` only | `path` + `start_line` (optional, `absentDefault: 1`) | Same |

When the agent sent `files_read({path: "foo.rs", start_line: 50})`, the system accepted it because the provider layer passed the raw JSON object through with no schema validation and the argv builder iterated over `execution.json` args and found `start_line: 50` in the tool call JSON.

Additionally, the read and write tool surfaces were split across separate tool names (`files_read`/`files_read_lines` and `files_write`/`files_write_lines`) despite sharing the same conceptual domain. This created unnecessary surface area for the LLM to navigate.

The system also silently accepted parameters absent from the LLM-facing schema but present in the execution spec, with no validation at the execution boundary.

## Decision

Two complementary decisions were made to address these issues.

### Decision 1: Consolidate Tool Definitions

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

Mode routing between multiple execution specs with the same tool name is handled by `paramCondition` — a new field on `ExecutionSpec` that declares which parameters must be present or absent for that spec to be selected (see [TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md)).

The same consolidation was applied to `notes_read`/`notes_read_lines` and `notes_write`/`notes_write_lines` across both full and read-only skill variants (`files`, `files-read`, `notes`, `notes-read`).

### Decision 2: Schema-Enforced Validation

The tool schema is the contract. The executor validates tool call parameters against the schema before execution — undeclared parameters and type mismatches are rejected immediately with a clear error.

| Violation | Example | Behavior |
|-----------|---------|----------|
| Undeclared parameter | Agent sends `start_line` to a schema that only declares `path` | Reject — parameter not in schema |
| Type mismatch | Agent sends `start_line: "50"` when schema declares `start_line: integer` | Reject — type does not match schema |
| Valid call | Agent sends parameters that match the schema | Execute — schema is the contract |

A startup validation (`check_schema_execution_alignment`) warns when a tool's schema declares a parameter that has no corresponding execution handler, preventing the reverse drift case.

## Alternatives Considered

| Alternative | Why Not Chosen |
|-------------|----------------|
| **Add missing parameters to schemas without consolidating tools** | Fixes the mismatch but leaves the split tool surface (`files_read`/`files_read_lines`) in place — more tools for the LLM to navigate, no overwrite guard for whole-file writes |
| **Keep separate tool names, add overwrite to `files_write` only** | Partially addresses the overwrite safety gap but does not reduce tool surface or eliminate the schema/runtime mismatch for read tools |
| **Schema validation only, no tool consolidation** | Prevents future mismatches but does not fix existing ones — `files_read` would still lack `start_line` in the schema unless the parameter was added without consolidation |
| **Consolidate tools, no schema validation** | Fixes the surface area but leaves the execution boundary unguarded — new mismatches could silently reappear |

## Consequences

**Positive:**

- **All schema/runtime mismatches eliminated.** Consolidated tools declare all parameters the execution layer accepts.
- **Smaller tool surface.** The LLM navigates fewer tool names — `files_read` and `files_write` instead of four separate tools.
- **Overwrite guard.** Whole-file writes to existing files are blocked without explicit `overwrite: true`, closing a safety gap where the previous design silently overwrote existing files.
- **`paramCondition` routing.** Multi-mode tools (same name, different execution) are now a first-class pattern, reusable for future tool designs.
- **Schema is the contract.** The execution boundary validates against the schema, preventing silent acceptance of undeclared parameters.
- **Partial match hints.** When the agent provides `start_line` without `original_content` (or vice versa), the error message identifies which paired parameter is missing — the agent learns both what is missing and why.

**Negative:**

- **More parameters per tool.** `files_write` has five parameters instead of the two or four that the separate tools had. This is more schema surface for the LLM to understand, though the modes are clearly separated by parameter presence.
- **`overwrite` is silently ignored in surgical edit mode.** Agents that pass `overwrite` with `start_line`/`original_content` will not get an error — the parameter is simply not routed. This is acceptable because `original_content` already provides the verification mechanism for surgical edits.
- **Schema/execution alignment must be maintained.** Every schema parameter must have a corresponding execution handler. The startup warning catches the reverse drift case, but it is a build-time concern, not a runtime enforcement.

## References

- [TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) — `paramCondition` field, `truncationHint` `{next_start}` derivation
- [TOOL_PARAMETER_NAMING.md](TOOL_PARAMETER_NAMING.md) — Tool and parameter naming conventions
- [DIAGNOSTIC_HINTS.md](DIAGNOSTIC_HINTS.md) — Hint conditions including `whenArg` for overwrite guard hints
