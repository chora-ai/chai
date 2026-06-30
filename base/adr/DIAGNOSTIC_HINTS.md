---
status: accepted
---

# Diagnostic Hints in Tool Output

## Context

When a chai tool encounters a condition the agent could handle better, the traditional approach is to write a directive in SKILL.md instructing the agent how to behave. However, SKILL.md content is loaded into context every turn, creating an ongoing cost regardless of whether the situation ever arises. Directives also require the agent to recall and apply general instructions to specific situations — a task where LLMs are unreliable.

During the Round 3 skills audit, two bug fixes in `files_replace` demonstrated a better pattern:

1. **Leading-whitespace hint**: When a pattern matches 0 times but would match with indentation normalization, the tool appends `"hint: pattern did not match, but would match with leading-whitespace normalization — check indentation"`. This replaced the need for a directive like "always match indentation exactly."

2. **Regex error suggestion**: When `files_replace` encounters a regex parse error, the error message suggests using `literal: true`. This reduced the need for agents to pre-emptively know about literal mode.

3. **Line-diff hint in `verify_original`**: When `files_write_lines` rejects an `expected_content` mismatch (after all five validation stages fail), the error message includes a line-diff hint identifying the first line that differs between expected and actual content (e.g., `hint: first difference at line 2 of the content (file line 5) — expected: "c", actual: ""`). File line numbers allow the agent to cross-reference with `files_read` output. This replaced the previous byte-offset-only hint, which was difficult to map to line boundaries.

In both cases, the tool teaches the agent at the point of failure, rather than requiring preemptive instruction. The agent receives guidance exactly when needed, at zero cost when the situation doesn't arise.

Subsequent implementation of git skill hints revealed two distinct hint mechanisms with different tradeoffs, leading to this revised decision.

## Decision

Adopt diagnostic hints as a first-class design pattern for chai skill tools, formalized in the skills-design SKILL.md under "Diagnostic Hints Over Directives." When evaluating whether to add a directive to SKILL.md, first check whether a tool-level hint would be more effective.

Hints are short, actionable messages appended to tool output when the tool detects a condition the agent could handle better. They are non-blocking (the tool still returns its result or error) and are not a substitute for enforcement (if the tool can enforce the correct behavior, it should).

### Three Hint Mechanisms

Hints are implemented via one of three mechanisms, chosen based on what the hint needs to inspect:

**1. `hintConditions` (preferred for simple output-inspection and exit-code hints)**

When a hint can be determined by a simple condition — substring match in the output, exit-code check, non-empty output, or parameter-value check — declare it inline in `tools.json` using the `hintConditions` field. Each condition is a declarative entry that the executor evaluates after `postProcess` and before truncation. No separate script file is needed.

This is the preferred mechanism for simple hints because:
- **Single source of truth** — hint logic lives in `tools.json` alongside the execution spec, not in a separate file.
- **No shell boilerplate** — the 6-line stdin-buffer/grep/echo pattern is eliminated; hints are one-liner declarations.
- **Lower maintenance surface** — no need to keep hint scripts in sync across skill variants.
- **Testable in Rust** — hint condition matching is Rust code with unit tests, not untested shell scripts.

Condition types: `match` (substring), `exitCode` (integer or `"nonzero"`), `notEmpty` (boolean), `whenArg` (parameter-value). Multiple conditions on the same entry use AND logic. Multiple entries are all evaluated; all matching entries produce hints. The `hint` field supports `{param_name}` template variables for dynamic hint text.

Critical implementation detail: When using `exitCode: "nonzero"` (or a specific non-zero code), the tool must also declare `successExitCodes` for that exit code. Without `successExitCodes`, the executor's error propagation short-circuits before `hintConditions` is evaluated. The correct pattern is: `successExitCodes` controls pipeline flow (whether the output reaches `hintConditions`), and `hintConditions` inspects the output to generate hints.

**2. `postProcess` scripts (for complex output-inspection hints)**

When a hint requires output transformation, multi-step logic, external commands, or structured data parsing that cannot be expressed as a simple condition, implement it as a `postProcess` shell script in the skill's `scripts/` directory. The script receives the tool's combined output on stdin, inspects it, and appends one-line hints when conditions are detected. Non-matching output passes through unchanged.

Use `postProcess` scripts when:
- The hint requires filtering or transforming the output (e.g., collapsing progress lines, classifying errors).
- The hint runs an external command (e.g., `chai skill validate`) or reads a file from disk.
- The hint uses structured data parsing (e.g., counting annotated parameters, arithmetic comparison).

Critical implementation detail: `postProcess` only runs when the command exits with a code treated as success (0 or in `successExitCodes`). If the hint targets an error condition, the tool must declare `successExitCodes` for that exit code — otherwise the error propagates before `postProcess` runs. The executor always merges stderr into stdout (appended after stdout with a newline separator) before the post-process step, so hint scripts can match against diagnostics written to stderr (e.g., compiler warnings). This applies to all success-path exit codes (0 and any codes in `successExitCodes`).

**3. Binary-level hints (only when internal state is required)**

When a hint requires computation or state that the command's output does not expose, the hint must be generated by the binary itself (e.g., `chai file replace` appending the leading-whitespace hint after comparing pattern indentation against file content). This is the exception, not the rule.

Binary-level hints are acceptable when:
- The hint condition depends on data not present in stdout/stderr (e.g., comparing pattern against file content with normalization).
- The hint is tightly coupled to the binary's internal error-handling path and replicating it externally would be fragile.

Binary-level hints are not acceptable when:
- The condition is detectable from the command's output alone. Use `hintConditions` or `postProcess` instead.
- The hint could be added by a skill author without modifying the chai binary.

### Choosing the Mechanism

| Condition | Mechanism | Example |
|-----------|-----------|---------|
| Output contains a known error string | `hintConditions` `match` | git's "not a git repository" → hint about specifying a valid repo path |
| Command exits with a specific code | `hintConditions` `exitCode` + `successExitCodes` | grep exit 1 (no matches) → hint about broader pattern |
| Output is non-empty (search results) | `hintConditions` `notEmpty` | `files_search` has matches → hint about using `files_read_lines` |
| Hint depends on a parameter value | `hintConditions` `whenArg` | `git_diff` with `ref: "main"` → hint about changes since diverging |
| Hint text includes a parameter value | `hintConditions` `{param}` template | `git_reset` → "reset to {ref} — use git_status" |
| Hint requires output transformation | `postProcess` script | cargo check filtering progress lines, classifying errors |
| Hint runs an external command | `postProcess` script | `skills_validate` running `chai skill validate` |
| Hint requires comparing data not in output | Binary-level | `files_replace` leading-whitespace normalization check |
| Hint is embedded in the binary's error message | Binary-level | `files_replace` regex error suggesting `literal: true` |
| Hint requires comparing expected vs actual content line-by-line | Binary-level | `files_write_lines` line-diff hint in `verify_original` error |

When in doubt, start with `hintConditions`. Only escalate to `postProcess` when the condition cannot be expressed as a simple declarative entry, and to binary-level when the script cannot access the information it needs.

### Hint Format Convention

All diagnostic hints — whether emitted by `hintConditions`, `postProcess` scripts, or the binary — must follow the same format: a standalone line starting with `hint:`.

```
hint: <short, actionable message>
```

This convention is not cosmetic. The `truncate_output()` function preserves lines starting with `hint:` when truncating tool output: non-hint lines are truncated to `maxOutputLines`, then hint lines are appended before the truncation notice. A hint that does not start with `hint:` at the beginning of its line (e.g., embedded inline within another line) cannot be detected and will be lost to truncation.

Binary-level hints that previously embedded the hint inline (e.g., `0 replacements in path (hint: …)`) must emit the hint as a separate line to be preserved by truncation.

**Blank line before every hint**: Each hint must be preceded by a blank line, separating it from the preceding content or from another hint. This applies to all hint sources:

| Source | Pattern |
|--------|---------|
| `hintConditions` | The executor automatically prepends a blank line and `hint: ` prefix before each matching condition's hint text |
| `postProcess` scripts | `printf '%s\n' "$var"` (to restore trailing newline stripped by command substitution), then `echo ""` then `echo "hint: …"` before each hint |
| Binary `println!` | `println!("\nhint: …")` (the `\n` combines with `println!`'s trailing newline to produce a blank line) |
| Binary `anyhow::bail!` | Separate each hint from the preceding text with `\n` (e.g., `…\n{}\n{}` for two hints, where each starts with `\nhint:`) |

The `postProcess` pattern requires `printf '%s\n'` (not `printf '%s'`) because the standard idiom `output=$(cat)` uses command substitution, which strips trailing newlines. Without the `\n`, `echo ""` only replaces the stripped newline — it terminates the last line of output but does not produce a visible blank line before the hint. Using `printf '%s\n'` restores the trailing newline so the subsequent `echo ""` produces the intended blank separator.

When multiple hints fire in the same output, each hint gets its own preceding blank line — hints must not appear as a dense block with no visual separation.

### Truncation Notices

When a tool's output exceeds `maxOutputLines`, the executor truncates the non-hint content and appends a truncation notice. Tools can customize this notice via the `truncationHint` field in their `tools.json` execution spec.

**Template variables** available in `truncationHint`:

| Variable | Meaning |
|----------|---------|
| `{kept}` | Number of lines retained after truncation |
| `{total}` | Total lines before truncation |
| `{omitted}` | Number of lines omitted (`{total}` − `{kept}`) |
| `{next_start}` | The 1-indexed line number of the first omitted line (for tools with a continuation tool like `files_read_lines`) |

**Philosophy**: The purpose of truncation is to provide a preview, not to make the agent follow up with another tool call to read the full content. The truncation notice should tell the agent there is more content and how to read it *if necessary*, not imply the agent must read the rest.

**Convention**: Truncation notices must frame continuation as optional, not imperative.

| Phrasing | Style | Example |
|----------|-------|---------|
| ✅ Informational + optional | `"{omitted} more lines available. To continue reading, use X with start_line: {next_start}; omit end_line to read the rest."` | States fact neutrally; "to continue reading" is optional framing |
| ❌ Imperative | `"Use X with start_line: {next_start} to read the remaining lines."` | Implies the agent should follow up, defeating the purpose of truncation |

For tools without a continuation tool (e.g., search tools that return matching lines), the generic notice (`Narrow your query path, pattern, or range to reduce results.`) already uses suggestive phrasing.

### `CHAI_EXIT_CODE` Environment Variable

`postProcess` scripts receive the main command's exit code as the `CHAI_EXIT_CODE` environment variable. This is essential for scripts that need to distinguish between a successful command and an error condition admitted by `successExitCodes`.

The canonical pattern for a `postProcess` script that inspects the exit code:

```sh
input=$(cat)

if [ "${CHAI_EXIT_CODE:-0}" != "0" ]; then
    printf '%s\n' "$input"
    echo ""
    echo "hint: <message for the error case>"
else
    printf '%s\n' "$input"
fi
```

Two implementation rules demonstrated by the `hint-not-found.sh` bug fix:

1. **Buffer stdin before processing** — Use `input=$(cat)` to capture all of stdin into a variable. Never pipe stdin directly into `grep` or any other command that reads stdin, because the first consumer will drain the pipe and subsequent commands (including `cat` used to pass output through) will receive nothing or partial content.

2. **Check `CHAI_EXIT_CODE` instead of pattern-matching output** — When a hint depends on whether the command failed, use the exit code as the signal. Pattern-matching output for error strings produces false positives when the file content itself contains those strings (e.g., documentation, test fixtures, or the script itself). The exit code is an unambiguous signal provided by the runtime.

## Alternatives Considered

| Alternative | Why Not Chosen |
|------------|----------------|
| More directives in SKILL.md | Ongoing context cost every turn; agent may not recall or apply the directive when the situation arises |
| Silent auto-correction | Removes agent agency and can produce incorrect results when the tool's assumption is wrong (e.g., silently accepting mismatched indentation produces misindented output) |
| Tool-level enforcement for everything | Some conditions are genuinely ambiguous — the tool can detect the issue but shouldn't impose a single resolution |
| Binary-level hints only (original decision) | Hints are hidden inside the binary, not visible in the skill package. Adding or modifying hints requires Rust changes and a binary rebuild. Skill authors cannot modify hints without touching the chai codebase. The `successExitCodes` + `postProcess` pattern demonstrated that most hints only need output inspection, not internal state. |
| `postProcess` scripts only (revised decision) | Most hint scripts are trivial boilerplate (buffer stdin, grep, echo). The 6-line boilerplate pattern discourages adding hints where they would be valuable because the simplest possible hint requires a separate file. `hintConditions` eliminates the boilerplate for the common case, reserving `postProcess` for hints that need output transformation or external commands. |

## Consequences

- **Smaller SKILL.md files**: Directives that can be replaced by hints reduce per-turn context cost.
- **Better agent experience**: Agents receive guidance at the point of failure, which is more effective than preemptive instruction.
- **Skill authoring discipline**: Each proposed directive now has a clear checklist — can the tool enforce it? Can the tool hint at it? Only if neither works should the directive remain in SKILL.md.
- **Three-tier hint architecture**: Simple hints use `hintConditions` (declarative, in `tools.json`); complex hints use `postProcess` scripts (in the skill's `scripts/` dir); hints requiring internal state use the binary. Skill authors can add and modify `hintConditions` without touching shell scripts or the chai binary.
- **`successExitCodes` awareness required**: Skill authors implementing error-condition hints (via `hintConditions` `exitCode` or `postProcess`) must declare `successExitCodes` on the execution spec. Forgetting this is the most common mistake — the hint exists but never fires because the error propagates first. The skills-design SKILL.md documents this requirement.
- **Directive-to-enforcement conversion**: The skill directives audit demonstrated the checklist in practice. Across 16 skills, 14 directives were deleted (already enforced by tools/sandbox or redundant with hints/workflow docs), 11 were moved from hard directives to soft guidelines (workflow guidance that does not constrain behavior), and 3 were converted to tool enforcement (delete-confirmation hintConditions on `files_delete`/`files_delete_dir`/`notes_delete`/`notes_delete_dir`, expanded `git_reset` denyPattern to block `main`, and `git_branch_delete` hintCondition for "not fully merged"). The remaining 14 directives address issues that cannot be enforced at the tool level. The directive → guideline distinction reduced hard rules from 39 to 14 (64% reduction) while preserving useful guidance.
- **Tool descriptions are still inference**: The audit revealed that "tools over inference" conflates tool enforcement (zero per-turn cost, cannot be ignored) with tool descriptions (text the model reads every turn, same cost as SKILL.md). Moving content from SKILL.md to a tool description does not reduce inference — it moves it to a different channel. The skills-design SKILL.md now explicitly states "tools over inference means enforcement over directives, not descriptions over guidelines" and includes a "Tool Description Sizing" section establishing that tool descriptions should contain only functional specifications, not usage guidance or output format descriptions.
