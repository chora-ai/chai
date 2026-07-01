# Write Sandbox

Each profile has a **write sandbox** that restricts where skill tools may write. The sandbox enforces the **spatial dimension** of tool safety — the allowlist controls *what* runs, the sandbox controls *where* writes go. Both layers are needed for safe write-capable skills.

## What the Sandbox Does

When a skill tool has a **`writePath`** parameter (marked in its `execution.json`), the executor validates the resolved path against writable roots **before** running the command. If the path is outside all writable roots, the tool call is rejected with an error — the command is never spawned.

This prevents a model from using a write tool to modify files outside the authorized tree (e.g. turning `cat` into `cat > /etc/passwd`).

## Sandbox Location

The sandbox directory lives under each profile:

```text
~/.chai/profiles/<name>/sandbox/
```

**`chai init`** creates a `sandbox/` for each default profile, seeded with a template file (`AGENTS.md`). The sandbox is per-profile — all agents within a profile share the same sandbox. Re-running `chai init` preserves existing sandbox files.

## Writable Roots

Writable roots are computed at gateway startup from the sandbox directory:

1. The **sandbox directory itself** is always a writable root
2. Each **direct-child symlink** in the sandbox is resolved (`canonicalize`) and its target becomes an additional writable root

```text
~/.chai/profiles/assistant/sandbox/    ← writable root #1 (always)
  my-project/                          ← writable (under root #1)
  my-repo → ~/Code/my-repo/            ← target becomes writable root #2
  context → ../agents/orchestrator/    ← target becomes writable root #3
  notes.txt                            ← writable (under root #1)
```

Only **direct children** of `sandbox/` are scanned. Symlinks inside subdirectories are not followed. This keeps the authorization surface explicit and auditable.

## Granting Write Access

Agents cannot create symlinks — the `ln` binary is never allowlisted in any skill. Symlink creation is exclusively a user action, so each symlink is **declarative authorization**:

```bash
# Grant access to a code repository
ln -s ~/Code/my-repo ~/.chai/profiles/assistant/sandbox/my-repo

# Grant access to the agent's own context directory
ln -s ~/.chai/profiles/assistant/agents/orchestrator ~/.chai/profiles/assistant/sandbox/context
```

Removing the symlink revokes access. No configuration file, no capability tier — the filesystem is the policy.

## How Path Validation Works

For each `writePath`-annotated argument, before the command runs:

1. **Exclude** `.git/` directories — reject any path whose canonical form contains a `.git` path component (see below)
2. **Resolve** the path to a canonical absolute path (resolving `..`, `.`, and symlinks)
3. For new files that don't exist yet, canonicalize the parent directory and append the filename
4. **Check** whether the canonical path starts with at least one writable root
5. If it passes all checks, proceed; if not, reject the tool call

This mechanically prevents path traversal, symlink escape, CWD-based implicit writes, and direct writes to `.git/` directories.

## `.git/` Directory Exclusion

The write sandbox unconditionally rejects any write target whose canonical path contains a `.git` **path component**. This check runs **before** the writable-root prefix check, so `.git/` directories are excluded even when they fall within a writable root.

### Why This Matters

The `git` skill uses deny patterns and branch protection to constrain how git state is modified. Without `.git/` exclusion, a model could bypass these protections entirely by writing directly to `.git/` files:

| Attack Vector | Method | Protection Bypassed |
|---|---|---|
| Branch rewrite | Write to `.git/refs/heads/main` | `git_commit`/`git_push` deny patterns on `main` |
| Branch deletion | Delete files in `.git/refs/heads/` | `git_branch_delete` deny pattern |
| Force switch | Write to `.git/HEAD` | Any branch-scoped deny |
| Hook injection | Write executable to `.git/hooks/` | Binary allowlist and sandbox constraints |
| Config manipulation | Write to `.git/config` | Remote URL and protection integrity |
| Object injection | Write to `.git/objects/` | Repository history integrity |

### Scope

The check uses **path-component matching** — it matches `.git` as a complete directory segment, not as a substring. Files like `.gitignore` and `.gitmodules` at the repository root are **not** affected.

The runtime path-like value check also rejects unannotated `positional` and `flag` parameters that target a `.git/` directory (starting with `.git/` or containing `/.git/` as a component). Parameters annotated with `unsafePath` bypass this check.

## What the Sandbox Does Not Cover

**Binary-mediated writes** — where the model provides a semantic identifier and the binary resolves the write target internally — are not subject to sandbox validation. For example, `chai skill write-skill-md` takes a skill name (not a filesystem path) and the binary resolves it to the skills directory. The executor never sees a filesystem path, so sandbox validation cannot apply. Security for binary-mediated writes depends on the binary itself.

## When the Sandbox Directory Is Missing

If the `sandbox/` directory does not exist for a profile, the gateway's behavior depends on the `sandbox.mode` setting:

| Mode | When sandbox directory is missing | When sandbox directory exists |
|------|-----------------------------------|-------------------------------|
| `"strict"` (default) | Gateway **refuses to start** | Normal sandbox |
| `"current"` | Gateway uses the **current working directory** as the sole writable root; path validation remains active | Normal sandbox (same as `strict`) |
| `"unsafe"` | Gateway starts **without** a sandbox; CWD confinement and path validation are disabled | Normal sandbox |

### Gateway Refuses to Start Without a Sandbox (Strict Mode)

By default (`sandbox.mode: "strict"`), the gateway **refuses to start** when the sandbox directory is missing. This prevents a degraded security state where CWD confinement and path validation are silently disabled. The error message includes the expected sandbox path and instructions to fix the issue:

- **Re-run `chai init`** — This recovers the sandbox directory for existing profiles. Other profile files are not modified.
- **Set `sandbox.mode: "current"`** — This uses the current working directory as the sandbox root. Path validation and CWD confinement remain active. This is useful when running the gateway from a project directory.
- **Set `sandbox.mode: "unsafe"`** — This explicitly opts in to running without a sandbox. The gateway will start and log a warning that CWD confinement and path validation are disabled. This should only be used when you intentionally do not want a sandbox.

```json
{
  "sandbox": {
    "mode": "current"
  }
}
```

```json
{
  "sandbox": {
    "mode": "unsafe"
  }
}
```

### Recovering a Deleted Sandbox

If you accidentally delete the `sandbox/` directory, running `chai init` will re-create it and seed template files. Existing files in the profile are never modified by `chai init`.

## CWD Restriction

When a write-path tool runs, the executor sets the command's working directory to the sandbox root. This prevents binaries from writing to implicit CWD-relative locations outside the authorized tree.

## Try It

The journey [Desktop — start/stop gateway and detection](../journey/03-desktop-start-stop-gateway.md) exercises the desktop app's gateway management, including how the sandbox directory is laid out under `~/.chai/profiles/<name>/`.
