---
status: stable
---

# Tools Schema (tools.json)

When a skill directory contains a `tools.json` file, the loader parses it and attaches tool definitions, an allowlist, and per-tool execution mapping to the skill. This allows skills to declare their tools declaratively so a generic executor can run them without per-skill code.

**Parameters (JSON Schema):** Each tool’s **`parameters`** object uses the same **JSON Schema subset** used across LLM **function / tool** APIs: typically `type: "object"`, **`properties`**, **`required`**, and per-argument **`type`**, **`description`**, and optional constraints. That matches what **OpenAI** (tools / function parameters), **Ollama** (`tools` in chat), and **OpenAI-compatible** servers expect. Chai forwards the descriptor’s tool list to the active **`Provider`** without rewriting the schema. For examples and field conventions, see vendor docs (e.g. OpenAI function-calling parameter shape).

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
| `name` | string | Tool name (e.g. `notesmd_search`). Must match an execution spec. |
| `description` | string (optional) | Short description for the model. |
| `parameters` | object | JSON Schema for arguments (see **Parameters (JSON Schema)** above). |

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
| `postProcess` | object (optional) | Post-process the command's stdout through a script before returning the result to the model. See below. Default: not set. |

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
| `resolveCommand` | object (optional) | Resolve the param value by running a **script** or an **allowlisted command**; trimmed stdout becomes the new value. On failure or empty stdout, the original value is kept. See below. Default: not set. |
| `optional` | boolean (optional) | When `true`, a missing or null JSON parameter is omitted from argv. Exception: for `positional` with `resolveCommand` set, a missing parameter is passed to the resolver as an empty string so the resolver can still produce a value (e.g. default paths). Default: not set (required). |
| `disambiguateAfterSkippedPositionals` | boolean (optional) | For `kind: "positional"` only: when `true`, the executor inserts `--` before this argument’s value if any earlier optional positional in the same `args` list was skipped. Use when a path must be disambiguated from an omitted ref (e.g. `git diff`). Default: not set. |
| `writePath` | boolean (optional) | When `true`, this parameter is a filesystem write target. The executor validates the resolved value against the per-profile write sandbox before execution. If validation fails, the tool call is rejected. Only applies to `positional` and `flag` kinds (not `flagifboolean`). Default: not set. See **[epic/WRITE_SANDBOX.md](../epic/WRITE_SANDBOX.md)**. |

#### `resolveCommand` (object)

Use either **script** (no allowlist entry) or **binary** + **subcommand** (allowlisted).

| Field | Type | Description |
|-------|------|-------------|
| `script` | string (optional) | Name of a file in the skill’s **`scripts/`** directory (e.g. `"resolve-daily-path"` → `scripts/resolve-daily-path.sh`). The executor runs it via `sh` with no allowlist entry, and only files under the skill’s `scripts/` dir are executed. Script name must not contain `..`, `/`, or `\`. |
| `binary` | string (optional) | Binary name for allowlisted command resolution (must be in the skill’s allowlist). Use when not using `script`. |
| `subcommand` | string (optional) | Subcommand for allowlisted command (must be in allowlist for that binary). Use when not using `script`. |
| `args` | array of strings | Arguments; `"$param"` is replaced by the current param value. |

When `script` is set, the executor runs `sh <skill_dir>/scripts/<script> <args...>`. When `binary` and `subcommand` are set, the executor runs them via the allowlist. No extra setup (allowlist entry or separate binary) is required for scripts.

#### `postProcess` (object)

Post-processes the command's stdout through a script or allowlisted command. The raw stdout is piped to the post-processor's **stdin**; its own stdout becomes the tool result returned to the model. On failure or empty stdout, the original output is returned unmodified.

Use either **script** (no allowlist entry) or **binary** + **subcommand** (allowlisted), same as `resolveCommand`.

| Field | Type | Description |
|-------|------|-------------|
| `script` | string (optional) | Name of a file in the skill's **`scripts/`** directory (e.g. `"parse-rss"`). Same path rules as `resolveCommand.script`. |
| `binary` | string (optional) | Binary name for allowlisted post-processing (must be in the skill's allowlist). |
| `subcommand` | string (optional) | Subcommand for allowlisted command (must be in allowlist for that binary). |
| `args` | array of strings | Additional arguments passed to the script or command. No `$param` substitution (the input comes via stdin). |

**Design notes:**
- `postProcess` is set on the **execution spec** (per-tool), not on individual args. It transforms the final stdout, not a parameter value.
- Stdin piping (not command-line args) is used because tool output can be large (RSS feeds, HTML pages, search results).
- The symmetry with `resolveCommand` is intentional: `resolveCommand` mediates input (parameter → resolved value), `postProcess` mediates output (stdout → structured result).

**Example:** Parse RSS XML into structured text:
```json
{
  "tool": "rss_check_feed",
  "binary": "curl",
  "subcommand": "-sf --max-time 10",
  "args": [
    { "param": "feed", "kind": "positional", "resolveCommand": { "script": "resolve-feed-url", "args": ["$param"] } }
  ],
  "postProcess": {
    "script": "parse-rss"
  }
}
```

## Example (minimal)

One tool, one positional argument:

```json
{
  "tools": [
    {
      "name": "notesmd_search",
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
      "tool": "notesmd_search",
      "binary": "notesmd-cli",
      "subcommand": "search",
      "args": [{ "param": "query", "kind": "positional" }]
    }
  ]
}
```

## Implementation Notes

- **Loader**: `load_skills` reads `tools.json` from each skill dir; on success, sets `SkillEntry.tool_descriptor`. On parse error, logs a warning and leaves `tool_descriptor` as `None`.
- **Gateway**: Tool list and executor are built only from skills that have a `tools.json` descriptor. There is no hardcoded skill code in the lib; skills without a descriptor contribute no tools. When **`skills.contextMode`** is **`readOnDemand`**, the gateway also registers a **`read_skill(skill_name)`** tool and uses an executor that returns that skill’s SKILL.md content in-process; see [CONTEXT.md](CONTEXT.md).
- **Conversion**: `ToolDescriptor::to_tool_definitions()` produces `Vec<ToolDefinition>` in the shape expected by the active LLM **`Provider`** (Ollama-native and OpenAI-compat backends accept the same function-tool schema in practice). `ToolDescriptor::to_allowlist()` produces `exec::Allowlist` for the safe exec layer. The generic executor uses the execution mapping to build argv (applying `normalizeNewlines` and `resolveCommand` when set) and runs via the allowlist.
- **Scripts**: A skill may place scripts in a **`scripts/`** directory and reference them in `resolveCommand.script`. The executor runs only files under that directory via `sh`; no allowlist entry is needed.
- **Resolvers**: Param resolution is generic (run a script or an allowlisted command, use stdout). Skill-specific logic (e.g. resolving a bare date to a daily-note path) can live in a script in the skill’s `scripts/` dir or in a separate binary the skill allowlists; lib, CLI, and desktop contain no skill- or tool-specific code.
- **Post-processing**: When `postProcess` is set on an execution spec, the executor pipes the command’s stdout to the post-processor’s stdin and returns the post-processor’s stdout instead. On failure or empty output, the original stdout is returned unmodified. Same script resolution rules as `resolveCommand` (skill’s `scripts/` dir, no allowlist needed).
