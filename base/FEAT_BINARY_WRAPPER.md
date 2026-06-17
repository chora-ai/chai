# FEAT: Binary Wrapper for Skill Execution

Add a `binaryWrapper` field to execution specs so that skills can invoke binaries through a wrapper command (e.g. `nix develop --command`), and extend `metadata.requires.bins` with OR-group semantics so a skill loads when any one group of required binaries is available.

## Problem

On NixOS, development tools like `cargo` are not on the system PATH. They are made available via `nix develop`, which spawns a shell with the appropriate packages. The chai gateway runs outside this shell, so a skill declaring `metadata.requires.bins: ["cargo"]` is silently skipped — the loader's `bin_on_path` check fails.

The agent currently has no way to invoke tools that require a nix environment. The only binary-override mechanism is `CHAI_BIN`, which only applies to the `"chai"` binary itself. There is no general way to prefix a command with a wrapper.

## Design

### 1. `binaryWrapper` in Execution Specs

Add an optional `binaryWrapper` field to each entry in `tools.json`'s `execution` array. When present, the executor constructs the command as:

```
Command::new(wrapper[0]).args(wrapper[1..]).arg(binary).args(subcommand).args(user_args)
```

Instead of the current:

```
Command::new(binary).args(subcommand).args(user_args)
```

#### Example

A `cargo` skill targeting a nix environment:

```json
{
  "tool": "cargo_check",
  "binary": "cargo",
  "binaryWrapper": ["nix", "develop", "--command"],
  "subcommand": "check",
  "args": [...]
}
```

This produces: `nix develop --command cargo check ...`

#### Allowlist Implications

The allowlist currently gates on `(binary, subcommand)` pairs from the `allowlist` field of `tools.json`. With a wrapper, the **actually executed** binary is `nix`, not `cargo`. Two options:

- **Option A (recommended): The allowlist validates the declared `binary`, not the wrapper.** The wrapper is transparent to the allowlist — it is an execution detail, not a security boundary. The skill author has already declared which binary and subcommand the tool uses; the wrapper just determines *how* it is invoked. The allowlist entry remains `"cargo": ["check", "test", "build"]`.

- **Option B: The allowlist also validates the wrapper binary.** The wrapper binary (`nix`) must also appear in the allowlist with the appropriate subcommands (`develop`). This is more restrictive but adds a second gate.

Option A is recommended because the wrapper is a transport mechanism, not a privilege escalation. The skill author has already committed to running `cargo check` — wrapping it through `nix develop --command` does not change the security posture. The wrapper binary itself does not introduce new capabilities beyond what the declared binary+subcommand already permits.

#### Working Directory

The wrapper command runs in the same working directory as the underlying command would have. No change needed — `current_dir` is set on the `Command` before spawn, and the wrapper inherits it.

#### `resolve_binary`

The current `resolve_binary()` function is only called with the declared `binary` name. With a wrapper, `resolve_binary` should still resolve the declared binary (e.g. `"cargo"` → PATH lookup), but this resolved path is then passed as the first argument after the wrapper's `--command`. The wrapper binary itself should also go through `resolve_binary` so that `CHAI_BIN`-style overrides are possible for wrapper binaries in the future.

### 2. OR-Group Semantics for `metadata.requires.bins`

Currently, `bins` is a flat list — all binaries must be present. Extend this to support OR-groups: a list of lists where the skill loads if **any one group** has all its binaries on PATH.

#### YAML Frontmatter

```yaml
metadata:
  requires:
    bins: [["cargo"], ["nix"]]   # load if cargo is on PATH, OR if nix is on PATH
```

This is backward-compatible: a flat list of strings `["git", "curl"]` means "all must be present" (AND), same as today. A list of lists `[["cargo"], ["nix"]]` means "any one group must be fully present" (OR of ANDs). The two representations are distinct: a list of strings has type `Vec<String>`, a list of lists has type `Vec<Vec<String>>`.

#### Skill Loading Behavior

When OR-groups are present, the loader must also determine **which group matched**, because this determines whether `binaryWrapper` should be applied at execution time. Specifically:

- If `["cargo"]` matched (cargo is on PATH) → no wrapper needed.
- If `["nix"]` matched (nix is on PATH, cargo is not) → apply the `binaryWrapper`.

The loader should record the matched group so the executor can decide whether to apply the wrapper.

#### Implementation Sketch

The `Requires` struct in `loader.rs` currently deserializes `bins: Option<Vec<String>>`. Change to a custom deserializer that accepts either:

- `["git", "curl"]` → `BinsRequirement::All(Vec<String>)` (all must be present — current behavior)
- `[["cargo"], ["nix"]]` → `BinsRequirement::AnyOf(Vec<Vec<String>>)` (any group must be fully present)

Both representations deserialize cleanly from YAML/JSON because YAML distinguishes between a list of scalars and a list of lists.

## Implementation Requirements

### Descriptor Changes (`descriptor.rs`)

1. Add `binary_wrapper: Option<Vec<String>>` to `ExecutionSpec` with `#[serde(default)]` and `#[serde(rename = "binaryWrapper")]`.

2. No changes to `ToolDescriptor`, `ArgMapping`, or other structs.

### Loader Changes (`loader.rs`)

1. Replace `Requires.bins: Option<Vec<String>>` with a custom `BinsRequirement` enum:

   ```rust
   enum BinsRequirement {
       All(Vec<String>),           // all must be on PATH (backward compatible)
       AnyOf(Vec<Vec<String>>),    // any group must be fully on PATH
   }
   ```

2. Implement custom `Deserialize` for `BinsRequirement` that accepts both `["a", "b"]` (flat strings → `All`) and `[["a"], ["b"]]` (nested → `AnyOf`).

3. Extend the loading check: when `AnyOf`, iterate groups and find the first where all binaries are on PATH. Record which group matched.

4. Add a `matched_bin_group` field to `SkillEntry` (or store it alongside the entry) so the executor knows whether a wrapper should be applied.

### Executor Changes (`exec.rs`)

1. Extend `run_with_codes_and_exit` and `run_with_stdin_with_codes_and_exit` to accept an optional `binary_wrapper: Option<&[String]>` parameter.

2. When a wrapper is present, construct the command as:

   ```rust
   let mut cmd = Command::new(&wrapper[0]);
   cmd.args(&wrapper[1..]);
   cmd.arg(&resolved);  // the declared binary
   cmd.args(subcommand.split_whitespace());
   cmd.args(args);
   ```

   When no wrapper, the existing `Command::new(&resolved)` path is unchanged.

3. The allowlist check continues to validate `(binary, subcommand)` — not the wrapper binary. No allowlist changes needed.

### Generic Tool Executor Changes (`tools/generic/mod.rs`)

1. Extend the executor's `map` to carry the `binary_wrapper` from the `ExecutionSpec`. Since `ExecutionSpec` already stores this field (added in descriptor changes), the executor just needs to read it when calling the allowlist.

2. The executor must also know whether the wrapper *should* be applied, based on which bin group matched during loading. If the `["cargo"]` group matched, no wrapper; if the `["nix"]` group matched, apply the wrapper.

### Execution Spec Selection

A single `ExecutionSpec` in `tools.json` has a fixed `binaryWrapper` field. But the wrapper should only be applied conditionally — only when the nix group matched. Two approaches:

- **Option A (recommended): Two execution entries per tool.** The `tools.json` declares two execution specs for the same tool name, one with `binaryWrapper` and one without. The loader picks the appropriate one based on which bin group matched.

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

  The `condition` field links the execution spec to a bin group index. The loader filters execution specs to only those whose condition is satisfied.

- **Option B: The executor applies the wrapper dynamically.** The `ExecutionSpec` always has the `binaryWrapper` field. The executor checks whether the matched bin group requires the wrapper. This is simpler but couples the executor to loader semantics.

Option A is recommended because it keeps the executor unaware of bin group logic — the loader handles selection, and the executor just runs what it's given. The `condition` field is a general mechanism that could support other loading-time decisions in the future.

### Validation

1. `skills_validate` must accept `binaryWrapper` as a valid field in execution entries.
2. `skills_validate` must accept the nested-list form of `bins` in frontmatter.
3. When `binaryWrapper` is present, validate that the array is non-empty (at least one element — the wrapper binary).
4. When `condition.binGroup` is present, validate that the index corresponds to a valid group in `bins`.

### Security Considerations

- The wrapper binary is **not** allowlisted — the allowlist gates the declared `binary` and `subcommand`, not the transport. This is acceptable because the wrapper does not grant the tool new capabilities; it only changes how the declared command is reached.
- The wrapper binary must be on PATH (checked by the bin group gate). If `nix` is not on PATH, the skill does not load.
- `binaryWrapper` is an author-declared field in `tools.json`, not an agent-provided parameter. The agent cannot inject an arbitrary wrapper at runtime.

## Scope

This feature is an infrastructure change. It does not include the `cargo` skill itself — that is tracked separately in `FEAT_SKILL_CARGO.md`. Once `binaryWrapper` and OR-group bins are implemented, the `cargo` skill can be authored to use them.
