# Write Sandbox

Each profile has a **write sandbox** that restricts where skill tools may write. The sandbox enforces the **spatial dimension** of tool safety — the allowlist controls *what* runs, the sandbox controls *where* writes go. Both layers are needed for safe write-capable skills.

## What the Sandbox Does

When a skill tool has a **`writePath`** parameter (marked in its `tools.json`), the executor validates the resolved path against writable roots **before** running the command. If the path is outside all writable roots, the tool call is rejected with an error — the command is never spawned.

This prevents a model from using a write tool to modify files outside the authorized tree (e.g. turning `cat` into `cat > /etc/passwd`).

## Sandbox Location

The sandbox directory lives under each profile:

```text
~/.chai/profiles/<name>/sandbox/
```

**`chai init`** creates a `sandbox/` for each default profile, seeded with template files (`AGENTS.md`, `README.md`). The sandbox is per-profile — all agents within a profile share the same sandbox. Re-running `chai init` preserves existing sandbox files.

## Writable Roots

Writable roots are computed at gateway startup from the sandbox directory:

1. The **sandbox directory itself** is always a writable root
2. Each **direct-child symlink** in the sandbox is resolved (`canonicalize`) and its target becomes an additional writable root

```text
~/.chai/profiles/assistant/sandbox/    ← writable root #1 (always)
  my-project/                          ← writable (under root #1)
  linked-repo → ~/Code/my-repo/       ← target becomes writable root #2
  workspace → ../agents/orchestrator/  ← target becomes writable root #3
  notes.txt                            ← writable (under root #1)
```

Only **direct children** of `sandbox/` are scanned. Symlinks inside subdirectories are not followed. This keeps the authorization surface explicit and auditable.

## Granting Write Access

Agents cannot create symlinks — the `ln` binary is never allowlisted in any skill. Symlink creation is exclusively a user action, so each symlink is **declarative authorization**:

```bash
# Grant access to a code repository
ln -s ~/Code/my-repo ~/.chai/profiles/assistant/sandbox/my-repo

# Grant access to the agent's own context directory
ln -s ~/.chai/profiles/assistant/agents/orchestrator ~/.chai/profiles/assistant/sandbox/workspace
```

Removing the symlink revokes access. No configuration file, no capability tier — the filesystem is the policy.

## How Path Validation Works

For each `writePath`-annotated argument, before the command runs:

1. **Resolve** the path to a canonical absolute path (resolving `..`, `.`, and symlinks)
2. For new files that don't exist yet, canonicalize the parent directory and append the filename
3. **Check** whether the canonical path starts with at least one writable root
4. If it does, proceed; if not, reject the tool call

This mechanically prevents path traversal, symlink escape, and CWD-based implicit writes.

## What the Sandbox Does Not Cover

**Binary-mediated writes** — where the model provides a semantic identifier and the binary resolves the write target internally — are not subject to sandbox validation. For example, `chai skill write-skill-md` takes a skill name (not a filesystem path) and the binary resolves it to the skills directory. The executor never sees a filesystem path, so sandbox validation cannot apply. Security for binary-mediated writes depends on the binary itself.

## When the Sandbox Directory Is Missing

If the `sandbox/` directory does not exist for a profile, the sandbox has no writable roots and all `writePath` validations fail. Skills that do not use `writePath` args (read-only tools) are unaffected. **`chai init`** creates the directory, so this should not occur in normal use.

## CWD Restriction

When a write-path tool runs, the executor sets the command's working directory to the sandbox root. This prevents binaries from writing to implicit CWD-relative locations outside the authorized tree.


## Try It

The journey [Desktop — start/stop gateway and detection](../journey/04-desktop-start-stop-gateway.md) exercises the desktop app's gateway management, including how the sandbox directory is laid out under `~/.chai/profiles/<name>/`.
