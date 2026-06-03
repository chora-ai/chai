---
status: complete
---

# Epic: Write Sandbox (Per-Profile Path Boundary Enforcement)

**Summary** — A **per-profile write sandbox** at **`<profileRoot>/sandbox/`** restricts where skill tools may write when parameters are marked **`writePath`** in `tools.json`. The executor validates resolved values against **writable roots** (the sandbox directory plus canonicalized targets of direct-child symlinks) before running the command. Together with the allowlist (which controls *what* runs), the sandbox controls *where* path-argument writes go. **`ln`** is never allowlisted; symlink creation stays a user action, so each symlink is declarative authorization.

**Status** — **Complete.** Core runtime enforcement is **implemented** (`WriteSandbox`, `writePath` on arg mappings, generic executor validation, optional CWD on allowlisted exec, gateway wiring, [TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md)). Rollout items are **done**: **`chai init`** creates **`sandbox/`** per profile seeded with template files (**`AGENTS.md`**, **`README.md`**) from the bundled profile config, the sandbox is documented in the user guides ([07-sandbox.md](../../docs/guides/07-sandbox.md)) and configuration reference, and bundled skills ship with **`writePath: true`** tools (see **[BUNDLED_SKILLS.md](BUNDLED_SKILLS.md)**). Binary-mediated writes are outside sandbox path validation by design.

## Problem Statement

Without a spatial boundary, an allowlisted binary could still write anywhere the process can reach. Path-argument tools (e.g. a future `files` write tool with an explicit filesystem `path`) need mechanical enforcement so a model cannot turn `cat` into `cat > /etc/passwd` outside an authorized tree. Binary-mediated writes (semantic ids resolved inside `notesmd-cli`, `chai skill`, etc.) remain the binary's responsibility; the sandbox applies only where the executor sees a path string.

The allowlist enforces the **action dimension** (command identity). The sandbox enforces the **spatial dimension** (filesystem location). Neither layer alone is sufficient: the allowlist without a sandbox allows tools to write anywhere; a sandbox without an allowlist allows any binary to run within the boundary. Safe write-capable skills require the composition of both.

## Goal

- Tools with **`writePath`**-annotated arguments have their write targets validated against per-profile writable roots **before** execution
- Users grant write access to directories outside the sandbox by creating symlinks inside it — the symlink IS the authorization
- The sandbox is **per-profile**, shared by orchestrator and all subagents within that profile
- Path traversal, symlink escape, and CWD-based implicit writes are mechanically prevented
- The **`tools.json`** schema documents **`writePath`** on arg mappings so write surfaces are explicit and auditable

## Current State

### Implemented

- **`WriteSandbox`** ([`exec.rs`](../../crates/lib/src/exec.rs)) — writable roots from `<profileRoot>/sandbox/`, direct-child symlink targets, `validate()`, `has_roots()`, unit tests in `sandbox_tests`.
- **`Allowlist::run`** — optional `working_dir`; sets `Command::current_dir` when provided.
- **`ArgMapping::write_path`** ([`descriptor.rs`](../../crates/lib/src/skills/descriptor.rs)) — deserializes JSON **`writePath`** (plus `optional`, `resolveCommand`, etc.).
- **`GenericToolExecutor`** ([`generic.rs`](../../crates/lib/src/tools/generic.rs)) — holds `Option<WriteSandbox>`; validates all `writePath` args before spawn; passes sandbox root as CWD when a write-path tool runs.
- **`ChaiPaths::sandbox_dir`** ([`profile.rs`](../../crates/lib/src/profile.rs)) — `profile_dir.join("sandbox")`.
- **Gateway** ([`server.rs`](../../crates/lib/src/gateway/server.rs)) — builds `WriteSandbox::new(&paths.sandbox_dir())` when the directory exists; passes it into `GenericToolExecutor::from_descriptors` for each agent.
- **Spec** — [TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) documents **`writePath`**.
- **`chai init`** ([`init.rs`](../../crates/lib/src/init.rs)) — creates **`sandbox/`** per profile during initialization, seeded with template files (**`AGENTS.md`**, **`README.md`**) from **`config/profiles/<name>/sandbox/`**.
- **User docs** — [07-sandbox.md](../../docs/guides/07-sandbox.md) documents the sandbox model, writable roots, symlink-as-authorization, and limitations; [03-configuration.md](../../docs/guides/03-configuration.md) mentions `sandbox/` in the profile directory listing.
- **Bundled skills** — multiple skills ship **`writePath: true`** tools: `files` (write, append, delete, delete-dir, write-lines), `kb` (write, append, delete), `kb-frontmatter` (edit, delete), `kb-daily` (write, append), `kb-wikilink-write` (rename), `git-remote` (clone). See **[BUNDLED_SKILLS.md](BUNDLED_SKILLS.md)**.

## Scope

### In Scope (original); status

| Item | Status |
|------|--------|
| **`WriteSandbox` in `exec.rs`** | Done |
| **`writePath` on `ArgMapping`** | Done |
| **Validation in `generic.rs`** | Done |
| **CWD on allowlisted exec** | Done |
| **Gateway + `sandbox_dir()`** | Done |
| **`writePath` in TOOLS_SCHEMA** | Done |
| **Layout / init docs + `chai init` `sandbox/`** | Done |
| **Write tools in bundled skills** | Done (tracked under BUNDLED_SKILLS) |

- **Binary-mediated write validation** — binaries that resolve write targets internally (e.g., `chai skill write-*`, `notesmd-cli create`) are not subject to sandbox path validation. Their write safety depends on the binary itself. See **Two Categories of Write Operations** below.
- **Per-agent sandbox isolation** — all agents within a profile share one sandbox. Per-agent subdirectories are deferred until a concrete use case demonstrates the need.
- **OS-level sandboxing** (containers, seccomp, landlock) — this epic is userspace path validation. Kernel-level enforcement is a possible follow-on.

## Dependencies

- **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** — `profileRoot`, `ChaiPaths` — **complete**; **`sandbox/`** is the write-boundary directory under each profile.
- **[AGENT_ISOLATION.md](AGENT_ISOLATION.md)** — per-agent executor setup — **complete**; each agent's `GenericToolExecutor` receives the same profile sandbox instance.

## Design

### Two Categories of Write Operations

Write operations in Chai fall into two categories with different enforcement mechanisms:

1. **Path-argument writes** — the model provides a filesystem path as a tool parameter (e.g., `files_write_file` with a `path` arg). The executor CAN validate the path against sandbox boundaries because the write target is explicit in the arguments. This is the category the sandbox enforces.

2. **Binary-mediated writes** — the model provides a semantic identifier, and the binary resolves the write target internally. Examples:
   - `chai skill write-skill-md` takes `skill_name` → resolves to `<skills_root>/<skill_name>/SKILL.md`
   - `notesmd-cli create` takes a note name → resolves to `<vault_dir>/<path>.md`

   The executor never sees a filesystem path, so sandbox path validation cannot apply. Security depends on the binary rejecting traversal in semantic identifiers and confining writes to its expected directory. The allowlist controls *which* binary-mediated write operations are available; the *where* enforcement is the binary's responsibility.

The sandbox applies to **category 1 only**. In practice, this primarily serves `files` (write variant) and `git clone`. Most other writes (`notesmd`, `skills`) are binary-mediated.

### Writable Roots Computation

At construction time, the sandbox scans `<profileRoot>/sandbox/` and builds a set of writable roots:

1. The sandbox directory itself is always a writable root
2. For each symlink directly in the sandbox directory, `canonicalize()` the target and add it as a writable root
3. Non-symlink children (files, directories) are writable by virtue of being under the sandbox root — no special handling needed

```text
~/.chai/profiles/assistant/sandbox/    ← writable root #1 (always)
  my-project/                          ← writable (under root #1)
  linked-repo → ~/Code/my-repo/       ← canonicalized target becomes writable root #2
  workspace → ../agents/orchestrator/  ← canonicalized target becomes writable root #3
  feeds.txt                            ← writable (under root #1)
```

Symlinks deeper than the sandbox root (i.e., inside subdirectories) are NOT scanned — only direct children of `sandbox/`. This keeps the authorization surface explicit and flat.

### Path Validation Algorithm

For each `writePath`-annotated argument, before execution:

```
fn validate_write_path(path: &str, writable_roots: &[PathBuf]) -> Result<PathBuf, String> {
    let canonical = std::fs::canonicalize(path)
        .or_else(|_| {
            // Path may not exist yet (creating a new file).
            // Canonicalize the parent, then append the filename.
            let parent = Path::new(path).parent()?;
            let name = Path::new(path).file_name()?;
            std::fs::canonicalize(parent).map(|p| p.join(name))
        })
        .map_err(|e| format!("cannot resolve write path: {}", e))?;

    for root in writable_roots {
        if canonical.starts_with(root) {
            return Ok(canonical);
        }
    }
    Err(format!("write path outside sandbox: {}", canonical.display()))
}
```

Key properties:
- `canonicalize()` resolves `..`, `.`, and symlinks to absolute paths — prevents traversal
- For new files (path doesn't exist yet), canonicalize the parent directory and append the filename
- Prefix check against writable roots — the canonical path must start with at least one root
- Validation happens at execution time, not startup — handles dynamic filesystem changes

### CWD Restriction

The executor sets `Command::current_dir()` to the sandbox root (or a specific writable root if determinable from context). This prevents binaries from writing to implicit CWD-relative locations. Commands like `git commit` that write relative to CWD are constrained to the sandbox.

### `writePath` in `tools.json`

New optional boolean field on arg mappings:

```json
{
  "param": "path",
  "kind": "positional",
  "writePath": true
}
```

When `writePath` is `true`, the executor validates the resolved parameter value against the sandbox before executing the command. If validation fails, the tool call is rejected with an error message — the command is never spawned.

### Symlink-as-Authorization

The critical security property: agents cannot create symlinks. The `ln` binary must never appear in any skill's allowlist. Symlink creation is exclusively a user action:

```bash
# User grants write access to a code repository
ln -s ~/Code/my-repo ~/.chai/profiles/assistant/sandbox/my-repo

# User grants agent access to its own context files
ln -s ~/.chai/profiles/assistant/agents/orchestrator ~/.chai/profiles/assistant/sandbox/workspace
```

Each symlink is a declarative authorization. Removing the symlink revokes access. No configuration file, no capability tier — the filesystem IS the policy.

### Agent Context via Symlink

Agents updating their own context files (e.g., `MEMORY.md`) can be handled through the existing symlink mechanism rather than a separate capability tier:

```text
~/.chai/profiles/<profile>/sandbox/
  workspace → ~/.chai/profiles/<profile>/agents/orchestrator/
```

This grants the agent write access to its own context through the same path-argument validation. Authorization is explicit (user creates symlink), revocable (user removes symlink), and opt-in per profile.

### Decisions

| Question | Decision |
|----------|----------|
| **Sandbox location** | `<profileRoot>/sandbox/` — per-profile, not global. Shared by orchestrator and all subagents. |
| **Symlink scan depth** | Direct children of `sandbox/` only. No recursive scan. |
| **New file creation** | Canonicalize parent + append filename. Parent must exist and be under a writable root. |
| **Missing sandbox dir** | No writable roots — all `writePath` validations fail. Skills without `writePath` args are unaffected. |
| **Per-agent isolation** | Open question, deferred. All agents in a profile share the sandbox. |
| **Binary-mediated writes** | Out of scope for sandbox validation. Binary authors must enforce their own path confinement. |

## Requirements

- [x] **`WriteSandbox` struct** — Construct from a profile sandbox directory path. Scan direct-child symlinks, compute writable roots. Expose `validate(&self, path: &str) -> Result<PathBuf, String>`.
- [x] **`writePath` on `ArgMapping`** — `write_path` in `descriptor.rs`; JSON **`writePath`**.
- [x] **Executor integration** — `GenericToolExecutor` takes `Option<WriteSandbox>`; validates `writePath` args before `allowlist.run()`.
- [x] **CWD restriction** — `Allowlist::run(..., working_dir)` sets `Command::current_dir()` when provided.
- [x] **Gateway wiring** — `WriteSandbox::new(&paths.sandbox_dir())` when building executors per agent.
- [x] **`ChaiPaths::sandbox_dir`** — returns `profile_dir.join("sandbox")`.
- [x] **Spec** — [TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) documents **`writePath`**.
- [x] **Tests** — Unit tests for `WriteSandbox` in [`exec.rs`](../../crates/lib/src/exec.rs) (`sandbox_tests`).
- [x] **`chai init`** — create **`sandbox/`** under each new profile, seeded with template files (**`AGENTS.md`**, **`README.md`**) from **`config/profiles/<name>/sandbox/`**.
- [x] **Documentation** — [07-sandbox.md](../../docs/guides/07-sandbox.md) user guide; [03-configuration.md](../../docs/guides/03-configuration.md) profile directory listing.
- [x] **Bundled write tools** — skills that use **`writePath: true`** (see **[BUNDLED_SKILLS.md](BUNDLED_SKILLS.md)**).

## Technical Reference

| Topic | Code / doc area |
|-------|-----------------|
| Safe exec layer (allowlist) | [`crates/lib/src/exec.rs`](../../crates/lib/src/exec.rs) |
| Generic tool executor | [`crates/lib/src/tools/generic.rs`](../../crates/lib/src/tools/generic.rs) |
| Tool descriptor / arg mapping | [`crates/lib/src/skills/descriptor.rs`](../../crates/lib/src/skills/descriptor.rs) |
| Profile paths | [`crates/lib/src/profile.rs`](../../crates/lib/src/profile.rs) |
| Gateway executor setup | [`crates/lib/src/gateway/server.rs`](../../crates/lib/src/gateway/server.rs) |
| Tools schema spec | [`spec/TOOLS_SCHEMA.md`](../spec/TOOLS_SCHEMA.md) |
| Skill format spec | [`spec/SKILL_FORMAT.md`](../spec/SKILL_FORMAT.md) |

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| **1** | **Core sandbox** — `WriteSandbox`, writable roots, `validate()`, unit tests | Done |
| **2** | **Schema + executor** — `writePath` on `ArgMapping`, validation in `GenericToolExecutor`, CWD on `Allowlist::run()` | Done |
| **3** | **Gateway + spec** — Wire sandbox at startup, `sandbox_dir()`, TOOLS_SCHEMA | Done |
| **4** | **Rollout** — `chai init`, profile docs, bundled skills with `writePath` | Done |

## Open Questions

- **Per-agent isolation** — Should agents within a profile get isolated sandbox subdirectories? Deferred until a use case demonstrates the need. The concern: a smaller model might write to the wrong file in a shared sandbox. The mitigant: the skill's tool schema constrains what the model knows about; the allowlist constrains what operations are possible; the sandbox constrains where writes land. Three layers, and the first two are mechanical.
- **Recursive symlink scanning** — Should symlinks inside subdirectories of `sandbox/` also grant write access? Current design says no — only direct children. This keeps the authorization surface flat and auditable. Revisit if users need to grant access to trees of related directories.

## Related Epics and Docs

| Topic | Where |
|-------|-------|
| Runtime profiles (`profileRoot`) | [RUNTIME_PROFILES.md](RUNTIME_PROFILES.md) |
| Per-agent executor setup | [AGENT_ISOLATION.md](AGENT_ISOLATION.md) |
| Allowlist and safe exec layer | [exec.rs](../../crates/lib/src/exec.rs) |
| Tools schema | [spec/TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) |
| Skill format and `tools.json` | [spec/SKILL_FORMAT.md](../spec/SKILL_FORMAT.md) |
| Orchestration (delegation) | [ORCHESTRATION.md](ORCHESTRATION.md) |
| Tool approval (future) | [TOOL_APPROVAL.md](TOOL_APPROVAL.md) |
