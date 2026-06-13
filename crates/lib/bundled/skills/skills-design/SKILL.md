---
description: Design principles for chai skill tools — tools over inference, surface reduction, SKILL.md sizing.
capability_tier: minimal
metadata:
  requires:
    bins: []
---

# Design Principles for Skill Tools

## How Skills Are Structured

A skill is a directory containing three components:

- **`SKILL.md`** — Agent-facing instructions. Written in Markdown with optional YAML frontmatter. Loaded into the agent's context every turn, so every line has an ongoing cost.
- **`tools.json`** — Tool definitions, execution mapping, and command allowlist. Declares typed tool schemas the model can call and maps each tool to a CLI binary and subcommand. See `TOOLS_SCHEMA.md` for the full schema.
- **`scripts/`** (optional) — Helper scripts referenced by `resolveCommand` or `postProcess` in tools.json. Run via `sh` with no allowlist entry needed.

A skill without `tools.json` contributes instructions only — no callable tools. A skill with `tools.json` adds callable tools on top of that knowledge.

## Tools Over Inference

Prefer tool enforcement over instruction-based guidance. When a tool can enforce a behavior or validate a condition, let the tool do it instead of writing a directive in SKILL.md. A smaller, sharper skill surface is more efficient and more usable by smaller LLMs.

Concretely: before adding a directive, check whether a tool could enforce it instead. When a tool gains new behavior (validation, feedback, diff output), check whether existing directives are now redundant.

## Verification Over Instruction

When correctness depends on state (e.g., editing a line range that shifts after each edit), prefer a tool-side verification check over an agent-side instruction. The agent provides a snapshot of the state it expects (like `original_content`), and the tool rejects the operation if the actual state has diverged. This is more reliable than instructing the agent to "always re-read before editing" because the tool enforces it.

When a tool verifies content matches, the comparison should tolerate LLM representation differences (Unicode confusables, trailing whitespace). Use a multi-stage comparison: exact, NFC-normalized, Unicode-to-ASCII folded, then trailing-whitespace-tolerant. Non-exact matches should be logged as warnings but accepted. When the match succeeds via trailing-whitespace tolerance, preserve the original file's trailing whitespace in the replacement content.

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

Keep only examples that demonstrate composed workflows or non-obvious parameter relationships that the schema alone cannot convey. Single-parameter calls inferable from the schema are not worth the cost.

## Content-Passing Channel Selection

Choose the correct `ArgKind` for each parameter based on content type:

- **`stdin`** — arbitrary content, multi-line values, or text likely to contain special characters. Only one stdin parameter per tool.
- **`tempfile`** — verification tokens or content that must coexist with stdin. No size limits, no encoding issues.
- **`flag`** — only for short, controlled values (paths, identifiers, booleans, numbers). Vulnerable to quoting issues in the LLM JSON → gateway → CLI chain.

Never pass arbitrary text content as a CLI flag.

## Unbounded Output Protection

Tools that can return arbitrarily large results must enforce a result cap and communicate truncation to the agent. Do not rely on the agent to predict result sizes or pre-limit queries. Set `maxOutputLines` on the execution spec for any tool whose output can be unbounded (search tools, diff tools, log tools). The executor truncates output to the specified line count and appends a notice with the total line count and a suggestion to narrow the query. Truncation applies after `postProcess` but before `sideRead` — side-read content is never truncated.

## Sandbox Security

The agent operates within a sandbox that restricts filesystem access. Every tool that reads or writes files must participate in sandbox enforcement — this is a security boundary, not a preference.

### Path Annotations

For each `positional` and `flag` parameter, choose the correct annotation:

| Annotation | When to Use |
|---|---|
| *(no annotation)* | Default. The parameter is not a filesystem path. The executor rejects values that look like paths (starting with `/` or `~`, starting with `file://`, or containing `..`). Most parameters need no annotation. |
| `readPath: true` | The parameter is a filesystem read target (e.g., a `path` parameter on a read tool). Path-like values are expected and allowed; the executor validates them against the sandbox. |
| `writePath: true` | The parameter is a filesystem write target (e.g., a `path` parameter on a write tool). Same validation as `readPath`, plus parent directories are auto-created for new files. |
| `unsafePath: true` | The parameter needs unrestricted path access outside the sandbox. The executor skips all validation. **Every use must be justified.** The gateway logs a startup warning. Use sparingly. |

`workingDir` parameters are implicitly validated as read paths — no explicit `readPath` annotation needed.

### Resolve Scripts and Path Parameters

When a parameter uses `resolveCommand`, the resolve script may transform a short value into a filesystem path (e.g., `my-note` → `/home/user/.chai/kb/my-note`). The default security check only inspects the agent-provided value before resolution — it does not see the resolved result. If the final resolved value is a filesystem path, the parameter still needs `readPath` or `writePath` so the sandbox validates the resolved path. The annotation applies to the parameter regardless of how the path value is constructed — directly from the agent, via resolve script, or both.

### Design Checklist

For every parameter in a skill's `args` array:

1. Is it a filesystem path? If yes, annotate with `readPath` or `writePath`.
2. Is it a write target? If yes, use `writePath` (auto-creates parent dirs).
3. Does it need unrestricted path access outside the sandbox? If yes, use `unsafePath` and document why. Expect a startup warning.
4. Otherwise, no annotation needed — the default is safe.

## Disallowed Values

When certain parameter values must always be rejected regardless of the agent's intent, enforce this at the tool level using `denyPattern` on the execution spec — not by instruction. The executor rejects matching values before the command runs. Use this for cases like protecting specific git branches from writes, blocking dangerous flags, or refusing reserved identifiers. When the parameter is omitted, `denyResolveCommand` can resolve the current value (e.g., the current branch) and `denyAlwaysResolve` can enforce a check even when no value is provided.

## Skill Naming and Variant Conventions

Skills are organized into **base skills** and **variant skills** using a naming convention based on hyphens.

### Base Skills

A base skill has no hyphen in its name (e.g., `git`, `files`, `kb`, `skills`). It provides the standard set of operations for its domain.

### Variant Skills

A hyphenated skill name indicates a variant of the base skill (the part before the hyphen). Two patterns:

- **`<base>-read`** — read-only, minimal variant. Strips all write tools. `capability_tier: minimal`. Declares `variant_of: <base>`. Example: `git-read` provides status, log, diff, show, and branch — no staging, committing, or branch creation.
- **`<base>-<extension>`** — extension variant that adds capabilities to the base skill's domain. Self-contained with only the extension-specific tools. Can be used alongside the base skill or independently. `capability_tier` reflects the variant's own surface. Does **not** declare `variant_of` because its tools are complementary, not overlapping. Examples: `git-remote` adds clone, pull, push, and remote; `kb-wikilink` adds wikilink discovery and rename.

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
