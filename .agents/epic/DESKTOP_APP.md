---
status: in-progress
---

# Epic: Desktop Application

**Summary** — Track improvements to **`crates/desktop`** (egui/eframe): what exists today, gaps versus the gateway and config, short-term UX wins that need no new backend contracts, **constrained read/write** of config, workspace, and skill files (markdown + JSON) as preparation for **project** roots, and longer-term ideas (full explorer, broader editing) that depend on projects or new APIs.

**Status** — In-progress. Constrained file editing (config, workspace, skills) is the active short-term focus. UX polish and medium-term goals follow. Long-term explorer depends on projects design.

---

## Problem Statement

The desktop app is a functional operator console but lacks in-app editing of the files users already manage — `config.json`, per-agent **`AGENTS.md`** under **`agents/<id>/`**, and skill files under **`~/.chai/skills`**. Users must leave the app to edit these files and restart the gateway manually. Additionally, several fields already available from `status` and `config.json` are not yet surfaced in the UI, and UX consistency (loading states, spacing, accessibility) has gaps. The app also has no filesystem visibility into what the orchestrator "sees," which limits its usefulness as a control surface beyond gateway lifecycle management.

---

## Goal

A desktop app that serves as the primary control surface for Chai: operators can inspect and edit all gateway-relevant config, profile-local agent context, and shared skill packages in-app, receive clear feedback on when changes require a gateway restart, and navigate live runtime state (status, context, skills, tools, logs) with consistent UX. The app complements external editors (Cursor, Obsidian) for broader workflows but owns the gateway lifecycle and Chai-specific configuration surface.

---

## Current State

The desktop app is a **local control UI** bundled as **`chai-desktop`**. It does **not** embed the gateway as a library; it may **spawn** the `chai gateway` subprocess or attach to an **already listening** gateway on the configured bind/port.

| Area | Behavior |
|------|----------|
| **Header** | Start/stop gateway when the app spawned it; if another process owns the port, shows disabled "Gateway running". **Runtime profile:** shows the **persistent** active profile (from **`~/.chai/active`**); **ComboBox** rewrites the symlink when the gateway is **not** running (control **disabled** while the gateway is up — same rule as **`chai profile switch`**). Optional UX: hint when **effective** profile (e.g. **`CHAI_PROFILE`**) differs from persistent — see **Requirements** → **Runtime profiles**. |
| **Probe** | Periodic TCP connect to `gateway.bind`:`gateway.port` (~1 Hz) to detect liveness. |
| **WebSocket** | When responding: `connect` (device identity or token + device pairing) then **`status`**; caches `GatewayStatusDetails`. |
| **Chat** | **`agent`** RPC over WebSocket; provider/model overrides; session list with **`session.message`** / orchestration events for timelines; hint for **`/help`** and **Ctrl/Cmd+Enter** when gateway is running. |
| **Screens** | Sidebar: **Chat** (ungrouped), **Runtime** (**Status**, Context, Tools), **Source** (Config, Skills), **Diagnostics** (Logs). **Status** is gateway **`status`** only (orchestrator + worker rows in **`agents.entries`** + discovery + context mode); merged **Tools** JSON lives on **Tools**; **Config** has full on-disk agents policy. |
| **Config** | Reads **`config.json`** via `lib::config::load_config` (same as CLI); **Config** screen is a read-only summary (no JSON editor): orchestrator **agent context directory** (**`orchestrator_context_dir`**), workers, session cap, delegation policy fields when set. |
| **Context** | **`status.agents.entries`**: **Agent** combo (orchestrator vs each worker) from each row’s **`systemContext`**; orchestrator **read-on-demand** keeps two columns (system text + skill bodies from disk); worker rows use a single scroll (full text from gateway). Falls back to a single orchestrator string when **`entries`** is absent. |
| **Skills** | Lists enabled/disabled skill entries from disk; detail pane for SKILL.md + **tools.json** (read-only; no save). |
| **Agent context** | **`AGENTS.md`** is not edited in-app; **Config** may show the orchestrator **agent context directory** (**`agents/<id>/`**). |
| **Logs** | In-memory buffer fed by gateway stderr/stdout when started from desktop. |

### Shipped

**2025-03-25** — Config: orchestrator **agent context directory** (**`orchestrator_context_dir`** → **`agents/<orchestratorId>/`**); **Workers** with **`effective_worker_defaults`**; **`maxSessionMessages`**; delegation caps (**per turn**, **per session**, **per provider**), **blocked providers**, **instruction routes**, orchestrator **delegate allowed models**. Chat: muted hint **`use /help for commands; ctrl/cmd+enter to send`** when gateway is running.

**2025-03-25 (follow-up)** — **Status** screen **Agents** section: **Orchestrator** fields only from gateway **`status`** (id, date, default provider/model). No **`config.json`** fallback on **Status** when the gateway is down or status is pending.

**2025-03-25 (follow-up 2)** — **Status** **Models**: discovery lists for all backends from **`status`** (no **`enabledProviders`** filter). **Orchestration catalog** shows all rows. Subtitle states **Status** is gateway **`status`** only; **Config** for on-disk agents config.

**2025-03-25 (follow-up 3)** — Worker summaries on **Status**: desktop collects **`status.payload.agents.entries`** rows with **`role`**: **`worker`** (`id`, **`defaultProvider`**, **`defaultModel`**) into **`GatewayStatusDetails.workers`**; **Status** **Agents** lists those effective defaults; **Config** still shows full on-disk **`agents`** configuration.

**2025-03-25 (follow-up 4)** — Shared desktop UI helpers: **`app/ui/spacing`**, **`dashboard`** (two-column layout, section groups, key/value rows), **`readonly_code`**, **`view_toggle`**, **`layout::central_padded`**; **Config**, **Status**, **Context**, **Skills**, and **Tools** screens refactored to use them (consistent spacing, no duplicated dashboard widgets).

**2025-03-25 (follow-up 5)** — **Accessibility / readability**: **`dashboard::kv`** uses a fixed-width key column (**`KV_LABEL_COLUMN_WIDTH`**) so values line up; keys and values use default body text (no **weak** on keys). Removed **`small()`** from Config/Status summary content and view toggles; grid column headers use **strong**; secondary hints keep **weak** only where appropriate.

---

## Scope

### In Scope

**Short-term (current system, no new gateway contracts):**

- Constrained file editing: `config.json`, profile **`agents/<id>/AGENTS.md`** (orchestrator and optionally workers), and per-skill `SKILL.md` / `tools.json` — read and write for this fixed artifact set only
- Apply/restart banner after saves that require gateway restart
- UX polish: empty/loading states, sessions panel scroll and truncation, logs clear button, header tri-state
- Quality of life: reveal config in file manager, persist window size and last chat provider/model
- Surfacing remaining `status` / `config.json` fields not yet shown (per-worker allowlists, provider enumeration)

**Medium-term (some gateway or shared contract required):**

- Streaming assistant tokens (requires gateway streaming/SSE path)
- Unified connection panel (WebSocket + HTTP health)
- Skills/Context parity via `status` skill payloads to avoid drift

### Out of Scope

- **General file manager / arbitrary path editing** — deferred to long-term; depends on named project directories and allowlist policy
- **File explorer (read-only)** — depends on projects design and canonical path rules; see Long-Term phase below and [RAG_VECTOR.md](RAG_VECTOR.md)
- **Broader in-app editing** (multi-file, arbitrary project roots) — out of scope until projects exist; Chai complements Cursor/Obsidian rather than replacing them
- **Per-session or per-project scope in Chat** — depends on gateway session metadata from projects

---

## Design

### Gaps: What the System Exposes but the Desktop Does Not (Yet)

These are **additive** to the current stack: either **`status`** already returns the data, or **`lib::config`** can supply it without new gateway methods.

#### From Gateway **`status`**

- **Skills fields** — **`skillsContextFull`**, **`skillsContextBodies`**, **`skillsContext`** are available for richer **Context** / **Skills** parity when avoiding duplicate disk reads (optional optimization).

#### From **`config.json`** (read locally)

- **Per-worker `delegateAllowedModels`** — Not shown on **Config** (orchestrator-level allowlist is shown).
- **Providers block** — Partial: Ollama/LMS base URLs when set; other provider entries may exist in config but are **not** enumerated in the same way as in **README**.

#### Operational

- **HTTP health** — Gateway exposes **GET /** JSON health; desktop uses **TCP only**. Not wrong, but a **health summary** (e.g. protocol version) could use the same HTTP client if desired later.
- **External gateway** — When the user runs the gateway elsewhere, **Logs** only capture subprocess output from **Start gateway** in-app; **no** remote log tail (expected limitation).

### Constrained File Editing Design

The desktop can still implement **read and write** for a **fixed set of artifacts** the user already manages: **`config.json`**, **`agents/<orchestratorId>/AGENTS.md`** (and optionally worker dirs), and **skill** files under the resolved skills root (**`SKILL.md`**, **`tools.json`**). Focus on **markdown** and **JSON** only — matching what the stack already uses.

**Why this is valuable**

- **Same skills you need for projects later** — Path resolution (via `lib::config`: `default_config_path`, `orchestrator_context_dir`, `default_skills_dir`), dirty-state, save/discard, and validation.
- **High-signal locations** — Users already edit these; in-app editing reduces friction and prepares UX for **multi-root** explorers without requiring the full **projects** abstraction first.
- **Narrow scope** — Avoid arbitrary binary files and arbitrary paths until **allowlists** are defined.

**Design caveats (bake in early)**

- **Apply vs restart** — The gateway loads **config** and **skills** at startup; **agent context** is built at startup from **`agents/<id>/AGENTS.md`**. After a save, show a clear **"restart gateway to apply"** (or equivalent) when the running process will not pick up changes live.
- **Concurrency** — Detect **external modification** (mtime or content hash) since open; offer **reload** before overwrite.
- **Validation** — **`config.json`**: parse as JSON and validate with the same rules as **`load_config`** (or fail with a readable error). **`tools.json`**: parse as JSON and validate against **[TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md)** or a **serde** round-trip through existing descriptor types where practical; pretty-print on save for diff-friendly files.
- **Scope creep** — UI copy should state these are **Chai config / agent context dirs / skills roots** only — not a general file manager (that remains Long-Term).

**Suggested order**

1. **`config.json`** + **`agents/<id>/AGENTS.md`** — Few files, largest usability win.
2. **Per-skill `SKILL.md` and `tools.json`** — More surface area; **`tools.json`** requires stricter validation.

### Potential Future Improvements (Not Decided)

Larger directions worth revisiting when there is time; **no commitment** — trade-offs and scope need discussion.

- **Split `ChaiApp` state** — Separate gateway lifecycle, WebSocket/cache, and per-screen UI state into smaller structs to reduce merge conflicts and clarify dependencies (may pair with a small "screen context" type passed into screen functions).
- **Dashboard / screen modules** — If **`config.rs`** / **`status.rs`** keep growing, consider submodules (e.g. `config_summary.rs`) or screen-local private types; only if file size hurts navigation.
- **egui persistence** — eframe **storage** for window geometry and optional UI prefs (last chat model, sidebar width); must not fight explicit **`config.json`** defaults in confusing ways.
- **Thin egui reference** — Short internal note (patterns used here: **`CentralPanel`**, **`ScrollArea`**, **`TextEdit`**, ids) — optional; official **egui** docs remain the source of truth.

### Baseline Assessment

The desktop is already a **credible operator console**: gateway control, **live status**, **Context** inspection, **Skills** inspection, and **Chat** with **delegation** timeline support. The largest **documentation gap** on the **Config** screen is largely addressed for agent context paths, workers, and delegation; remaining gaps include **per-worker allowlists** and full **provider** enumeration. The largest **product gap** relative to user mental models is **no filesystem visibility** of what the orchestrator "sees," which the long-term **explorer** addresses. **Constrained file editing** (config, **`AGENTS.md`**, skill markdown/JSON) is the recommended **bridge**: it delivers value immediately and exercises patterns (paths, validation, apply/restart) needed for **projects** without waiting for the full multi-root design. Short-term work should prioritize **surfacing existing config and status fields**, **polishing** discovery, sessions, and logs, and **incremental** editing support as above.

---

## Requirements

### Constrained File Editing

- [ ] **Config editor** — Open `config.json` from resolved path; syntax-colored or plain **TextEdit**; **Save** after JSON validation; **Revert** / reload from disk.
- [ ] **Orchestrator `AGENTS.md`** — Edit **`agents/<orchestratorId>/AGENTS.md`** (path from **`orchestrator_context_dir`**); create file if missing (optional; mirror **`chai init`** behavior).
- [ ] **Skill files** — From **Skills** screen: edit **SKILL.md** and **tools.json** with save; validate **JSON** before write; optional **format** button.
- [ ] **Apply banner** — After any save that requires it, prompt to **restart gateway** (when desktop owns the subprocess, offer **Restart** action).

### Information Density and Trust

- [x] **Status screen** — **Agents** block: orchestrator + workers, **`date`** when status loaded; pointer to **Context** for full message (shipped 2025-03-25, revised same day).
- [x] **Config screen** — Orchestrator agent context directory, workers, **`maxSessionMessages`**, delegation policy (shipped 2025-03-25).
- [x] **Chat** — **`/help`** and **Ctrl/Cmd+Enter** hint (shipped 2025-03-25).

### Runtime profiles (desktop)

Optional polish on top of **[RUNTIME_PROFILES.md](RUNTIME_PROFILES.md)** (core switcher is **shipped**).

- [ ] **Persistent vs effective hint** — When the app environment could set **`CHAI_PROFILE`** (or a future equivalent), surface when **effective** profile ≠ symlink target (**`~/.chai/active`**), matching **`chai profile current`** (two-line persistent + effective). No post-switch **restart** prompt: switching is only allowed with the gateway **stopped**, so the next **start** already uses the new profile.

### UX and Visual Design

- [ ] **Empty and loading states** — Align copy when **`status`** is refetching (some screens already avoid flash; apply the same pattern everywhere).
- [ ] **Sessions panel** — Ensure **scroll** and **long session id** truncation with **full id on hover** (if egui allows).
- [ ] **Logs** — **Clear buffer** button; optional **line cap** to avoid unbounded memory; **monospace** is already used.
- [ ] **Header** — Clearer **tri-state**: probing / not responding / responding (possibly with color or subtitle), without duplicating the sidebar.
- [x] **Spacing and type** — Named constants in **`app/ui/spacing.rs`** and shared dashboard helpers (shipped 2025-03-25 follow-up 4).
- [x] **Accessibility (readability)** — Less **small** / **weak** on primary labels; **kv** table-style alignment; headers **strong** (shipped 2025-03-25 follow-up 5).
- [ ] **Accessibility (DPI)** — Confirm behavior under system scaling and egui **UI scale**; document if anything is still fixed-pixel.

### Quality of Life

- [ ] **Open config path** — Button or menu: **"Reveal config in file manager"** / **copy path** (resolved **`config_path`** from **`lib::config::load_config`**, i.e. **`<profile>/config.json`** under **`~/.chai/profiles/`**).
- [ ] **Persist UI state** — eframe **storage** for **window size** and optionally **last Chat provider/model** (optional; must not override explicit config defaults confusingly).

### Medium-Term

- [ ] **Streaming assistant tokens** — Only if the gateway exposes a **streaming** `agent` or SSE path; today the desktop expects a **single** `agent` response.
- [ ] **Unified "connection" panel** — Test **WebSocket** + **HTTP health** in one place for supportability.
- [ ] **Skills / Context** — Prefer **`status`** skill payloads when present to avoid **drift** between gateway and disk (edge cases: config changed since gateway start).

---

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 — UX foundation | Shared UI helpers, spacing constants, dashboard widgets, accessibility/readability pass | Complete |
| 2 — Config and status surfacing | Effective workspace, workers, delegation policy, status agents/models screens | Complete |
| 3 — Constrained file editing | Read/write `config.json`, **`agents/<id>/AGENTS.md`**, skill `SKILL.md` / `tools.json`; apply/restart banner | In progress |
| 4 — UX polish | Loading/empty states, sessions panel, logs clear, header tri-state, optional profile persistent/effective hint, DPI accessibility, QoL items | Pending |
| 5 — Medium-term contracts | Streaming tokens, unified connection panel, Skills/Context status parity | Pending |
| 6 — Long-term (projects) | Read-only file explorer over project roots, broader in-app editing, per-session/project scope | Pending |

---

## Follow-ups

_Items that do not block any phase but are worth revisiting._

- Keyboard shortcuts overlay (**?** key).
- Copy **session id** to clipboard from Chat or Sessions.
- Export transcript (markdown).
- Light/dark **theme** toggle if not tied to system only.
- Notification when gateway **process exits unexpectedly** (subprocess path).

---

## Related Epics and Docs

- [RUNTIME_PROFILES.md](RUNTIME_PROFILES.md) — **`~/.chai/active`**, **`CHAI_PROFILE`**, CLI **`chai profile`**; desktop **core** profile switcher is specified there.
- [adr/DESKTOP_FRAMEWORK.md](../adr/DESKTOP_FRAMEWORK.md)
- [RAG_VECTOR.md](RAG_VECTOR.md) — projects + retrieval alignment
- [spec/CONTEXT.md](../spec/CONTEXT.md) — what the gateway sends as context
- [spec/TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) — tools.json validation reference
