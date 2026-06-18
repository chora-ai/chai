# AUDIT: Configuration Consistency, Structure, and Naming

Cross-cutting audit of Chai's configuration values for consistency, proper placement within configuration objects, and alignment with gateway status. Covers `config.json` on-disk format, `config.rs` types and serde names, the gateway `status` WebSocket payload, the desktop config/status screens, and the supporting spec documents.

---

## Scope

- **`config.json`** on-disk format (per-profile)
- **`crates/lib/src/config.rs`** — types, serde names, resolution helpers
- **`crates/lib/src/gateway/server.rs`** — `status` payload construction
- **`crates/lib/src/gateway/protocol.rs`** — protocol types
- **`crates/desktop/src/app/screens/config.rs`** — desktop config dashboard
- **`crates/desktop/src/app/screens/gateway.rs`** — desktop status dashboard
- **`base/spec/CONFIGURATION.md`** — config spec
- **`base/spec/GATEWAY_STATUS.md`** — status spec
- **`base/spec/PROVIDERS.md`** — provider spec
- **`base/spec/CHANNELS.md`** — channel spec
- **`base/spec/ORCHESTRATION.md`** — orchestration/delegation spec
- **`base/spec/CONTEXT.md`** — context spec
- **`base/spec/PROFILES.md`** — profile spec
- **`base/spec/DESKTOP.md`** — desktop spec
- **`base/FEAT_DESKTOP_JSON.md`** — desktop.json proposal

---

## 1. Serde Naming Consistency

All structs and enums use `#[serde(rename_all = "...")]` consistently. No raw Rust snake_case field names leak into the JSON format.

| Type | `rename_all` | Spot Check | Verdict |
|------|-------------|------------|---------|
| `Config` | `camelCase` | `skills` → `"skills"` | ✅ |
| `SkillsConfig` | `camelCase` | `lock_mode` → `"lockMode"` | ✅ |
| `GatewayConfig` | `camelCase` | *(removed — `unsafe_sandbox` moved to `SandboxConfig.disabled`)* | ✅ |
| `SandboxConfig` | `camelCase` | `disabled` → `"disabled"` | ✅ |
| `GatewayAuthConfig` | `camelCase` | `mode` → `"mode"`, `token` → `"token"` | ✅ |
| `GatewayAuthMode` | `lowercase` | `None` → `"none"`, `Token` → `"token"` | ✅ |
| `ChannelsConfig` | `camelCase` | nested structs use `camelCase` | ✅ |
| `TelegramChannelConfig` | `camelCase` | `bot_token` → `"botToken"`, `webhook_secret` → `"webhookSecret"` | ✅ |
| `MatrixChannelConfig` | `camelCase` | `access_token` → `"accessToken"`, `room_ids` → `"roomIds"` | ✅ |
| `SignalChannelConfig` | `camelCase` | `http_base` → `"httpBase"` | ✅ |
| `SkillLockMode` | `camelCase` | `Strict` → `"strict"`, `Warn` → `"warn"` | ✅ |
| `SkillContextMode` | `camelCase` | `Full` → `"full"`, `ReadOnDemand` → `"readOnDemand"` | ✅ |
| `EndpointType` | `kebab-case` | `Ollama` → `"ollama"`, `OpenaiCompat` → `"openai-compat"` | ✅ |
| `ModelDiscovery` | `camelCase` | `Auto` → `"auto"`, `Lmstudio` → `"lmstudio"`, `Static` → `"static"` | ✅ |
| `AgentRole` | `lowercase` | `Orchestrator` → `"orchestrator"`, `Worker` → `"worker"` | ✅ |
| `AgentDefinition` | `camelCase` | `enabled_skills` → `"enabledSkills"` | ✅ |
| `WorkerConfig` | `camelCase` | `default_provider` → `"defaultProvider"` | ✅ |
| `ProviderDefinition` | `camelCase` | `endpoint_type` → `"endpointType"`, `api_key` → `"apiKey"`, `static_models` → `"staticModels"` | ✅ |

**Verdict:** Serde naming is fully consistent across all configuration types. No issues.

---

## 2. Config Block Structure and Top-Level Shape

### 2.1 Top-Level Keys

**config.json:**
```json
{
  "gateway": { },
  "channels": { },
  "providers": [ ],
  "agents": [ ],
  "skills": {
    "lockMode": "strict"
  }
}
```

**status payload:**
```json
{
  "gateway": { },
  "channels": { },
  "providers": { },
  "sandbox": { },
  "agents": [ ],
  "skills": { }
}
```

The spec and code align on the top-level shape. Every top-level key in config is a structured block — no orphan scalars. The config and status payloads now have symmetric top-level key names: `gateway`, `channels`, `providers`, `sandbox`, `agents`, `skills`.

- **`providers`** is an array in config, a map in status. This is an intentional structural transform — the status keys by provider id for client convenience. **No issue.**
- **`agents`** is an array in both config and status. The status entries mirror the config shape with additional derived fields (`systemContext`, `tools`, `skillsContext`). **No issue.**
- **`skills`** in config holds shared settings (`lockMode`); in status it holds derived runtime data (`packagesDiscovered`, `lockMode`, `lockGeneration`, `lockedSkills`). **No issue.**

---

## 3. Config ↔ Status Field Alignment

### 3.1 `gateway` Block

| config.json | status | Alignment |
|-------------|--------|-----------|
| `gateway.bind` | `gateway.bind` | ✅ Same |
| `gateway.port` | `gateway.port` | ✅ Same |
| `gateway.auth.mode` | `gateway.auth` (string) | ✅ Intentional: status flattens auth to its mode string (no secrets) |
| `gateway.auth.token` | (omitted) | ✅ Intentional: secret redaction |
| `sandbox.disabled` | `sandbox.disabled`, `sandbox.roots` | ✅ Promoted to top-level `sandbox` block |

### 3.2 `channels` Block

| config.json | status | Alignment |
|-------------|--------|-----------|
| `channels.telegram.botToken` | (omitted) | ✅ Secret redaction |
| `channels.telegram.webhookUrl` | `channels.telegram.transport` | ✅ Config stores the URL; status reports the effective transport mode |
| `channels.telegram.webhookSecret` | (omitted) | ✅ Secret redaction |
| `channels.matrix.*` | `channels.matrix.*` (runtime fields) | ✅ Config secrets omitted; runtime state added |
| `channels.signal.httpBase` | `channels.signal.*` (runtime fields) | ✅ |

### 3.3 `providers` Block

| config.json | status | Alignment |
|-------------|--------|-----------|
| `providers[].id` | `providers.<id>` (map key) | ✅ Array → map transform |
| `providers[].endpointType` | `providers.<id>.endpointType` | ✅ Now documented in status spec (was F3, resolved) |
| `providers[].apiKey` | (omitted) | ✅ Secret redaction |
| `providers[].baseUrl` | (omitted) | ✅ Not in status |
| `providers[].defaultModel` | (omitted) | ✅ Not in status |
| `providers[].modelDiscovery` | `providers.<id>.modelDiscovery` | ✅ Now exposed in status (was F4, resolved) |
| `providers[].staticModels` | (omitted) | ✅ Not in status |
| (not in config) | `providers.<id>.models` | ✅ Derived from discovery |

### 3.4 `agents` Block

| config.json | status | Alignment |
|-------------|--------|-----------|
| `agents[].id` | `agents[].id` | ✅ |
| `agents[].role` | `agents[].role` | ✅ |
| `agents[].defaultProvider` | `agents[].defaultProvider` | ✅ (effective/resolved) |
| `agents[].defaultModel` | `agents[].defaultModel` | ✅ (effective/resolved) |
| `agents[].enabledProviders` | `agents[].enabledProviders` | ✅ (orchestrator: array; worker: null) |
| `agents[].enabledSkills` | `agents[].enabledSkills` | ✅ Flat on entry, mirrors config (was F10, resolved) |
| `agents[].contextMode` | `agents[].contextMode` | ✅ Flat on entry, mirrors config (was F10, resolved) |
| `agents[].maxToolLoopsPerTurn` | `agents[].maxToolLoopsPerTurn` | ✅ Orchestrator: value or null; Worker: null |
| `agents[].maxDelegationsPerTurn` | `agents[].maxDelegationsPerTurn` | ✅ Orchestrator: value or null; Worker: null |
| `agents[].maxDelegationsPerSession` | `agents[].maxDelegationsPerSession` | ✅ Orchestrator: value or null; Worker: null |
| `agents[].maxDelegationsPerWorker` | `agents[].maxDelegationsPerWorker` | ✅ Orchestrator: object or null; Worker: null |
| (not in config) | `agents[].systemContext` | ✅ Derived from AGENT.md + skills |
| (not in config) | `agents[].tools` | ✅ Derived from skill descriptors |
| (not in config) | `agents[].skillsContext` | ✅ Derived (renamed from `skillsContextBodies`, was F10) |

### 3.5 `skills` Block

| config.json | status | Alignment |
|-------------|--------|-----------|
| `skills.lockMode` | `skills.lockMode` | ✅ Mirrors config intent in status |
| (not in config) | `skills.packagesDiscovered` | ✅ Derived from disk scan |
| (not in config) | `skills.lockGeneration` | ✅ Derived from lockfile (was F6, resolved) |
| (not in config) | `skills.lockedSkills` | ✅ Derived from lockfile (was F6, resolved) |

---

## 4. Findings

### F1: `gateway.unsafeSandbox` Not in Status Payload — RESOLVED

**Category:** Config ↔ status gap — **Fixed**

`config.json` has `gateway.unsafeSandbox` which controls whether the gateway starts without a sandbox directory. This is an important operational flag, but the `status` payload did not include it. An operator checking status cannot see whether the gateway is running in unsafe mode.

**Resolution:** Promoted to top-level `sandbox` block with `disabled` and `roots` fields. Desktop config and gateway screens show a dedicated Sandbox section.

### F2: `webhookUrl` vs `transport` Semantic Mismatch

**Category:** Naming consistency (informational, no action needed)

In `config.json`, `channels.telegram.webhookUrl` is set to a URL string to enable webhook mode. In the `status` payload, `channels.telegram.transport` reports `"longPoll"` or `"webhook"`. These are complementary but use different naming. The relationship is documented in GATEWAY_STATUS.md. **No issue.**

### F3: `endpointType` Emitted in Status But Not Documented in Spec — RESOLVED

**Category:** Spec ↔ code divergence — **Fixed**

The code in `server.rs` emitted `endpointType` in the `providers.<id>` status object, but GATEWAY_STATUS.md only listed `discovery` and `models`. The desktop `parse_providers_block` already consumed `endpointType`.

**Resolution:** Updated GATEWAY_STATUS.md to include `endpointType` in the `providers` field table.

### F4: `modelDiscovery` Type Not Exposed in Status Payload — RESOLVED

**Category:** Config ↔ status gap — **Fixed**

In `config.json`, `modelDiscovery` is a per-provider enum (`"auto"`, `"lmstudio"`, `"static"`) that controls *how* models are discovered. In the `status` payload, `discovery` was a *boolean* indicating *whether* the provider was in the orchestrator's `enabledProviders` scope. This was misleading: for a provider with `modelDiscovery: "static"`, `discovery: true` implied active polling when in fact the gateway just read the static list from config. The `discovery` boolean conflated two separate concepts — agent-level scope (is this provider in `enabledProviders`?) and provider-level behavior (how are models listed?) — and the scope question is already answered by `agents[].enabledProviders` on the orchestrator row.

**Resolution:** Replaced the `discovery` boolean with a `modelDiscovery` string field on each provider entry in the gateway status payload. The `modelDiscovery` value mirrors the config enum (`"auto"`, `"lmstudio"`, `"static"`), clearly communicating how models were populated. The `discovery` boolean was removed as redundant — whether a provider is in the discovery scope is derivable from `agents[].enabledProviders`. The `models` array is now always emitted directly (empty when out of scope or unreachable, populated otherwise). Updated `server.rs` (status construction), `protocol.rs` (doc comments), `types.rs` (`ProviderStatusInfo`), `gateway.rs` (desktop status display), and `GATEWAY_STATUS.md` (spec).

### F5: Delegation and Session Caps Not in Status Payload — RESOLVED

**Category:** Config ↔ status gap — **Fixed**

The following orchestrator-only fields from `config.json` were not surfaced in the `status` payload:

- `maxToolLoopsPerTurn`
- `maxDelegationsPerTurn`
- `maxDelegationsPerSession`
- `maxDelegationsPerWorker`

The desktop config screen displays these from `config.json` directly. Clients connected to a remote gateway (via the split deployment model) would have no way to discover the effective limits without accessing the config file directly.

**Resolution:** Added these fields to `agents[]` in the status payload. Orchestrator rows include the configured values (or `null` when unset); worker rows use `null`.

### F6: `skills.lockGeneration` and `skills.lockedSkills` Missing from Status — RESOLVED

**Category:** Spec ↔ code divergence — **Fixed**

GATEWAY_STATUS.md specified `lockGeneration` and `lockedSkills` in the `skillPackages` block, but the code never emitted them. Additionally, the top-level block was named `skillPackages` in code/spec but did not mirror any config key.

**Resolution:**
1. Renamed the status block from `skillPackages` to `skills`, creating config↔status symmetry.
2. Added `lockMode` to the `skills` status block (mirrors `config.skills.lockMode`).
3. Implemented `lockGeneration` and `lockedSkills` by reading lockfile metadata at startup into `GatewayState` fields.
4. Updated the desktop to parse and display all five `skills` block fields.

### F7: Matrix Env Var Resolution Not Centralized

**Category:** Pattern inconsistency (low severity) — **Fixed**

Matrix config fields have env override support documented in `MatrixChannelConfig` doc comments, but the resolution logic was scattered between `server.rs` (`matrix_channel_configured`) and the adapter crate. Telegram and Signal have dedicated `resolve_*` helpers in `config.rs`.

**Resolution:**
1. Added `resolve_matrix_homeserver`, `resolve_matrix_access_token`, `resolve_matrix_user`, `resolve_matrix_password`, `resolve_matrix_user_id`, `resolve_matrix_device_id` helpers to `config.rs`, following the Telegram/Signal pattern.
2. Moved `matrix_channel_configured` from `server.rs` to `config.rs`, using the new resolve helpers internally.
3. Updated `matrix.rs` (`connect_matrix_client`) to use the centralized resolve helpers instead of inline `std::env::var` calls.
4. Updated the desktop config screen to use `matrix_channel_configured` and `resolve_matrix_access_token` for accurate "configured" detection (now respects env vars).

### F8: Desktop Config Screen Does Not Show `skills.lockMode` — RESOLVED

**Category:** Display gap — **Fixed**

The desktop config dashboard did not display the `skills.lockMode` value. A new "Skills" section has been added to the config dashboard showing the lock mode.

### F9: Desktop Config Screen Does Not Show `unsafeSandbox` — RESOLVED

**Category:** Display gap — **Fixed**

`sandbox.disabled` is shown in a dedicated Sandbox section on the desktop config dashboard (right column, above Agents) and the desktop status dashboard (right column, above Agents). The status screen also shows `roots` count.

### F10: `enabledSkills` and `contextMode` Unnecessarily Nested Under `skills` in Status — RESOLVED

**Category:** Config ↔ status structural mismatch — **Fixed**

In `config.json`, `enabledSkills` and `contextMode` are flat fields on each `agents[]` entry. In the `status` payload, they were nested under a `skills` sub-object (`agents[].skills.enabledSkills`, `agents[].skills.contextMode`), creating a structural mismatch. The `skills` nesting grouped them with `skillsContextBodies`, but `enabledSkills` and `contextMode` are agent-level configuration properties that belong alongside `defaultProvider`, `defaultModel`, etc. The nested `skills` key also created confusing ambiguity with the top-level `skills` block.

Additionally, `skillsContextBodies` used a "Bodies" suffix inconsistent with other context fields (e.g., `systemContext`, not `systemContextBody`).

**Resolution:**

1. Flattened `enabledSkills` and `contextMode` to top-level fields on each `agents[]` object, matching their placement in `config.json`. Eliminated the `agents[].skills` sub-object entirely.
2. Renamed `skillsContextBodies` → `skillsContext` for consistency with `systemContext`.
3. Updated `GATEWAY_STATUS.md`, `AGENTS.md`, `server.rs`, `protocol.rs`, `types.rs`, `gateway.rs`, `agent.rs`, and `gateway.rs` (desktop screen).

---

## 5. Worker Entry Field Handling

### 5.1 Silently Ignored Worker Fields — RESOLVED

**Category:** Validation gap — **Fixed**

In `config.json`, the `agents` array accepts entries with `role: "worker"`. The `AgentDefinition` struct allows all fields on both orchestrator and worker entries, but `agents_from_array` previously discarded the following fields when processing worker entries without error:

- `enabled_providers` → set to `None`
- `max_delegations_per_turn` → set to `None`
- `max_delegations_per_session` → set to `None`
- `max_delegations_per_worker` → set to `None`
- `max_tool_loops_per_turn` → set to `None`

**Resolution:** Added validation in `agents_from_array` that rejects orchestrator-only fields on worker entries at parse time, with clear error messages identifying the field and the worker id. Since backwards compatibility is not a concern before v0.1.0, strict rejection is appropriate.

### 5.2 `maxToolLoopsPerTurn` on Worker Entries — RESOLVED

**Category:** Placement/semantics — **Fixed**

The `maxToolLoopsPerTurn` field is set on the orchestrator entry in config but applies to **both** orchestrator and worker turns. The orchestrator's value is used globally via `agents.max_tool_loops_per_turn`. The ORCHESTRATION spec documents this: "Applies to both orchestrator and worker (delegate) turns."

**Resolution:** Documented in CONFIGURATION.md that `maxToolLoopsPerTurn` is orchestrator-only and applies globally to both orchestrator and worker turns. Now rejected on worker entries per Finding 5.1.

## 6. Environment Variable Resolution Patterns
### 6.1 Resolution Helper Coverage

| Config Field | Env Override | Resolution Helper in `config.rs` | Status |
|-------------|-------------|----------------------------------|--------|
| `gateway.auth.token` | `CHAI_GATEWAY_TOKEN` | `resolve_gateway_token` | ✅ |
| `channels.telegram.botToken` | `TELEGRAM_BOT_TOKEN` | `resolve_telegram_token` | ✅ |
| `channels.telegram.webhookSecret` | `TELEGRAM_WEBHOOK_SECRET` | `resolve_telegram_webhook_secret` | ✅ |
| `channels.matrix.roomIds` | `MATRIX_ROOM_ALLOWLIST` | `resolve_matrix_room_allowlist` | ✅ |
| `channels.signal.httpBase` | `SIGNAL_CLI_HTTP` | `resolve_signal_daemon_config` (in `signal.rs`) | ⚠️ See F7 |
| `channels.signal.account` | `SIGNAL_CLI_ACCOUNT` | `resolve_signal_daemon_config` (in `signal.rs`) | ⚠️ See F7 |
| `channels.matrix.homeserver` | `MATRIX_HOMESERVER` | `resolve_matrix_homeserver` | ✅ |
| `channels.matrix.accessToken` | `MATRIX_ACCESS_TOKEN` | `resolve_matrix_access_token` | ✅ |
| `channels.matrix.user` | `MATRIX_USER` | `resolve_matrix_user` | ✅ |
| `channels.matrix.password` | `MATRIX_PASSWORD` | `resolve_matrix_password` | ✅ |
| `channels.matrix.userId` | `MATRIX_USER_ID` | `resolve_matrix_user_id` | ✅ |
| `channels.matrix.deviceId` | `MATRIX_DEVICE_ID` | `resolve_matrix_device_id` | ✅ |
| `providers[].apiKey` | `<VAR_NAME>` syntax | `resolve_provider_api_key` + `resolve_env_ref` | ✅ |

### 6.2 Provider `apiKey` Resolution

The `<VAR_NAME>` syntax for `apiKey` is well-implemented and well-tested. **No issues.**

### 6.3 `.env` File Loading

The `.env` file is loaded once per process via `load_profile_env` using `std::sync::OnceLock`. This is idempotent and correct. **No issues.**

---

## 7. Structural Change: Top-Level `skills` Block

### What Changed

The orphan `skillLockMode` scalar at the config top level was replaced by a structured `skills` block containing `lockMode`. The status payload key was renamed from `skillPackages` to `skills`, and the missing `lockGeneration` and `lockedSkills` fields were implemented.

### Rationale

1. **Eliminates the orphan scalar** — every top-level config key is now a structured block, consistent with `gateway`, `channels`, `providers`, and `agents`.
2. **Creates config↔status symmetry** — `skills` in both config and status, mirroring the other blocks.
3. **Provides a home for future skill settings** — discovery root override, registry configuration, auto-update behavior, etc. can all be added to the `skills` block without more breaking changes.
4. **Resolves the spec↔code divergence** — `lockGeneration` and `lockedSkills` are now emitted by the gateway.
5. **Cheap to do before v0.1.0** — no backwards compat burden, small surface area.

### What Moved and What Stayed

| Property | Before | After | Why |
|----------|--------|-------|-----|
| `lockMode` | Config top-level `skillLockMode` | `skills.lockMode` in config | Gets a proper home alongside future skill settings |
| Per-agent `enabledSkills` | `agents[].enabledSkills` | Stays | Per-agent selection from shared pool |
| Per-agent `contextMode` | `agents[].contextMode` | Stays | Per-agent behavior setting |
| Status block name | `skillPackages` | `skills` | Mirrors config key |
| `lockGeneration` | Spec only (not emitted) | `skills.lockGeneration` in status | Implemented |
| `lockedSkills` | Spec only (not emitted) | `skills.lockedSkills` in status | Implemented |
| `lockMode` in status | Not present | `skills.lockMode` in status | Mirrors config intent |

### Files Changed

| Area | Change |
|------|--------|
| `crates/lib/src/config.rs` | Added `SkillsConfig` struct with `lock_mode: SkillLockMode`. Replaced `Config.skill_lock_mode` with `Config.skills: SkillsConfig`. |
| `crates/lib/src/gateway/server.rs` | Renamed `skillPackages` → `skills` in status payload. Added `lockMode`, `lockGeneration`, `lockedSkills` to the `skills` block. Added lockfile metadata fields to `GatewayState`. Read lockfile at startup for status data. |
| `crates/lib/src/gateway/protocol.rs` | Updated comments from `skillPackages` to `skills`. |
| `crates/lib/src/init.rs` | Updated doc comments from `skillLockMode` to `skills.lockMode`. |
| `crates/desktop/src/app/types.rs` | Renamed `skill_packages_*` → `skills_*`. Added `skills_lock_mode`, `skills_lock_generation`, `skills_locked_count` fields. Updated doc comments. |
| `crates/desktop/src/app/state/gateway.rs` | Updated status parsing from `"skillPackages"` → `"skills"`. Parse new fields. |
| `crates/desktop/src/app/screens/gateway.rs` | Updated section header to "Skills". Use new field names. Display `lockMode`, `lockGeneration`, `lockedSkills`. |
| `crates/desktop/src/app/screens/config.rs` | Added "Skills" section showing `lockMode`. |
| `base/spec/CONFIGURATION.md` | Replaced top-level `skillLockMode` with `skills` block. Updated top-level shape, block table, and relationship text. |
| `base/spec/GATEWAY_STATUS.md` | Renamed `skillPackages` → `skills`. Added `lockMode`, `lockGeneration`, `lockedSkills` to field table. Added `endpointType` to providers table (resolves F3). Updated model array description (strings, not objects). |
| `base/spec/PROFILES.md` | Updated `skillLockMode` references to `skills.lockMode`. |
| `base/spec/SKILL_PACKAGES.md` | Updated `skillLockMode` references to `skills.lockMode`. |
| `base/spec/AGENTS.md` | Updated "no top-level `skills`" text to describe the new `skills` block. |
| `base/spec/SKILL_FORMAT.md` | Updated `skillLockMode` reference to `skills.lockMode`. |
| `base/README.md` | Updated `skillLockMode` reference to `skills.lockMode`. |
| `base/adr/SKILL_PACKAGES.md` | Updated `skillLockMode` references to `skills.lockMode`. |
| `docs/guides/03-configuration.md` | Updated `skillLockMode` references to `skills.lockMode`. |
| `docs/guides/06-skills.md` | Updated `skillLockMode` references to `skills.lockMode`. |
| `docs/guides/08-cli-reference.md` | Updated `skillLockMode` reference to `skills.lockMode`. |
| `docs/guides/11-troubleshooting.md` | Updated `skillLockMode` references and JSON examples to `skills.lockMode`. |
| `docs/journey/01-gateway-cli-health-and-ws.md` | Updated `skillPackages` references to `skills`. |
| `docs/journey/13-profile-manage.md` | Updated `skillLockMode` reference to `skills.lockMode`. |

---

## 8. Dual `defaultModel` Fields

**Category:** Potential confusion (informational)

Two separate `defaultModel` fields exist:

1. **`ProviderDefinition.defaultModel`** — Default model for that provider.
2. **`AgentDefinition.defaultModel`** — Default model for that agent entry.

Resolution priority: `agents[].defaultModel` → `providers[].defaultModel` → `EndpointType::default_model()`. This is correctly implemented and documented. **No issue.**

---

## 9. `desktop.json` Interaction (Upcoming)

The FEAT_DESKTOP_JSON proposal adds `~/.chai/desktop.json` for desktop-specific settings. Key observations:

1. **No overlap with `config.json`.** The proposal explicitly states "nothing moves out of config.json."
2. **`gateway.connectUrl` vs `gateway.bind:port`.** Different precedence chain; `desktop.json` is additive.
3. **Profile independence.** `desktop.json` lives at `~/.chai/` (not per-profile).
4. **No `config.json` schema changes needed.** The `skills` block introduced here does not conflict.

---

## 10. Summary of Findings

| ID | Category | Severity | Description | Status |
|----|----------|----------|-------------|--------|
| F1 | Config ↔ status gap | Low | `gateway.unsafeSandbox` not in status payload | ✅ Resolved |
| F2 | Naming consistency | Info | `webhookUrl` (config) vs `transport` (status) | Documented, no action |
| F3 | Spec ↔ code divergence | Medium | `endpointType` emitted but not in spec | ✅ Resolved |
| F4 | Config ↔ status gap | Medium | `discovery` boolean misleading; `modelDiscovery` type not exposed | ✅ Resolved |
| F5 | Config ↔ status gap | Medium | Delegation/session caps not in status payload | ✅ Resolved |
| F6 | Spec ↔ code divergence | High | `lockGeneration`/`lockedSkills` in spec but not emitted | ✅ Resolved |
| F7 | Pattern inconsistency | Low | Matrix env var resolution not centralized | ✅ Resolved |
| F8 | Display gap | Low | Desktop config screen omits `skills.lockMode` | ✅ Resolved |
| F9 | Display gap | Low | Desktop config screen omits `unsafeSandbox` | ✅ Resolved |
| F10 | Config ↔ status mismatch | Medium | `enabledSkills`/`contextMode` nested under `skills` in status; `skillsContextBodies` naming | ✅ Resolved |
| 5.1 | Validation gap | Medium | Orchestrator-only fields silently ignored on worker entries | ✅ Resolved |

### Resolved in This Audit Cycle

- **F1:** Added `unsafeSandbox` to the `gateway` status block. Desktop gateway status screen shows a yellow warning when enabled.
- **F3:** Added `endpointType` to the `providers` field table in GATEWAY_STATUS.md.
- **F5:** Added `maxToolLoopsPerTurn`, `maxDelegationsPerTurn`, `maxDelegationsPerSession`, `maxDelegationsPerWorker` to the orchestrator row in `agents[]` in the status payload. Worker rows use `null`.
- **F6:** Implemented `lockGeneration` and `lockedSkills` in the gateway status payload. Renamed `skillPackages` → `skills` for config↔status symmetry.
- **F7:** Added `resolve_matrix_homeserver`, `resolve_matrix_access_token`, `resolve_matrix_user`, `resolve_matrix_password`, `resolve_matrix_user_id`, `resolve_matrix_device_id` helpers to `config.rs`. Moved `matrix_channel_configured` from `server.rs` to `config.rs`. Updated `matrix.rs` and desktop config screen to use centralized helpers.
- **F8:** Added "Skills" section to desktop config dashboard showing `lockMode`.
- **F9:** Added `unsafeSandbox` warning to desktop config and status dashboard Gateway sections.
- **F10:** Flattened `enabledSkills` and `contextMode` to top-level fields on each `agents[]` object (matching config). Renamed `skillsContextBodies` → `skillsContext`. Eliminated the `agents[].skills` sub-object. Updated server.rs, protocol.rs, types.rs, gateway.rs, agent.rs, gateway screen, GATEWAY_STATUS.md, and AGENTS.md.
- **5.1:** Added validation in `agents_from_array` that rejects orchestrator-only fields on worker entries at parse time.
- **5.2:** Documented in CONFIGURATION.md that `maxToolLoopsPerTurn` is orchestrator-only and applies globally. Now rejected on worker entries per Finding 5.1.
- **F4:** Replaced the misleading `discovery` boolean with a `modelDiscovery` string (`"auto"`, `"lmstudio"`, `"static"`) on each provider entry in the gateway status payload. The `discovery` boolean conflated agent-level scope (`enabledProviders`) with provider-level behavior; scope is already derivable from `agents[].enabledProviders`. Updated `server.rs`, `protocol.rs`, `types.rs`, `gateway.rs` (desktop screen), and `GATEWAY_STATUS.md`.
- **Structural:** Introduced top-level `skills` block in `config.json`, replacing the orphan `skillLockMode` scalar.

### Open Items (Prioritized)

**All open items from this audit have been resolved.**
