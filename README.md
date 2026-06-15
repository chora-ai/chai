# chai

A multi-agent management system.

## Documentation

For user guides and other documentation, see [docs](docs/README.md).

## Packages

- **`crates/cli`** — Command-line interface for the multi-agent management system
- **`crates/desktop`** — Graphical user interface for the multi-agent management system
- **`crates/lib`** — Shared business logic for the multi-agent management system

## Test

```bash
# Test all crates
cargo test

# Test a specific crate
cargo test -p cli
cargo test -p desktop
cargo test -p lib
```

## Build

```bash
# Build all crates
cargo build

# Build CLI
cargo build -p cli

# Build Desktop
cargo build -p desktop

# Build with experimental adapters
cargo build -p cli --features matrix,signal
cargo build -p desktop --features matrix,signal
```

## Install

```bash
# Install CLI
cargo install --path crates/cli

# Install Desktop
cargo install --path crates/desktop

# Install with experimental adapters
cargo install --path crates/cli --features matrix,signal
cargo install --path crates/desktop --features matrix,signal
```

## Run

```bash
# Run CLI
chai help

# Run Desktop
chai-desktop
```

## Channels

| Channel | Build Feature | Status |
|---------|---------------|--------|
| Telegram | (always included) | Supported |
| Matrix | `--features matrix` | Experimental (opt-in) |
| Signal | `--features signal` | Experimental (opt-in) |

Telegram is included by default. Matrix and Signal require opt-in feature flags at build time and are experimental — they work for basic messaging but are still being hardened. See [Connections](docs/guides/04-connections.md) for setup instructions.

## License

Licensed under the LGPL-3.0: https://www.gnu.org/licenses/lgpl-3.0.en.html
