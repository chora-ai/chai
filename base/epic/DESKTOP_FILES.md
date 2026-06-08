---
status: draft
---

# Epic: Desktop File Explorer and Editing

**Summary** — Add file explorer and constrained file editing to `crates/desktop` so operators can browse, read, and write Chai config, agent context, and skill files in-app, and lay the groundwork for a general read-only file explorer over project roots.

**Status** — Proposed (not implemented).

## Problem Statement

The desktop app has no filesystem visibility into what the orchestrator "sees." Users must leave the app to edit `config.json`, per-agent `AGENT.md` files, and skill packages, then restart the gateway manually. The app already reads these files for display but provides no write path. This limits the desktop's usefulness as a control surface beyond gateway lifecycle management.

## Goal

A desktop app screen (or screens) where operators can browse, inspect, and edit Chai-relevant files in-app — starting with a fixed artifact set (`config.json`, agents, skills) and growing into a read-only file explorer over project roots once the projects abstraction exists. The app must communicate clearly when edits require a gateway restart and detect concurrent external modifications.

## Scope

### In Scope

**Short-term — Constrained file editing (current system, no new gateway contracts):**

- Read and write `config.json` from resolved profile path
- Read and write `agents/<id>/AGENT.md` (orchestrator and workers)
- Read and write per-skill `SKILL.md` and `tools.json` under the resolved skills root
- Apply/restart banner after saves that require gateway restart
- JSON validation before writing `config.json` and `tools.json`
- External modification detection (mtime or content hash) with reload prompt
- Scope messaging: UI copy states these are Chai config/agent/skill paths only

**Medium-term — Read-only file explorer (some gateway or shared contract required):**

- Read-only file explorer over sandbox

**Long-term — Broader in-app editing (depends on explorer):**

- Edit mode in file explorer for sandbox files

### Out of Scope

- General file manager for arbitrary paths
- Other medium-term gateway contract changes

## Design

### Constrained File Editing

The desktop can implement read and write for a **fixed set of artifacts** the user already manages: `config.json`, `agents/<orchestratorId>/AGENT.md` (and optionally worker dirs), and skill files under the resolved skills root (`SKILL.md`, `tools.json`). Focus on markdown and JSON — matching what the stack already uses.

**Why this is valuable**

- **Same skills you need for projects later** — Path resolution (via `lib::config`: `default_config_path`, `orchestrator_context_dir`, `default_skills_dir`), dirty-state, save/discard, and validation patterns all transfer to multi-root explorers.
- **High-signal locations** — Users already edit these; in-app editing reduces friction and prepares UX for multi-root without requiring the full projects abstraction first.
- **Narrow scope** — Avoid arbitrary binary files and arbitrary paths until allowlists are defined.

### Design Decisions

#### Apply vs Restart

The gateway loads config and skills at startup; agent context is built at startup from `agents/<id>/AGENT.md`. After a save, show a clear "restart gateway to apply" (or equivalent) when the running process will not pick up changes live. When the desktop owns the subprocess, offer a **Restart** action.

#### Concurrency

Detect **external modification** (mtime or content hash) since open; offer **reload** before overwrite.

#### Validation

- **`config.json`**: Parse as JSON and validate with the same rules as `load_config` (or fail with a readable error).
- **`tools.json`**: Parse as JSON and validate against [TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) or a serde round-trip through existing descriptor types where practical; pretty-print on save for diff-friendly files.
- **`AGENT.md`** / **`SKILL.md`**: Markdown text; no structural validation beyond UTF-8.

#### Scope Messaging

UI copy should state these are **Chai config / agent context dirs / skills roots** only — not a general file manager.

### File Explorer (Medium-Term)

A read-only file explorer would show files the orchestrator can see. Key requirements:

- **Read-only** initially — no in-app editing of sandbox files
- **Project-root scoped** — bounded by same sandbox policy (first-level only symlinks)

### Edit Mode (Long-Term)

An edit mode within the file explorer. Key requirements:

- **Edit mode** — switch between read-mode and edit-mode
- **Sandbox scoped** — bounded by same sandbox policy (first-level only symlinks)

## Requirements

### Constrained File Editing

- [ ] **Config editor** — Open `config.json` from resolved path; syntax-colored or plain TextEdit; Save after JSON validation; Revert / reload from disk.
- [ ] **Orchestrator `AGENT.md`** — Edit `agents/<orchestratorId>/AGENT.md` (path from `orchestrator_context_dir`); create file if missing (optional; mirror `chai init` behavior).
- [ ] **Skill files** — From Skills screen: edit SKILL.md and `tools.json` with save; validate JSON before write; optional format button.
- [ ] **Apply banner** — After any save that requires it, prompt to restart gateway (when desktop owns the subprocess, offer Restart action).
- [ ] **Concurrency detection** — Detect external modification since open; offer reload before overwrite.

### File Explorer (Medium-Term)

- [ ]

### Edit Mode (Long-Term)

- [ ]

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 — Constrained editing | Read/write `config.json`, `agents/<id>/AGENT.md`, skill `SKILL.md` / `tools.json`; apply/restart banner; concurrency detection | In progress |
| 2 — Read-only explorer | File explorer over sandbox | Pending |
| 3 — Edit mode | Edit mode in file explorer| Pending |

## Related Epics and Docs

- [spec/DESKTOP.md](../spec/DESKTOP.md) — Current state of the desktop application
- [FEAT_DESKTOP_UX.md](../FEAT_DESKTOP_UX.md) — UX polish and quality-of-life improvements
- [RAG_VECTOR.md](RAG_VECTOR.md) — Projects + retrieval alignment
- [adr/DESKTOP_FRAMEWORK.md](../adr/DESKTOP_FRAMEWORK.md) — Why egui/eframe
- [spec/TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) — tools.json validation reference
- [spec/CONTEXT.md](../spec/CONTEXT.md) — What the gateway sends as context
