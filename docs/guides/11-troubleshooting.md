# Troubleshooting

Common issues and how to resolve them. If your problem isn't listed here, check the gateway log output (`RUST_LOG=debug chai gateway` for verbose logging) and the [Testing Playbooks](../testing/README.md) for provider-specific setup procedures.

## Gateway

### Gateway Fails to Start

**"address already in use"** — Another process is using the port. Either stop that process or change `gateway.port` in `config.json`:

```json
{ "gateway": { "port": 15152 } }
```

Or override per run:

```bash
chai gateway --port 15152
```

**"gateway is running" when running `chai profile switch`** — The gateway holds an advisory lock. Stop the gateway before switching profiles.

### Gateway Starts but No Response From Model

This almost always means the configured provider is unreachable or the model isn't loaded.

1. **Check the provider is running.** For Ollama: `ollama list` should show your model. For LM Studio: the model must be loaded in the LM Studio UI (or `modelDiscovery: "lmstudio"` must be set for automatic retry on unload).
2. **Check the model id.** The `defaultModel` value must exactly match what the provider expects. Use `ollama list` for Ollama, `GET /api/v1/models` for LM Studio, or the provider's model catalog for cloud APIs.
3. **Check connectivity.** The gateway defaults to `http://127.0.0.1:11434` for Ollama and `http://127.0.0.1:1234/v1` for LM Studio. If you changed these, verify the URLs are correct.
4. **Check the logs.** Start with `RUST_LOG=info chai gateway` and look for provider discovery errors or connection failures. If the gateway is already running, use `chai logs recent` or `chai logs search --pattern error` to read from the in-memory log buffer without restarting.

### Gateway Immediately Exits

Check `~/.chai/` exists and has the expected structure. Run `chai init` to ensure the scaffold is complete:

```bash
chai init
```

This is safe to re-run — it won't overwrite existing files.

## Provider Errors

### Ollama: "Connection Refused"

Ollama is not running. Start it:

```bash
ollama serve
```

Or use the system tray application. Verify with:

```bash
ollama list
```

### Ollama: Model Not Found

The model name in your config doesn't match what's available locally. Pull the model first:

```bash
ollama pull llama3.2:3b
```

Then verify it appears in `ollama list`. The model id must match exactly (including tag, e.g. `:3b`).

### LM Studio: "Unloaded" Error

The requested model isn't loaded in LM Studio. Either:

- Load the model in the LM Studio UI before sending messages, or
- Configure `modelDiscovery: "lmstudio"` so chai uses LM Studio's native model list and automatically retries on "unloaded" errors:

```json
{ "id": "lms", "endpointType": "openai-compat", "modelDiscovery": "lmstudio" }
```

### Cloud Provider: Authentication Failed

The API key is missing or invalid. Either:

- Set the `"apiKey"` field in the provider object in `config.json`, or

Environment variables take precedence over `config.json` values when both are set.

### Cloud Provider: Model Not Found

The model id in `defaultModel` must match the provider's exact model name. Check the provider's model catalog (NearAI, NVIDIA NIM, etc.) for the correct id. For NVIDIA NIM, use `modelDiscovery: "static"` with a `staticModels` list — NIM does not expose a `/v1/models` endpoint.

## Channels

### Telegram: Bot Not Responding

1. **Verify the bot token.** Set `channels.telegram.botToken` or `TELEGRAM_BOT_TOKEN`. You can get a token from [@BotFather](https://t.me/BotFather) on Telegram.
2. **Check the gateway logs.** Look for Telegram polling or webhook errors.
3. **For webhook mode**, ensure `webhookUrl` is publicly accessible and `webhookSecret` matches.

### Matrix: Bot Not Joining Rooms

The bot user must be **invited** to rooms before it can read or send messages. Invite the bot from Element (or another Matrix client). The bot doesn't auto-join rooms.

### Matrix: Echo Loop (Bot Responds to Itself)

Set `channels.matrix.userId` to the bot's Matrix id (e.g., `@my-bot:matrix.org`). The gateway uses this to filter out the bot's own messages. If using token auth, also set `MATRIX_USER_ID`.

### Signal: Connection Refused

Signal requires a running signal-cli HTTP daemon. Verify it's running:

```bash
# Check if the daemon is listening
curl http://127.0.0.1:7583/api/v1/about
```

Set `channels.signal.httpBase` or `SIGNAL_CLI_HTTP` to match the daemon's address.

## Skills

### Skills Not Loading

1. **Check `enabledSkills`.** Skills must be explicitly listed on each agent entry in `config.json`. An empty or omitted `enabledSkills` means no skills are loaded.
2. **Check the skill exists.** Run `chai skill list` to see installed skills and their status.
3. **Check the `active` symlink.** Each skill under `~/.chai/skills/` should have an `active` symlink pointing to `versions/<hash>/`. If the symlink is broken, the skill won't load.
4. **Check `metadata.requires.bins`.** Skills that declare required binaries (e.g., `["git"]`) are only loaded when every binary is on `PATH`.

### Capability Tier Warning at Startup

The gateway warns when an enabled skill's declared `capability_tier` exceeds the likely capability of the configured model. You can:

- Ignore the warning (the skill may still work with a weaker model, just less reliably)
- Switch to a larger model
- Use a lower-tier variant of the skill (e.g., `files-read` instead of `files`)

See [Choosing a Provider and Model](10-choosing-a-provider.md#skill-capability-tiers) for the tier table.

### Skill Version Directory Hash Mismatch

The directory name under `versions/` must be the 12-hex-character truncated SHA-256 of the skill's content. If you edited files in a version snapshot directly, the hash no longer matches. To fix this:

1. Use `chai skill write-*` commands (which create proper new snapshots), or
2. Follow the manual workflow in [Skills → Manual Workflow](06-skills.md#manual-workflow), which includes computing the correct hash

**Do not** edit files in place under `versions/<hash>/` — those directories are immutable by design.

### Skills Lock Verification Failure

The gateway refuses to start in `strict` mode (the default `skills.lockMode`) when the lockfile is missing, any enabled skill has no lock entry (unpinned), or an enabled skill's active version doesn't match its pinned hash in `skills.lock`. This can happen when:

- You created a profile manually without running `chai skill lock` — Run `chai skill lock` to create the lockfile.
- You enabled a new skill in `config.json` but didn't re-lock — Run `chai skill lock` to add the new skill to the lockfile.
- You updated a skill with `chai skill write-*` but didn't re-lock — Run `chai skill lock` to update the lock.
- You rolled back a skill manually by repointing the `active` symlink — Run `chai skill lock` to re-pin.
- The lock file is stale after a `chai init` update — Run `chai skill lock` to re-pin at the current versions.

To bypass the check temporarily, set `skills.lockMode` to `"warn"` in your profile's `config.json`:

```json
{
  "skills": {
    "lockMode": "warn"
  }
}
```

This logs warnings instead of refusing to start. See [Skills → Skill Lock Mode](06-skills.md#skill-lock-mode) for details.

### Tool Validation Fails

Run `chai skill validate <name>` to check tool descriptor files for schema errors. Common issues:

- Missing required file: `allowlist.json` or `execution.json` (when `tools.json` is present)
- Tool name in `execution.json` that doesn't match any entry in `tools.json`
- Binary in `execution.json` that isn't listed in `allowlist.json`
- Subcommand in `execution.json` that isn't in the binary's allowlist in `allowlist.json`

## Profiles

### Profile Switch Fails

The gateway must be stopped before switching profiles. The advisory lock at `~/.chai/gateway.lock` prevents concurrent access.

If the gateway crashed and the lock file is stale (no gateway process running but the lock exists), delete it:

```bash
rm ~/.chai/gateway.lock
```

### Wrong Profile Active After `chai init`

`chai init` preserves the existing `~/.chai/active` symlink — it only sets the symlink to `profiles/assistant/` when no valid symlink already exists. If the symlink was changed, switch back:

```bash
chai profile switch <name>
```

## Desktop App

### Desktop Cannot Connect to Gateway

1. **The gateway must be running.** Start it from the header button or via `chai gateway`.
2. **Check the bind address.** The desktop connects to the address configured in `gateway.bind` and `gateway.port`. If you changed these, the desktop must use the same values.
3. **Stale device token.** If `~/.chai/active/device_token` holds a token the gateway no longer recognizes (e.g., `paired.json` was deleted from the profile directory), the desktop should automatically re-pair. If it doesn't, delete the token file and restart the desktop:
   ```bash
   rm ~/.chai/active/device_token
   ```

### Desktop Gateway Fails to Start

The desktop spawns `chai gateway` as a subprocess. If it fails to start or crashes, the error is shown in the header (visible from any screen) and on the Gateway screen. Common causes:

- **Config parse error** — Invalid JSON in `config.json`. The error message includes the file path and the specific parse error.
- **Missing `chai` binary** — The desktop cannot find the `chai` binary next to itself or on `PATH`.
- **`CHAI_BIN` in `.env` points to a non-existent path** — The profile's `.env` sets `CHAI_BIN` to a path that does not exist. Remove or fix the line in `.env`.
- **Other startup failures** — Same causes as CLI gateway issues (port in use, provider unreachable, sandbox errors). The error message from the gateway log is displayed directly (e.g. "sandbox directory not found at...").

For full log output, check the **Logging** screen.

## Chat

### Model Returns No Tool Calls (But Skills Are Enabled)

- **The model is too small.** Smaller models (especially under 7B) may not reliably produce tool calls. Try a larger model.
- **The model doesn't support tool calling.** Not all models support the OpenAI-style function calling interface. Check the model's documentation.
- **`contextMode` is `readOnDemand`** and the model isn't calling `read_skill`. Try `contextMode: "full"` so the model sees full skill content in its system context.

### Tool Loop Limit Reached

The gateway stops the turn after `maxToolLoopsPerTurn` (when set) consecutive tool calls. This is a safety net against runaway loops. The desktop shows an amber banner; the CLI chat emits a `session.tool_loop_limit` event.

- If this happens legitimately (a complex task needs many steps), increase the limit in `config.json`:
  ```json
  { "agents": [{ "id": "orchestrator", "role": "orchestrator", "maxToolLoopsPerTurn": 200 }] }
  ```
- If the loop seems stuck, the model may be repeating the same tool call — this usually indicates the model is confused. Send a corrective message to redirect it.

## General Debugging Tips

1. **Run with debug logging:** `RUST_LOG=debug chai gateway` shows every provider request, tool execution, and agent turn. If the gateway is already running with debug logging enabled, use `chai logs recent --level debug` or `chai logs search --pattern "your query"` to inspect the log buffer.
2. **Validate your config:** Start the gateway and check for warnings at startup — they flag missing providers, mismatched agent references, and skill capability gaps.
3. **Test providers independently:** Before configuring chai, verify the provider works directly:
   - Ollama: `ollama run llama3.2:3b`
   - LM Studio: Use the built-in chat UI
   - Cloud APIs: `curl` with your API key
4. **Check file permissions:** Ensure `~/.chai/` and its contents are readable/writable by your user. The gateway and CLI need both read and write access.
