---
status: accepted
---

# Write Sandbox

Per-profile path boundary enforcement for skill tools with `writePath`-annotated arguments, using symlink-as-authorization, canonical-path validation, and a secure-by-default runtime path-like value check.

## Context

Chai's allowlist controls *what* commands a skill tool may run (the action dimension). However, an allowlisted binary with a filesystem path argument could write anywhere the process can reach. A tool like `files_write_file` accepting a `path` parameter had no spatial boundary — the model could potentially write to `/etc/passwd` or any other location accessible to the process. The allowlist alone was insufficient: it enforced command identity but not filesystem location. Safe write-capable skills required both the action dimension (allowlist) and the spatial dimension (sandbox) composed together.

## Decision

A **per-profile write sandbox** at `<profileRoot>/sandbox/` restricts where skill tools may write when parameters are marked `writePath` in `tools.json`:

- **Path validation before execution.** For each `writePath`-annotated argument, the executor canonicalizes the resolved path and validates it against writable roots before spawning the command. If validation fails, the tool call is rejected — the command never runs.
- **Writable roots.** The sandbox directory itself is always a writable root. For each symlink that is a direct child of the sandbox directory, the canonicalized target of that symlink is also a writable root. Only direct children are scanned — symlinks deeper in the tree are not.
- **Symlink-as-authorization.** Agents cannot create symlinks because `ln` is never allowlisted in any skill. Symlink creation is exclusively a user action. Each symlink is declarative authorization: creating one grants write access, removing one revokes it on the next gateway restart (the writable root set is frozen at construction time). The filesystem IS the policy — no configuration file or capability tier.
- **Binary-mediated writes are out of scope.** When the model provides a semantic identifier and the binary resolves the write target internally (e.g., `chai skill write-skill-md` takes `skill_name`), the executor never sees a filesystem path and cannot validate it. Security for binary-mediated writes depends on the binary itself. The allowlist controls *which* binary-mediated operations are available; *where* enforcement is the binary's responsibility.
- **CWD restriction.** When no `workingDir` argument is present and no sandbox-validated path provides a working directory, the executor sets `Command::current_dir()` to the sandbox root. This ensures relative paths in unannotated parameters resolve within the sandbox boundary. When a sandbox-validated `workingDir` or path argument resolves to a specific directory, that directory takes precedence. When no sandbox exists, no CWD override is applied — the process inherits the gateway's working directory.
- **Missing sandbox directory.** When the sandbox directory does not exist, there are no writable roots and all `writePath` validations fail. Skills without `writePath` arguments are unaffected.
- **The sandbox is per-profile, shared by all agents.** The orchestrator and all workers within a profile share one sandbox. Per-agent sandbox subdirectories are deferred until a concrete use case demonstrates the need.
- **Secure-by-default runtime path-like value check.** Unannotated `positional` and `flag` parameters are subject to a runtime check that rejects path-like values (absolute paths starting with `/`, home-relative paths starting with `~`, `file://` URLs, and paths containing `..` traversal). This closes the vulnerability where unannotated parameters allowed unrestricted filesystem access. Parameters that legitimately carry path-like values must be annotated with `readPath: true` or `writePath: true`. Parameters that intentionally need unrestricted access must be annotated with `unsafePath: true` (which triggers a startup warning).

## Alternatives Considered

| Alternative | Why not |
|-------------|---------|
| **Allowlist only** (no sandbox, prior state) | An allowlisted binary can write anywhere. Path-argument tools have no spatial boundary. |
| **OS-level sandboxing** (containers, seccomp, landlock) | Significantly more complex, platform-specific, and harder to reason about. Userspace path validation is sufficient for the current threat model. Kernel-level enforcement is a possible follow-on. |
| **Recursive symlink scanning** (symlinks inside subdirectories also grant access) | Keeps the authorization surface flat and auditable. Direct children only make it easy to see exactly what is authorized by listing the sandbox directory. |
| **Per-agent sandbox isolation** | No concrete use case yet. The three-layer model (skill schema constrains what the model knows, allowlist constrains what operations run, sandbox constrains where writes land) mitigates the risk of a shared sandbox. Deferred. |
| **Capability-tier or config-based authorization** (instead of symlinks) | Adds config surface and a separate authorization mechanism. Symlinks are simpler, already understood, and have built-in revocation (remove the link). No new config keys or formats needed. |
| **Separate writable roots config file** | Another file to manage and keep in sync. The symlink approach uses an existing OS primitive and makes authorization visible via `ls -l`. |
| **Validate-all-by-default** (all positional/flag params sandbox-validated) | Requires every non-path parameter (patterns, refs, names, URLs) to carry an explicit opt-out. The runtime heuristic + CWD approach achieves equivalent security with fewer annotations — most parameters need nothing at all. |
| **Opt-in `readPath`/`writePath` only** (prior state) | The gate is open by default. Omitting annotations is a vulnerability, not a safe state. The `skills` skill's `skill_name` parameter had no annotation and allowed path traversal (AUDIT_SKILLS item 20). |

## Consequences

- **Write-path tools are mechanically confined.** The model cannot direct writes outside authorized roots, regardless of the path string it provides. Traversal (`..`), symlinks within the target path, and CWD-relative paths are all resolved and checked.
- **Authorization is explicit and auditable.** Listing the sandbox directory shows exactly which external directories the agent may write to. No hidden config — the filesystem is the source of truth.
- **Binary-mediated writes require trusting the binary.** The sandbox does not apply to semantic-id writes. Skill authors must ensure their binaries reject traversal and confine writes.
- **Read-path validation reuses the sandbox.** `readPath` arguments are validated against the same writable roots, so agents can only read within directories they could also write to. This keeps the readable surface aligned with the writable surface.
- **The sandbox is shared across agents within a profile.** If stronger isolation between agents is needed in the future, the design can be extended with per-agent subdirectories.
- **Unannotated parameters reject path-like values by default.** The runtime check catches absolute paths, home-relative paths, `file://` URLs, and directory traversal in parameters without `readPath`/`writePath`/`unsafePath` annotations. The CWD defaults to the sandbox root, confining relative paths. Together, these make the default state safe without requiring skill authors to annotate every parameter.
- **`unsafePath` parameters are visible at startup.** The gateway logs a warning for each `unsafePath` parameter in enabled skills, making escape hatches auditable.

## References

- [spec/SANDBOX.md](../spec/SANDBOX.md) — Behavioral contract for the sandbox model.
- [spec/TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) — `writePath` and `readPath` field definitions in `tools.json`.
