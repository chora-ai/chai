---
name: skills-design
description: Design principles for chai skill tools — tools over inference, surface reduction, SKILL.md sizing.
metadata:
  requires:
    bins: []
generated_from:
  spec_version: "1.0"
  capability_tier: minimal
---

# Design Principles for Skill Tools

Skills are packages that give agents structured tool surfaces backed by the allowlist executor. Each skill is a directory containing `SKILL.md` (agent-facing instructions), `tools.json` (tool definitions, allowlist, and execution mapping), and optional `scripts/` (resolve and postProcess scripts).

## Tools Over Inference

Put weight on tool behavior over instruction-based guidance. Every directive in SKILL.md that a tool can enforce or communicate is a candidate for removal — the tool should do the work so the LLM doesn't have to infer. A smaller, sharper skill surface is more efficient and more usable by smaller LLMs.

Examples from the `files` skill:
- Tool validates paths and file existence → no need for SKILL.md to say "verify before deleting"
- `files_write_lines` verifies `original_content` before applying the patch → no need for SKILL.md to say "re-read to get fresh line numbers"; the tool rejects stale edits and tells the agent to re-read
- Tool returns a diff after `files_write_lines` → no need for verbose caution about boundary alignment; the agent sees errors immediately
- Removed `files_append` from the `files` skill surface (the CLI subcommand remains for kb skills) — every tool in the surface adds inference load because the LLM must distinguish between them

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

## Duplicated CLI Subcommands vs. Skill Tools

CLI subcommands (like `chai file append`) can be used by multiple skills with different allowlist entries. Removing a tool from one skill's surface doesn't require removing the underlying CLI subcommand if other skills still need it.

## SKILL.md Sizing

SKILL.md is loaded into the LLM's context on every turn. Every line has an ongoing cost. Guidelines for keeping it lean:
- Don't repeat what the tool schema or tool output already communicates
- Don't include workflow recipes that are obvious compositions of the tools
- Don't include tool lists that are redundant with the API schema
- Condense caution blocks when the tool provides automatic feedback
- The `files` skill SKILL.md was reduced from ~9.6KB to ~6.3KB with no loss of effectiveness by applying these cuts
