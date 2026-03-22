# NVIDIA NemoClaw Reference

Reference for **[NVIDIA NemoClaw](https://www.nvidia.com/en-us/ai/nemoclaw/)**—NVIDIA’s open source **reference stack** for running **[OpenClaw](https://openclaw.ai/)** inside **[NVIDIA OpenShell](https://github.com/NVIDIA/OpenShell)** with managed inference and policy. Use this when comparing “secured OpenClaw” offerings or NVIDIA agent tooling; this project (**Chai**) is not NemoClaw.

**Not to be confused with:** [NVIDIA_NIM_REFERENCE.md](NVIDIA_NIM_REFERENCE.md) documents Chai’s optional **`nim`** backend (direct HTTP to the **hosted NIM API** at `integrate.api.nvidia.com`). **NemoClaw** is a separate installer + sandbox + OpenShell product; it is not the same as wiring NIM into Chai’s config.

## Purpose and How to Use

- **Purpose:** Summarize NemoClaw’s positioning, install path, sandbox/inference model, and vocabulary so we can distinguish it from vanilla OpenClaw, IronClaw, and Chai’s own gateway.
- **How to use:** When researching NVIDIA’s OpenClaw packaging or OpenShell policies, start with the official links below, then this summary.

## Official Resources

- **Product / overview:** https://www.nvidia.com/en-us/ai/nemoclaw/ — “Run Autonomous Agents More Safely”; NemoClaw as OpenClaw plus **NVIDIA Agent Toolkit** / **OpenShell** guardrails; local **Nemotron** and “privacy router” messaging; install one-liner (`curl … | bash`), `nemoclaw onboard`.
- **Repository:** https://github.com/NVIDIA/NemoClaw — reference stack, installer scripts, `nemoclaw` CLI plugin, blueprints, docs.
- **Documentation (upstream):** https://docs.nvidia.com/nemoclaw/latest/ — cited in the GitHub project metadata.

## Status and Scope

- **Alpha / early preview:** The [NVIDIA/NemoClaw README](https://github.com/NVIDIA/NemoClaw) states **alpha** software (early preview from **March 16, 2026** in that README), **not production-ready**, with interfaces and behavior subject to change.

## Chai vs NemoClaw (Feature Summary)

Minimal scan. **Yes** = supported, **Partial** = subset or different shape, **No** = not present, **N/A** = not applicable. Chai’s optional **`nim`** backend is a direct **hosted NIM HTTP client**—not the NemoClaw product ([NVIDIA_NIM_REFERENCE.md](NVIDIA_NIM_REFERENCE.md)).

| Feature | Chai | NemoClaw |
|---------|------|----------|
| **Upstream OpenClaw** bundled / in sandbox | No | Yes |
| **OpenShell** policy (network, fs, process, inference) | No | Yes |
| **Blueprint**-driven install + sandbox lifecycle | No | Yes |
| **`nemoclaw`** CLI (onboard, connect, status) | No (`chai` CLI) | Yes |
| Default inference path (NVIDIA cloud via router) | Partial (`nim` optional) | Yes (OpenShell-routed Nemotron) |
| Local **Ollama** / **vLLM** as first-class backends | Yes | Partial (experimental per upstream README) |
| Requires container runtime + OpenShell stack | No | Yes |

## What NemoClaw Is (From Upstream)

### Positioning

- **OpenClaw inside a sandbox:** NemoClaw installs **OpenShell** and uses a **versioned blueprint** to create a **sandboxed** environment where OpenClaw runs; onboarding **creates a fresh OpenClaw instance inside the sandbox** ([README](https://github.com/NVIDIA/NemoClaw)).
- **NVIDIA Agent Toolkit / OpenShell:** Marketing page describes **OpenShell** as enforcing **policy-based** privacy and security guardrails; the repo describes **declarative policy** over network, filesystem, and inference routing.
- **Inference:** Default path described in the README: inference calls are **intercepted** by OpenShell and **routed to NVIDIA cloud** (e.g. **`nvidia/nemotron-3-super-120b-a12b`**); **API key** from **[build.nvidia.com](https://build.nvidia.com)** during `nemoclaw onboard`. **Local** options (**Ollama**, **vLLM**) are noted as **experimental**, with extra caveats on macOS (OpenShell host-routing).

### How It Fits Together (Repository)

| Piece | Role (per [README](https://github.com/NVIDIA/NemoClaw)) |
|-------|--------------------------------------------------------|
| **Plugin** | TypeScript **`nemoclaw`** CLI: onboard, connect, status, logs. |
| **Blueprint** | Versioned Python artifact: sandbox creation, policy, inference setup. |
| **Sandbox** | Isolated OpenShell container running OpenClaw with policy-enforced egress and filesystem. |
| **Inference** | NVIDIA cloud calls via OpenShell gateway (transparent to the agent in the described design). |

Blueprint lifecycle: **resolve** artifact → **verify** digest → **plan** resources → **apply** via OpenShell CLI.

### Protection Layers (Summary)

| Layer | Role |
|-------|------|
| **Network** | Blocks unauthorized outbound connections; hot-reloadable. |
| **Filesystem** | Restricts reads/writes (e.g. outside `/sandbox` and `/tmp`); locked at creation. |
| **Process** | Blocks privilege escalation and dangerous syscalls; locked at creation. |
| **Inference** | Reroutes model API calls to controlled backends; hot-reloadable. |

Unlisted hosts may be **blocked** with **operator approval** surfaced in the TUI (per README).

### Install and Commands (Pointers)

- **Install:** `curl -fsSL https://www.nvidia.com/nemoclaw.sh | bash` (also listed on the NVIDIA page).
- **Onboard:** `nemoclaw onboard` — wizard for gateway, providers, sandbox.
- **Connect:** `nemoclaw <name> connect` — shell inside sandbox; then `openclaw tui` or `openclaw agent …` as in the README.
- **Uninstall:** `uninstall.sh` from the repo (with flags such as `--yes`, `--keep-openshell`, `--delete-models`).

**Prerequisites (high level):** Linux (Ubuntu 22.04+), Node.js 20+, container runtime (Docker primary on Linux), OpenShell installed; hardware minimums (e.g. **8 GB RAM**) documented in the README due to image size and OOM risk during push.

## Possible Future Use

- **Policy ideas:** When designing allowlists, egress approval, or inference routing, NemoClaw’s **OpenShell + blueprint** split is a concrete reference.
- **Terminology:** “Reference stack,” “blueprint,” and “sandbox” are NVIDIA-specific terms in this ecosystem—avoid conflating them with Chai’s `gateway` or `agents` config.
