---
description: Design principles for chai skill tools -- tools over inference, surface reduction, SKILL.md sizing.
capability_tier: minimal
metadata:
  requires:
    bins: []
---

# Design Principles for Skill Tools

Skills are packages that give agents structured tool surfaces backed by the allowlist executor. Each skill is a directory containing `SKILL.md` (agent-facing instructions), `tools.json` (tool definitions, allowlist, and execution mapping), and optional `scripts/` (resolve and postProcess scripts).

## Tools Over Inference

Put weight on tool behavior over instruction-based guidance. Every directive in SKILL.md that a tool can enforce or communicate is a candidate for removal -- the tool should do the work so the LLM doesn't have to infer. A smaller, sharper skill surface is more efficient and more usable by smaller LLMs.

Examples from the `files` skill:
- Tool validates paths and file existence -> no need for SKILL.md to say "verify before deleting"
- `files_write_lines` verifies `original_content` before applying the patch -> no need for SKILL.md to say "re-read to get fresh line numbers"; the tool rejects stale edits and tells the agent to re-read
- Tool returns a diff after `files_write_lines` -> no need for verbose caution about boundary alignment; the agent sees errors immediately
- Removed `files_append` from the `files` skill surface (the CLI subcommand remains for kb skills) -- every tool in the surface adds inference load because the LLM must distinguish between them

## Verification Over Instruction

When an agent must perform a multi-step operation where correctness depends on state (e.g. editing a line range that shifts after each edit), prefer a tool-side verification check over an agent-side instruction. The agent provides a snapshot of the state it expects (like `original_content`), and the tool rejects the operation if the actual state has diverged. This is more reliable than instructing the agent to "always re-read before editing" because the tool enforces it.

## Tool Surface Reduction

Before adding a new tool to a skill, ask:
1. Does it do something the existing tools cannot compose?
2. Is it used frequently enough to justify the inference cost of being in the surface?
3. Could the tool's output be enhanced instead of adding a separate tool?

Before adding a new parameter to a tool, ask:
1. Does it enable behavior the tool cannot currently provide?
2. Is it more efficient than adding an agent-side workflow instruction (per "Tools Over Inference")?

## CLI Subcommands Are Shared

A CLI subcommand can serve multiple skills with different allowlist entries. Removing a tool from one skill's surface does not require removing the underlying subcommand if other skills still reference it. Skill surfaces are independent of CLI structure.

## SKILL.md Sizing

SKILL.md is loaded into the LLM's context on every turn. Every line has an ongoing cost. Guidelines for keeping it lean:
- Don't repeat what the tool schema or tool output already communicates
- Don't include workflow recipes that are obvious compositions of the tools
- Don't include tool lists that are redundant with the API schema
- Condense caution blocks when the tool provides automatic feedback

### Examples Sizing

Examples justify their per-turn context cost when they demonstrate composed workflows or non-obvious parameter relationships that the schema alone cannot convey. Single-parameter calls inferable from the schema are not worth the cost. Keep only examples that prevent real mistakes.

### Directive Audit

When adding a new directive to SKILL.md, check whether the tool could enforce it instead. When a tool gains new behavior, check whether existing directives are now redundant. This keeps the directive set minimal as the skill evolves.

## Content-Passing Channel Selection

Choose the correct `ArgKind` for each parameter based on content type:
- **`stdin`** -- arbitrary content, multi-line values, or text likely to contain special characters. Only one stdin parameter per tool.
- **`envvar`** -- verification tokens or content that must coexist with stdin; subject to OS environment variable size limits.
- **`flag`** -- only for short, controlled values (paths, identifiers, booleans, numbers). Vulnerable to quoting issues in the LLM JSON -> gateway -> CLI chain.

Never pass arbitrary text content as a CLI flag.

## Verification Comparison

When a tool verifies that content matches (e.g., `original_content` in `files_write_lines`), the comparison must be Unicode-aware. LLMs frequently substitute ASCII lookalikes for Unicode characters (smart quotes for ASCII quotes, em dashes for `--`, etc.), and strict byte-for-byte comparison causes false rejections that force the agent to fall back to less safe operations (e.g., full file rewrite without verification).

Use a multi-stage comparison:
1. **Exact match** -- fast path, no false negatives
2. **NFC-normalized match** -- handles Unicode normalization form differences (NFD vs NFC)
3. **Unicode-to-ASCII folded match** -- handles LLM substitution of common confusables

Non-exact matches should be logged as warnings but accepted, since the verification is an optimistic concurrency check, not a security boundary. The agent already has write access; rejecting a valid edit over a Unicode representation difference is strictly worse than accepting it.

## Unbounded Output Protection

Tools that can return arbitrarily large results must enforce a result cap and communicate truncation to the agent. Do not rely on the agent to predict result sizes or pre-limit queries. A truncated result plus a narrowing hint is safer than an unbounded output that can exceed context length.

The `maxOutputLines` field on the execution spec enforces this at the tool level. Set it on any tool whose output can be unbounded (search tools, diff tools, log tools). The executor truncates output to the specified line count and appends a notice with the total line count and a suggestion to narrow the query. Truncation applies after `postProcess` but before `sideRead` — side-read content is never truncated.

## Frontmatter Conventions

SKILL.md frontmatter contains only fields consumed at runtime:
- `description` -- catalog and system context display
- `capability_tier` -- startup validation warnings (`minimal`, `moderate`, or `full`)
- `model_variant_of` -- variant overlap detection (only for variant skills)
- `metadata.requires.bins` -- skill loading gate (skill skipped when binaries are absent)

Do not add derivation tracking, generation metadata, or unpopulated speculative fields to SKILL.md frontmatter. The directory name is the authoritative skill name; a `name` field is not needed.
