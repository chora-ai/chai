---
description: Design principles for skill packages — tools over inference, surface reduction, SKILL.md sizing.
capability_tier: minimal
metadata:
  requires:
    bins: []
---

# Design Principles for Skill Tools

## How Skills Are Structured

A skill is a directory containing three components:

- **`SKILL.md`** — Agent-facing instructions. Written in Markdown with optional YAML frontmatter. Loaded into the agent's context every turn, so every line has an ongoing cost.
- **`tools.json`** — Tool definitions, execution mapping, and command allowlist. Declares typed tool schemas the model can call and maps each tool to a CLI binary and subcommand. Schema conformance is enforced by `skills_validate`.
- **`scripts/`** (optional) — Helper scripts referenced by `resolveCommand` or `postProcess` in tools.json. Run via `sh` with no allowlist entry needed.

A skill without `tools.json` contributes instructions only — no callable tools. A skill with `tools.json` adds callable tools on top of that knowledge.

## Tools Over Inference

Prefer tool enforcement over instruction-based guidance. When a tool can enforce a behavior or validate a condition, let the tool do it instead of writing a directive in SKILL.md. A smaller, sharper skill surface is more efficient and more usable by smaller LLMs.

Concretely: before adding a directive, check whether a tool could enforce it instead. When a tool gains new behavior (validation, feedback, diff output), check whether existing directives are now redundant.

**Tools over inference means enforcement over directives, not descriptions over guidelines.** Tool enforcement — where the tool *does* something (validates, rejects, hints) — is genuinely better than a directive because it costs zero per-turn context and cannot be ignored. Tool *descriptions* are still inference: text the model reads every turn. Moving content from SKILL.md to a tool description moves inference from one channel to another; it does not reduce inference.

## Verification Over Instruction

When correctness depends on state (e.g., editing a line range that shifts after each edit), prefer a tool-side verification check over an agent-side instruction. The agent provides a snapshot of the state it expects (like `original_content`), and the tool rejects the operation if the actual state has diverged. This is more reliable than instructing the agent to "always re-read before editing" because the tool enforces it.

## Diagnostic Hints Over Directives

When a tool cannot enforce a behavior but the agent would benefit from guidance, prefer emitting a diagnostic hint in the tool's output over writing a directive in SKILL.md. A hint is a short, contextual message appended to the tool response that suggests the agent's next action — it teaches at the point of failure rather than requiring the agent to recall an instruction.

Hints are more effective than directives for three reasons:

1. **Just-in-time delivery** — the agent receives guidance exactly when it encounters the problem, not preemptively every turn.
2. **Context-free SKILL.md** — every directive removed from SKILL.md reduces per-turn context cost. A hint costs zero when the relevant path isn't taken.
3. **Discoverability** — even agents that haven't read the directive carefully will see the hint when they hit the issue.

### When to Use Hints

| Situation | Hint? | Why |
|-----------|-------|-----|
| The tool can detect a likely error and suggest a fix | ✅ | The agent learns from the specific failure rather than from a general instruction |
| The tool detects a suboptimal but valid usage | ✅ | Gentle guidance without enforcement — the agent can ignore it |
| The tool could auto-correct but the correction might be wrong | ✅ | Hint instead of silently accepting — the agent decides what to do |
| The tool can enforce the correct behavior directly | ❌ | Use enforcement instead (denyPattern, validation, defaults) |
| The guidance applies to all calls, not just error cases | ❌ | Use a brief directive in SKILL.md instead |

### Hint Implementation: hintConditions (Preferred for Simple Hints)

When a hint can be determined by a simple condition — substring match in the output, exit-code check, non-empty output, or parameter-value check — declare it inline in `tools.json` using the `hintConditions` field. No separate script file is needed.

Condition types: `match` (substring), `exitCode` (integer or `"nonzero"`), `notEmpty` (boolean), `whenArg` (parameter-value). Multiple conditions on the same entry use AND logic. Multiple entries are all evaluated; all matching entries produce hints. The `hint` field supports `{param_name}` template variables for dynamic hint text.

**Critical**: When using `exitCode: "nonzero"` (or a specific non-zero code), the tool must also declare `successExitCodes` for that exit code. Without `successExitCodes`, the executor's error propagation short-circuits before `hintConditions` is evaluated. The correct pattern: `successExitCodes` controls pipeline flow (whether the output reaches `hintConditions`), and `hintConditions` inspects the output to generate hints.

Pattern: `successExitCodes` → `hintConditions` → hint on error output.

### Hint Implementation: postProcess Scripts (For Complex Hints)

When a hint requires output transformation, multi-step logic, external commands, or structured data parsing, implement it as a `postProcess` script in `tools.json`. Each script receives the tool's output on stdin, inspects it for error conditions, and appends one-line hints when conditions are detected. Non-matching output passes through unchanged.

Use `postProcess` scripts when the hint:
- Requires filtering or transforming the output (e.g., collapsing progress lines, classifying errors).
- Runs an external command (e.g., `chai skill validate`) or reads a file from disk.
- Uses structured data parsing (e.g., counting annotated parameters, arithmetic comparison).

**Critical**: `postProcess` only runs when the command exits with a code treated as success (0 or in `successExitCodes`). If the hint targets an error condition (e.g., a command that exits 128 on "not a repository"), the tool must declare `successExitCodes` for that exit code — otherwise the error propagates before `postProcess` runs.

When `successExitCodes` admits a non-zero exit code, the executor includes stderr in the output (appended after stdout) so that `postProcess` scripts can match against error messages written to stderr.

### Hint Implementation: Binary-Level (Exception)

When a hint requires computation or state not present in stdout/stderr, it must be generated by the underlying binary. This is the exception — use it only when `hintConditions` and `postProcess` cannot access the information needed.

Two cases where binary-level hints are necessary:

1. **The hint condition depends on data not in the output** — e.g., comparing pattern indentation against file content. A `postProcess` script only sees stdout; it cannot inspect the file or the pattern.

2. **The binary's exit codes are not specific enough** — e.g., exit code 1 for regex errors, file-not-found, and permission denied. Adding `successExitCodes: [1]` would suppress *all* these errors as successful results.

When in doubt, start with `hintConditions`. Escalate to `postProcess` when the condition cannot be expressed declaratively, and to binary-level when the script cannot access the information it needs.

### Hint Design Rules

- Hints must be **short** — one line, no explanations. The agent infers the action.
- Hints must be **actionable** — suggest what the agent should do differently, not just describe the problem.
- Hints must be **non-blocking** — the tool still returns its result (or error). The hint augments, it doesn't replace.
- Hints are **not a substitute for enforcement** — if the tool can enforce the correct behavior, enforce it. Only hint when enforcement would be wrong or impossible.
- Hints must **start with `hint:` on their own line** — the truncation logic preserves lines beginning with `hint:` so hints survive `maxOutputLines` truncation.
- Hints must be **preceded by a blank line** — each hint is separated from the preceding content (or from another hint) by a blank line. For `hintConditions`, the executor automatically prepends the blank line and `hint: ` prefix. In `postProcess` scripts, use `printf '%s\n' "$var"` (not `printf '%s'`) to restore the trailing newline that command substitution strips, then `echo ""` before each `echo "hint: …"`. When multiple hints fire, each gets its own preceding blank line.

## Tool Surface Reduction

Every tool in a skill surface adds inference load — the LLM must distinguish between all available tools on every call. Before adding a new tool, confirm:

1. It does something existing tools cannot compose.
2. It is used frequently enough to justify the inference cost.
3. The tool's output could not be enhanced instead.

Before adding a new parameter to a tool, confirm:

1. It enables behavior the tool cannot currently provide.
2. It is more efficient than an agent-side workflow instruction.

## SKILL.md Sizing

SKILL.md is loaded into context every turn. Keep it lean:

- Don't repeat what the tool schema or tool output already communicates.
- Don't include workflow recipes that are obvious compositions of the tools.
- Don't include tool lists that are redundant with the API schema.
- Condense caution blocks when the tool provides automatic feedback.

Keep only examples that demonstrate composed workflows or non-obvious parameter relationships that the schema alone cannot convey.

### Section Structure

SKILL.md uses a consistent section hierarchy so the agent can distinguish hard rules from helpful context:

- **`## Skill Directives`** — hard rules the agent must follow (safety, workflow constraints, tool-choice guidance). Every directive applies to all relevant calls, not just error cases.
- **`## Skill Guidelines`** (optional) — soft context that helps the agent use the skill correctly but does not constrain behavior (configuration formats, non-obvious tool behavior, resolution order). Guidelines are not rules — the agent should not hesitate or second-guess when they don't apply.
- **`## <Named Workflow>`** (optional, for meta-skills) — composed multi-step procedures (e.g., generation workflow, security audit). Only use when the skill's purpose is to guide the agent through a structured process.

Do not add prose paragraphs after the directives list. Any additional content that survives the sizing filter belongs in a distinct section with a clear heading so the agent understands the difference between must-follow directives and helpful guidelines.

## Tool Description Sizing

Tool descriptions are loaded into context every turn alongside SKILL.md — the same sizing discipline applies. Every word in a description costs tokens on every turn, whether the agent calls that tool or not.

- **Tool descriptions are functional specs, not usage guides.** A description should tell the agent what the tool does and how to parameterize it — nothing more. Tool-selection guidance ("use this instead of that when...") belongs in SKILL.md guidelines, not in tool descriptions.
- **Output formats are discoverable.** Describing the output format preemptively (e.g., "Returns lines in the format {line_number}\\t{content}") is over-documentation. The agent discovers output format on first use. Output format descriptions should only be included when the format is ambiguous or could be misinterpreted — which is rare.
- **Tool descriptions should be lean.** The agent does not need to be told everything about a tool before calling it — it needs enough to select the right tool and provide the right parameters. Behavioral details (auto-retry, fallback modes, trailing whitespace handling) belong in SKILL.md guidelines or are discovered through tool output.
- **Tool-selection guidance belongs in SKILL.md guidelines.** When one tool is preferred over another for a specific scenario, that is advisory content. It belongs in `## Skill Guidelines` where it's clearly marked as soft guidance, not in the tool description where it inflates the schema.
- **Behavioral warnings that apply across calls belong in SKILL.md.** Content like "work from bottom to top when making multiple edits" applies across multiple tool calls, not at selection time. It belongs in SKILL.md (directive or guideline depending on severity), not in the tool description.

## Content-Passing Channel Selection

Choose the correct `ArgKind` for each parameter based on content type:

- **`stdin`** — arbitrary content, multi-line values, or text likely to contain special characters. Only one stdin parameter per tool.
- **`tempfile`** — verification tokens or content that must coexist with stdin. No size limits, no encoding issues.
- **`flag`** — only for short, controlled values (paths, identifiers, booleans, numbers). Vulnerable to quoting issues in the LLM JSON → gateway → CLI chain.

Never pass arbitrary text content as a CLI flag.

## Unbounded Output Protection

Tools that can return arbitrarily large results must enforce a result cap and communicate truncation to the agent. Set `maxOutputLines` on the execution spec for any tool whose output can be unbounded (search tools, diff tools, log tools). Truncation applies after `postProcess` but before `sideRead` — side-read content is never truncated.

Lines starting with `hint:` are preserved through truncation: the executor separates hint lines from non-hint lines, truncates only the non-hint content, then appends the preserved hints before the truncation notice.

### Custom Truncation Notices

When a tool has a continuation tool (e.g., `files_read` → `files_read_lines`), customize the truncation notice with the `truncationHint` field on the execution spec. The `truncationHint` string supports these template variables:

| Variable | Meaning |
|----------|---------|
| `{kept}` | Number of lines retained after truncation |
| `{total}` | Total lines before truncation |
| `{omitted}` | Number of lines omitted (`{total}` − `{kept}`) |
| `{next_start}` | The 1-indexed line number of the first omitted line |

**Convention**: Truncation notices must frame continuation as optional, not imperative. The purpose of truncation is to provide a preview — the notice should tell the agent there is more content and how to read it *if necessary*, not imply the agent must follow up.

✅ `"{omitted} more lines available. To continue reading, use X with start_line: {next_start}; omit end_line to read the rest."`

❌ `"Use X with start_line: {next_start} to read the remaining lines."`

For tools without a continuation tool (e.g., search tools), the executor uses a generic notice that already uses suggestive phrasing — no `truncationHint` is needed.

## Sandbox Security

The agent operates within a sandbox that restricts filesystem access. Every tool that reads or writes files must participate in sandbox enforcement — this is a security boundary, not a preference.

### Path Annotations

For each `positional` and `flag` parameter, choose the correct annotation:

| Annotation | When to Use |
|---|---|
| *(no annotation)* | Default. The parameter is not a filesystem path. The executor rejects values that look like paths (starting with `/` or `~`, starting with `file://`, or containing `..`). Most parameters need no annotation. |
| `readPath: true` | The parameter is a filesystem read target. Path-like values are expected and allowed; the executor validates them against the sandbox. |
| `writePath: true` | The parameter is a filesystem write target. Same validation as `readPath`, plus parent directories are auto-created for new files. |
| `unsafePath: true` | The parameter needs unrestricted path access outside the sandbox. The executor skips all validation. **Every use must be justified.** The gateway logs a startup warning. Use sparingly. |

`workingDir` parameters are implicitly validated as read paths — no explicit `readPath` annotation needed.

### Resolve Commands and Path Parameters

When a parameter uses `resolveCommand`, the resolve command may transform a short value into a filesystem path (e.g., `my-note` → an absolute path). The default security check only inspects the agent-provided value before resolution — it does not see the resolved result. If the final resolved value is a filesystem path, the parameter still needs `readPath` or `writePath` so the sandbox validates the resolved path.

**Warning:** This means `resolveCommand` commands can produce paths that bypass validation if the output parameter isn't also annotated with `readPath` or `writePath`. When a resolve command constructs a path from multiple parameters, ALL parameters that contribute to the path must be validated — otherwise a traversal in an unvalidated contributor (e.g., `scope` containing `../`) can escape the sandbox. Parameters only referenced via `$name` in `resolveCommand.args` are not in the execution `args` array and are not validated by the sandbox. The resolve command itself must reject dangerous values (e.g., `..` in path components).

### Upward Traversal by External Commands

Some CLI commands traverse upward from the working directory to find project-root markers: `git` searches for `.git`, `cargo` searches for `Cargo.toml`, `hg` searches for `.hg`, etc. When a `workingDir` parameter points to a sandbox subdirectory that doesn't contain its own project root, the command may escape the sandbox boundary by finding a project root in a parent directory.

**If a skill uses `workingDir` with a command that traverses upward, the resolve command must verify that the command's resolved project root is inside the sandbox.** Run the command's discovery mechanism (e.g., `git rev-parse --git-dir`, `cargo locate-project`) from the resolved working directory and validate the result is within the sandbox before allowing the command to proceed.

Bundled skills use `chai resolve` subcommands for this validation (e.g., `chai resolve repo-path`, `chai resolve cargo-path`). Custom skills may use shell scripts via the `resolveCommand.script` mechanism, following the same validation pattern.

**Example: Git Upward Traversal** — When the git skill's `repo` parameter points to a sandbox subdirectory that doesn't contain a `.git` directory, `git` traverses upward to find the nearest `.git`. If the sandbox root doesn't have its own `.git`, git discovers and operates on a repository outside the sandbox — leaking commit history, branch names, and file contents, and potentially allowing writes to commits and branches outside the sandbox. The fix: `chai resolve repo-path` runs `git rev-parse --git-dir` and validates the result is inside the sandbox before allowing the command.

**Symlinked directories in the sandbox** must be handled carefully. The sandbox may contain symlinked entries whose physical targets are outside the sandbox root. These entries are granted access because the user placed them in the sandbox. When comparing physical/canonical paths (e.g., from `pwd -P` or `cargo locate-project`), the resolve command must check the path against both the physical sandbox root AND the physical targets of symlinked entries at the top level of the sandbox directory. Without this, `pwd -P` canonicalization causes false positive rejections on valid symlinked entries. `chai resolve` handles this automatically.

### Design Checklist

For every parameter in a skill's `args` array:

1. Is it a filesystem path? If yes, annotate with `readPath` or `writePath`.
2. Is it a write target? If yes, use `writePath` (auto-creates parent dirs).
3. Does it need unrestricted path access outside the sandbox? If yes, use `unsafePath` and document why. Expect a startup warning.
4. Otherwise, no annotation needed — the default is safe.
5. Does the resolve command construct paths from parameters not in the `args` array (e.g., `$scope` in `resolveCommand.args`)? If yes, the resolve command must reject dangerous values (`..`, absolute paths) since the sandbox cannot validate them.
6. Does any tool use a `workingDir` parameter with a command that traverses upward to find project-root markers (`.git`, `Cargo.toml`, `.hg`, etc.)? If yes, the resolve command must verify the resolved project root is inside the sandbox. For symlinked directories, the resolve command must also check symlinked entries at the top level of the sandbox directory — otherwise valid symlinked directories will be incorrectly rejected. Bundled skills use `chai resolve` for this validation.

## Disallowed Values

When certain parameter values must always be rejected regardless of the agent's intent, enforce this at the tool level using `denyPattern` on the execution spec — not by instruction. The executor rejects matching values before the command runs. Use this for cases like protecting specific git branches from writes, blocking dangerous flags, or refusing reserved identifiers. When the parameter is omitted, `denyResolveCommand` can resolve the current value (e.g., the current branch) and `denyAlwaysResolve` can enforce a check even when no value is provided.

## Skill Naming and Variant Conventions

Skills are organized into **base skills** and **variant skills** using a naming convention based on hyphens.

### Base Skills

A base skill has no hyphen in its name (e.g., `git`, `files`, `notes`, `skills`). It provides the standard set of operations for its domain.

### Variant Skills

A hyphenated skill name indicates a variant of the base skill (the part before the hyphen). Two patterns:

- **`<base>-read`** — read-only, minimal variant. Strips all write tools. `capability_tier: minimal`. Declares `variant_of: <base>`.
- **`<base>-<extension>`** — extension variant that adds capabilities to the base skill's domain. Self-contained with only the extension-specific tools. Can be used alongside the base skill or independently. `capability_tier` reflects the variant's own surface. Does **not** declare `variant_of` because its tools are complementary, not overlapping.

### Self-Containment

Each skill must be self-contained. SKILL.md must not reference tools from other skills or assume another skill is co-enabled. Extension variants must define only their own tools in `tools.json` — they do not duplicate the base skill's tool surface.

### `variant_of` Field

Frontmatter field that links a variant to its base skill. Used by startup validation to warn when overlapping skills are co-enabled for the same agent. Only declare `variant_of` when the variant's tool surface is a subset of the base skill's (i.e., enabling both creates redundancy). Extension variants with complementary (non-overlapping) tools should not declare `variant_of`.

## Frontmatter Conventions

SKILL.md frontmatter contains only fields consumed at runtime:

- `description` — catalog and system context display
- `capability_tier` — startup validation warnings (`minimal`, `moderate`, or `full`)
- `variant_of` — variant overlap detection (only for variant skills with overlapping tools)
- `metadata.requires.bins` — skill loading gate (skill skipped when binaries are absent)

Do not add derivation tracking, generation metadata, or unpopulated speculative fields to SKILL.md frontmatter. The directory name is the authoritative skill name; a `name` field is not needed.
