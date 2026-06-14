# CLI Reference

This reference covers every command in the `chai` CLI. Install with `cargo install --path crates/cli`. Add `--features matrix` for the experimental Matrix channel, `--features signal` for the experimental Signal channel, or `--features matrix,signal` for both. Run `chai --help` or `chai <command> --help` for built-in usage text.

## Global

```bash
chai                    # Prints a short usage reminder
chai --help             # Full help text
```

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
| `--profile <NAME>` | Override the active profile for this process (alternative to `CHAI_PROFILE`) |
| `--port <PORT>` | Override `gateway.port` for this run |

The gateway holds an advisory lock at `~/.chai/gateway.lock` while running. Only one gateway can run at a time for a given configuration directory.

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
```

| Flag | Description |
|------|-------------|
| `--profile <NAME>` | Profile for config resolution (must match the running gateway's profile) |
| `--session <ID>` | Resume an existing session by id |

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
chai profile switch <name>          # Change the active symlink (gateway must be stopped)
```

| Subcommand | Description |
|-----------|-------------|
| `list` | Print all profile directories under `~/.chai/profiles/` |
| `current` | Print the persistent profile from `~/.chai/active`. If `CHAI_PROFILE` is set and differs, also shows the effective override. |
| `switch <name>` | Repoint `~/.chai/active` to `profiles/<name>/`. Fails if the gateway is running. |

The `CHAI_PROFILE` environment variable overrides `~/.chai/active` for a single process without changing the symlink.

## `chai skill`

Manage skill packages — inspection, creation, updates, validation, and version pinning.

```bash
chai skill list                                           # Show installed skills and status
chai skill read --skill-name <name> --file skill_md       # Read SKILL.md
chai skill read --skill-name <name> --file tools_json     # Read tools.json
chai skill validate --skill-name <name>                   # Validate tools.json
chai skill init --name <name> --description "..."         # Create a new skill
chai skill delete --skill-name <name>                     # Remove a skill package
```

### Writing Content

Each write command creates a **new versioned snapshot** — the current active snapshot is never edited in place.

```bash
# Write SKILL.md from a flag or stdin
chai skill write-skill-md --skill-name <name> --content '...'
echo "..." | chai skill write-skill-md --skill-name <name>

# Write tools.json (validated before writing)
chai skill write-tools-json --skill-name <name> --content '...'
echo '...' | chai skill write-tools-json --skill-name <name>

# Write a script
chai skill write-script --skill-name <name> --script-name <base> --content '...'
echo '...' | chai skill write-script --skill-name <name> --script-name <base>
```

When `--content` is omitted, content is read from stdin. The `--content` flag accepts values that begin with dashes (e.g. YAML frontmatter).

### Version Pinning

Skills are shared across profiles. Lockfiles pin active versions for reproducibility.

```bash
chai skill lock                      # Pin current active versions to skills.lock
chai skill generations               # List saved lock generations
chai skill rollback <generation>     # Restore a saved generation and repoint active symlinks
```

The default `skillLockMode` is `strict` — the gateway refuses to start when an enabled skill's active version does not match its locked hash. However, `strict` mode has no effect until a `skills.lock` file exists for the active profile. `chai init` generates the lock for profiles it creates; for manually created profiles, you must run `chai skill lock` yourself. See [Skills → Skill Lock Mode](06-skills.md#skill-lock-mode) for details.

### Discovery

```bash
chai skill discover <binary>                    # Show a CLI's help output
chai skill discover <binary> --subcommand <sub> # Show subcommand help
```

Discovers a CLI binary's interface by running its `--help`. Useful when building `tools.json` for a new binary.

For more on skills, see [Skills](06-skills.md).

## `chai file`

File operations primarily designed for skill tool backends. These commands are lower-level than the skill system and are typically used by scripts or when working outside a skill's tool execution context.

Most commands accept content via `--content` or stdin (when `--content` is omitted).

### Reading

```bash
chai file read-lines --path <PATH> --start-line <N> [--end-line <N>]
```

Read a range of lines from a file. Output format: `{line_number}|{content}`. Line numbers are 1-indexed and inclusive on both ends. When `--end-line` is omitted, only `--start-line` is read.

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

The `patch` command replaces lines `[start_line, end_line]` with new content. If `--end-line` is omitted, only `--start-line` is replaced. When `--original-content` is provided, the tool verifies it matches the file before applying the patch — if it doesn't match, the edit is rejected.

### Find and Replace

```bash
chai file replace --path <PATH> --pattern <PATTERN> --replacement <REPLACEMENT> [--line-numbers]
```

Replace all occurrences of a regex pattern in a file. The pattern is matched against the full file content with multiline mode enabled (`^` and `$` match line boundaries). Supports capture groups (`$1`–`$9`) in the replacement string. Use `$$` for a literal `$`. Use an empty replacement to delete matches. Returns a diff of all changes made.

**Line deletion** — Match the line content plus its trailing newline and replace with an empty string to delete the line entirely:

```bash
chai file replace --path config.toml --pattern 'obsolete_field: None,\n' --replacement ''
```

**Capture groups** — Bump a version number across all matching lines:

```bash
chai file replace --path config.toml --pattern 'version = "(\d+)\.(\d+)\.(\d+)"' --replacement 'version = "$1.$2.4"'
```

When `--line-numbers` is omitted, it defaults to true (line numbers shown in the diff). If zero matches are found, exits 0 with a "0 replacements" message.

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

Frontmatter is the YAML block between `---` delimiters at the top of a Markdown file. `frontmatter-edit` creates the block if absent and adds the key if missing. `frontmatter-delete` is a no-op if the key doesn't exist.

### Renaming With Wikilink Update

```bash
chai file rename-note --from <OLD_PATH> --to <NEW_PATH> --root <SEARCH_DIR>
```

Move a Markdown note and update all `[[old-name]]` and `[[old-name|...]]` wikilinks in `.md` files under `--root`. The parent directory of `--to` must exist.
