---
status: accepted
---

# SKILL_DESCRIPTOR_SPLIT

## Context

The skill system's `tools.json` file has a single root object with three top-level sections: `tools` (tool schemas for the LLM), `allowlist` (binary→subcommand security grants), and `execution` (per-tool implementation mapping). These three sections serve fundamentally different roles, are consumed by different parts of the system, are authored with different concerns, and change at different rates:

| Section | Audience | Purpose | Change Rate |
|---------|----------|---------|-------------|
| `tools` | The LLM (agent) | Tool names, descriptions, parameter schemas — what the model sees and calls | Rare for stable skills |
| `allowlist` | The runtime executor | Binary→subcommand enforcement — what the security boundary permits | Very rare |
| `execution` | The runtime executor | Binary, subcommand, arg mapping, hints, deny patterns, postProcess — how to actually run each tool | Most common |

This monolithic format creates several problems:

1. **Mixed concerns in skill authoring.** The `tools` section is about *communication* (what to tell the model). The `allowlist` section is about *security* (what the system is allowed to do). The `execution` section is about *implementation* (how to make it work). Skill authors must edit one large JSON file that intermingles three different mental models.

2. **The allowlist is buried.** The allowlist is a security document — every entry is a capability grant. In the current format it is embedded inside `tools.json` alongside descriptive and implementation details, making it easy to overlook during security review. The git skill's `tools.json` is 923 lines; its `allowlist` is 20 lines buried within.

3. **Variant skills duplicate all three sections.** Read-only variants (`files-read`, `git-read`) must contain a complete copy of the base skill's `tools.json`, even though they differ only in which tools are present. The `files-read` skill has 192 lines of `tools.json` that are a strict subset of `files`'s 210 lines.

4. **The data model already separates them.** The Rust `ToolDescriptor` struct has three distinct fields (`tools`, `allowlist`, `execution`). The `to_tool_definitions()` method only reads `tools`, `to_allowlist()` only reads `allowlist`, and the executor uses `execution` independently. The file format bundles what the code already treats as separate.

5. **Different change rates create unnecessary coupling.** A hint tweak in `execution` requires editing the same file that contains the `allowlist` and the `tools` schema. Any change to any section means validating the whole file. The most volatile section is coupled to the most security-critical one.

## Decision

Split `tools.json` into three files with distinct responsibilities:

| File | Content | Audience |
|------|---------|----------|
| `tools.json` | Tool definitions: name, description, parameter schemas | The LLM (agent) |
| `allowlist.json` | Binary→subcommand security grants | The runtime executor |
| `execution.json` | Per-tool execution mapping: binary, subcommand, args, hints, deny patterns, postProcess, sideRead | The runtime executor |

### File Schemas

**`tools.json`** — Tool definitions for the LLM. Contains only the `tools` array from the current format:

```json
[
  {
    "name": "files_read",
    "description": "Read the contents of a file.",
    "parameters": {
      "type": "object",
      "required": ["path"],
      "properties": {
        "path": {
          "type": "string",
          "description": "File path relative to the sandbox root (use ./ prefix)"
        }
      }
    }
  }
]
```

The root is an array, not an object. This is the simplest possible format: a list of tool schemas that the loader converts directly to `Vec<ToolDefinition>` for the LLM. No wrapper object needed because there is only one concern.

**`allowlist.json`** — Security grants for the runtime. Contains only the `allowlist` object from the current format:

```json
{
  "cat": [""],
  "ls": [""],
  "grep": ["", "-E"],
  "chai": ["file write", "file delete", "file patch", "file read-lines", "file delete-dir", "file replace"]
}
```

The root is the same `binary → subcommands` map. A standalone security document that can be reviewed independently.

**`execution.json`** — Implementation mapping for the runtime. Contains only the `execution` array from the current format:

```json
[
  {
    "tool": "files_read",
    "binary": "cat",
    "subcommand": "",
    "args": [
      {
        "param": "path",
        "kind": "positional",
        "readPath": true
      }
    ],
    "successExitCodes": [1],
    "hintConditions": [
      {
        "exitCode": "nonzero",
        "hint": "file not found — use files_list to browse available files"
      }
    ],
    "maxOutputLines": 500,
    "truncationHint": "output truncated: {kept} of {total} lines shown; {omitted} more lines available. To continue reading, use files_read with start_line: {next_start}."
  }
]
```

The root is an array. Each entry maps a tool name to its execution logic — the same `ExecutionSpec` objects from the current format.

### Loader Behavior

The loader reads all three files and constructs the same `ToolDescriptor` struct. The in-memory representation is unchanged; only the serialization format changes.

1. Read `tools.json` → deserialize as `Vec<ToolSpec>` → `ToolDescriptor.tools`
2. Read `allowlist.json` → deserialize as `HashMap<String, Vec<String>>` → `ToolDescriptor.allowlist`
3. Read `execution.json` → deserialize as `Vec<ExecutionSpec>` → `ToolDescriptor.execution`

If `tools.json` is absent, the skill has no tools (same as current behavior). If `allowlist.json` or `execution.json` is absent but `tools.json` is present, the skill is invalid — a skill with tool definitions must declare its security grants and execution mapping. The loader logs a warning and treats the skill as having no descriptor, consistent with current parse-error behavior.

### Backward Compatibility

The current single-file `tools.json` format (root object with `tools`, `allowlist`, `execution` keys) continues to be supported during a migration period. The loader detects the format:

- If `tools.json` has a root object with `tools`/`allowlist`/`execution` keys → legacy format, parse as before.
- If `tools.json` has a root array → new format, parse as `Vec<ToolSpec>`, then read `allowlist.json` and `execution.json`.

This allows a gradual migration: bundled skills can be migrated in batches, and custom skills continue to work until their authors adopt the new format. A deprecation warning is logged when the legacy format is detected.

### Validation

Cross-file consistency is validated by `skills_validate`:

- Every tool name in `execution.json` must have a matching entry in `tools.json`.
- Every `(binary, subcommand)` pair in `execution.json` must be present in `allowlist.json`.
- Every tool name in `tools.json` must have a matching entry in `execution.json`.

These are the same consistency checks currently performed within a single file, extended to work across three files.

### Skill Directory Layout

Before:
```
<skill_dir>/
  scripts/
  SKILL.md
  tools.json
```

After:
```
<skill_dir>/
  scripts/
  allowlist.json
  execution.json
  SKILL.md
  tools.json
```

The `scripts/` directory is unchanged. It is referenced only by `execution.json` entries (via `resolveCommand.script` and `postProcess.script`), which is the correct scope.

### Impact on the `skills` Skill

The `skills` skill's authoring tools must be updated:

- `skills_write_tools_json` → writes only the `tools.json` file (tool definitions).
- New `skills_write_allowlist_json` → writes `allowlist.json`.
- New `skills_write_execution_json` → writes `execution.json`.
- `skills_init` → creates `tools.json`, `allowlist.json`, and `execution.json` (or just `tools.json` for a context-only skill that gains tools later).
- `skills_validate` → validates cross-file consistency across the three files.

The `skills-read` skill's `skills_read_file` tool gains two new `file` enum values: `allowlist_json` and `execution_json`.

### Impact on the Versioned Package Layout

The content-addressed hash in `versions/<hash>/` covers all files in the skill directory. After the split, the hash includes `tools.json`, `allowlist.json`, and `execution.json` individually. Changing any one file produces a new hash — same as changing `tools.json` today. No changes to the versioning model are needed.

### Impact on the Desktop Skill Editor

The desktop skill editor (epic `DESKTOP_FILES`) currently shows `SKILL.md` and `tools.json` as read-only. After the split, it shows four files: `SKILL.md`, `tools.json`, `allowlist.json`, `execution.json`. JSON validation applies to each file independently.

## Alternatives Considered

| Alternative | Why Not Chosen |
|-------------|----------------|
| **Keep single `tools.json`** | Does not address mixed concerns, buried allowlist, or variant duplication. The current format works but does not scale to the variant-sharing and deeper-validation improvements the project needs. |
| **Two-file split (`tools.json` + `allowlist.json`)** | Isolates the allowlist (the strongest reason for separation) but keeps `tools` and `execution` coupled. These two sections change together more often than `allowlist` changes, but they serve different audiences and have different concerns. The two-file split also does not enable variant sharing as cleanly, because `execution.json` and `tools.json` remain coupled in one file. |
| **Keep single `tools.json` with section annotations** | Adds comments or markers within the JSON to visually separate sections. Does not actually separate the concerns — the file is still monolithic, the allowlist is still buried, and variant skills still duplicate the whole thing. JSON does not support comments, so annotations would require a custom preprocessor. |
| **YAML or TOML instead of JSON** | Would allow comments and a more human-readable format, but breaks compatibility with JSON Schema tool definitions (which must be valid JSON for LLM function-calling APIs) and would require rewriting the entire loader and validation pipeline. The three-file split achieves the separation goals without changing the data format. |

## Consequences

**Positive:**

- **Clearer separation of concerns.** Skill authors work with smaller, focused files. Security reviewers can audit `allowlist.json` independently. API design reviews focus on `tools.json`. Implementation reviews focus on `execution.json`.
- **Allowlist becomes a first-class security document.** Standalone, auditable, and subject to its own validation rules. This enables the deeper security validation proposed in the skill architecture audit's L4 improvement.
- **Variant skills can share files.** The split is a structural precondition for the skill inheritance/composition system (audit L1). A variant can reference a base skill's `execution.json` and `allowlist.json` without duplicating them, while providing only its own `tools.json` with the subset of tools it exposes.
- **Different change rates are decoupled.** A hint tweak in `execution.json` does not require touching the security document or the API schema. The most volatile section is isolated from the most security-critical one.
- **Smaller, more reviewable files.** The `files` skill goes from one 210-line file to three files of ~80/10/120 lines. The `git` skill goes from one 923-line file to three files of ~200/20/700 lines. Each file has a single purpose.

**Negative:**

- **More files per skill.** The skill directory goes from 2 files (`SKILL.md`, `tools.json`) to 4 files (`SKILL.md`, `tools.json`, `allowlist.json`, `execution.json`). This is a small increase and each file is simpler than the original.
- **Cross-file consistency must be validated.** Tool names in `execution.json` must match `tools.json`. Binary/subcommand pairs in `execution.json` must be in `allowlist.json`. This is a validation concern, not a conceptual one — `skills_validate` already checks cross-references within a single file.
- **Migration effort.** All 16 bundled skills must be migrated from the single-file format to the three-file format. The loader, validator, `skills` skill, `skills-read` skill, and desktop skill editor must be updated. The legacy format is supported during migration to allow gradual adoption.
- **The `skills` skill's authoring workflow gains two new tools.** `skills_write_allowlist_json` and `skills_write_execution_json` replace the single `skills_write_tools_json`. This is a surface increase, but it aligns the authoring API with the new file structure and each tool has a clearer purpose.

## References

- [TOOLS_SCHEMA.md](../spec/TOOLS_SCHEMA.md) — Current `tools.json` schema specification
- [SKILL_FORMAT.md](../spec/SKILL_FORMAT.md) — Skill directory layout and frontmatter conventions
- [SKILL_PACKAGES.md](../spec/SKILL_PACKAGES.md) — Versioned package model (content-addressed hashing)
- [DIAGNOSTIC_HINTS.md](DIAGNOSTIC_HINTS.md) — Three-tier hint architecture (hintConditions, postProcess, binary-level)
- [WRITE_SANDBOX.md](WRITE_SANDBOX.md) — Per-profile write sandbox (consumes `writePath`/`readPath` from execution specs)
