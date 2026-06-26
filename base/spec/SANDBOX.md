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
| **Path-argument writes** | The model provides a filesystem path as a tool parameter (e.g., `files_write` with a `path` arg) | **Yes** | Executor validates the path against sandbox writable roots before execution |
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

Path canonicalization happens at execution time (not startup), so renames and moves within writable roots are handled correctly. However, the set of writable roots is frozen at gateway construction time — adding or removing symlinks in the sandbox directory requires a gateway restart to take effect.

## `.git/` Directory Exclusion

The `.git/` directory is a special filesystem namespace that must only be modified through git's own tools, not through arbitrary file writes. The sandbox rejects writes that target any `.git/` directory, regardless of whether the path falls within a writable root.

After canonicalizing a write target and before the prefix check, the executor verifies that no path component is `.git`. If the canonical path contains a `.git` component, the write is rejected with an error message. This applies to both `writePath`-annotated parameters (enforced by `WriteSandbox::validate()`) and unannotated parameters (enforced by the path-like value check).

This exclusion prevents the `files` skill from bypassing the `git` skill's defense-in-depth model:

| Attack Vector | Method | Protection Bypassed |
|---------------|--------|---------------------|
| Branch rewrite | Write to `.git/refs/heads/main` | `git_commit`/`git_push` deny patterns on `main` |
| Branch deletion | Delete files in `.git/refs/heads/` | `git_branch_delete` deny pattern |
| Force switch | Write to `.git/HEAD` | Any branch-scoped deny |
| Hook injection | Write executable to `.git/hooks/` | Binary allowlist and sandbox constraints |
| Config manipulation | Write to `.git/config` | Remote URL and protection integrity |
| Object injection | Write to `.git/objects/` | Repository history integrity |

Read operations within `.git/` are unaffected — reading git state is not a security concern in the current threat model. The `git` skill's own tools are unaffected because they execute git binaries directly, which modify `.git/` through git's own mechanisms rather than through the files skill's write sandbox.

### Scope

The `.git/` component check uses path-component matching (not prefix matching), so `.gitignore` and `.gitmodules` files are not affected — they do not contain a `.git` path component.

### Escape Hatch

If a future use case requires `.git/` write access, an `unsafePath`-annotated parameter would be the explicit escape hatch. No bundled skill uses `unsafePath`, and any skill that does triggers a startup warning.

## Upward Traversal Protection

Some CLI commands traverse upward from the working directory to find project-root markers: `git` searches for `.git`, `cargo` searches for `Cargo.toml`, `hg` searches for `.hg`, etc. When a `workingDir` parameter points to a sandbox subdirectory that does not contain its own project root, the command may escape the sandbox boundary by finding a project root in a parent directory.

This is a distinct attack surface from path traversal (`..`) — the working directory path itself is inside the sandbox, but the command's internal discovery mechanism resolves to a location outside it. The sandbox validator only checks that the `workingDir` value is inside the sandbox; it does not know which command will run or how that command discovers its project root.

### Resolve-Script Validation

The defense against upward traversal is implemented in the skill's resolve script (the `resolveCommand` configured on the `workingDir` parameter). After resolving the working directory, the resolve script:

1. Runs the command's discovery mechanism from the resolved working directory (e.g., `git rev-parse --git-dir`, `cargo locate-project`).
2. Verifies that the resolved project root is inside the sandbox.
3. Exits with a non-zero code if the project root is outside the sandbox, which causes the executor to reject the tool call.

This validation is skill-specific because the discovery mechanism differs per command. The resolve script is the correct layer because it has access to both the resolved path and the command's discovery tool.

### Symlinked Directories

The sandbox may contain symlinked entries whose physical targets are outside the sandbox root (e.g., `sandbox/my-repo → ~/Code/my-repo`). These entries are granted access because the user placed them in the sandbox. Resolve scripts that use physical/canonical paths for comparison (e.g., via `pwd -P`) must check against both the physical sandbox root AND the physical targets of symlinked entries at the top level of the sandbox directory. Without this, canonicalization causes false-positive rejections on valid symlinked entries.

### Affected Skills

| Skill | Parameter | Discovery Mechanism | Validation Script |
|-------|-----------|---------------------|-------------------|
| `git`, `git-read`, `git-remote` | `repo` (`workingDir`) | `git rev-parse --git-dir` | `resolve-repo-path.sh` |
| `cargo` | `path` (`workingDir`) | `cargo locate-project` | `resolve-cargo-path.sh` |

Skills that do not use `workingDir` with upward-traversing commands are not affected.

### Resolve-Script Error Propagation

When a resolve command exits with a non-zero code, the executor rejects the tool call instead of silently falling back to the unresolved parameter value. This is critical for validation — if resolve-script errors were swallowed, a validation check that detects an upward traversal escape would be silently bypassed. The error propagation ensures that resolve-script validation is effective: a rejected tool call prevents the command from running with an unvalidated working directory.

### Clone-Path Validation

The `git-remote` skill's `git_clone` tool uses a `path` parameter (annotated with `writePath`) that specifies the clone target directory. The `resolve-clone-path.sh` script validates that absolute clone paths are inside the sandbox before allowing the clone to proceed. Relative paths are prefixed with the sandbox root as before.

## CWD Restriction

When no `workingDir` argument is present and no sandbox-validated path provides a working directory, the executor sets `Command::current_dir()` to the sandbox root. This prevents binaries from writing to implicit CWD-relative locations outside the sandbox, and ensures that relative paths in unannotated parameters resolve within the sandbox boundary even if they don't match the path-like value heuristic (e.g., `etc/passwd` without a leading `/`). When a sandbox-validated `workingDir` or path argument resolves to a specific directory, that directory takes precedence. When no sandbox exists (only possible when `sandbox.mode` is `"unsafe"` and the sandbox directory is missing), no CWD override is applied — the process inherits the gateway's working directory. When `sandbox.mode` is `"current"` and the sandbox directory is missing, the CWD is used as the sole writable root, so CWD restriction naturally confines writes to the current directory.

## Read-Path Validation

Arguments annotated with `readPath` in `tools.json` are validated against the same writable roots. Agents can only read within directories they could also write to. This keeps the readable surface aligned with the writable surface — there is no separate "readable roots" concept.

## Default Path-Like Value Check

Arguments of kind `positional` or `flag` with no path annotation are subject to a runtime path-like value check by default. Values that start with `/`, start with `~`, start with `file://`, contain `..` as a path component, or target a `.git/` directory (starting with `.git/` or containing `/.git/` as a component) are rejected unless the parameter is annotated with `readPath: true`, `writePath: true`, or `unsafePath: true`. This makes the default safe — unannotated parameters cannot be used to access paths outside the sandbox or write to git's internal state.

Arguments annotated with `unsafePath: true` skip all sandbox validation and the runtime path-like value check. This is an escape hatch for parameters that intentionally need unrestricted path access. **Every use must be justified.** The gateway logs a startup warning for each `unsafePath` parameter in enabled skills.

## Missing Sandbox Directory

When the sandbox directory does not exist at profile root, there are no writable roots from the profile sandbox. The gateway's behavior depends on the `sandbox.mode` configuration setting (see [CONFIGURATION.md](CONFIGURATION.md)):

| Mode | Sandbox directory missing | Sandbox directory present |
|------|--------------------------|---------------------------|
| **`strict`** (default) | Gateway **refuses to start** | Normal sandbox with writable roots |
| **`current`** | Gateway uses the **current working directory** as the sole writable root; CWD confinement and path validation remain active | Normal sandbox with writable roots (identical to `strict`) |
| **`unsafe`** | Gateway starts **without** a sandbox; CWD confinement and path validation are disabled | Normal sandbox with writable roots |

### `strict` Mode (Default)

The gateway **refuses to start** when the sandbox directory is missing. The error message includes the expected path and instructions to either re-run `chai init` (which recovers the sandbox for existing profiles) or set `sandbox.mode` to `"current"` or `"unsafe"`.

### `current` Mode

When the sandbox directory is missing, the gateway falls back to using the current working directory as the sole writable root. All path validation and CWD restriction still apply — writes are confined to the CWD tree. This is useful for development workflows where the gateway is launched from a project directory. When the sandbox directory exists, `"current"` behaves identically to `"strict"`.

### `unsafe` Mode

The gateway starts without a sandbox and logs a warning that CWD confinement and path validation are disabled. This bypasses the default-closed security model and should only be used when the operator explicitly accepts the risk of running without path restrictions.

### Recovery via `chai init`

`chai init` recovers a deleted sandbox directory for an existing profile: if the profile directory exists but the `sandbox/` subdirectory is missing, it is re-created and seeded with template files. Existing files within the profile are never modified.

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
- **Removing** a symlink revokes access on the next gateway restart — the writable root set is frozen at construction time, so runtime changes to the sandbox directory's symlinks are not detected until the gateway is restarted
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

If a profile directory already exists but its `sandbox/` subdirectory has been deleted, `chai init` re-creates the sandbox and seeds template files without modifying other profile files.

## Related Documents

| Document | Purpose |
|----------|---------|
| [adr/WRITE_SANDBOX.md](../adr/WRITE_SANDBOX.md) | Architectural decision for the sandbox model |
| [TOOLS_SCHEMA.md](TOOLS_SCHEMA.md) | `writePath` and `readPath` field definitions in `tools.json` |
| [PROFILES.md](PROFILES.md) | Profile directory structure (`sandbox/` location) |
| [AGENTS.md](AGENTS.md) | Per-agent context directories and shared sandbox |
