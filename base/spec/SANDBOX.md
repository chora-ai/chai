---
status: stable
---

# Write Sandbox

This document specifies how the **per-profile write sandbox** enforces filesystem path boundaries for skill tools with `writePath`-annotated arguments. For the architectural decision, see [adr/WRITE_SANDBOX.md](../adr/WRITE_SANDBOX.md). For the `writePath` and `readPath` field definitions in `tools.json`, see [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md).

## Purpose

The sandbox controls **where** path-argument writes go. Together with the allowlist (which controls **what** runs), it provides two-layer enforcement: the allowlist restricts command identity; the sandbox restricts filesystem location. Neither layer alone is sufficient.

## Two Categories of Write Operations

Write operations in Chai fall into two categories with different enforcement mechanisms:

| Category | How the write target is specified | Sandbox applies? | Enforcement |
|----------|-----------------------------------|-----------------|-------------|
| **Path-argument writes** | The model provides a filesystem path as a tool parameter (e.g., `files_write_file` with a `path` arg) | **Yes** | Executor validates the path against sandbox writable roots before execution |
| **Binary-mediated writes** | The model provides a semantic identifier, and the binary resolves the write target internally (e.g., `chai skill write-skill-md` takes `skill_name`) | **No** | Security depends on the binary rejecting traversal and confining writes to its expected directory |

The sandbox applies to **path-argument writes only**. In practice, this primarily serves the `files` skill (write variants) and `git-remote` (clone). Most other writes (notes, skills) are binary-mediated.

## Writable Roots

At construction time, the sandbox scans `<profileRoot>/sandbox/` and builds a set of writable roots:

1. **The sandbox directory itself** is always a writable root.
2. For each **symlink that is a direct child** of the sandbox directory, `canonicalize()` the target and add it as a writable root.
3. Non-symlink children (files, directories) are writable by virtue of being under the sandbox root — no special handling needed.

```text
~/.chai/profiles/assistant/sandbox/    ← writable root #1 (always)
  my-project/                          ← writable (under root #1)
  linked-repo → ~/Code/my-repo/       ← canonicalized target becomes writable root #2
  workspace → ../agents/orchestrator/  ← canonicalized target becomes writable root #3
  feeds.txt                            ← writable (under root #1)
```

### Scan Depth

**Only direct children** of `sandbox/` are scanned for symlinks. Symlinks deeper in the tree (inside subdirectories) are **not** scanned. This keeps the authorization surface explicit, flat, and auditable.

## Path Validation

For each `writePath`-annotated argument, before execution:

1. **Canonicalize** the resolved path. `canonicalize()` resolves `..`, `.`, and symlinks to absolute paths, preventing traversal attacks.
2. **For new files** (path doesn't exist yet), canonicalize the parent directory and append the filename. The parent must exist and be under a writable root.
3. **Prefix check** — the canonical path must start with at least one writable root.
4. If validation passes, execution proceeds. If validation fails, the tool call is rejected with an error message — the command is **never spawned**.

Validation happens at execution time, not startup, so it handles dynamic filesystem changes.

## CWD Restriction

The executor sets `Command::current_dir()` to the sandbox root when a write-path tool runs. This prevents binaries from writing to implicit CWD-relative locations outside the sandbox. Commands like `git commit` that write relative to CWD are constrained.

## Read-Path Validation

Arguments annotated with `readPath` in `tools.json` are validated against the same writable roots. Agents can only read within directories they could also write to. This keeps the readable surface aligned with the writable surface — there is no separate "readable roots" concept.

## Default Path-Like Value Check

Arguments of kind `positional` or `flag` with no path annotation are subject to a runtime path-like value check by default. Values that start with `/`, start with `~`, start with `file://`, or contain `..` as a path component are rejected unless the parameter is annotated with `readPath: true`, `writePath: true`, or `unsafePath: true`. This makes the default safe — unannotated parameters cannot be used to access paths outside the sandbox.

Arguments annotated with `unsafePath: true` skip all sandbox validation and the runtime path-like value check. This is an escape hatch for parameters that intentionally need unrestricted path access. **Every use must be justified.** The gateway logs a startup warning for each `unsafePath` parameter in enabled skills.

## Default Working Directory

When no `workingDir` argument is present and no sandbox-validated path provides a working directory, the executor sets `Command::current_dir()` to the sandbox root. This ensures that relative paths in unannotated parameters resolve within the sandbox boundary, even if they don't match the path-like value heuristic (e.g., `etc/passwd` without a leading `/`).

## Missing Sandbox Directory

When the sandbox directory does not exist at profile root, there are no writable roots. All `writePath` and `readPath` validations fail. Skills without path-annotated arguments are **unaffected** — they continue to work normally.

## Symlink-as-Authorization

The critical security property: **agents cannot create symlinks**. The `ln` binary must never appear in any skill's allowlist. Symlink creation is exclusively a user action:

```bash
# Grant write access to a code repository
ln -s ~/Code/my-repo ~/.chai/profiles/assistant/sandbox/my-repo

# Grant agent access to its own context files
ln -s ~/.chai/profiles/assistant/agents/orchestrator ~/.chai/profiles/assistant/sandbox/workspace
```

Each symlink is **declarative authorization**:
- **Creating** a symlink grants write access to its target
- **Removing** a symlink revokes access — immediately, no restart required (validation is at execution time)
- No configuration file, no capability tier — **the filesystem IS the policy**

### Agent Context via Symlink

Agents that need to update their own context files (e.g., `MEMORY.md`) can be granted access through the symlink mechanism:

```text
~/.chai/profiles/<profile>/sandbox/
  workspace → ~/.chai/profiles/<profile>/agents/orchestrator/
```

This is authorization by the same mechanism as any other external directory — explicit, revocable, and opt-in per profile.

## Shared Across Agents

The sandbox is **per-profile**, shared by the orchestrator and all workers within that profile. There is no per-agent sandbox isolation. The three-layer defense mitigates the risk of a shared sandbox:

1. **Skill schema** constrains what the model knows about (available tools and parameters)
2. **Allowlist** constrains what operations are possible (command identity)
3. **Sandbox** constrains where writes land (filesystem location)

## Initialization

`chai init` creates `<profileRoot>/sandbox/` under each default profile, seeded with template files (`AGENTS.md`, `README.md`) from the bundled profile configuration. Template files are only written when they do not already exist — re-running `chai init` preserves user modifications.

## Related Documents

| Document | Purpose |
|----------|---------|
| [adr/WRITE_SANDBOX.md](../adr/WRITE_SANDBOX.md) | Architectural decision for the sandbox model |
| [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md) | `writePath` and `readPath` field definitions in `tools.json` |
| [PROFILES.md](PROFILES.md) | Profile directory structure (`sandbox/` location) |
| [AGENTS.md](AGENTS.md) | Per-agent context directories and shared sandbox |
