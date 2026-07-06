# CLI Reference

This reference covers every command in the `chai` CLI. Install with `cargo install --path crates/cli`. Add `--features matrix` for the experimental Matrix channel, `--features signal` for the experimental Signal channel, or `--features matrix,signal` for both. Run `chai --help` or `chai <command> --help` for built-in usage text.

## Global

```bash
chai                    # Prints a short usage reminder
chai --help             # Full help text
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CHAI_HOME` | `~/.chai` | Overrides the chai home directory. Set to an absolute or relative path. Empty string falls back to the default. |
| `RUST_LOG` | `info` (gateway), `warn` (other) | Log level for the `env_logger` crate. |

## `chai version`

Print the installed version.

```bash
chai version
```

## `chai init`

Create the `~/.chai/` home directory with an `active` symlink, bundled profiles, bundled skills, and a `skills.lock` for each newly seeded profile. Safe to re-run on an existing configuration — existing files are never overwritten and user customizations are preserved. See [Configuration](03-configuration.md#initialization) for the full re-run behavior.

```bash
chai init
```

## `chai gateway`

Start the gateway (HTTP + WebSocket server). The gateway loads the active profile, discovers models from configured providers, resolves enabled skills, and listens for incoming connections.

```bash
chai gateway                        # Defaults from config
chai gateway --profile developer    # Use a specific profile
chai gateway --port 8080            # Override the port
```

| Flag | Description |
|------|-------------|
| `--profile <NAME>` | Override the active profile for this command |
| `--port <PORT>` | Override `gateway.port` for this run |

The gateway holds a per-profile advisory lock at `~/.chai/profiles/<name>/gateway.lock` while running. Multiple gateways can run simultaneously on different profiles; only one gateway is allowed per profile.

Log output uses the `RUST_LOG` environment variable. The default level for `chai gateway` is `info`; for all other commands it is `warn`.

```bash
RUST_LOG=debug chai gateway    # Verbose logging
```

## `chai chat`

Start an interactive chat session with the default agent through the running gateway. The gateway must already be started (via `chai gateway` or the desktop app).

```bash
chai chat                           # New session, active profile
chai chat --session <ID>            # Continue an existing session
chai chat --profile developer       # Connect to the gateway on a specific profile
chai chat --agent researcher        # Use a specific orchestrator
```

| Flag | Description |
|------|-------------|
| `--profile <NAME>` | Profile for config resolution (must match the running gateway's profile) |
| `--session <ID>` | Resume an existing session by id |
| `--agent <ID>` | Select which orchestrator to use (defaults to the first orchestrator) |

**Chat commands** (typed as messages):
| Command | Description |
|---------|-------------|
| `/new` | Start a new session (clears conversation history) |
| `/help` | Show available commands |
| `/exit` or `/quit` | Exit the chat |

## `chai profile`

Manage profiles — independent configuration trees under `~/.chai/profiles/<name>/`.

```bash
chai profile list                   # List all profile names
chai profile current                # Show persistent and effective profile
chai profile switch <name>          # Change the active symlink (always allowed)
```

| Subcommand | Description |
|-----------|-------------|
| `list` | Print all profile directories under `~/.chai/profiles/` |
| `current` | Print the active profile from `~/.chai/active`. |
| `switch <name>` | Repoint `~/.chai/active` to `profiles/<name>/`. Always succeeds — switching is independent of running gateways. |

## `chai session`

Manage sessions — list, delete, or clear sessions for the active profile. These commands operate directly on the session store on disk; no gateway connection is required.

```bash
chai session list                            # List sessions for the active profile
chai session list --profile developer        # List sessions for a specific profile
chai session list --agent researcher         # List sessions for a specific orchestrator
chai session delete <ID>                     # Delete a session by id
chai session clear                           # Delete all sessions for the default orchestrator
chai session clear --agent researcher        # Delete all sessions for a specific orchestrator
```

| Subcommand | Description |
|-----------|-------------|
| `list` | List sessions from disk. Shows session id (shortened), message count, timestamp, and channel binding (if any). Sorted by most recently updated. |
| `delete <ID>` | Delete a session by id. Removes the session and its binding from disk. |
| `clear` | Delete all sessions from disk. Reports the count of deleted sessions. |

| Flag | Description |
|------|-------------|
| `--profile <NAME>` | Override the active profile (available on all subcommands) |
| `--agent <ID>` | Scope to a specific orchestrator's session store (available on `list` and `clear`) |

## `chai skill`

Manage skill packages — inspection, creation, updates, validation, and version pinning.

```bash
chai skill list                                           # Show installed skills and status
chai skill read <name> --file skill_md                     # Read SKILL.md
chai skill read <name> --file tools_json                   # Read tools.json
chai skill read <name> --file allowlist_json               # Read allowlist.json
chai skill read <name> --file execution_json               # Read execution.json
chai skill validate <name>                                 # Validate tool descriptor files
chai skill init <name> --description "..."                 # Create a new skill
chai skill delete <name>                                   # Remove a skill package
```

### Writing Content

Each write command creates a **new versioned snapshot** — the current active snapshot is never edited in place.

```bash
# Write SKILL.md from a flag or stdin
chai skill write-skill-md <name> --content '...'
echo "..." | chai skill write-skill-md <name>

# Write tools.json (validated before writing)
chai skill write-tools-json <name> --content '...'
echo '...' | chai skill write-tools-json <name>

# Write allowlist.json (validated before writing)
chai skill write-allowlist-json <name> --content '...'
echo '...' | chai skill write-allowlist-json <name>

# Write execution.json (validated before writing)
chai skill write-execution-json <name> --content '...'
echo '...' | chai skill write-execution-json <name>

# Write a script
chai skill write-script <name> <base> --content '...'
echo '...' | chai skill write-script <name> <base>
```

When `--content` is omitted, content is read from stdin. The `--content` flag accepts values that begin with dashes (e.g. YAML frontmatter).

### Dry Run

Preview what a tool call would execute without running the command. This walks the execution pipeline (sandbox validation, deny pattern checks, argv building, stdin extraction, temp file computation, subcommand resolution) and returns the result as JSON. Useful for verifying `execution.json` and `allowlist.json` mappings are correct during skill authoring.

```bash
chai skill dry-run <tool> --args '<json>' [--simulated-output '<text>'] [--profile <name>]
```

| Flag | Description |
|------|-------------|
| `<TOOL>` | Tool name to preview (e.g. `git_commit`, `files_write`) |
| `--args <ARGS>` | Tool call arguments as JSON (e.g. `'{"message": "feat: add feature"}'`) |
| `--simulated-output <TEXT>` | Simulated command output for post-execution pipeline preview (postProcess, hintConditions, truncation) |
| `--profile <PROFILE>` | Profile name for sandbox resolution (uses default profile if omitted) |

**Pipeline behavior:**

- **Sandbox validation fail** → Returns partial result with `sandbox_validation.status = "fail"` and downstream fields empty (nothing can be computed without valid paths).
- **Deny pattern fail** → Returns full preview with `deny_patterns.status = "fail"` and argv/subcommand/resolved_params still computed. The deny failure is informational — it shows what *would* be blocked while revealing the argv mapping.
- **Both pass** → Full preview with argv, stdin_content, temp_files, resolved_params, and post_pipeline metadata.
- **Simulated output** → When provided and the spec has postProcess/hintConditions/truncation, the actual post-processing pipeline runs on the simulated output.

### Version Pinning

Skills are shared across profiles. Lockfiles pin active versions for reproducibility.

```bash
chai skill lock                      # Pin current active versions to skills.lock
chai skill generations               # List saved lock generations
chai skill rollback <generation>     # Restore a saved generation and repoint active symlinks
```

The default `skills.lockMode` is `strict` — the lockfile acts as a complete manifest, so the gateway refuses to start when the lockfile is missing, any enabled skill has no lock entry (unpinned), or any pinned skill's active version does not match its locked hash. `chai init` generates the lock for profiles it creates; for manually created profiles, you must run `chai skill lock` yourself (or set `lockMode` to `"warn"`). See [Skills → Skill Lock Mode](06-skills.md#skill-lock-mode) for details.

### Discovery

```bash
chai skill discover <binary>                    # Show a CLI's help output
chai skill discover <binary> --subcommand <sub> # Show subcommand help
```

Discovers a CLI binary's interface by running its `--help`. Useful when building tool descriptor files for a new binary.

For more on skills, see [Skills](06-skills.md).

## `chai file`

Tool backend for file operations. These commands are lower-level than the skill system and are typically used by scripts or when working outside a skill's tool execution context.

Most commands accept content via `--content` or stdin (when `--content` is omitted).

### Reading

```bash
chai file read-lines --path <PATH> --start-line <N> [--end-line <N>]
```

Read a range of lines from a file. Output format: `{line_number}\t{content}`. Line numbers are 1-indexed and inclusive on both ends. When `--end-line` is omitted, reads from `--start-line` to the end of the file.

### Writing

```bash
# Create or overwrite a file
chai file write --path <PATH> --content '...'
echo '...' | chai file write --path <PATH>

# Append to a file (creates if absent)
chai file append --path <PATH> --content '...'
echo '...' | chai file append --path <PATH>

# Replace a range of lines (patch)
chai file patch --path <PATH> --start-line <N> [--end-line <N>] \
  --original-content '...' --content '...'
```

The `patch` command replaces lines `[start_line, end_line]` with new content. When `--end-line` is omitted, the replacement range is inferred from the number of lines in `--original-content`. When `--original-content` is provided (or `--original-content-file`), the tool verifies it matches the file before applying the patch — if it doesn't match, the edit is rejected. Use `--original-content-file` to read the original content from a file instead of passing it as a CLI flag (avoids encoding issues for multi-line content).

### Find and Replace

```bash
chai file replace --path <PATH> (--pattern <PATTERN> | --pattern-file <FILE>) [--replacement <REPLACEMENT>] [--line-number] [--literal] [--dry-run]
```

Replace all occurrences of a regex pattern in a file. The pattern is matched against the full file content with multiline mode enabled (`^` and `$` match line boundaries). Supports capture groups (`$1`–`$9`) in the replacement string. Use `$$` for a literal `$`. Use an empty replacement to delete matches. Returns a diff of all changes made.

**Diff line number convention** — Both `patch` and `replace` return diffs with post-edit line numbers: removed lines (`-` prefix) use original-file line numbers; added lines (`+` prefix) and context-after lines use new-file line numbers. This means context lines after an insertion show their shifted positions, not the original numbers.

Either `--pattern` or `--pattern-file` is required. When `--replacement` is omitted, the replacement is read from stdin.

**Dry run** — Preview what would change without modifying the file:

```bash
chai file replace --path config.toml --pattern 'obsolete_field' --replacement 'new_field' --dry-run
```

**Line deletion** — Match the line content plus its trailing newline and replace with an empty string to delete the line entirely:

```bash
chai file replace --path config.toml --pattern 'obsolete_field: None,\n' --replacement ''
```

**Capture groups** — Bump a version number across all matching lines:

```bash
chai file replace --path config.toml --pattern 'version = "(\d+)\.(\d+)\.(\d+)"' --replacement 'version = "$1.$2.4"'
```

**Literal mode** — When the pattern contains regex metacharacters (source code, markdown tables, JSON) that should be matched as-is:

```bash
chai file replace --path config.toml --pattern '| header |' --replacement '| updated |' --literal
```

Capture groups (`$1`–`$9`) are not supported in literal mode.

| Flag | Description |
|------|-------------|
| `--pattern <PATTERN>` | Regex search pattern (extended regex, multiline mode) |
| `--pattern-file <FILE>` | Read the pattern from a file (avoids CLI encoding issues for multi-line patterns; takes precedence over `--pattern`) |
| `--replacement <REPLACEMENT>` | Replacement string (falls back to stdin when omitted) |
| `--line-number` | Show line numbers in the diff output (off by default) |
| `--literal` | Treat the pattern as literal text instead of regex |
| `--dry-run` | Preview the replacement diff without modifying the file |

If zero matches are found, exits 0 with a "0 replacements" message.

### Deleting

```bash
chai file delete --path <PATH>        # Delete a file (refuses directories)
chai file delete-dir --path <PATH>    # Delete an empty directory (refuses non-empty)
```

### Frontmatter

```bash
chai file frontmatter-read --path <PATH>                    # Read YAML frontmatter
chai file frontmatter-edit --path <PATH> --key <K> --value <V>  # Set a frontmatter key
chai file frontmatter-delete --path <PATH> --key <K>        # Remove a frontmatter key
```

Frontmatter is the YAML block between `---` delimiters at the top of a Markdown file. `frontmatter-edit` creates the block if absent and adds the key if missing. Use `--value-file` instead of `--value` to read the value from a file (avoids encoding issues for multi-line values). `frontmatter-delete` is a no-op if the key doesn't exist.

### Renaming With Wikilink Update

```bash
chai file rename --from <OLD_PATH> --to <NEW_PATH> [--scope <SEARCH_DIR>]
```

Move a Markdown note and update all `[[old-name]]` and `[[old-name|...]]` wikilinks in `.md` files under `--scope`. When `--scope` is omitted, it defaults to the current working directory. The parent directory of `--to` must exist.

## `chai logs`

Tool backend for log operations. Read and search the gateway's in-memory log buffer. These commands query the running gateway via its HTTP API — the gateway must be running for them to return data.

### Recent Lines

```bash
chai logs recent [--lines N] [--level LEVEL]
```

Return the most recent N lines from the gateway log buffer. The `--lines` flag controls how many lines to return (default: 50, max: 200). Use `--level` to filter by severity (`info`, `warn`, `error`, `debug`).

### Search

```bash
chai logs search --pattern <PATTERN> [--context N]
```

Search log lines for a substring pattern. Matching lines are prefixed with `>` and surrounded by context lines (default: 2). Useful for finding specific events like `finish_reason`, token counts, or error messages.

## `chai resolve`

Sandbox-aware path resolution for tool parameter validation. This subcommand is primarily used by bundled skill `resolveCommand` entries — it validates that resolved paths are inside the sandbox before allowing tool calls to proceed. Each variant resolves the sandbox root from `$HOME/.chai/active/sandbox` and outputs the validated path on stdout (exit 0) or an error on stderr (exit 1).

```bash
chai resolve repo-path [--path <PATH>]       # Validate git repo path (.git inside sandbox)
chai resolve cargo-path [--path <PATH>]      # Validate cargo workspace path (Cargo.toml inside sandbox)
chai resolve clone-path [--path <PATH>]      # Validate clone target path (inside sandbox)
chai resolve file-path [--path <PATH>]       # Validate file path (inside sandbox)
chai resolve sandbox [--path <PATH>]         # Validate generic path is inside sandbox
```

| Variant | What It Validates | Use Case |
|---------|-------------------|----------|
| `repo-path` | Runs `git rev-parse --git-dir` and checks the `.git` directory is inside the sandbox | Git skill `workingDir` resolution |
| `cargo-path` | Runs `cargo locate-project` and checks the `Cargo.toml` directory is inside the sandbox | Cargo skill `workingDir` resolution |
| `clone-path` | Validates absolute clone targets are inside the sandbox; prefixes relative paths with the sandbox root | Git-remote `git_clone` path resolution |
| `file-path` | Validates a file path is inside the sandbox (handles non-existent paths via ancestor-walk canonicalization) | Generic file path validation |
| `sandbox` | Validates a generic path is inside the sandbox (no project-root discovery) | Generic sandbox boundary check |

When `--path` is omitted or empty, the working directory defaults to the sandbox root. The subcommand handles symlinked directories by checking against both the canonical sandbox root and the physical targets of symlinked entries at the top level of the sandbox directory.
