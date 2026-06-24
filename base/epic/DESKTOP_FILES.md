---
status: draft
---

# Epic: Desktop File Explorer and Editing

**Summary** — Add a file explorer and plain-text editor to `crates/desktop` so operators can browse, read, and write files across three root directories — agents, skills, and sandbox — starting with the agents directory as phase 1 and expanding to skills and sandbox in subsequent phases. A new-files screen provides a command-line-free alternative for creating symlinks into the sandbox via a native file explorer.

**Status** — Draft (not implemented). The Files screen is wired up (`Screen::Files`, `screens/files.rs` stub, sidebar entry commented out) but contains no functionality.

## Problem Statement

The desktop app has no filesystem visibility into what the orchestrator "sees." Users must leave the app to edit per-agent `AGENT.md` files, skill files, and sandbox content, then restart the gateway manually. The app already reads some of these files for display (agents, skills, config) but provides no write path. For the sandbox, there is no visibility at all — users who want to bring files or directories into the sandbox must use the command line to create symlinks. This limits the desktop's usefulness as a control surface beyond gateway lifecycle management.

## Current State

- **`Screen::Files`** variant exists in the `Screen` enum and is routed to `screens::files::ui_files_screen`.
- The screen implementation is a **stub** displaying "File explorer not yet implemented."
- The sidebar entry for Files is **commented out** with `TODO: see base/epic/DESKTOP_FILES.md`.
- The Config screen is read-only (no JSON editor). The Skills screen shows SKILL.md and `tools.json` read-only.
- No sandbox content is surfaced anywhere in the desktop.

## Goal

A desktop Files screen where operators can browse, inspect, and edit files across **three root directories** — agents, skills, and sandbox — using a plain-text editor that handles markdown, JSON, and any file format renderable as plain text (e.g., source code in various programming languages). The screen must communicate clearly when edits require a gateway restart, detect concurrent external modifications, and provide a GUI alternative for creating symlinks into the sandbox.

## Scope

### In Scope

**Phase 1 — Agents directory (markdown editing, simplest root):**

- Browse the agents directory tree (`<profileRoot>/agents/`) showing all agent subdirectories and their `AGENT.md` files
- Read and write `AGENT.md` for each agent (orchestrator and workers)
- Plain-text editor for markdown files
- Apply/restart banner after saves (agent context is built at startup)
- External modification detection (mtime or content hash) with reload prompt

**Phase 2 — Skills directory (JSON editing, versioned layout navigation):**

- Browse the skills directory tree (`~/.chai/skills/`) showing skill packages, their version snapshots, and `active` symlinks
- Read and write `SKILL.md` and `tools.json` for the active version of each skill
- JSON validation before writing `tools.json`; pretty-print on save
- Plain-text editor for markdown and JSON
- Awareness of versioned layout: navigate `versions/<hash>/` directories; show which version is `active`
- Concurrency detection and apply/restart banner
- UI must warn or prevent edits that could break the skill versioning model (see Open Questions)

**Phase 3 — Sandbox directory (symlinked directories, arbitrary file formats, new-files screen):**

- Browse the sandbox directory tree (`<profileRoot>/sandbox/`) including symlinked directories (followed transparently)
- Read and write any file that can be rendered as plain text (markdown, JSON, source code, config files, etc.)
- Read-only display for binary files (or skip with a message)
- Plain-text editor for all editable file formats
- Creating and removing symlinks in the sandbox directory using a native file explorer dialog — a command-line-free alternative for bringing external files and directories into the sandbox
- Concurrency detection
- No `.git/` directory writes (same policy as the write sandbox spec — see [spec/SANDBOX.md](../spec/SANDBOX.md))

### Out of Scope

- General file manager for arbitrary paths outside the three roots
- Rich syntax highlighting or language-specific IDE features (the editor is plain text, not a code editor)
- Binary file editing
- Creating or modifying the `active` symlink for skill versions (this is a versioning operation, not a file edit — see Open Questions)
- Gateway contract changes beyond what already exists

## Design

### Root Directory Model

The Files screen presents three root directories as separate top-level sections (or tabs) in a tree view:

| Root | Path | Content | Phase |
|------|------|---------|-------|
| **Agents** | `<profileRoot>/agents/` | Per-agent `AGENT.md` files (markdown) | 1 |
| **Skills** | `~/.chai/skills/` | Skill packages with versioned layout (`SKILL.md`, `tools.json`, `scripts/`) | 2 |
| **Sandbox** | `<profileRoot>/sandbox/` | Mixed content: markdown, JSON, code, symlinks to external directories | 3 |

Each root is independent. Users navigate within one root at a time. The tree view shows the directory structure; selecting a file opens it in the editor pane.

### Editor

The editor is a **plain-text editor** for all file formats. It handles:

- **Markdown** — `AGENT.md`, `SKILL.md`, and any `.md` file. No structural validation beyond UTF-8.
- **JSON** — `tools.json` and any `.json` file. Parse as JSON and validate before write; pretty-print on save for diff-friendly files. Invalid JSON is rejected with a readable error.
- **Source code and other text formats** — `.rs`, `.py`, `.js`, `.toml`, `.yaml`, etc. Open as plain text; save as-is. No syntax validation.

The editor is **not** an IDE. No syntax highlighting, no autocomplete, no linting. The goal is an easy way to read and edit text files, not to replace a development environment.

### Apply vs Restart

The gateway loads config and skills at startup; agent context is built at startup from `agents/<id>/AGENT.md`. After a save, show a clear "restart gateway to apply" (or equivalent) when the running process will not pick up changes live. When the desktop owns the subprocess, offer a **Restart** action.

This applies to:

- **Agent files** — `AGENT.md` is read at startup; edits require a restart.
- **Skill files** — `SKILL.md` and `tools.json` are loaded at startup; edits require a restart.
- **Sandbox files** — The sandbox is a working directory for the agent; edits may or may not require a restart depending on context. For phase 3, default to showing the restart banner with an option to dismiss (the user may know the file is read on-demand).

### Concurrency

Detect **external modification** (mtime or content hash) since open; offer **reload** before overwrite. This is critical for all three roots since users may edit the same files outside the desktop (e.g., with a text editor or CLI).

### Validation

- **JSON files**: Parse as JSON before write; reject invalid JSON with a readable error. For `tools.json`, validate against [TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) or a serde round-trip through existing descriptor types where practical.
- **Markdown files**: No structural validation beyond UTF-8.
- **Other text files**: No validation beyond UTF-8.

### Skills Directory — Versioned Layout

The skills directory uses a versioned layout (see [spec/SKILL_PACKAGES.md](../spec/SKILL_PACKAGES.md)):

```
skills/<name>/
  active → versions/a1b2c3d/
  versions/
    a1b2c3d/
      SKILL.md
      tools.json
      scripts/
    f8e9d0b/
      SKILL.md
      tools.json
      scripts/
```

The tree view should show this structure clearly:

- Each skill package as a top-level node under Skills
- The `active` symlink resolved to show which version is current (e.g., "active → a1b2c3d")
- Version snapshots as sub-nodes, with the active one indicated visually
- Only the **active version** is editable; historical versions are read-only

Editing a skill's `SKILL.md` or `tools.json` always writes to the active version's files. This is consistent with how the gateway reads skills (resolves `active` symlink).

**Editing skills carries risk**: the skill lockfile (`skills.lock`) pins the active version's content hash. If the user edits a skill file, the on-disk content no longer matches the locked hash. On gateway restart with `lockMode: "strict"` (the default), the gateway will refuse to start. The desktop must communicate this clearly (see Open Questions).

### Sandbox Directory — Symlinks and New-Files Screen

The sandbox directory contains:

- Regular files and directories (markdown, notes, etc.)
- **Symlinks** to external directories (the primary mechanism for granting the agent access to external content)

The tree view follows symlinks transparently — when a symlink points to an external directory, its contents are shown as if they were local. This matches the sandbox spec: the gateway canonicalizes symlink targets and adds them as writable roots.

**New-files screen** — A dedicated UI (likely a modal or sub-screen) for creating and removing symlinks in the sandbox directory:

- **Add**: A button opens a native file explorer dialog (via `rfd` or equivalent). The user selects a file or directory; the desktop creates a symlink in `<profileRoot>/sandbox/` pointing to the selected target. The symlink name defaults to the target's filename.
- **Remove**: A button next to each symlink in the sandbox tree removes the symlink (not the target). This revokes write access on the next gateway restart.
- **Rename**: Optionally allow renaming the symlink (changing the name it appears under in the sandbox without changing the target).
- This is a **command-line-free alternative** to `ln -s <target> ~/.chai/profiles/<profile>/sandbox/<name>`.

Symlink creation and removal require a gateway restart to take effect (the writable root set is frozen at construction time). The desktop should show the apply/restart banner after these operations.

### Scope Messaging

UI copy should make clear which root the user is browsing (Agents, Skills, or Sandbox) and that these are Chai-specific directories — not a general file manager for arbitrary paths.

## Requirements

### Phase 1 — Agents Directory

- [ ] **Agents tree view** — Browse `<profileRoot>/agents/` showing all agent subdirectories and their `AGENT.md` files.
- [ ] **Agent file editor** — Open and edit `AGENT.md` for any agent (orchestrator or worker) in a plain-text editor.
- [ ] **Save** — Write edited `AGENT.md` back to disk.
- [ ] **Revert** — Discard unsaved changes and reload from disk.
- [ ] **Apply/restart banner** — After saving `AGENT.md`, prompt to restart gateway (when desktop owns the subprocess, offer Restart action).
- [ ] **Concurrency detection** — Detect external modification since open; offer reload before overwrite.
- [ ] **Create missing `AGENT.md`** — When an agent's `AGENT.md` does not exist, offer to create it (mirror `chai init` behavior).
- [ ] **Sidebar entry** — Uncomment the Files entry in the sidebar, placed under the "Agents" group.
- [ ] **UTF-8 validation** — Reject non-UTF-8 content before write.

### Phase 2 — Skills Directory

- [ ] **Skills tree view** — Browse `~/.chai/skills/` showing skill packages, version snapshots, and the `active` symlink.
- [ ] **Active version indicator** — Visually indicate which version is `active` in the tree view.
- [ ] **Skill file editor** — Open and edit `SKILL.md` and `tools.json` from the active version in a plain-text editor.
- [ ] **Historical versions read-only** — Files in non-active version snapshots are read-only.
- [ ] **JSON validation** — Parse and validate `tools.json` before write; reject invalid JSON with a readable error; pretty-print on save.
- [ ] **Save** — Write edited files back to disk (active version only).
- [ ] **Revert** — Discard unsaved changes and reload from disk.
- [ ] **Apply/restart banner** — After saving skill files, prompt to restart gateway.
- [ ] **Concurrency detection** — Detect external modification since open; offer reload before overwrite.
- [ ] **Lockfile mismatch warning** — Warn the user that editing skill files may cause the lockfile to mismatch on restart (see Open Questions).
- [ ] **Root switching** — Allow the user to switch between the Agents and Skills roots in the tree view.

### Phase 3 — Sandbox Directory

- [ ] **Sandbox tree view** — Browse `<profileRoot>/sandbox/` including symlinked directories (followed transparently).
- [ ] **Plain-text editor** — Open and edit any file that can be rendered as plain text (markdown, JSON, source code, config files, etc.).
- [ ] **Binary file handling** — Show a message for binary files that cannot be rendered as text (or display read-only with a "this is a binary file" notice).
- [ ] **Save** — Write edited files back to disk.
- [ ] **Revert** — Discard unsaved changes and reload from disk.
- [ ] **Concurrency detection** — Detect external modification since open; offer reload before overwrite.
- [ ] **No `.git/` writes** — Reject writes targeting any `.git/` directory component (same policy as the write sandbox spec).
- [ ] **New-files screen** — GUI for creating symlinks in the sandbox directory:
  - [ ] **Add symlink** — Open a native file explorer dialog; create a symlink in the sandbox directory pointing to the selected file or directory.
  - [ ] **Remove symlink** — Remove a symlink from the sandbox directory (not the target).
  - [ ] **Rename symlink** — Optionally rename a symlink without changing the target.
- [ ] **Apply/restart banner** — After creating or removing symlinks, prompt to restart gateway (writable root set is frozen at construction time).
- [ ] **Root switching** — Allow the user to switch between Agents, Skills, and Sandbox roots in the tree view.

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 — Agents directory | Browse agents tree; edit `AGENT.md` per agent; apply/restart banner; concurrency detection; sidebar entry | Pending |
| 2 — Skills directory | Browse skills tree with versioned layout; edit active `SKILL.md` / `tools.json`; JSON validation; lockfile mismatch warning | Pending |
| 3 — Sandbox directory | Browse sandbox tree with symlinked dirs; plain-text editor for any text format; binary file handling; new-files screen for symlink management; `.git/` write exclusion | Pending |

## Open Questions

- **Skill editing and lockfile interaction**: When the user edits a skill file in the active version, the on-disk content no longer matches the pinned hash in `skills.lock`. On gateway restart with `lockMode: "strict"`, the gateway refuses to start. Options: (a) warn the user before saving that this will invalidate the lockfile and suggest running `chai skill lock` afterward, (b) offer an in-app "re-lock" action that updates the lockfile, (c) only allow editing when `lockMode: "warn"`, (d) something else. The right UX here needs further exploration.
- **Symlink name conflicts**: When the user adds a symlink via the new-files screen and a file or directory with the same name already exists in the sandbox, what happens? Options: (a) auto-append a suffix, (b) prompt for a different name, (c) refuse and show an error. This should be resolved before implementing phase 3.
- **Sandbox edit restart semantics**: Sandbox files are used by the agent during tool execution, not loaded at startup like config and skills. Some sandbox file edits may take effect immediately (the agent reads the file on the next tool call), while others may not. Should the restart banner always appear for sandbox edits, or only when the change affects the writable root set (symlink add/remove)? This needs clarification during phase 3 design.
- **Native file dialog library**: The new-files screen requires a native file explorer dialog. `rfd` (Rust File Dialog) is the common choice for egui apps, but it adds a dependency. Evaluate options during phase 3 implementation.

## Related Epics and Docs

- [spec/DESKTOP.md](../spec/DESKTOP.md) — Current state of the desktop application
- [spec/AGENTS.md](../spec/AGENTS.md) — Per-agent context directories and `AGENT.md` files
- [spec/SKILL_FORMAT.md](../spec/SKILL_FORMAT.md) — Skill directory layout, `SKILL.md` content, and frontmatter
- [spec/SKILL_PACKAGES.md](../spec/SKILL_PACKAGES.md) — Skill package versioned layout, content hashing, and lockfiles
- [spec/SANDBOX.md](../spec/SANDBOX.md) — Write sandbox, writable roots, symlink-as-authorization
- [spec/PROFILES.md](../spec/PROFILES.md) — Profile directory structure (agents, sandbox, skills.lock)
- [spec/TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) — `tools.json` validation reference
- [spec/CONTEXT.md](../spec/CONTEXT.md) — What the gateway sends as context (system message, skills)
- [adr/DESKTOP_FRAMEWORK.md](../adr/DESKTOP_FRAMEWORK.md) — Why egui/eframe
