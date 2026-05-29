---
status: current
---

# Claw Ecosystem

**Single source of truth** for the high-level matrix comparing **OpenClaw** (upstream), **IronClaw**, **NemoClaw**, and **Chai** (this monorepo). The per-project references link here so the table is maintained in one place:

- [OPENCLAW.md](OPENCLAW.md) — OpenClaw protocol and concepts; [Chai vs OpenClaw (Feature Summary)](OPENCLAW.md#chai-vs-openclaw-feature-summary); [Chai vs OpenClaw (Detailed)](OPENCLAW.md#chai-vs-openclaw-detailed) for pairing and skills.
- [IRONCLAW.md](IRONCLAW.md) — IronClaw architecture and providers; [Chai vs IronClaw (Feature Summary)](IRONCLAW.md#chai-vs-ironclaw-feature-summary).
- [NEMOCLAW.md](NEMOCLAW.md) — NemoClaw sandbox and OpenShell; [Chai vs NemoClaw (Feature Summary)](NEMOCLAW.md#chai-vs-nemoclaw-feature-summary).

## Purpose

- **Purpose:** One table to update when any row (stack, gateway, LLM, tools, maturity) changes for these four systems.
- **How to use:** Edit **this file** only for the matrix; keep long-form detail in the linked references above.

## Comparison Table

| Area | OpenClaw | IronClaw | NemoClaw | Chai (this repo) |
|------|----------|----------|----------|------------------|
| **Lineage** | Reference **upstream** “personal AI” gateway ([openclaw.ai](https://openclaw.ai/)) | **Rust reimplementation** inspired by OpenClaw; tracks parity in `FEATURE_PARITY.md` | **Reference stack**: runs **upstream OpenClaw** inside **NVIDIA OpenShell** ([NemoClaw repo](https://github.com/NVIDIA/NemoClaw)) | **OpenClaw-inspired** WebSocket protocol; **independent** codebase—not a fork |
| **Primary implementation** | TypeScript / Node; plugins | Rust; PostgreSQL + pgvector | TS **`nemoclaw`** CLI, Python **blueprints**, container **sandbox**; OpenClaw (Node) inside | Rust (`crates/lib`, CLI, desktop) |
| **Relationship to OpenClaw code** | N/A (it is OpenClaw) | Clean-room style parity | **Bundles** OpenClaw in sandbox; fresh instance on onboard | Does **not** embed OpenClaw; custom `connect` / `status` / `agent` |
| **Gateway / control plane** | WS **v3**, control UI, operator vs **node**, many RPCs | Web gateway, channels, agent loop (per upstream README) | **`nemoclaw`** orchestrates OpenShell + sandbox; inference **routed** by OpenShell | WS **v1**; methods `health`, `status`, `send`, `agent` |
| **LLM integration** | **Model providers**, catalog, plugins | **`LLM_BACKEND`**, `providers.json`, many named + OpenAI-compat providers | Default: **NVIDIA cloud** Nemotron via OpenShell; **Ollama** / **vLLM** experimental | Configurable backends: **ollama**, **lms**, **vllm**, **nim** (`crates/lib/src/providers/`) |
| **Channels** | Many (Telegram, Discord, Slack, Signal, …) | REPL, HTTP, WASM-packaged channels, … (per README) | Whatever OpenClaw exposes **inside** the sandbox | **Telegram** (long-poll or webhook); designed to extend |
| **Tools & isolation** | Tool policy, optional **sandbox**, exec approvals | **WASM** sandbox, MCP, Docker orchestrator, leak scanning | **OpenShell** policy layers: network, filesystem, process, **inference** routing | **`tools.json`** + optional scripts; process execution; **no** WASM / OpenShell |
| **Config / state** | `~/.openclaw/openclaw.json` | DB + `~/.ironclaw/.env` bootstrap | Nemoclaw + OpenShell + sandbox state | **`~/.chai/profiles/<name>/config.json`**, **`~/.chai/active`**, per-profile **`paired.json`** / device material |
| **Maturity / positioning** | Broad ecosystem, docs at [docs.openclaw.ai](https://docs.openclaw.ai/) | OSS, security-first narrative ([ironclaw.com](https://www.ironclaw.com/)) | **Alpha** early preview (per [NemoClaw README](https://github.com/NVIDIA/NemoClaw)); NVIDIA-packaged | POC / evolving monorepo |

## Focus Notes (Chai vs Others)

These supplement the matrix; they are not duplicated inside the table.

- **IronClaw vs Chai:** IronClaw is a **full alternative gateway** in Rust with WASM-isolated tools and PostgreSQL-backed workspace memory. Chai is a **smaller** Rust gateway focused on Telegram + declarative skills; use IronClaw as a **reference for provider and sandbox patterns**, not as a spec to match.
- **NemoClaw vs Chai:** NemoClaw **packages** OpenClaw and **routes** inference through OpenShell (default cloud Nemotron). Chai does **not** use NemoClaw or OpenShell; optional **`nim`** is a **thin HTTP client** to the hosted NIM API ([NVIDIA_NIM.md](NVIDIA_NIM.md)), not the NemoClaw product stack.
- **Chai vs OpenClaw (pairing, skills, protocol rows):** See [OPENCLAW.md § Chai vs OpenClaw (Detailed)](OPENCLAW.md#chai-vs-openclaw-detailed).
