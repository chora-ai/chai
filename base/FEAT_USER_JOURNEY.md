# FEAT: Improve User Journeys

Improve the user journeys at `docs/journey/` so they form a coherent, progressive testing path with consistent quality, less duplication, and better coverage of chai's features.

## Problem

- **No on-ramp journey.** There is no journey for initial setup (`chai init`, first config, first provider). The numbering starts at gateway health checks, but a new user needs to get to a running state first.
- **README is a flat link list.** No descriptions, no guidance on who the journeys are for or how to use them, and no indication of a recommended order beyond the file numbering.
- **Inconsistent depth.** Journey 05 (Telegram, ~8.5 KB) is thorough with two modes, detailed troubleshooting, and a summary table. Journey 03 (send, ~1.6 KB) is a single negative test. Journey 09 (Signal, ~3 KB) is compact. The quality gap is large.
- **Duplicated setup content.** Gateway startup, WebSocket connect, and Ollama prerequisites are repeated verbatim across 01–04 and again in 06–07. There is no shared prerequisite section or common reference.
- **Thin coverage of features.** No journeys exist for: multi-agent configuration, provider setup beyond Ollama defaults, gateway auth, sandbox configuration, `chai profile` management, skill creation, or the `chai skill` CLI commands.
- **Skill journeys duplicate each other heavily.** 06 (notesmd) and 07 (obsidian) share identical sections ("Telegram message format for local models", "Context size", nearly identical "If something fails" entries). The duplication makes maintenance harder and increases the risk of the two drifting out of sync.
- **Journey 03 is essentially a negative test.** It verifies the `send` method returns an error for a non-existent channel and defers the success case to channel-specific journeys. It works as a thin API smoke test but doesn't stand on its own as a meaningful user journey.
- **No cross-references to guides.** The journeys don't link to the conceptual guides (`docs/guides/`) for background, and the guides (as noted in FEAT_USER_GUIDES R9) don't link to the journeys for hands-on procedures.
- **Inconsistent summary tables.** Journeys 05–09 include summary tables; 01–04 don't. The format is the same idea applied inconsistently.

## Recommendations

### R1 — Add an on-ramp journey (00-setup) [high priority]

Create `00-setup-init.md` covering:
- Prerequisites (Rust toolchain, Ollama install, optional Telegram/Matrix/Signal accounts).
- `chai init` and what it creates.
- Configure a first provider (Ollama with a default model).
- Verify the config file is correct.
- Start the gateway and confirm it responds (`curl` health check — link to 01 for full details).

This gives new users a single entry point. All other journeys can reference it as "you have completed 00 or equivalent" in their prerequisites.

### R2 — Enrich the README [high priority]

Rewrite `docs/journey/README.md` to include:
- A one-sentence description of each journey (so readers can choose without opening every file).
- Who these are for (new users walking through chai for the first time; developers verifying behavior after changes; QA before a release).
- A recommended path through the journeys (e.g., "Start with 00, then 01–03 for core gateway, then 04 for desktop, then pick your channel and skill journeys").
- A note on the relationship to the guides (journeys are hands-on; guides are conceptual).

### R3 — Harmonize depth and structure [high priority]

Bring the shorter journeys (01–04, 09) up to the standard set by 05 and 08. Specifically:
- **01–04:** Add summary tables, expand "If something fails" sections, and ensure every step has a clear expected outcome.
- **09 (Signal):** Add the same depth of troubleshooting as 05 (Telegram) — daemon check failures, multi-account issues, non-text message handling are already there but could benefit from more detail on the SSE event parsing and what the gateway does with different message types.
- **03 (send):** Either expand into a complete journey (success case with a registered channel, not just the error case) or merge its API-level test into 01 or 02 and repurpose the number.

### R4 — Reduce duplication across skill journeys [medium priority]

06 (notesmd) and 07 (Obsidian) share several identical sections. Extract the shared content into a common reference:
- Option A: Add a `00-skill-prerequisites.md` (or similar) covering shared skill setup (Ollama with tool-calling model, `chai init`, gateway startup, "Telegram message format for local models", "Context size") and have 06/07 link to it for the shared parts, keeping only the skill-specific steps and troubleshooting.
- Option B: Keep the files self-contained but move the shared sections into named, linkable anchors in a single file and cross-reference. This is more fragile but avoids splitting the flow.

Option A is recommended: it reduces maintenance surface and keeps the individual skill journeys focused.

### R5 — Add missing journeys [medium priority]

New journeys for uncovered features, using the filename convention from AGENTS.md:

- **`10-provider-ollama.md`** — Install Ollama, pull a model, configure `agents.defaultModel`, verify the model responds via the gateway agent method. Separate from 00 because it's a deeper dive and may be referenced from multiple journeys.
- **`11-provider-openai.md`** — Configure an OpenAI-compatible provider (API key, model name, base URL override), verify it responds. Extend to other cloud providers (Anthropic, etc.) as they are added.
- **`12-agent-multi.md`** — Configure multiple agents with different models/providers, verify routing by sending messages that target different agents.
- **`13-gateway-auth.md`** — Enable token auth, verify connect with and without the token, verify unauthorized requests are rejected, verify protected HTTP routes (e.g., Matrix verification endpoints).
- **`14-profile-manage.md`** — Create a second profile, switch profiles, verify the gateway uses the active profile's config, clean up.

These don't all need to be written immediately; prioritize based on which features are most used and most likely to confuse new users.

### R6 — Add a journey for skill creation [medium priority]

Create a journey (e.g., `15-skill-create.md`) that walks through creating a minimal custom skill from scratch:
- Create the skill directory (`SKILL.md`, `tools.json`, a simple script).
- Enable the skill on an agent.
- Start the gateway, verify the skill is loaded.
- Send an agent message that triggers the skill's tool.
- Verify the tool output appears in the reply.

This complements 06/07 (which test existing skills) and the skills guide (which covers the concepts).

### R7 — Fix cross-references to guides [medium priority] ✅

- Add links from each journey to the relevant guide section for background (e.g., 05 → `03-configuration.md#channels`, 06/07 → `06-skills.md`).
- Ensure the guides reference the journeys for hands-on procedures (this is the mirror of R9 from FEAT_USER_GUIDES, which is already tracked there).

### R8 — Normalize summary tables and "If something fails" [low priority]

- Add summary tables to journeys 01–04 (matching the format in 05–09).
- Ensure every journey has an "If something fails" section (01 has a "Notes" section that partially serves this role; convert to the standard format).
- Standardize the heading: use "If Something Fails" (title case, matching the convention in 02, 05–09) consistently.

### R9 — Clarify journey 03 or merge it [low priority]

Journey 03 (`gateway-ws-send`) is thin: it tests the `send` method against a non-existent channel and treats the error as the expected outcome. Options:
- **Expand** it to include a success case by having the user set up a minimal channel first (or reference the channel journeys and add a "revisit this after completing 05" step).
- **Merge** the WebSocket `send` API test into journey 02 (which already covers the WebSocket `agent` method) and either drop 03 or repurpose its number for a new journey.

The merge option is simpler and avoids a journey that feels incomplete on its own.

### R10 — Add version/context markers [maintenance]

Each journey documents current behavior but doesn't indicate when it was last validated. Add a lightweight convention:
- A comment or metadata line at the top of each journey: `<!-- Last validated: YYYY-MM-DD | Protocol: 1 | Default model: llama3.2 -->`
- This helps readers know if a journey may be stale and makes it easy to find which ones need updating after a breaking change.

### R11 — Consider a shared troubleshooting reference [future]

If more journeys are added (R5, R6), the "If something fails" sections will continue to duplicate common issues (Ollama not running, model missing, config not found). Consider extracting a shared `docs/journey/TROUBLESHOOTING.md` that covers the common cases, linked from each journey's "If something fails" section. The individual journeys would then only list issues unique to that flow. This is premature while there are only 9 journeys but becomes worthwhile at 12–15+.

## Progress

| Session | Work Done |
|---------|-----------|
| 1 | Initial audit and FEAT file created |
| 2 | R7 complete: added "Background" links in all 9 journey files pointing to relevant guides. Updated journey README with one-line descriptions and guide cross-references per journey. |
