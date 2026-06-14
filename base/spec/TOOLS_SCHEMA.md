---
status: stable
---

# Tools Schema (tools.json)

When a skill directory contains a `tools.json` file, the loader parses it and attaches tool definitions, an allowlist, and per-tool execution mapping to the skill. This allows skills to declare their tools declaratively so a generic executor can run them without per-skill code.

**Parameters (JSON Schema):** Each tool's **`parameters`** object uses the same **JSON Schema subset** used across LLM **function / tool** APIs: typically `type: "object"`, **`properties`**, **`required`**, and per-argument **`type`**, **`description`**, and optional constraints. That matches what **OpenAI** (tools / function parameters), **Ollama** (`tools` in chat), and **OpenAI-compatible** servers expect. Chai forwards the descriptor's tool list to the active **`Provider`** without rewriting the schema. For examples and field conventions, see vendor docs (e.g. OpenAI function-calling parameter shape).

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
| `name` | string | Tool name (e.g. `files`, `git`, `kb`). Must match an execution spec. |
| `description` | string (optional) | Short description for the model. |
| `parameters` | object | JSON Schema for arguments (see **Parameters (JSON Schema)** above). |

### `allowlist` (object)

- **Keys**: Binary name (e.g. `chai`).
- **Values**: Array of allowed subcommand strings (e.g. `["file read", "file write"]`).

Only (binary, subcommand) pairs listed here may be executed. The safe exec layer enforces this.

### `execution` (array of execution spec)

Each element:

| Field | Type | Description |
|-------|------|-------------|
| `tool` | string | Tool name (must match a `tools[].name`). |
| `binary` | string | Binary to run (e.g. `chai`). Must be a key in `allowlist`. |
| `subcommand` | string | Subcommand (e.g. `files read`). Must be in `allowlist[binary]`. The value is split by whitespace and each token is prepended before the `args` list when building the command. This allows fixed flags to be encoded as part of the subcommand (e.g. `"-E"` for `grep -E`). |
| `args` | array (optional) | Order of arguments: how each JSON param becomes a CLI arg. |
| `successExitCodes` | array of integers (optional) | Exit codes to treat as success (in addition to 0). Use when a non-zero exit is a normal result, not an error (e.g. `[0, 1]` for `grep` where exit 1 means no matches). Exit codes not in this list (and not 0) surface as tool errors. Default: only exit 0 is success. |
| `postProcess` | object (optional) | Post-process the command's stdout through a script before returning the result to the model. See below. Default: not set. |
| `sideRead` | object (optional) | After the command (and any `postProcess`) completes, look for a file relative to a path parameter and append its contents to the tool result. Silently skipped when the file is absent. See below. Default: not set. |
| `maxOutputLines` | integer (optional) | Maximum number of output lines to return to the model. When set, output exceeding this limit is truncated and a notice is appended indicating how many lines were omitted and suggesting the agent narrow its query. This prevents unbounded tool output (e.g. from `grep` or `git diff`) from exceeding the model's context window. Applies after `postProcess` but before `sideRead` (side-read content is not counted against the limit and is always appended in full). Default: not set (no limit). |

#### `args` (array of arg mapping)

Each element:

| Field | Type | Description |
|-------|------|-------------|
| `param` | string | JSON parameter name (e.g. `query`). |
| `kind` | string | `"positional"`, `"flag"`, `"flagifboolean"`, `"stdin"`, or `"workingdir"`. Default `positional`. `"stdin"` pipes the parameter value to the child process's stdin instead of passing it as a CLI argument. Required for any parameter that may contain multiline content — `kind: "flag"` causes `clap` to break on newlines. Only one `stdin` arg is allowed per execution spec. `"workingdir"` sets the process working directory to the resolved value (the value is validated against the sandbox like `readPath` and used as `current_dir`, but is **not** added to argv). |
| `flag` | string (optional) | For `kind: "flag"`, the flag name. Single-character names produce short flags (e.g. `"n"` → `-n`); multi-character names produce long flags (e.g. `"path"` → `--path`). If absent, uses `param` (always a long flag). For `flag`, if the parameter is missing or null, the flag and value are omitted (optional params). |
| `flagIfTrue` | string (optional) | For `kind: "flagifboolean"`, the flag to emit when the param is true (e.g. `"--overwrite"`). |
| `flagIfFalse` | string (optional) | For `kind: "flagifboolean"`, the flag to emit when the param is false (e.g. `"--append"`). |
| `resolveCommand` | object (optional) | Resolve the param value by running a **script** or an **allowlisted command**; trimmed stdout becomes the new value. On failure or empty stdout, the original value is kept. See below. Default: not set. |
| `optional` | boolean (optional) | When `true`, a missing or null JSON parameter is omitted from argv. Exception: when `resolveCommand` is set, a missing parameter is passed to the resolver as an empty string so the resolver can still produce a value (e.g. default paths). This applies to `kind: "positional"`, `"flag"`, and `"workingdir"`. Default: not set (required). |
| `disambiguateAfterSkippedPositionals` | boolean (optional) | For `kind: "positional"` only: when `true`, the executor inserts `--` before this argument's value if any earlier optional positional in the same `args` list was skipped. Use when a path must be disambiguated from an omitted ref (e.g. `git diff`). Default: not set. |
| `writePath` | boolean (optional) | When `true`, this parameter is a filesystem write target. The executor validates the resolved value against the per-profile write sandbox before execution. If validation fails, the tool call is rejected. Only applies to `positional` and `flag` kinds (not `flagifboolean` or `workingdir`). Default: not set. See **[SANDBOX.md](SANDBOX.md)**. |
| `readPath` | boolean (optional) | When `true`, this parameter is a filesystem read target. The executor validates the resolved value against the per-profile write sandbox before execution. If validation fails, the tool call is rejected. Applies to `positional` and `flag` kinds. `workingdir` args are implicitly validated as read paths — no need to set `readPath: true` on them. Default: not set. See **[SANDBOX.md](SANDBOX.md)**. |
| `unsafePath` | boolean (optional) | When `true`, this parameter is a filesystem path that intentionally needs unrestricted access — it may receive values that resolve outside the sandbox. The executor skips all sandbox validation and the runtime path-like value check. **Every use must be justified.** The gateway logs a startup warning for each `unsafePath` parameter in enabled skills. Default: not set. |
| `normalizeNewlines` | boolean (optional) | **Deprecated — do not use.** Previously performed a second decode of `\n`/`\t` escape sequences after `serde_json` had already decoded them, causing a double-decode bug that corrupted written content. The field is retained in the schema for backward compatibility but should never be set to `true`. |

#### `resolveCommand` (object)

Use either **script** (no allowlist entry) or **binary** + **subcommand** (allowlisted).

| Field | Type | Description |
|-------|------|-------------|
| `script` | string (optional) | Name of a file in the skill's **`scripts/`** directory (e.g. `"resolve-feed-path"` → `scripts/resolve-feed-path.sh`). The executor runs it via `sh` with no allowlist entry, and only files under the skill's `scripts/` dir are executed. Script name must not contain `..`, `/`, or `\`. |
| `binary` | string (optional) | Binary name for allowlisted command resolution (must be in the skill's allowlist). Use when not using `script`. |
| `subcommand` | string (optional) | Subcommand for allowlisted command (must be in allowlist for that binary). Use when not using `script`. |
| `args` | array of strings | Arguments; `"$param"` is replaced by the current param value; `"$param_name"` (e.g. `"$kb_root"`) is replaced by the corresponding parameter value from the tool call JSON (empty string if absent or null). |

When `script` is set, the executor runs `sh <skill_dir>/scripts/<script> <args...>`. When `binary` and `subcommand` are set, the executor runs them via the allowlist. No extra setup (allowlist entry or separate binary) is required for scripts.

#### `postProcess` (object)

Post-processes the command's stdout through a script or allowlisted command. The raw stdout is piped to the post-processor's **stdin**; its own stdout becomes the tool result returned to the model. On failure or empty stdout, the original output is returned unmodified.

Use either **script** (no allowlist entry) or **binary** + **subcommand** (allowlisted), same as `resolveCommand`.

| Field | Type | Description |
|-------|------|-------------|
| `script` | string (optional) | Name of a file in the skill's **`scripts/`** directory (e.g. `"parse-rss"`). Same path rules as `resolveCommand.script`. |
| `binary` | string (optional) | Binary name for allowlisted post-processing (must be in the skill's allowlist). |
| `subcommand` | string (optional) | Subcommand for allowlisted command (must be in allowlist for that binary). |
| `args` | array of strings | Additional arguments passed to the script or command. `"$param_name"` (e.g. `"$kb_root"`) is replaced by the corresponding parameter value from the tool call JSON (empty string if absent or null). |

**Design notes:**
- `postProcess` is set on the **execution spec** (per-tool), not on individual args. It transforms the final stdout, not a parameter value.
- Stdin piping (not command-line args) is used because tool output can be large (RSS feeds, HTML pages, search results).
- The symmetry with `resolveCommand` is intentional: `resolveCommand` mediates input (parameter → resolved value), `postProcess` mediates output (stdout → structured result).

#### `sideRead` (object)

Appends a file's contents to the tool result when the file exists. After the main command and any `postProcess` step, the executor looks for `<resolved-path-param>/<filename>`. If found and non-empty, its contents are appended under a labeled separator. Silently skipped when the file is absent, unreadable, or empty.

| Field | Type | Description |
|-------|------|-------------|
| `pathParam` | string | Name of the arg param whose resolved value is the directory to look in (e.g. `"path"`). Must be a param present in this tool's `args` list. |
| `filename` | string | Filename to look for within that directory (e.g. `"AGENTS.md"`). Must not contain path separators or `..`. |
| `label` | string (optional) | Label shown as a section header before the appended content (e.g. `"Project Instructions"`). Defaults to the filename when absent. |
| `oncePerSession` | boolean (optional) | When `true`, append this file's content at most once per session per unique resolved path. Subsequent tool calls that resolve to the same path within the same session are silently skipped. When no session is present (e.g. direct turn calls without a session store), the check is bypassed and the file is always appended. Default: not set (always append). |

**Output shape:** The appended block is separated from the main output by a blank line and a `--- <label> ---` header line, followed by the file's trimmed content:

```
<main tool output>

--- AGENTS.md (BOF) ---

<file contents>

--- AGENTS.md (EOF) ---
```

**Design notes:**
- `sideRead` is set on the **execution spec** (per-tool), not on individual args. It augments the final result with a related file from the filesystem.
- The three execution-spec output hooks are complementary: `postProcess` transforms the command's stdout; `sideRead` conditionally appends a nearby file. Both run after the command; `postProcess` runs first.
- `oncePerSession` deduplication is keyed on `(session_id, path_param_value, filename)`. The seen set is recorded before the file is read; if the file is absent on the first call, subsequent calls within the same session will also be skipped. This prevents repeated file-not-found probes.
- The `filename` field is a static value from the skill descriptor (not model-supplied input), so path traversal in it is a misconfigured skill rather than a model attack. The executor rejects filenames containing `..`, `/`, or `\` as a defense-in-depth measure.
- Because the `pathParam` value is already validated against the sandbox by a `readPath` annotation on the corresponding arg, the derived `<path>/<filename>` path is also within the sandbox by construction.

**Example:** Automatically surface `AGENTS.md` from a listed directory, at most once per session:

```json
{
  "tool": "files_list_dir",
  "binary": "ls",
  "subcommand": "",
  "args": [
    { "param": "long", "kind": "flagifboolean", "flagIfTrue": "-l" },
    { "param": "all", "kind": "flagifboolean", "flagIfTrue": "-a" },
    { "param": "path", "kind": "positional", "readPath": true }
  ],
  "sideRead": {
    "pathParam": "path",
    "filename": "AGENTS.md",
    "oncePerSession": true
  }
}
```

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

## Example

One tool, one positional argument:

```json
{
  "tools": [
    {
      "name": "files_read_lines",
      "description": "Read a range of lines from a file with line numbers. Returns lines in the format {line_number}|{content}. Use this instead of files_read_file when you only need a specific portion of a file to reduce context usage.",
      "parameters": {
        "type": "object",
        "required": ["path", "start_line"],
        "properties": {
          "path": {
            "type": "string",
            "description": "File path relative to the sandbox root (use ./ prefix)"
          },
          "start_line": {
            "type": "integer",
            "description": "Line number to start reading at (1-indexed, inclusive)"
          },
          "end_line": {
            "type": "integer",
            "description": "Line number to end reading at (1-indexed, inclusive). Defaults to start_line (single line read) if omitted."
          }
        }
      }
    }
  ],
  "allowlist": {
    "chai": [ "file read-lines"]
  },
  "execution": [
    {
      "tool": "files_read_lines",
      "binary": "chai",
      "subcommand": "file read-lines",
      "args": [
        { "param": "path", "kind": "flag", "flag": "path", "readPath": true },
        { "param": "start_line", "kind": "flag", "flag": "start-line" },
        {
          "param": "end_line",
          "kind": "flag",
          "flag": "end-line",
          "optional": true
        }
      ]
    }
  ]
}
```

## Implementation Notes

- **Loader**: `load_skills` reads `tools.json` from each skill dir; on success, sets `SkillEntry.tool_descriptor`. On parse error, logs a warning and leaves `tool_descriptor` as `None`.
- **Gateway**: Tool list and executor are built only from skills that have a `tools.json` descriptor. There is no hardcoded skill code in the lib; skills without a descriptor contribute no tools. When **`skills.contextMode`** is **`readOnDemand`**, the gateway also registers a **`read_skill(skill_name)`** tool and uses an executor that returns that skill's SKILL.md content in-process; see [CONTEXT.md](CONTEXT.md).
- **Conversion**: `ToolDescriptor::to_tool_definitions()` produces `Vec<ToolDefinition>` in the shape expected by the active LLM **`Provider`** (Ollama-native and OpenAI-compat backends accept the same function-tool schema in practice). `ToolDescriptor::to_allowlist()` produces `exec::Allowlist` for the safe exec layer. The generic executor uses the execution mapping to build argv (applying `resolveCommand` when set) and runs via the allowlist.
- **Scripts**: A skill may place scripts in a **`scripts/`** directory and reference them in `resolveCommand.script`. The executor runs only files under that directory via `sh`; no allowlist entry is needed.
- **Resolvers**: Param resolution is generic (run a script or an allowlisted command, use stdout). Skill-specific logic (e.g. resolving a bare date to a daily-note path) can live in a script in the skill's `scripts/` dir or in a separate binary the skill allowlists; lib, CLI, and desktop contain no skill- or tool-specific code.
- **Post-processing**: When `postProcess` is set on an execution spec, the executor pipes the command's stdout to the post-processor's stdin and returns the post-processor's stdout instead. On failure or empty output, the original stdout is returned unmodified. Same script resolution rules as `resolveCommand` (skill's `scripts/` dir, no allowlist needed).
- **Side reads**: When `sideRead` is set on an execution spec, the executor appends the named file's contents (relative to the resolved path parameter) to the tool result after `postProcess`. The file is read from disk without going through the allowlist. When `oncePerSession` is `true`, the executor maintains a per-session seen set (keyed by session id and resolved path) and skips re-appending the same file. Silently skipped when the file is absent, empty, or the filename fails the traversal check. The `pathParam` value used for both file lookup and `oncePerSession` deduplication is the canonical (absolute, symlink-resolved) path, ensuring correct behavior regardless of whether the caller provides a relative or absolute path.
- **Output truncation**: When `maxOutputLines` is set on an execution spec, the executor truncates the tool's output to the specified number of lines if the output exceeds that limit. Truncation applies after `postProcess` but before `sideRead` — side-read content is not counted against the line limit and is always appended in full. When truncation occurs, the output ends with a notice indicating how many lines were shown, the total line count, how many lines were omitted, and a suggestion to narrow the query. This prevents unbounded tool output (e.g. from `grep`, `git diff`, or `git log`) from exceeding the model's context window and terminating the session.
- **Stdin validation**: When a `kind: "stdin"` parameter is required (no `optional: true`), `extract_stdin_content` validates that the parameter is present and non-null in the tool call arguments. Missing required stdin params produce an error ("missing required parameter: {param}") instead of silently falling through to the no-stdin code path.
- **Stdin pipe scoping**: All sites that write to a child process's stdin pipe use `child.stdin.take().ok_or_else(...)` with a block scope that drops the pipe before calling `wait_with_output()`. This guarantees (1) the child sees EOF on stdin before the parent waits, and (2) pipe unavailability surfaces as an error rather than being silently skipped.
- **Resolve script idempotency**: `resolveCommand` scripts are invoked twice for `writePath`/`readPath` parameters — first in `validate_write_paths()` (result canonicalized and substituted into args), then again in `build_argv()` on the already-resolved value. Scripts that prepend a root path must check whether the input is already absolute and return it unchanged. The idempotent pattern is: `case "$path" in /*) echo "$path"; exit 0 ;; esac`.
- **Working directory args**: `kind: "workingdir"` args are implicitly treated as `readPath` for sandbox validation and set the process's `current_dir` to the canonical resolved path. They are excluded from argv — the value only sets the process CWD, not a CLI argument. When `resolveCommand` is set, the resolver runs with an empty string when the param is omitted, defaulting to the sandbox root. This pattern is used by git skills (`git status`, `git log`, etc.) where the target repository may be in a symlinked subdirectory of the sandbox.
- **Short vs long flags**: For `kind: "flag"`, single-character `flag` values produce short flags (`-n`) and multi-character values produce long flags (`--path`). This matches the universal CLI convention and is consistent with `flagifboolean`, where `flagIfTrue` / `flagFalse` values are emitted as-is (e.g. `"-l"`, `"--cached"`).
