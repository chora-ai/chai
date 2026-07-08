---
status: stable
---

# Tools Schema

When a skill directory contains tool descriptor files, the loader parses them and attaches tool definitions, an allowlist, and per-tool execution mapping to the skill. This allows skills to declare their tools declaratively so a generic executor can run them without per-skill code.

The tool descriptor is split across three files with distinct responsibilities:

| File | Root Type | Content | Audience |
|------|-----------|---------|----------|
| `tools.json` | Array | Tool definitions: name, description, parameter schemas | The LLM (agent) |
| `allowlist.json` | Object | Binary→subcommand security grants | The runtime executor |
| `execution.json` | Array | Per-tool execution mapping: binary, subcommand, args, hints, deny patterns, postProcess, sideRead | The runtime executor |

**Parameters (JSON Schema):** Each tool's **`parameters`** object uses the same **JSON Schema subset** used across LLM **function / tool** APIs: typically `type: "object"`, **`properties`**, **`required`**, and per-argument **`type`**, **`description`**, and optional constraints. That matches what **OpenAI** (tools / function parameters), **Ollama** (`tools` in chat), and **OpenAI-compatible** servers expect. Chai forwards the descriptor's tool list to the active **`Provider`** without rewriting the schema. For examples and field conventions, see vendor docs (e.g. OpenAI function-calling parameter shape).

## Naming Conventions

All bundled skills follow consistent naming conventions for tool names and parameter names. Skill authors creating custom skills should follow the same conventions for a predictable API surface. See [adr/TOOL_PARAMETER_NAMING.md](../adr/TOOL_PARAMETER_NAMING.md) for the decision rationale.

### Tool Names

Pattern: `{skill}_{verb}` with noun suffix only for disambiguation. The `{skill}_` prefix is always the skill directory name. For sub-skills that introduce new tools, the sub-skill name becomes a middle segment: `{skill}_{subskill}_{verb}`.

| Example | Pattern | Notes |
|---------|---------|-------|
| `files_read` | `{skill}_{verb}` | Primary read operation — no noun suffix |
| `files_delete_dir` | `{skill}_{verb}_{noun}` | Noun suffix disambiguates from `files_delete` (file) |
| `notes_wikilink_find_backlinks` | `{skill}_{subskill}_{verb}_{noun}` | Sub-skill introduces a middle segment; `find_` prefix for query operations |

### Parameter Names

| Semantic | Convention | Example |
|----------|-----------|---------|
| Target the tool operates on directly | `path` | `files_read` → `path: "./README.md"` |
| Repository root (git skills) | `repo` | `git_status` → `repo: "./chai"` |
| Directory to narrow search or operation | `scope` | `notes_daily_read` → `scope: "my-notes"` |
| Qualified identifier | `{domain}_name` | `git_branch_create` → `branch_name: "feat/search"` |
| Multi-value parameter | Plural form | `git_add` → `paths: "src/main.rs"` |
| Numeric count or offset | `integer` type | `git_log` → `count: 5` (not string `"5"`) |
| External binary flags | Align to binary's flag names | `--ignore-case` → `ignore_case` |
| Chai binary flags | CLI flags align to ADR conventions | `git diff-lines` uses `--repo` (not `--path`) and `--path` (not `--file-path`); `file rename` uses `--scope` (not `--root`) |

## File Location

- **`tools.json`**: `<skill_dir>/tools.json` — Tool definitions array (same directory as `SKILL.md`).
- **`allowlist.json`**: `<skill_dir>/allowlist.json` — Security grants object.
- **`execution.json`**: `<skill_dir>/execution.json` — Execution mapping array.
- **Optional**: If `tools.json` is absent, the skill has no descriptor; the skill contributes no tools and does not function for tool execution. If `allowlist.json` or `execution.json` is absent but `tools.json` is present, the loader logs a warning and treats the skill as having no descriptor.
- Everything tool-related is defined in these three files; the lib is generic and has no skill-specific code.

## Schema

### `tools.json` (array of tool spec)

Root is an array. Each element:

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Tool name (e.g. `files`, `git`, `notes`). Must match an execution spec. |
| `description` | string (optional) | Short description for the model. |
| `parameters` | object | JSON Schema for arguments (see **Parameters (JSON Schema)** above). |

### `allowlist.json` (object)

- **Keys**: Binary name (e.g. `chai`).
- **Values**: Array of allowed subcommand strings (e.g. `["file read", "file write"]`).

Only (binary, subcommand) pairs listed here may be executed. The safe exec layer enforces this.

### `execution.json` (array of execution spec)

Each element:

| Field | Type | Description |
|-------|------|-------------|
| `tool` | string | Tool name (must match a `tools[].name`). |
| `binary` | string | Binary to run (e.g. `chai`). Must be a key in `allowlist`. |
| `subcommand` | string | Subcommand (e.g. `files read`). Must be in `allowlist[binary]`. The value is split by whitespace and each token is prepended before the `args` list when building the command. This allows fixed flags to be encoded as part of the subcommand (e.g. `"-E"` for `grep -E`). |
| `binaryWrapper` | array of strings (optional) | Wrap the binary invocation through a command prefix (e.g. `["nix", "develop", "--command"]`). When present, the executor constructs `wrapper[0] wrapper[1..] binary subcommand args...` instead of `binary subcommand args...`. The allowlist validates the declared `binary` and `subcommand`, not the wrapper — the wrapper is a transport mechanism, not a privilege escalation. Must be a non-empty array when set. Default: not set. |
| `condition` | object (optional) | Condition that must be satisfied for this execution spec to be selected by the loader. See below. Default: not set. |
| `paramCondition` | object (optional) | Parameter-based condition for selecting between multiple execution specs with the same tool name at runtime. See below. Default: not set. |
| `args` | array (optional) | Order of arguments: how each JSON param becomes a CLI arg. |
| `successExitCodes` | array of integers (optional) | Exit codes to treat as success (in addition to 0). Use when a non-zero exit is a normal result, not an error (e.g. `[0, 1]` for `grep` where exit 1 means no matches). The executor always merges stderr into stdout (appended after stdout with a newline separator) so that `postProcess` hint scripts can inspect diagnostics written to stderr (e.g., compiler warnings). Exit codes not in this list (and not 0) surface as tool errors and bypass `postProcess`. Default: only exit 0 is success. |
| `postProcess` | object (optional) | Post-process the command's merged output (stdout + stderr) through a script before returning the result to the model. See below. Default: not set. |
| `hintConditions` | array (optional) | Inline hint conditions evaluated after `postProcess` and before truncation. Each matching condition appends a `hint:` line to the output. See below. Default: not set. |
| `sideRead` | object (optional) | After the command (and any `postProcess`) completes, look for a file relative to a path parameter and append its contents to the tool result. Silently skipped when the file is absent. See below. Default: not set. |
| `maxOutputLines` | integer (optional) | Maximum number of output lines to return to the model. When set, output exceeding this limit is truncated and a notice is appended indicating how many lines were omitted. This prevents unbounded tool output (e.g. from `grep` or `git diff`) from exceeding the model's context window. Applies after `postProcess` but before `sideRead` (side-read content is not counted against the limit and is always appended in full). Default: not set (no limit). |
| `truncationHint` | string (optional) | Per-tool truncation notice template. When set, replaces the generic "Narrow your query path, pattern, or range to reduce results." notice with a tool-specific message. Template variables: `{kept}` = non-hint lines shown, `{total}` = total lines (including hints), `{omitted}` = non-hint lines omitted, `{next_start}` = the line number of the first omitted line. When output lines are prefixed with line numbers in the format `{number}\t{content}` (e.g. `files_read`, `git_diff_lines`), `{next_start}` is derived from the last kept line number + 1 — so pagination hints reference the correct file line. Otherwise, `{next_start}` = `kept + 1` (output-line numbering). JSON key: `truncationHint`. Default: not set (generic notice). |

#### `condition` (object)

Condition that must be satisfied for an execution spec to be selected by the loader. When present, the loader filters execution specs to only those whose condition matches the loading context. This keeps the executor unaware of bin group logic — the loader handles selection, and the executor just runs what it is given.

| Field | Type | Description |
|-------|------|-------------|
| `binGroup` | integer | Index of the bin group (in `metadata.requires.bins` OR-groups) that must have matched for this execution spec to be selected. For example, `0` means the first group matched (e.g. `["cargo"]`), `1` means the second group matched (e.g. `["nix"]`). Must be a non-negative integer. |

When multiple execution specs share the same `tool` name but differ in `binaryWrapper` and `condition`, the loader selects only those whose `condition.binGroup` matches the group that satisfied the `bins` requirement. Specs with no `condition` are always included. When `bins` uses the flat (All) form, specs with a `condition` are filtered out (since no group index is available).

**Example:** Two execution specs for the same tool, one direct and one wrapped:

```json
"execution": [
  {
    "tool": "cargo_check",
    "binary": "cargo",
    "subcommand": "check",
    "condition": { "binGroup": 0 },
    "args": [...]
  },
  {
    "tool": "cargo_check",
    "binary": "cargo",
    "binaryWrapper": ["nix", "develop", "--command"],
    "subcommand": "check",
    "condition": { "binGroup": 1 },
    "args": [...]
  }
]
```

With `bins: [["cargo"], ["nix"]]`, if `cargo` is on PATH, group 0 matches and the first spec is selected. If only `nix` is on PATH, group 1 matches and the second (wrapped) spec is selected.

#### `paramCondition` (object)

Parameter-based condition for selecting between multiple execution specs with the same tool name at runtime. Unlike `condition` (which is resolved by the loader at load time based on bin groups), `paramCondition` is resolved by the executor at call time based on which parameters the agent provided.

| Field | Type | Description |
|-------|------|-------------|
| `present` | array of strings (optional) | Parameter names that must be present (non-null) in the tool call JSON |
| `absent` | array of strings (optional) | Parameter names that must be absent (or null) from the tool call JSON |

All constraints use AND logic. An entry without `paramCondition` is the default — it matches when no other entry's condition is satisfied.

**Selection rules:**

1. If only one execution spec exists for a tool name, it is always used.
2. If multiple specs exist, the executor first checks entries with `paramCondition`. If exactly one matches, it is used.
3. If no `paramCondition` matches, the executor falls back to the default entry (no `paramCondition`).
4. If multiple `paramCondition` entries match, the tool call is rejected as ambiguous.
5. If no entry matches (no `paramCondition` matches and no default exists), the executor checks for **partial matches** — entries where at least one `present` parameter was provided but others were missing. When partial matches are found, the error message includes a hint connecting the missing parameters to the provided ones (e.g., `parameter(s) 'abort' must be provided together with 'continue'`).

**Example:** Rebase and cherry-pick tools use `paramCondition` to route `continue`/`abort` operations to the appropriate CLI flags:

```json
[
  {
    "tool": "git_rebase",
    "subcommand": "rebase continue",
    "paramCondition": { "present": ["continue"] },
    "binary": "chai",
    "args": [...]
  },
  {
    "tool": "git_rebase",
    "subcommand": "rebase abort",
    "paramCondition": { "present": ["abort"] },
    "binary": "chai",
    "args": [...]
  }
]
```

Note: `files_write` previously used `paramCondition` to route between whole-file write and surgical edit modes, but has since been split into `files_write` (whole-file only) and `files_edit` (surgical edit only), eliminating the need for `paramCondition` on the write surface.

#### `args` (array of arg mapping)

Each element:

| Field | Type | Description |
|-------|------|-------------|
| `param` | string (optional) | JSON parameter name (e.g. `query`). Required for all kinds except `literal` (which pushes a fixed value and reads nothing from the tool call JSON). |
| `kind` | string | `"positional"`, `"flag"`, `"flagifboolean"`, `"stdin"`, `"workingdir"`, `"tempfile"`, or `"literal"`. Default `positional`. `"stdin"` pipes the parameter value to the child process's stdin instead of passing it as a CLI argument. Required for any parameter that may contain multiline content — `kind: "flag"` causes `clap` to break on newlines. Only one `stdin` arg is allowed per execution spec. `"workingdir"` sets the process working directory to the resolved value (the value is validated against the sandbox like `readPath` and used as `current_dir`, but is **not** added to argv). `"tempfile"` writes the value to a temporary file and passes the file path as a flag — use for content-rich parameters that cannot use stdin (because stdin is already in use) or that must match file content byte-for-byte (e.g. verification tokens). `"literal"` pushes a fixed value directly onto argv with no parameter read from the tool call JSON — use for command flags like `--continue` and `--abort` that are always present when the tool is called. |
| `value` | string (optional) | For `kind: "literal"`, the fixed value to push onto argv (e.g. `"--continue"`, `"--abort"`). Required when `kind` is `"literal"`. |
| `flag` | string (optional) | For `kind: "flag"` or `kind: "tempfile"`, the flag name. Single-character names produce short flags (e.g. `"n"` → `-n`); multi-character names produce long flags (e.g. `"path"` → `--path`). If absent, uses `param` (always a long flag). Leading dashes are stripped before prefixing, so pre-dashed values like `"-p"` also produce the correct flag. For `flag`, if the parameter is missing or null, the flag and value are omitted (optional params). |
| `flagIfTrue` | string (optional) | For `kind: "flagifboolean"`, the flag to emit when the param is true (e.g. `"--overwrite"`). |
| `flagIfFalse` | string (optional) | For `kind: "flagifboolean"`, the flag to emit when the param is false (e.g. `"--append"`). |
| `resolveCommand` | object (optional) | Resolve the param value by running a **script** or an **allowlisted command**; trimmed stdout becomes the new value. On empty stdout, the original value is kept. On failure (non-zero exit), the tool call is rejected — this enables resolve scripts to perform validation (e.g., verifying a git repository root is inside the sandbox). See below. Default: not set. |
| `optional` | boolean (optional) | When `true`, a missing or null JSON parameter is omitted from argv. Exception: when `resolveCommand` is set, a missing parameter is passed to the resolver as an empty string so the resolver can still produce a value (e.g. default paths). This applies to `kind: "positional"`, `"flag"`, and `"workingdir"`. Default: not set (required). |
| `split` | boolean (optional) | For `kind: "positional"` only: when `true`, split the value on whitespace and push each element as a separate argv entry. Use for tools that accept multiple positional arguments (e.g. `git add file1.rs file2.rs`, `git cherry-pick abc1234 def5678`). Default: not set. |
| `disambiguateAfterSkippedPositionals` | boolean (optional) | For `kind: "positional"` only: when `true`, the executor inserts `--` before this argument's value if any earlier optional positional in the same `args` list was skipped. Use when a path must be disambiguated from an omitted ref (e.g. `git diff`). Default: not set. |
| `absentDefault` | JSON value (optional) | The value to use when the parameter is absent from the tool call JSON. For `flagIfBoolean`, this provides a boolean default (previously, absent booleans were always treated as false). For `flag`, this provides a string or numeric default (e.g., `"warn"` for a level parameter, `"10"` for a count). For `positional`, this provides a string or numeric default (e.g., `"HEAD~1"` for a ref parameter). The schema `"default"` field is only a hint to the LLM; `absentDefault` is enforced by the executor. Default: not set. |
| `subcommandOverride` | string (optional) | For `kind: "flagifboolean"` only: when set and the boolean parameter evaluates to true, overrides the execution spec's `subcommand` with this value. The arg does not produce an argv entry — its purpose is to control the subcommand, not to add a CLI flag. The override subcommand must be in the allowlist (the allowlist check happens at execution time). The spec's default subcommand is used when the boolean is false or absent. Use for tools where a boolean flag switches the git subcommand (e.g., `force: true` switches from `branch -d` to `branch -D`). Default: not set. |
| `denyPattern` | string (optional) | A regex pattern that the resolved parameter value must **not** match. When set, the executor checks the resolved value against this pattern before executing the command. If the value matches, the operation is rejected with an error. This is a tool-level enforcement mechanism for constraints that the schema cannot express (e.g., branch protection). Default: not set. |
| `denyResolveCommand` | object (optional) | A resolve command that provides the effective value to check against `denyPattern`. Same structure as `resolveCommand`. When `denyAlwaysResolve` is false (default), the raw parameter value is checked directly when present, and this command is only invoked when the parameter is absent or empty. When `denyAlwaysResolve` is true, this command always provides the value to check — the raw parameter value may be unrelated to what the deny pattern matches (e.g., the param is a working directory path, but the deny pattern checks the current branch name). Default: not set. |
| `denyAlwaysResolve` | boolean (optional) | When `true`, `denyResolveCommand` always provides the value to check against `denyPattern`, even when the raw parameter value is present. This is needed when the parameter value is not the thing being denied (e.g., a path parameter whose value is a directory, but the deny pattern checks the git branch within that directory). Default: not set. |
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
| `args` | array of strings | Arguments; `"$param"` is replaced by the current param value; `"$param_name"` (e.g. `"$scope"`) is replaced by the corresponding parameter value from the tool call JSON (empty string if absent or null). |

When `script` is set, the executor runs `sh <skill_dir>/scripts/<script> <args...>`. When `binary` and `subcommand` are set, the executor runs them via the allowlist. No extra setup (allowlist entry or separate binary) is required for scripts.

#### `postProcess` (object)

Post-processes the command's merged output (stdout + stderr) through a script or allowlisted command. The merged output is piped to the post-processor's **stdin**; its own stdout becomes the tool result returned to the model. On failure or empty output, the original merged output is returned unmodified.

Use either **script** (no allowlist entry) or **binary** + **subcommand** (allowlisted), same as `resolveCommand`.

| Field | Type | Description |
|-------|------|-------------|
| `script` | string (optional) | Name of a file in the skill's **`scripts/`** directory (e.g. `"parse-rss"`). Same path rules as `resolveCommand.script`. |
| `binary` | string (optional) | Binary name for allowlisted post-processing (must be in the skill's allowlist). |
| `subcommand` | string (optional) | Subcommand for allowlisted command (must be in allowlist for that binary). |
| `args` | array of strings | Additional arguments passed to the script or command. `"$param_name"` (e.g. `"$scope"`) is replaced by the corresponding parameter value from the tool call JSON (empty string if absent or null). |

**Design notes:**
- `postProcess` is set on the **execution spec** (per-tool), not on individual args. It transforms the final merged output (stdout + stderr), not a parameter value.
- Stdin piping (not command-line args) is used because tool output can be large (RSS feeds, HTML pages, search results).
- The symmetry with `resolveCommand` is intentional: `resolveCommand` mediates input (parameter → resolved value), `postProcess` mediates output (merged output → structured result).

#### `hintConditions` (array of hint condition)

Declares inline hint conditions that the executor evaluates after `postProcess` and before truncation. Each matching condition appends a `hint:` line to the output with the standard blank-line separator. Hints from `hintConditions` and `postProcess` follow the same format and are preserved identically by `truncate_output()`.

`hintConditions` replaces simple `postProcess` hint scripts (those that only inspect output and append a static hint) with a declarative inline format. Reserve `postProcess` for hints that require output transformation, external commands, or multi-step logic. See [adr/DIAGNOSTIC_HINTS.md](../adr/DIAGNOSTIC_HINTS.md) for the full hint mechanism comparison.

Each element:

| Field | Type | Description |
|-------|------|-------------|
| `match` | string (optional) | Substring to search for in the post-processed output (stdout + stderr). Case-sensitive. When present, the hint fires if the string appears anywhere in the output. |
| `exitCode` | string or integer (optional) | Exit code condition. `"nonzero"` matches any non-zero exit code. An integer (e.g., `1`, `128`) matches that specific exit code. When present, the hint fires if the exit code matches. |
| `notEmpty` | boolean (optional) | When `true`, the hint fires only if the post-processed output is non-empty. When `false` or absent, output emptiness is not checked. |
| `whenArg` | object (optional) | Parameter-value conditions. Keys are parameter names; values are expected values (string, boolean, or number). All specified parameters must match their expected values for the condition to fire. Evaluated against the effective args (after `absentDefault` augmentation). |
| `hint` | string (required) | The hint message. The executor prepends `hint: ` and a blank-line separator before each hint. Supports `{param_name}` template variables that are replaced with the corresponding parameter value from the effective args. |

At least one of `match`, `exitCode`, `notEmpty`, or `whenArg` must be present. If multiple condition fields are present on the same entry, **all** must be true (AND logic). This prevents false-positive hints from matching output content that coincidentally contains the substring on success. Multiple `hintConditions` entries are all evaluated; all matching entries produce hints (not first-match-wins).

The exit code passed to `hintConditions` is the **main command's exit code** (same value passed to `CHAI_EXIT_CODE` for `postProcess` scripts), not the postProcess script's exit code.

**Critical**: When using `exitCode: "nonzero"` (or a specific non-zero code), the tool must also declare `successExitCodes` for that exit code. Without `successExitCodes`, the executor's error propagation short-circuits before `hintConditions` is evaluated. The correct pattern is: `successExitCodes` controls pipeline flow (whether the output reaches hintConditions), and `hintConditions` inspects the output to generate hints.

**Example:** Exit-code-based hint for a tool that returns file-not-found errors:

```json
{
  "tool": "files_read",
  "binary": "chai",
  "subcommand": "file read",
  "successExitCodes": [1],
  "hintConditions": [
    {
      "exitCode": "nonzero",
      "hint": "file not found — use files_list to list available files"
    }
  ],
  "args": [...]
}
```

**Example:** Compound condition (match + whenArg) for cherry-pick staging:

```json
"hintConditions": [
  {
    "match": "CONFLICT",
    "hint": "cherry-pick conflicts detected — resolve and stage them, then use git_cherry_pick_continue"
  },
  {
    "whenArg": { "no_commit": true },
    "hint": "cherry-pick staged — use git_commit to finalize"
  }
]
```

**Example:** Template variable for dynamic hint text:

```json
"hintConditions": [
  {
    "exitCode": 0,
    "hint": "reset to {ref} — use git_status to inspect the current state"
  }
]
```

**Design notes:**
- `hintConditions` and `postProcess` are independent and can coexist on the same execution spec. When both are present, `postProcess` runs first (transforming the output), then `hintConditions` matches against the transformed output using the original command's exit code.
- `hintConditions` only appends hints — it never filters, reorders, or modifies the existing output. This is the key distinction from `postProcess`: `hintConditions` is a pure hint-injection mechanism.
- Substring match (not regex) covers the majority of hint conditions. Every hint script that matches on output content uses `grep "literal string"`, not regex patterns. If regex is needed in the future, a `matchRegex` field can be added.

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
  "tool": "files_list",
  "binary": "ls",
  "subcommand": "",
  "args": [
    {
      "param": "long",
      "kind": "flagifboolean",
      "flagIfTrue": "-l"
    },
    {
      "param": "all",
      "kind": "flagifboolean",
      "flagIfTrue": "-a"
    },
    {
      "param": "path",
      "kind": "positional",
      "readPath": true
    }
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
    {
      "param": "feed",
      "kind": "positional",
      "resolveCommand": {
        "script": "resolve-feed-url",
        "args": ["$param"]
      }
    }
  ],
  "postProcess": {
    "script": "parse-rss"
  }
}
```

## Example

A consolidated read tool with optional line-range parameters — split across three files:

**`tools.json`**:
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
          "description": "File path relative to the sandbox root"
        },
        "start_line": {
          "type": "integer",
          "description": "Line number to start reading at (1-indexed, inclusive)"
        },
        "end_line": {
          "type": "integer",
          "description": "Line number to end reading at (1-indexed, inclusive)"
        }
      }
    }
  }
]
```

**`allowlist.json`**:
```json
{
  "chai": ["file read"]
}
```

**`execution.json`**:
```json
[
  {
    "tool": "files_read",
    "binary": "chai",
    "subcommand": "file read",
    "successExitCodes": [1],
    "hintConditions": [
      {
        "exitCode": "nonzero",
        "hint": "file not found — use files_list to list available files"
      }
    ],
    "maxOutputLines": 500,
    "truncationHint": "output truncated: {kept} of {total} lines shown; {omitted} more lines available. To continue reading, use files_read with start_line: {next_start}.",
    "args": [
      {
        "param": "path",
        "kind": "flag",
        "flag": "path",
        "readPath": true
      },
      {
        "param": "start_line",
        "kind": "flag",
        "flag": "start-line",
        "optional": true
      },
      {
        "param": "end_line",
        "kind": "flag",
        "flag": "end-line",
        "optional": true
      }
    ]
  }
]
```

## Legacy Format

The original `tools.json` used a single root object with three top-level keys:

```json
{
  "tools": [...],
  "allowlist": {...},
  "execution": [...]
}
```

This format is still supported for backward compatibility. The loader detects the format at load time:

- **Root object** with `tools`/`allowlist`/`execution` keys → legacy single-file format, parsed as before.
- **Root array** → new three-file format, companion files (`allowlist.json`, `execution.json`) are also read.

A deprecation warning is logged when the legacy format is detected. Both formats produce the same `ToolDescriptor` in memory.

## Implementation Notes

- **Loader**: `load_skills` reads `tools.json` from each skill dir and detects its format (root array → three-file, root object → legacy). For the three-file format, it also reads `allowlist.json` and `execution.json`, then constructs a `ToolDescriptor` from the three sources. On success, sets `SkillEntry.tool_descriptor`. On parse error (or missing companion files for the three-file format), logs a warning and leaves `tool_descriptor` as `None`. When `metadata.requires.bins` uses OR-groups and a group matches, the loader records the matched group index (`SkillEntry.matched_bin_group`) and filters execution specs: only specs with `condition.binGroup` equal to the matched index, or specs with no `condition`, are retained. This keeps the executor unaware of bin group logic — it receives a pre-filtered descriptor.
- **paramCondition routing**: When multiple execution specs share the same `tool` name and at least one has a `paramCondition`, the executor resolves which spec to use at call time based on which parameters the agent provided. It first checks entries with `paramCondition`; if exactly one matches, it is used. If no `paramCondition` matches, the executor falls back to the default entry (no `paramCondition`). If no entry matches and no default exists, the executor checks for partial matches — entries where at least one `present` parameter was provided but others were missing — and includes a hint in the error message identifying the missing paired parameters. This enables multi-mode tools (e.g., `git_rebase` routing to `rebase --continue` or `rebase --abort` based on the presence of `continue` or `abort`). `files_write` previously used `paramCondition` for whole-file vs surgical edit routing but was split into `files_write` + `files_edit` — see the note under the `paramCondition` example above.
- **Schema-enforced validation**: The executor validates tool call parameters against the tool's parameter schema before execution. Undeclared parameters (not present in the schema) and type mismatches are rejected immediately. This makes the schema the authoritative contract — the agent cannot provide parameters it was never told about. At startup, `check_schema_execution_alignment` warns when a tool's schema declares a parameter that has no corresponding execution handler, catching the reverse drift case.
- **Gateway**: Tool list and executor are built only from skills that have a `tools.json` descriptor. There is no hardcoded skill code in the lib; skills without a descriptor contribute no tools. When **`skills.contextMode`** is **`readOnDemand`**, the gateway also registers a **`read_skill(skill_name)`** tool and uses an executor that returns that skill's SKILL.md content in-process; see [CONTEXT.md](CONTEXT.md).
- **Conversion**: `ToolDescriptor::to_tool_definitions()` produces `Vec<ToolDefinition>` in the shape expected by the active LLM **`Provider`** (Ollama-native and OpenAI-compat backends accept the same function-tool schema in practice). `ToolDescriptor::to_allowlist()` produces `exec::Allowlist` for the safe exec layer. The generic executor uses the execution mapping to build argv (applying `resolveCommand` when set) and runs via the allowlist.
- **Binary wrappers**: When `binaryWrapper` is set on an execution spec, the executor constructs the command as `wrapper[0] wrapper[1..] resolved_binary subcommand args...` instead of `resolved_binary subcommand args...`. The allowlist validates the declared `binary` and `subcommand`, not the wrapper — the wrapper is a transport mechanism (e.g. `nix develop --command`), not a privilege escalation. The wrapper binary must be on PATH (guaranteed by the OR-group bin check at load time). `binaryWrapper` is an author-declared field in `execution.json`, not an agent-provided parameter; the agent cannot inject an arbitrary wrapper at runtime.
- **Scripts**: A skill may place scripts in a **`scripts/`** directory and reference them in `resolveCommand.script` (in `execution.json`). The executor runs only files under that directory via `sh`; no allowlist entry is needed.
- **Resolvers**: Param resolution is generic (run a script or an allowlisted command, use stdout). Skill-specific logic (e.g. resolving a bare date to a daily-note path) can live in a script in the skill's `scripts/` dir or in a separate binary the skill allowlists; lib, CLI, and desktop contain no skill- or tool-specific code.
- **Post-processing**: When `postProcess` is set on an execution spec, the executor pipes the command's merged output (stdout + stderr) to the post-processor's stdin and returns the post-processor's stdout instead. The executor always merges stderr into stdout (appended after stdout with a newline separator) before the post-process step, so hint scripts can inspect diagnostics written to stderr (e.g., compiler warnings). On failure or empty output, the original merged output is returned unmodified. Same script resolution rules as `resolveCommand` (skill's `scripts/` dir, no allowlist needed).
- **Hint conditions**: When `hintConditions` is set on an execution spec, the executor evaluates each condition against the post-processed output and the main command's exit code. Matching conditions append `hint:` lines with blank-line separators. This runs after `postProcess` and before `truncate_output`, so inline hints are present in the output for truncation to preserve. The effective args (augmented with `absentDefault` values) are passed for `whenArg` matching and `{param_name}` template expansion. When `exitCode: "nonzero"` is used, `successExitCodes` must also be declared — otherwise the executor's error propagation short-circuits before `hintConditions` is evaluated.
- **Side reads**: When `sideRead` is set on an execution spec, the executor appends the named file's contents (relative to the resolved path parameter) to the tool result after `postProcess`. The file is read from disk without going through the allowlist. When `oncePerSession` is `true`, the executor maintains a per-session seen set (keyed by session id and resolved path) and skips re-appending the same file. Silently skipped when the file is absent, empty, or the filename fails the traversal check. The `pathParam` value used for both file lookup and `oncePerSession` deduplication is the canonical (absolute, symlink-resolved) path, ensuring correct behavior regardless of whether the caller provides a relative or absolute path.
- **Output truncation**: When `maxOutputLines` is set on an execution spec, the executor truncates the tool's output to the specified number of lines if the output exceeds that limit. Truncation applies after `postProcess` and `hintConditions` but before `sideRead` — side-read content is not counted against the line limit and is always appended in full. Lines prefixed with `hint:` are preserved through truncation: non-hint lines are truncated to `maxOutputLines`, then hint lines are appended before the truncation notice. This ensures diagnostic hints (see [adr/DIAGNOSTIC_HINTS.md](../adr/DIAGNOSTIC_HINTS.md)) are never lost to truncation. When truncation occurs, the output ends with a notice indicating how many lines were shown, the total line count, how many lines were omitted, and — when `truncationHint` is set — a tool-specific message (e.g., `Use git_diff_lines with start_line: 201 to read the remaining lines.`); otherwise a generic "Narrow your query path, pattern, or range to reduce results." suggestion is used. The `truncationHint` template supports variables `{kept}`, `{total}`, `{omitted}`, and `{next_start}` that the executor expands at truncation time. When output lines are prefixed with line numbers in the format `{number}\t{content}`, `{next_start}` is derived from the last kept line number + 1 (so pagination hints reference the correct file line); otherwise, `{next_start}` = `kept + 1` (output-line numbering). This prevents unbounded tool output (e.g. from `grep`, `git diff`, or `git log`) from exceeding the model's context window and terminating the session, and — when a companion line-range tool exists — gives the agent a direct pagination path to recover the omitted content.
- **Stdin validation**: When a `kind: "stdin"` parameter is required (no `optional: true`), `extract_stdin_content` validates that the parameter is present and non-null in the tool call arguments. Missing required stdin params produce an error ("missing required parameter: {param}") instead of silently falling through to the no-stdin code path.
- **Stdin pipe scoping**: All sites that write to a child process's stdin pipe use `child.stdin.take().ok_or_else(...)` with a block scope that drops the pipe before calling `wait_with_output()`. This guarantees (1) the child sees EOF on stdin before the parent waits, and (2) pipe unavailability surfaces as an error rather than being silently skipped.
- **Resolve script idempotency**: `resolveCommand` scripts are invoked twice for `writePath`/`readPath` parameters — first in `validate_write_paths()` (result canonicalized and substituted into args), then again in `build_argv()` on the already-resolved value. Scripts that prepend a root path must check whether the input is already absolute and return it unchanged. The idempotent pattern is: `case "$path" in /*) echo "$path"; exit 0 ;; esac`.
- **Resolve command error propagation**: When a resolve command exits with a non-zero code, the executor rejects the tool call instead of silently falling back to the unresolved parameter value. This enables resolve commands to perform validation — e.g., `chai resolve repo-path` verifies that git would find its repository inside the sandbox and exits non-zero if the repository root is outside, preventing the git command from running. Before this behavior, resolve-command errors were silently swallowed and the raw parameter value was used, allowing tool calls to proceed with unvalidated paths.
- **Working directory args**: `kind: "workingdir"` args are implicitly treated as `readPath` for sandbox validation and set the process's `current_dir` to the canonical resolved path. They are excluded from argv — the value only sets the process CWD, not a CLI argument. When `resolveCommand` is set, the resolver runs with an empty string when the param is omitted, defaulting to the sandbox root. Bundled skills use `chai resolve` subcommands (e.g., `chai resolve repo-path`, `chai resolve cargo-path`) for sandbox-aware working-directory resolution; custom skills may use shell scripts via `resolveCommand.script`.
- **Short vs long flags**: For `kind: "flag"`, single-character `flag` values produce short flags (`-n`) and multi-character values produce long flags (`--path`). Leading dashes are stripped before prefixing, so both bare names (`"p"`) and pre-dashed values (`"-p"`) produce the correct flag. This matches the universal CLI convention and is consistent with `flagifboolean`, where `flagIfTrue` / `flagFalse` values are emitted as-is (e.g. `"-l"`, `"--cached"`).
- **Absent defaults**: When `absentDefault` is set on an arg, the executor uses that value when the parameter is absent from the tool call JSON. The schema `"default"` field is a hint to the LLM (it influences tool-call generation), but `absentDefault` is the executor-enforced value. This prevents drift between what the model thinks the default is and what the tool actually does. `absentDefault` supports any JSON value (strings, numbers, booleans) for `flag`, `flagIfBoolean`, and `positional` args. When `absentDefault` is used with `postProcess`, the executor augments the effective args map with absent defaults before passing it to `run_post_process`, so `"$param_name"` substitutions in postProcess args reflect the default value rather than an empty string.
- **Literal args**: `kind: "literal"` pushes a fixed value onto argv with no corresponding parameter in the tool call JSON. The `value` field specifies the string to push. `param` is not required for literal args (a placeholder is used internally). Use for command flags that are always present when the tool is called (e.g., `--continue` and `--abort` for git rebase/cherry-pick conflict resolution). Literal args are excluded from deny-pattern checks, sandbox validation, and absent-default augmentation.
- **Temp file args**: `kind: "tempfile"` writes the parameter value to a temporary file and passes the file path as a flag. The `flag` field specifies the CLI flag name (same naming rules as `kind: "flag"`). The executor manages temp file creation and cleanup. Use for content-rich parameters that cannot use stdin (because stdin is already in use) or that must match file content byte-for-byte (e.g., verification tokens like `old_content`). Temp file args are excluded from deny-pattern checks.
- **Verification tokens (`old_content`)**: When the `chai file edit` subcommand receives an `old_content` temp-file arg, the binary validates it against the actual file content using a five-stage cascade before applying the edit: (1) exact match, (2) NFC normalization, (3) Unicode-to-ASCII folding, (4) trailing-whitespace-tolerant match, (5) blank-line-boundary-tolerant match. Stage 5 strips leading and trailing blank lines from both the actual and expected content before comparing — this handles cases where the LLM includes or excludes blank lines at the range boundary differently from the file. Interior blank lines are not tolerated. When all stages fail, the error includes a line-diff hint identifying the first line that differs (see [adr/DIAGNOSTIC_HINTS.md](../adr/DIAGNOSTIC_HINTS.md)). When `start_line` is omitted, the binary searches for `old_content` across all line positions using the same cascade and requires exactly one match.
- **Blank-line collapse in `files_replace`**: The `chai file replace` subcommand automatically collapses runs of two or more consecutive blank (or whitespace-only) lines down to a single blank line before writing the file. This prevents double-blank-line artifacts that commonly result from deletion operations (e.g., deleting a function between two single-blank-line separators). The collapse is silent, applies to all four replacement code paths (literal, degenerate-regex fallback, regex, and trailing-whitespace-tolerant literal), and preserves the original trailing newline.
- **Split positional args**: When `split: true` is set on a `kind: "positional"` arg, the executor splits the value on whitespace and pushes each element as a separate argv entry. Use for tools that accept multiple positional arguments (e.g., `git add file1.rs file2.rs`, `git cherry-pick abc1234 def5678`).
- **Subcommand overrides**: When a `kind: "flagifboolean"` arg has `subcommandOverride` set and its boolean parameter evaluates to true, the executor uses the override subcommand instead of the execution spec's default `subcommand`. The arg does not produce an argv entry — it is skipped in `build_argv` because its purpose is to control subcommand selection, not to add a CLI flag. The override subcommand must be in the allowlist (the allowlist check happens at execution time). When the boolean is false or absent (after applying `absentDefault`), the spec's default subcommand is used. This is a reusable mechanism: any tool that needs a boolean flag to switch subcommands can use it without Rust code changes. Example: `git_branch_delete` uses `subcommandOverride: "branch -D"` on its `force` parameter, so `force: true` switches from `branch -d` to `branch -D` while `force: false` (or absent) keeps `branch -d`.
- **Deny patterns**: `denyPattern` is a regex checked against the resolved parameter value before command execution. If the value matches, the tool call is rejected. This enforces constraints that the JSON Schema cannot express — for example, protecting git branches (`denyPattern: "^(main|release/.*)$"`) or blocking deletion of bundled skill names. When `denyResolveCommand` is set, the executor runs that command to obtain the effective value to check; when `denyAlwaysResolve` is true, the resolve command runs regardless of whether the raw parameter is present. This supports cases where the parameter is a path but the deny pattern checks something derived from it (e.g., the git branch in that directory).
