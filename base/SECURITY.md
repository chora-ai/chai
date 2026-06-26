# Security

Known security considerations and vulnerabilities in Chai's security model. This document is intended for operators evaluating Chai's security posture and will be summarized in release notes.

## Threat Model

Chai's security boundary is between the **agent** and the **host system**. The agent operates within constraints enforced by the executor; the user (and other processes on the host) are trusted. Specifically:

- The **agent** cannot be trusted. It will attempt to access files and execute operations outside its intended boundaries if the model produces such requests.
- The **user** is trusted. They create symlinks, set environment variables, and manage the host system.
- Other **processes** on the host are trusted. They can modify the filesystem between validation and execution.

The sandbox model defends against the agent, not against user error, compromised host processes, or privilege escalation by external actors.

## Gateway Authentication

The gateway exposes a WebSocket interface for the desktop app and other clients. Two authentication modes control who can connect:

| Mode | Behavior |
|------|----------|
| `none` (default) | No shared secret. Only safe when binding to loopback (`127.0.0.1` or `::1`). |
| `token` | Requires `connect.auth.token` to match a configured secret (from `CHAI_GATEWAY_TOKEN` env or `gateway.auth.token` in config). |

**Loopback enforcement** — The gateway refuses to start when binding to a non-loopback address without token auth. This prevents accidental exposure on network interfaces.

## Device Pairing

Clients authenticate to the gateway using an Ed25519 challenge-response protocol:

1. **Challenge** — On WebSocket connection, the gateway sends a `connect.challenge` event with a random nonce and timestamp.
2. **Sign** — The client signs a canonical payload (device id, client id, role, scopes, timestamp, nonce) using its Ed25519 private key and sends a `connect` message with the signature and public key.
3. **Verify** — The gateway verifies the nonce matches the challenge, then validates the signature using `ed25519_dalek::VerifyingKey::verify_strict`.
4. **Pairing check** — If the device is already paired (found in `paired.json` by device id), the gateway returns the existing device token. If the device is new and provides a valid gateway token, the gateway auto-pairs it: generates a new UUID device token, stores the pairing entry, and persists to `paired.json`. If the device is new and has no gateway token, the connection is rejected.
5. **Reconnection** — Subsequent connections can authenticate with just the device token (no re-signing needed).

**Device identity** — Each profile has its own Ed25519 keypair stored in `device.json`. The device id is the first 16 characters of the base64-encoded public key. The private key is stored in plaintext JSON with default filesystem permissions.

## Execution Security

### Binary Allowlist

Each skill's `tools.json` declares which binaries and subcommands may run. Only (binary, subcommand) pairs in the allowlist are executed. This constrains the **action dimension** — what the agent can do.

### No Shell Execution

The executor passes arguments directly to `Command::new()` + `.args()`, which invokes `execvp` without a shell. Shell metacharacters (`;`, `|`, `$(...)`, backticks) in parameter values are treated as literal strings, not executed. This eliminates shell injection as an attack vector for the primary execution path.

**Exception** — Skill scripts (`resolveCommand.script`, `postProcess.script`) are run via `sh`, but script names are validated against path traversal (`..`, `/`, `\` are rejected), and the resolved path must be within the skill's `scripts/` directory. Arguments are passed as separate array elements to `sh`, preserving the no-shell-injection property.

### Deny Patterns

The `denyPattern` field on execution-spec arguments enforces semantic constraints that the JSON Schema cannot express. Before command execution, the resolved parameter value is checked against the pattern; if it matches, the tool call is rejected — the command is never spawned.

Bundled skills use deny patterns for:
- **Branch protection** — `git_push` and `git_branch_delete` reject operations on `main` and `release/*` branches. The `denyResolveCommand` mechanism resolves the current branch even when the agent omits the branch parameter, and `denyAlwaysResolve` ensures the check always runs.
- **Bundled skill protection** — `skills_delete` rejects deletion of bundled skill names.

### Skill Lock File Integrity

Each profile has a per-profile `skills.lock` that pins skill packages to exact content hashes. At gateway startup, the lock file is verified:

| Mode | Behavior |
|------|----------|
| `strict` (default) | Gateway refuses to start when any pinned skill's active version doesn't match the locked hash. |
| `warn` | Logs a warning and continues. |

This prevents tampering with skill definitions (including allowlists, deny patterns, and sandbox annotations) between gateway restarts. An agent that modifies a skill's `tools.json` within the sandbox will not change the pinned version used by the gateway.

## Sandbox Defense

The sandbox enforces filesystem boundaries through three layers:

1. **Runtime path-like value check** — Unannotated `positional` and `flag` parameters are inspected at runtime. Values matching a path-like pattern are rejected: absolute paths (`/etc/passwd`), home-relative paths (`~/.ssh/id_rsa`), directory traversal (`../../etc/passwd`), `file://` URLs (`file:///etc/passwd`), and `.git/` directory access (`.git/config`, `project/.git/refs`).
2. **CWD confinement** — When no `workingDir` argument is present and no sandbox-validated path provides a working directory, the executor sets `Command::current_dir()` to the sandbox root. When a sandbox-validated `workingDir` or path argument resolves to a specific directory, that directory takes precedence. When no sandbox exists, no CWD override is applied — the process inherits the gateway's working directory. By default (`sandbox.mode: "strict"`), the gateway refuses to start without a sandbox directory; operators can set `sandbox.mode` to `"current"` (CWD as writable root) or `"unsafe"` (no sandbox) to start without one (see [spec/CONFIGURATION.md](spec/CONFIGURATION.md)).
3. **Sandbox path validation** — Parameters annotated with `readPath` or `writePath` are validated against the sandbox's writable roots (canonicalized, prefix-checked) and checked for `.git/` directory access. The `.git/` directory is excluded from writes regardless of whether the path falls within a writable root. The command is never spawned if validation fails. Parameters annotated with `unsafePath` bypass all validation and the runtime path-like value check; no bundled skill uses `unsafePath`, and operators should review startup warnings before enabling skills that do.

### Read-Path Validation

`readPath`-annotated parameters are validated against the same writable roots as `writePath`. Agents can only read within directories they could also write to. There is no separate "readable roots" concept — the readable surface is aligned with the writable surface.

### Symlink-as-Authorization Model

The sandbox uses symlinks in the sandbox directory as authorization grants:

- **Agents cannot create symlinks.** The `ln` binary must never appear in any skill's allowlist. If a skill accidentally allowlists `ln`, the entire authorization model is compromised.
- **Symlinks are scanned at construction time.** The `WriteSandbox` scans direct children of the sandbox directory for symlinks when the gateway starts. The writable root set is frozen at construction time — adding or removing symlinks while the gateway is running has no effect until the gateway is restarted.
- **Only direct children are scanned.** Symlinks deeper in the directory tree are not scanned. This keeps the authorization surface flat and auditable.

## Agent Isolation

Each agent (orchestrator and workers) has its own context directory, skill configuration, system context, and tool list. Workers receive only their own `AGENT.md` and worker-specific skills — no orchestrator identity, no `delegate_task` tool, no worker roster. This prevents privilege escalation through role confusion.

The sandbox is **per-profile**, shared by the orchestrator and all workers. There is no per-agent sandbox isolation. The three-layer defense (skill schema, allowlist, sandbox) mitigates the risk of a shared sandbox.

## Channel Security

### Telegram

Bot tokens are resolved from `TELEGRAM_BOT_TOKEN` env or `channels.telegram.botToken` in config. Webhook secrets follow the same pattern. No E2EE — Telegram bot API does not support it.

### Matrix (Experimental)

- **E2EE** — Megolm/Olm encryption for encrypted rooms via `matrix-sdk`. Session keys are stored in the profile-local SQLite store.
- **SAS verification** — Interactive device verification via gateway HTTP routes (`/matrix/verification/*`). Requires Bearer token auth or loopback bind.
- **Room allowlist** — Optional restriction to listed room ids. Unset means all joined rooms.
- **Self-hosted recommendation** — For privacy-sensitive deployments, a self-hosted homeserver keeps metadata local.

### Signal (Experimental)

- **E2EE** — Signal protocol encryption, dependent on regular receipt of messages.
- **Key material** — signal-cli account keys are stored on the gateway host. The operator must keep signal-cli updated and protect key material on disk.

### Provider Privacy

- **Ollama** (default) — Local inference. Data stays on the machine.
- **NVIDIA NIM** — Hosted API; **not privacy-preserving**. All data sent to NVIDIA servers. Warning logged at startup when NIM is the default provider.
- **Other OpenAI-compatible providers** — Data leaves the machine. Operators must evaluate each provider's data handling.

## Secrets Management

Secrets are resolved from multiple sources with consistent precedence:

| Source | Precedence | Notes |
|--------|-----------|-------|
| Shell environment variables | Highest | Always override |
| Profile `.env` file | Medium | Loaded at startup; only sets unset variables |
| `config.json` fields | Lowest | Convenience for non-sensitive values |

Supported secrets include: gateway auth token (`CHAI_GATEWAY_TOKEN`), Telegram bot token (`TELEGRAM_BOT_TOKEN`), Matrix credentials (`MATRIX_ACCESS_TOKEN`, `MATRIX_PASSWORD`), and provider API keys. The `status` WebSocket payload never reveals which source supplied a secret.

## Known Vulnerabilities

### `file://` URL Information Disclosure (Mitigated)

**Status**: Mitigated (runtime heuristic rejects `file://` prefixes).

The runtime path-like value check rejects values starting with `file://`. This prevents the agent from using `file://` URLs to reference files outside the sandbox via tools that accept URL parameters (e.g., `git clone file:///home/user/private-repo`).

**Residual risk**: If a parameter is annotated with `unsafePath: true`, the `file://` check is bypassed along with all other validation. This is by design — `unsafePath` is the explicit escape hatch — but it means a skill with `unsafePath` on a URL parameter could be used to access `file://` URLs.

### `.git/` Directory Write Bypass (Mitigated)

**Status**: Mitigated (sandbox validation and runtime heuristic reject `.git/` directory writes).

The `files` skill could write to `.git/` directories within the sandbox, completely bypassing the `git` skill's branch protection and allowlist restrictions. This undermined the defense-in-depth model where git state is only modifiable through the `git` skill's constrained tools. Attack vectors included branch rewriting (writing to `.git/refs/heads/main`), hook injection (writing to `.git/hooks/`), config manipulation, and object injection.

The fix adds two layers of protection:
1. **`WriteSandbox::validate()`** rejects any write target whose canonical path contains a `.git` path component, before the writable-root prefix check.
2. **`check_path_like_value()`** rejects values that target a `.git/` directory (starting with `.git/` or containing `/.git/` as a component) in unannotated `positional` and `flag` parameters.

**Residual risk**: If a parameter is annotated with `unsafePath: true`, both the sandbox `.git/` exclusion and the runtime heuristic are bypassed. This is by design — `unsafePath` is the explicit escape hatch. No bundled skill uses `unsafePath`.

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

**Mitigation**: Operators who consider this a risk can set `maxToolLoopsPerTurn` to limit the number of probes per turn (omitted = no limit).

### Relative Path CWD Confinement (Accepted, Defense-in-Depth)

**Status**: Accepted (CWD confinement is defense-in-depth, not a primary boundary).

Layer 2 (CWD confinement to sandbox root) ensures that relative paths in unannotated parameters resolve within the sandbox. This is a defense-in-depth measure: the primary boundary is Layer 1 (heuristic rejection of path-like values). CWD confinement closes the gap for values that don't match the heuristic but could still resolve outside the sandbox (e.g., `etc/passwd` without a leading `/`).

**Residual risk**: When no sandbox exists (only possible when `sandbox.mode` is `"unsafe"` and the sandbox directory is missing), CWD confinement is disabled entirely — tool executions inherit the gateway's working directory. By default, the gateway refuses to start without a sandbox directory (see `sandbox.mode` in [spec/CONFIGURATION.md](spec/CONFIGURATION.md)). Operators who explicitly set `sandbox.mode: "unsafe"` accept the risk of running without CWD confinement; the gateway logs a warning at startup in this case. Operators who set `sandbox.mode: "current"` get CWD confinement with the current working directory as the sole writable root — less restrictive than a profile sandbox, but still enforcing path validation boundaries. Between the time the executor validates a parameter and the time the binary accesses the path, the filesystem can change (a directory could be symlinked or renamed). For annotated `readPath`/`writePath` parameters, this is mitigated by canonical path substitution — the binary receives the already-resolved absolute path. For unannotated parameters, no substitution occurs; the binary resolves the path against CWD at execution time. The window for exploitation is narrow and requires concurrent host-side filesystem changes.

### Symlink-as-Authorization Revocation Requires Restart (Accepted)

**Status**: Accepted (by design, with construction-time scanning).

The `WriteSandbox` scans direct-child symlinks at gateway construction time and stores their canonicalized targets in a frozen writable root set. Changes to the sandbox directory's symlinks — both additions and removals — are not detected while the gateway is running. Adding a symlink does not grant access until restart; removing a symlink does not revoke access until restart.

**Mitigation**: Restarting the gateway picks up symlink changes immediately. The flat, direct-child-only scan surface makes it easy to audit the current authorization state by listing the sandbox directory.

### Shared Sandbox Across Agents (Accepted, Deferred)

**Status**: Accepted (shared sandbox, per-agent isolation deferred).

The sandbox is per-profile, shared by the orchestrator and all workers. There is no per-agent sandbox isolation. If a worker agent writes a file, the orchestrator agent can read and modify it (and vice versa). The three-layer defense mitigates the risk: skill schema constrains what the model knows, the allowlist constrains what operations are possible, and the sandbox constrains where writes land.

### `unsafePath` Parameters (Accepted, Auditable)

**Status**: Accepted (escape hatch with startup warning).

Parameters annotated with `unsafePath: true` bypass all sandbox validation and the runtime path-like value check. The gateway logs a startup warning for each `unsafePath` parameter in enabled skills, making escape hatches visible at the operator level. No current bundled skill parameter uses `unsafePath`.

**Residual risk**: Third-party skills may use `unsafePath` without adequate justification. Operators should review startup warnings and audit any skill that uses `unsafePath`.

### Binary-Mediated Writes (Accepted)

**Status**: Accepted (by design, allowlist-constrained).

When the model provides a semantic identifier (not a path) and the binary resolves the write target internally, the sandbox does not apply. Security depends on the binary rejecting traversal and confining writes. The allowlist controls which binaries are available, and the deny pattern mechanism can enforce additional constraints on parameters.

### Device Scopes and Roles Declared but Not Enforced (Accepted, Deferred)

**Status**: Accepted (declared but not yet enforced).

The device pairing protocol includes `role` and `scopes` fields that are stored in `paired.json` but never checked during subsequent operations. All authenticated devices currently have the same access regardless of their declared role or scopes. This is a placeholder for future access control.

### Secrets Stored in Plaintext (Accepted)

**Status**: Accepted (host-side responsibility).

Profile-local secrets — the Ed25519 private key in `device.json`, device tokens in `paired.json`, and any credentials in `.env` or `config.json` — are stored on disk in plaintext with default filesystem permissions. This is a host-side concern within the current threat model (the user and host processes are trusted). Operators on shared or multi-user systems should set restrictive file permissions (e.g., `chmod 600`) on these files.

## Out of Scope

The following are explicitly outside Chai's current security model:

- **OS-level sandboxing** (containers, seccomp, landlock) — Userspace path validation is sufficient for the current threat model. Kernel-level enforcement is a possible future direction.
- **Resource exhaustion** — The agent can write arbitrarily large files within the sandbox, create arbitrarily many files, and consume unbounded tool output (subject to `maxOutputLines` and `maxToolLoopsPerTurn`). There are no disk quotas or memory limits enforced by the executor.
- **Rate limiting** — The gateway does not limit concurrent WebSocket connections, message rates, or agent turn frequency. An authenticated client can trigger unlimited LLM API calls, creating a cost DoS vector against paid providers.
- **TLS termination** — The gateway binds plain HTTP/WebSocket. TLS is the operator's responsibility (e.g., reverse proxy). Binding to non-loopback without TLS exposes the auth token and all data in cleartext.
- **WebSocket origin validation** — The gateway does not check the `Origin` header on WebSocket upgrades. On loopback this is mitigated by same-origin policy; on non-loopback deployments, cross-site WebSocket hijacking is possible without additional network controls.
- **Encryption at rest** — Session files are persisted to disk as plain JSON (see [spec/SESSIONS.md](spec/SESSIONS.md)), making conversation history readable to any process with filesystem access. Configuration files, device keys, and pairing tokens are also stored on disk without encryption. See "Secrets Stored in Plaintext" above and "Encryption at rest for session data" in Future Directions.
- **`CHAI_BIN` environment variable** — The `CHAI_BIN` env var overrides the `chai` binary path used by the executor. This is set by the user (or the launch environment), not the agent. If the gateway is launched with `CHAI_BIN` pointing to a compromised binary, all `chai` tool calls are subverted. This is a host-side concern, not an agent-facing vulnerability.
- **Session isolation across channels** — Sessions are implicitly isolated by `(channel_id, conversation_id)` keys, but WebSocket clients with valid authentication can interact with any session regardless of channel. There is no per-client or per-channel session access control.

## Future Directions

These are potential security enhancements that are not yet implemented:

- **Tool call approval** — Optional human-in-the-loop approval before tool execution. Tracked in [epic/TOOL_APPROVAL.md](epic/TOOL_APPROVAL.md).
- **Encryption at rest for session data** — Encrypt session files on disk so conversation history is not readable without the gateway's credentials. Session files are currently stored as plain JSON under the profile's `agents/<agentId>/sessions/` directory (see [spec/SESSIONS.md](spec/SESSIONS.md)). Relevant for multi-user or shared-host deployments.
- **Per-agent sandbox isolation** — Separate sandbox boundaries for each agent within a profile.
- **Enforced device scopes and roles** — Use the existing `role` and `scopes` fields from device pairing for authorization decisions.
- **Rate limiting and connection throttling** — Limit WebSocket connections, message rates, and agent turn frequency.

## Related Documents

| Document | Purpose |
|----------|---------|
| [spec/SANDBOX.md](spec/SANDBOX.md) | Behavioral contract for the sandbox model |
| [spec/TOOLS_SCHEMA.md](spec/TOOLS_SCHEMA.md) | `writePath`, `readPath`, `unsafePath`, `denyPattern` field definitions |
| [adr/WRITE_SANDBOX.md](adr/WRITE_SANDBOX.md) | Architectural decision for the sandbox model |
| [adr/AGENT_ISOLATION.md](adr/AGENT_ISOLATION.md) | Architectural decision for agent isolation |
| [spec/CONFIGURATION.md](spec/CONFIGURATION.md) | Gateway auth modes and secrets resolution |
| [spec/PROFILES.md](spec/PROFILES.md) | Profile directory structure and trust-sensitive resources |
| [spec/SESSIONS.md](spec/SESSIONS.md) | Session persistence, storage layout, and management |
| [epic/TOOL_APPROVAL.md](epic/TOOL_APPROVAL.md) | Draft: human-in-the-loop tool approval |
