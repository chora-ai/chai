# chai

A multi-agent management system.

## Documentation

For user guides and other documentation, see [docs](docs/README.md).

## Packages

- **`crates/cli`** — Command-line interface for the multi-agent management system
- **`crates/desktop`** — Graphical user interface for the multi-agent management system
- **`crates/lib`** — Shared business logic for the multi-agent management system

## Build

```bash
# Build everything (Telegram channel included by default)
cargo build

# Build with the experimental Matrix adapter
cargo build --features matrix

# Build with the experimental Signal adapter
cargo build --features signal

# Build with both experimental adapters
cargo build --features matrix,signal
```

## Test

```bash
cargo test
```

## Install

```bash
# CLI (Telegram included by default)
cargo install --path crates/cli

# Desktop (Telegram included by default)
cargo install --path crates/desktop

# With optional channel adapters
cargo install --path crates/cli --features matrix
cargo install --path crates/cli --features signal
cargo install --path crates/cli --features matrix,signal
```

## Run

```bash
# CLI
chai help
chai gateway
chai chat

# Desktop
chai-desktop
```

## Channels

| Channel | Build Feature | Status |
|---------|--------------|--------|
| Telegram | (always on) | Supported |
| Matrix | `--features matrix` | Experimental (opt-in) |
| Signal | `--features signal` | Experimental (opt-in) |

Telegram is included by default. Matrix and Signal require opt-in feature flags at build time and are experimental — they work for basic messaging but are still being hardened. See [Connections](docs/guides/04-connections.md) for setup instructions.
