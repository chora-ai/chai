# Programming Language Choice

This document records why **Rust** was chosen for the Chai desktop and CLI, so the context is preserved as the project evolves.

## Recommendation: Rust

The project uses **Rust** as the implementation language.

### Why Rust

- **Desktop app ecosystem** — Mature options for native UI (egui, iced) and web-embedded desktop (Tauri) with first-class Rust support. Fits the goal of a desktop app installable on Mac and Linux.
- **Tooling** — Cargo provides consistent builds, dependencies, and testing without extra build systems. Single toolchain for CLI, desktop, and shared library.
- **Safety and maintainability** — Memory and concurrency safety help when integrating with external services (e.g. local model APIs, browser automation) and async I/O.
- **Ecosystem fit** — Strong crates for async runtimes, HTTP clients, serialization, and logging. Aligns with local-model and privacy-focused tooling (Ollama, LM Studio, etc.).

### Alternatives Considered

- **C++** — Viable for desktop (Qt, etc.) but build and dependency setup are typically heavier. Rust keeps the stack uniform and dependency-minimal.
- **Go** — Good for CLI and services; desktop UI options are less mature than in Rust.
- **Node/TypeScript** — Used by OpenClaw; we prefer a single binary, no Node/WebView in the default stack, and stronger typing for long-lived agent code.

## Summary

| Aspect        | Rust                     | Alternative (e.g. C++)      |
|---------------|---------------------------|-----------------------------|
| Desktop UI    | egui, iced, Tauri         | Qt, etc.                    |
| Build / deps  | Cargo, single toolchain   | CMake or other build system |
| This project  | **Chosen**                | Not selected                |

See [DESKTOP_FRAMEWORK.md](DESKTOP_FRAMEWORK.md) for the desktop UI framework choice (egui/eframe).
