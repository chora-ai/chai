# Security

Known security considerations and vulnerabilities in Chai's security model. This document is intended for operators evaluating Chai's security posture and will be summarized in release notes.

## Threat Model

Chai's security boundary is between the **agent** and the **host system**. The agent operates within constraints enforced by the executor; the user (and other processes on the host) are trusted. Specifically:

- The **agent** cannot be trusted. It will attempt to access files and execute operations outside its intended boundaries if the model produces such requests.
- The **user** is trusted. They create symlinks, set environment variables, and manage the host system.
- Other **processes** on the host are trusted. They can modify the filesystem between validation and execution.

The sandbox model defends against the agent, not against user error, compromised host processes, or privilege escalation by external actors.

## Three-Layer Sandbox Defense

The sandbox enforces filesystem boundaries through three layers:

1. **Runtime path-like value check** — Unannotated `positional` and `flag` parameters are inspected at runtime. Values matching a path-like pattern are rejected: absolute paths (`/etc/passwd`), home-relative paths (`~/.ssh/id_rsa`), directory traversal (`../../etc/passwd`), and `file://` URLs (`file:///etc/passwd`).
2. **CWD confinement** — The executor sets `Command::current_dir()` to the sandbox root for all tool executions, so relative paths in unannotated parameters resolve within the sandbox. (TODO: Is this still accurate? Sandbox root rather than current directory?)
3. **Sandbox path validation** — Parameters annotated with `readPath` or `writePath` are validated against the sandbox's readable and writable roots (canonicalized, prefix-checked).

## Known Vulnerabilities

### `file://` URL Information Disclosure (Mitigated)

**Status**: Mitigated (runtime heuristic rejects `file://` prefixes).

The runtime path-like value check rejects values starting with `file://`. This prevents the agent from using `file://` URLs to reference files outside the sandbox via tools that accept URL parameters (e.g., `git clone file:///home/user/private-repo`).

**Residual risk**: If a parameter is annotated with `unsafePath: true`, the `file://` check is bypassed along with all other validation. This is by design — `unsafePath` is the explicit escape hatch — but it means a skill with `unsafePath` on a URL parameter could be used to access `file://` URLs.

### `sideRead` File Disclosure Within Sandbox (Accepted)

**Status**: Accepted (by design, with traversal protection).

The `sideRead` execution spec feature reads a file from disk and appends its contents to the tool result. The `filename` field in `sideRead` is validated against traversal (`..`, `/`, `\`), and the `pathParam` value is already validated against the sandbox by a `readPath` annotation. This means `sideRead` can only surface files within the sandbox boundary.

However, `sideRead` does not apply the same path-like value check that unannotated parameters receive. This is acceptable because:

1. The `filename` is a static value from the skill descriptor (not agent-supplied).
2. The `pathParam` is already sandbox-validated via `readPath`.
3. The derived path `<pathParam>/<filename>` is within the sandbox by construction.

### Exit Code Information Leak (Accepted)

**Status**: Accepted (low severity, inherent to command execution).

The `successExitCodes` field allows non-zero exit codes to be treated as success. An agent could infer filesystem state from exit codes: a binary that exits with code 0 when a file exists and code 1 when it doesn't reveals whether the file exists, even if the file's contents are not returned. This is an inherent property of command execution and cannot be fully mitigated without eliminating all filesystem-interacting commands.

**Mitigation**: Operators who consider this a risk can reduce `maxToolLoopIterations` (default 100) to limit the number of probes per turn.

### Relative Path CWD Confinement (Accepted, Defense-in-Depth)

**Status**: Accepted (CWD confinement is defense-in-depth, not a primary boundary).

Layer 2 (CWD confinement to sandbox root) ensures that relative paths in unannotated parameters resolve within the sandbox. This is a defense-in-depth measure: the primary boundary is Layer 1 (heuristic rejection of path-like values). CWD confinement closes the gap for values that don't match the heuristic but could still resolve outside the sandbox (e.g., `etc/passwd` without a leading `/`).

**Residual risk**: Between the time the executor validates a parameter and the time the binary accesses the path, the filesystem can change (a directory could be symlinked or renamed). For annotated `readPath`/`writePath` parameters, this is mitigated by canonical path substitution — the binary receives the already-resolved absolute path. For unannotated parameters, no substitution occurs; the binary resolves the path against CWD at execution time. The window for exploitation is narrow and requires concurrent host-side filesystem changes.

### Symlink-as-Authorization Model (Accepted)

**Status**: Accepted (by design, with constraints).

The sandbox uses symlinks in the sandbox directory as authorization grants. Creating a symlink grants the agent write access to its target; removing it revokes access. This model has inherent properties that operators should understand:

- **Agents cannot create symlinks.** The `ln` binary must never appear in any skill's allowlist. If a skill accidentally allowlists `ln`, the entire authorization model is compromised.
- **Symlinks are scanned at construction time.** The `WriteSandbox` scans direct children of the sandbox directory for symlinks when it is constructed. If a symlink is added or removed while the gateway is running, it will not take effect until the gateway is restarted. Validation happens at execution time against the constructed root set.
- **Only direct children are scanned.** Symlinks deeper in the directory tree are not scanned. This keeps the authorization surface flat and auditable.

### Shared Sandbox Across Agents (Accepted, Deferred)

**Status**: Accepted (shared sandbox, per-agent isolation deferred).

The sandbox is per-profile, shared by the orchestrator and all workers. There is no per-agent sandbox isolation. If a worker agent writes a file, the orchestrator agent can read and modify it (and vice versa). The three-layer defense mitigates the risk: skill schema constrains what the model knows, the allowlist constrains what operations are possible, and the sandbox constrains where writes land.

### `unsafePath` Parameters (Accepted, Auditable)

**Status**: Accepted (escape hatch with startup warning).

Parameters annotated with `unsafePath: true` bypass all sandbox validation and the runtime path-like value check. The gateway logs a startup warning for each `unsafePath` parameter in enabled skills, making escape hatches visible at the operator level. No current bundled skill parameter uses `unsafePath`.

**Residual risk**: Third-party skills may use `unsafePath` without adequate justification. Operators should review startup warnings and audit any skill that uses `unsafePath`.

## Out of Scope

The following are explicitly outside Chai's current security model:

- **OS-level sandboxing** (containers, seccomp, landlock) — Userspace path validation is sufficient for the current threat model. Kernel-level enforcement is a possible future direction.
- **Resource exhaustion** — The agent can write arbitrarily large files within the sandbox, create arbitrarily many files, and consume unbounded tool output (subject to `maxOutputLines` and `maxToolLoopIterations`). There are no disk quotas or memory limits enforced by the executor.
- **Binary-mediated writes** — When the agent provides a semantic identifier (not a path) and the binary resolves the write target internally, the sandbox does not apply. Security depends on the binary rejecting traversal and confining writes. The allowlist controls which binaries are available.
- **`CHAI_BIN` environment variable** — The `CHAI_BIN` env var overrides the `chai` binary path used by the executor. This is set by the user (or the launch environment), not the agent. If the gateway is launched with `CHAI_BIN` pointing to a compromised binary, all `chai` tool calls are subverted. This is a host-side concern, not an agent-facing vulnerability.

## Related Documents

| Document | Purpose |
|----------|---------|
| [spec/SANDBOX.md](spec/SANDBOX.md) | Behavioral contract for the sandbox model |
| [spec/TOOLS_SCHEMA.md](spec/TOOLS_SCHEMA.md) | `writePath`, `readPath`, and `unsafePath` field definitions |
| [adr/WRITE_SANDBOX.md](adr/WRITE_SANDBOX.md) | Architectural decision for the sandbox model |
