# Tools Schema (tools.json)

When a skill directory contains a `tools.json` file, the loader parses it and attaches tool definitions, an allowlist, and per-tool execution mapping to the skill. This allows skills to declare their tools declaratively so a generic executor can run them without per-skill code.

## File Location

- **Path**: `<skill_dir>/tools.json` (same directory as `SKILL.md`).
- **Optional**: If absent, the skill has no descriptor; the skill contributes no tools and does not function for tool execution. Everything tool-related is defined in `tools.json`; the lib is generic and has no skill-specific code.

## Schema

Root object:

| Field | Type | Description |
|-------|------|-------------|
| `tools` | array | Tool definitions for the LLM (name, description, parameters schema). |
| `allowlist` | object | Binary name → array of allowed subcommands. Only these (binary, subcommand) pairs may be run. |
| `execution` | array | Per-tool execution: how to run each tool (binary, subcommand, arg mapping). |

All keys are **camelCase** in JSON.

### `tools` (array of tool spec)

Each element:

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Tool name (e.g. `notesmd_cli_search`). Must match an execution spec. |
| `description` | string (optional) | Short description for the model. |
| `parameters` | object | JSON schema for parameters (same shape Ollama expects: `type`, `properties`, `required`, etc.). |

### `allowlist` (object)

- **Keys**: Binary name (e.g. `notesmd-cli`, `obsidian`).
- **Values**: Array of allowed subcommand strings (e.g. `["search", "search-content", "create"]`).

Only (binary, subcommand) pairs listed here may be executed. The safe exec layer enforces this.

### `execution` (array of execution spec)

Each element:

| Field | Type | Description |
|-------|------|-------------|
| `tool` | string | Tool name (must match a `tools[].name`). |
| `binary` | string | Binary to run (e.g. `notesmd-cli`). Must be a key in `allowlist`. |
| `subcommand` | string | Subcommand (e.g. `search`). Must be in `allowlist[binary]`. |
| `args` | array (optional) | Order of arguments: how each JSON param becomes a CLI arg. |

#### `args` (array of arg mapping)

Each element:

| Field | Type | Description |
|-------|------|-------------|
| `param` | string | JSON parameter name (e.g. `query`). |
| `kind` | string | `"positional"`, `"flag"`, or `"flagifboolean"`. Default `positional`. |
| `flag` | string (optional) | For `kind: "flag"`, the flag name (e.g. `content` → `--content`). If absent, uses `param`. For `flag`, if the parameter is missing or null, the flag and value are omitted (optional params). |
| `flagIfTrue` | string (optional) | For `kind: "flagifboolean"`, the flag to emit when the param is true (e.g. `"--overwrite"`). |
| `flagIfFalse` | string (optional) | For `kind: "flagifboolean"`, the flag to emit when the param is false (e.g. `"--append"`). |
| `normalizeNewlines` | boolean (optional) | When `true`, string values have literal `\n` and `\t` converted to real newlines and tabs before being passed to the CLI. Default: not set. |
| `resolveCommand` | object (optional) | Resolve the param value by running a **script** (when `skills.allowScripts` is true) or an **allowlisted command**; trimmed stdout becomes the new value. On failure or empty stdout, the original value is kept. See below. Default: not set. |

#### `resolveCommand` (object)

Use either **script** (no allowlist entry) or **binary** + **subcommand** (allowlisted).

| Field | Type | Description |
|-------|------|-------------|
| `script` | string (optional) | Name of a file in the skill’s **`scripts/`** directory (e.g. `"resolve-daily-path"` → `scripts/resolve-daily-path.sh`). Used only when **`skills.allowScripts`** is true in config; the executor runs it via `sh` with no allowlist entry. Script name must not contain `..`, `/`, or `\`. |
| `binary` | string (optional) | Binary name for allowlisted command resolution (must be in the skill’s allowlist). Use when not using `script`. |
| `subcommand` | string (optional) | Subcommand for allowlisted command (must be in allowlist for that binary). Use when not using `script`. |
| `args` | array of strings | Arguments; `"$param"` is replaced by the current param value. |

When `script` is set and `skills.allowScripts` is true, the executor runs `sh <skill_dir>/scripts/<script> <args...>`. When `binary` and `subcommand` are set, the executor runs them via the allowlist. No extra setup (allowlist entry or separate binary) is required for scripts when allowScripts is enabled.

## Example (minimal)

One tool, one positional argument:

```json
{
  "tools": [
    {
      "name": "notesmd_cli_search",
      "description": "Search notes by name.",
      "parameters": {
        "type": "object",
        "required": ["query"],
        "properties": {
          "query": { "type": "string", "description": "Search query for note names" }
        }
      }
    }
  ],
  "allowlist": {
    "notesmd-cli": ["search", "search-content", "create", "daily", "print", "print-default"]
  },
  "execution": [
    {
      "tool": "notesmd_cli_search",
      "binary": "notesmd-cli",
      "subcommand": "search",
      "args": [{ "param": "query", "kind": "positional" }]
    }
  ]
}
```

## Implementation Notes

- **Loader**: `load_skills` reads `tools.json` from each skill dir; on success, sets `SkillEntry.tool_descriptor`. On parse error, logs a warning and leaves `tool_descriptor` as `None`.
- **Gateway**: Tool list and executor are built only from skills that have a `tools.json` descriptor. There is no hardcoded skill code in the lib; skills without a descriptor contribute no tools. When **`skills.contextMode`** is **`readOnDemand`**, the gateway also registers a **`read_skill(skill_name)`** tool and uses an executor that returns that skill’s SKILL.md content in-process; see [AGENT_CONTEXT.md](AGENT_CONTEXT.md).
- **Conversion**: `ToolDescriptor::to_tool_definitions()` produces `Vec<ToolDefinition>` for the Ollama API. `ToolDescriptor::to_allowlist()` produces `exec::Allowlist` for the safe exec layer. The generic executor uses the execution mapping to build argv (applying `normalizeNewlines` and `resolveCommand` when set) and runs via the allowlist.
- **Scripts**: When **`skills.allowScripts`** is true in config, a skill may place scripts in a **`scripts/`** directory and reference them in `resolveCommand.script`. The executor runs only files under that directory via `sh`; no allowlist entry is needed. Default is false (scripts are not run).
- **Resolvers**: Param resolution is generic (run a script or an allowlisted command, use stdout). Skill-specific logic (e.g. resolving a bare date to a daily-note path) can live in a script in the skill’s `scripts/` dir (when allowScripts is true) or in a separate binary the skill allowlists; lib, CLI, and desktop contain no skill- or tool-specific code.
