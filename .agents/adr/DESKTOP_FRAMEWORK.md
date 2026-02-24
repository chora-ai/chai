# UI Frameworks for Desktop Application

This document explores UI framework choices for the Chai desktop application. The project uses **egui/eframe** for minimal dependencies and a pure-Rust workflow.

---

## Desktop Framework Options

### 1. Tauri

- Modern, secure, lightweight
- Uses web technologies for UI (HTML/CSS/JS or Rust-based UI)
- Excellent Rust integration
- Small binary size (relative to Electron)
- Can use Vue.js, React, or other web frameworks for UI

**Setup (optional):**
```bash
cargo install tauri-cli
# See: https://tauri.app/v1/guides/getting-started/setup/
```

**Best for:** Teams that prefer web tech for UI and are okay with Node.js + WebView in the stack.

---

### 2. egui (via eframe) — **Selected for this project**

- Pure Rust immediate-mode GUI
- No web dependencies
- Great for native-feeling apps
- Minimal dependency tree

**Dependencies:** Add to the desktop crate `Cargo.toml`:
```toml
eframe = { version = "0.24", features = ["default"] }
egui = "0.24"
```

**Best for:** Minimal dependencies, single binary, Rust-only toolchain. **Chosen for Chai Desktop.**

---

### 3. iced

- Rust-native GUI framework
- Inspired by Elm architecture
- Good for complex UIs

**Dependencies:** Add to `Cargo.toml`:
```toml
iced = "0.10"
```

**Best for:** More structured UI architecture and reactive patterns.

---

## Minimal dependencies comparison

| Option            | Extra stack       | Relative deps |
|-------------------|-------------------|---------------|
| **egui / eframe** | None              | Minimal       |
| iced              | None              | Low           |
| Tauri + web UI    | Node, WebView, JS | High          |

**Why egui/eframe for this project:**

- **Rust only** — no Node.js, npm, WebView, or frontend framework
- **Small dependency tree** — `eframe` + `egui` and their Rust deps (winit, glutin, etc.)
- **Single binary** — no separate frontend build; system deps are normal OS windowing/OpenGL

---

## Recommendation Summary

- **This project:** egui/eframe for Chai Desktop (minimal deps, pure Rust).
- **Alternative:** Tauri + Vue.js/React if you prefer web-based UI.
- **Alternative:** iced if you want a more Elm-like architecture while staying Rust-only.

See [PROGRAMMING_LANGUAGE.md](PROGRAMMING_LANGUAGE.md) for the overall language rationale.
