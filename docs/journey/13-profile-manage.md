# Journey: Profile — manage and switch

**Goal:** Create a second profile, switch the active profile, verify the gateway uses the new profile's config, and clean up.

**Background:** [Configuration → Profiles](../guides/03-configuration.md#profiles) · [CLI Reference → `chai profile`](../guides/08-cli-reference.md#chai-profile)

Profiles are independent configuration trees under `~/.chai/profiles/<name>/`. The active profile is determined by the `~/.chai/active` symlink. `chai init` creates two default profiles (`assistant` and `developer`); you can create more manually.

## Prerequisites

- **Setup complete** — You have installed chai, run `chai init`, and verified the gateway works with defaults (see [00-setup-init.md](00-setup-init.md)).
- **Gateway not running** — Profile switching requires the gateway to be stopped (it holds an advisory lock at `~/.chai/gateway.lock`).

## Steps

1. **List current profiles**
   ```bash
   chai profile list
   ```
   - **Expect:** `assistant` and `developer` (the two defaults created by `chai init`).

2. **Check the active profile**
   ```bash
   chai profile current
   ```
   - **Expect:** `assistant` (the default active profile set by `chai init`).

3. **Create a new profile manually**
   - Create the profile directory and a config file:
   ```bash
   mkdir -p ~/.chai/profiles/testing
   echo '{"agents":[{"id":"orchestrator","role":"orchestrator","defaultModel":"qwen3:8b"}]}' > ~/.chai/profiles/testing/config.json
   ```
   - Create the sandbox and agent context directories:
   ```bash
   mkdir -p ~/.chai/profiles/testing/sandbox
   mkdir -p ~/.chai/profiles/testing/agents/orchestrator
   ```
   - Seed a minimal AGENT.md:
   ```bash
   echo '# Orchestrator (testing profile)\n\nYou are a test assistant.' > ~/.chai/profiles/testing/agents/orchestrator/AGENT.md
   ```

4. **Verify the new profile appears**
   ```bash
   chai profile list
   ```
   - **Expect:** `assistant`, `developer`, and `testing`.

5. **Switch to the new profile**
   ```bash
   chai profile switch testing
   ```
   - **Expect:** Output confirming the switch. Verify:
   ```bash
   chai profile current
   ```
   - **Expect:** `testing`.
   - The symlink `~/.chai/active` now points to `profiles/testing/`.

6. **Start the gateway with the new profile**
   ```bash
   chai gateway
   ```
   - **Expect:** The gateway loads `~/.chai/profiles/testing/config.json`. If the model `qwen3:8b` is available in Ollama, the gateway discovers it.

7. **Verify the gateway uses the new config**
   - In another terminal:
   ```bash
   chai chat
   ```
   - Send: "What model are you?"
   - **Expect:** A reply. The model used depends on the `testing` profile's `defaultModel` setting.
   - Check gateway logs for the model being used with `RUST_LOG=info`.

8. **Test per-process profile override (optional)**
   - With the gateway still running on the `testing` profile, try:
   ```bash
   CHAI_PROFILE=assistant chai chat
   ```
   - **Expect:** This connects to the running gateway (which is on the `testing` profile). The `--profile` flag and `CHAI_PROFILE` control which gateway process to connect to; if a single gateway is running, all chat clients connect to it regardless of the flag.

9. **Stop the gateway and switch back**
   - Ctrl+C the gateway.
   ```bash
   chai profile switch assistant
   ```
   - **Expect:** Output confirming the switch back. Verify:
   ```bash
   chai profile current
   ```
   - **Expect:** `assistant`.

10. **Clean up the test profile (optional)**
    - Remove the test profile directory:
    ```bash
    rm -rf ~/.chai/profiles/testing
    ```
    - Verify it's gone:
    ```bash
    chai profile list
    ```
    - **Expect:** Only `assistant` and `developer`.

## If Something Fails

- **`chai profile switch` fails with "gateway is running"** — The gateway must be stopped before switching. Stop it with Ctrl+C or kill the process. If the gateway crashed and the lock is stale, remove it: `rm ~/.chai/gateway.lock`.
- **New profile not visible in `chai profile list`** — The profile directory must exist under `~/.chai/profiles/`. Ensure the directory was created correctly: `ls ~/.chai/profiles/`.
- **Gateway fails to start after switching** — The new profile's `config.json` may be invalid. Check the JSON: `cat ~/.chai/profiles/testing/config.json | python3 -m json.tool`. Common issues: missing commas, invalid field names.
- **Chat uses wrong model** — The model is determined by the active profile's `config.json`. Ensure the `defaultModel` field is set correctly in the profile you switched to, and that the model exists in Ollama (`ollama list`).
- **`~/.chai/active` symlink is broken** — If the symlink points to a profile directory that was deleted, commands will fail. Fix it by switching to an existing profile: `chai profile switch assistant`.
- **Cannot delete a profile while gateway is running** — The gateway holds a lock on the active profile. Stop the gateway, then delete the profile directory.

## Summary

| Step | Action | Expected Outcome |
|------|--------|-------------------|
| 1 | `chai profile list` | `assistant`, `developer` |
| 2 | `chai profile current` | `assistant` |
| 3 | Create `testing` profile | Directory + config + sandbox |
| 4 | `chai profile list` | `testing` appears |
| 5 | `chai profile switch testing` | Active → `testing` |
| 6 | `chai gateway` | Gateway loads testing config |
| 7 | `chai chat` | Model from testing profile used |
| 9 | `chai profile switch assistant` | Active → `assistant` |
| 10 | Remove `testing` profile (optional) | Profile deleted |

**Next:** [10 — Provider: Ollama and LM Studio](10-provider-ollama-lmstudio.md) · [12 — Gateway: auth](12-gateway-auth.md)
